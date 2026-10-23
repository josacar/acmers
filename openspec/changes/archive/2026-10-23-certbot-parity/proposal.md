## Why

`acmers` can issue certificates, but a certbot user cannot move to it safely: the
private key is written as raw DER, `cert.pem` and `fullchain.pem` are identical,
there is no `chain.pem`, and `renew` silently drops every SAN except the primary
domain. There is also no way to copy certificates to the paths services read or
to reload them after renewal, so every migration still needs a hand-written
script. This change closes the gap for the agreed scenario: DNS-01 only,
ECDSA P-256 keys, Let's Encrypt, and install-by-copy plus a reload command.

## What Changes

- Write a certbot-compatible file set per certificate: `cert.pem` (leaf),
  `chain.pem` (intermediates), `fullchain.pem` (leaf + chain), and
  `privkey.pem` (PKCS#8 PEM). Keep `key.pem` as a PEM compatibility file.
  **BREAKING**: `key.pem` changes from DER to PEM.
- Write files atomically and with restrictive permissions for private keys.
- Store the full domain list and issuance options in `renewal.json`; renew all
  SANs and wildcards instead of only the primary domain.
- Fix staging/account selection: accounts are keyed by ACME server, `--test` is
  no longer overridden by a previously stored production account, and staging
  certificates renew against staging.
- Add `--server` (already documented in the README) and persist it per
  certificate.
- Add install-and-reload options to `issue` and `renew`: `--cert-path`,
  `--key-path`, `--fullchain-path`, `--chain-path`, `--reloadcmd`,
  `--deploy-hook`, `--pre-hook`, and `--post-hook`. Paths and commands are
  persisted in `renewal.json`, files are copied atomically, and hooks run only
  when a certificate was actually renewed.
- Make `renew` renew all certificates when no `-d` is given, with `--all`,
  `--force`, `--dry-run`, `--days`, and `--keep-until-expiring`. Print a
  per-domain summary and exit non-zero on any failure. Keep `cron` as an alias
  for `renew --all`.
- Add `certificates`, `delete`, `show`, and `show-account` commands, plus
  `--reason` and `--delete-after-revoke` for `revoke`.
- Replace blind `--dnssleep` waiting with a DNS TXT propagation poll over UDP
  (standard library only), with configurable resolvers and fallback to sleep.
- Add `--preferred-chain` for Let's Encrypt alternate chains and use ARI
  (RFC 9773) suggested renewal windows when available.
- Update README, man page, systemd units, and add a certbot migration guide.

## Capabilities

### New Capabilities
- `certificate-storage`: on-disk certificate and key layout, file formats, permissions, and atomic writes.
- `renewal`: renewal configuration, SAN preservation, renewal windows, bulk renewal, and command behavior.
- `deployment-hooks`: copying certificates to configured paths and running reload/deploy hooks.
- `acme-account`: account selection per ACME server and staging/production isolation.
- `dns01-validation`: DNS TXT propagation polling and resolver configuration.

### Modified Capabilities

None. Existing specs cover packaging, CI, provider patterns, and test infrastructure; their requirements do not change.

## Impact

- **Code**: `src/main.rs`, `src/cli.rs`, `src/config.rs`, `src/crypto.rs`,
  `src/acme/order.rs`, new `src/renewal.rs` and `src/dns.rs`.
- **Files on disk**: `~/.acmers/<domain>/` gains `chain.pem` and `privkey.pem`;
  `key.pem` becomes PEM; `renewal.json` gains fields. Existing certificates
  remain renewable through a fallback path.
- **Packaging**: `debian/acmers.service`, `debian/acmers.timer`,
  `debian/acmers.1`, and README.
- **Dependencies**: none added. DNS polling uses `std::net::UdpSocket`; renewal
  parsing uses the existing JSON helpers.
- **Non-goals**: webroot/nginx/apache plugins, TLS-ALPN-01, RSA/P-384, EAB,
  non-Let's Encrypt CAs, and the `/etc/letsencrypt/live` symlink layout.
