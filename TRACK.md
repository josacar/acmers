# acmers — Rust ACME Client with 200+ DNS Providers

A minimal-dependency Rust CLI that implements the ACME protocol (RFC 8555) for
automatic SSL/TLS certificate issuance and renewal, with DNS-01 challenge
support for providers ported from the
[acme.sh](https://github.com/acmesh-official/acme.sh) project.

## Philosophy

acme.sh is a pure shell script that orchestrates system tools (`curl`, `openssl`).
`acmers` follows the same spirit: **minimal pure-Rust dependencies** — no async
runtime, no system tool calls for crypto/HTTP/DNS. Every line of protocol logic
is in Rust. The only subprocess execution is user-configured deployment and
reload hooks.

## Current status (0.4.0)

- ACME order, DNS-01 and standalone HTTP-01 challenges, finalize, and download.
- Certbot-compatible file set: `cert.pem`, `chain.pem`, `fullchain.pem`,
  `privkey.pem`, and `key.pem` (PEM).
- SAN-preserving bulk renewal with `--force`, `--dry-run`, `--days`, and
  `--keep-until-expiring`.
- Per-server ACME accounts with staging isolation.
- CA presets for Let's Encrypt, ZeroSSL, Google Trust Services, SSL.com,
  Buypass, and Sectigo, with External Account Binding for the CAs that require
  it.
- Cloudflare DNS-01 migration from certbot documented in
  [docs/MIGRATION.md](docs/MIGRATION.md).
- Install paths, reload/deploy/pre/post hooks, and persisted renewal settings.
- DNS TXT propagation polling over `std::net::UdpSocket`, with `--dns-servers`,
  `--dns-timeout`, and `--dnssleep` fallback.
- ARI (RFC 9773) renewal windows and `--preferred-chain`.
- Certificate management: `certificates`, `show`, `show-account`, `delete`, and
  `revoke --reason`.
- Packaged Debian service and timer: twice-daily renewal with a 12-hour
  randomized delay, `/etc/acmers/env` credentials, and a non-systemd cron
  fallback.
- Exclusive renewal lock with stale-PID recovery.

## Dependencies (8 crates, all Debian-packaged)

| Crate | Version | Purpose | Debian |
|-------|---------|---------|--------|
| `ureq` | 3.3 | HTTP client (sync, pure Rust TLS) | `librust-ureq-dev` |
| `serde_json` | 1.0 | JSON parsing | `librust-serde-json-dev` |
| `rcgen` | 0.14 | CSR creation, cert params | `librust-rcgen-dev` |
| `ring` | 0.17 | ECDSA keygen, JWS signing, SHA256 | `librust-ring-dev` |
| `lexopt` | 0.3 | CLI argument parser | `librust-lexopt-dev` |
| `time` | 0.3 | Certificate expiry and ARI timestamps | transitive only |
| `x509-parser` | 0.18 | X.509 certificate parsing | transitive only |
| `rustls-pki-types` | 1 | Private key DER types | transitive only |

Transitive total: ~80 crates. Release binary: ~3.6 MB.

## Architecture

```
src/
├── main.rs           CLI dispatch, issue, renew, revoke, lifecycle commands
├── cli.rs            Argument parser
├── crypto.rs         ECDSA keygen, JWS ES256, CSR, PEM encoding
├── http.rs           ureq wrapper (captures error bodies)
├── json.rs           serde_json field extraction helpers
├── base64.rs         Base64url encode/decode
├── error.rs          Error types
├── config.rs         Paths, atomic writes, permissions
├── renewal.rs        Renewal configuration and SAN preservation
├── deploy.rs         Install paths and reload/deploy hooks
├── dns.rs            UDP DNS TXT propagation polling
├── envfile.rs        KEY=VALUE credential file loading
├── lock.rs           Exclusive renewal lock
├── storage.rs        Certificate file set output
├── acme/
│   ├── account.rs    Per-server account registration and loading
│   ├── ari.rs        ARI (RFC 9773) renewal info
│   ├── directory.rs  ACME directory discovery
│   ├── challenge.rs  Authorization, challenge, polling
│   └── order.rs      Order lifecycle, finalize, chain download
└── providers/
    ├── helpers.rs    Common REST patterns
    ├── mod.rs        DnsProvider trait + registry (201 entries)
    └── *.rs          Individual provider implementations
```

## ACME protocol flow

1. `register --email user@domain.tld` creates an account per server.
2. `issue -d example.com -d '*.example.com' --dns cf --email ...` creates an
   order, solves DNS-01, finalizes the CSR, and downloads the chain.
3. `renew -d example.com` rereads `renewal.json` and renews every SAN.
4. `renew --all` renews every stored certificate that is due.
5. `revoke -d example.com --reason keycompromise` revokes and optionally
   deletes local state.
6. `cron` is an alias for `renew --all`.

### JWS signing (ES256)

- Account key: ECDSA P-256 generated via `ring`.
- JWK thumbprint: `base64url(sha256(canonical_jwk_json))`.
- Signature: P1363 raw R||S, 64 bytes.

## Certificate and state layout

```
~/.acmers/                    # per-user runs
/var/lib/acmers/              # root and packaged service
├── accounts/
│   └── <server>.json
└── <name>/
    ├── cert.pem              # leaf
    ├── chain.pem             # intermediates
    ├── fullchain.pem         # leaf + intermediates
    ├── privkey.pem           # PKCS#8 PEM, mode 0600
    ├── key.pem               # same PEM for compatibility
    └── renewal.json          # domains, server, install paths, hooks

/etc/acmers/env               # CF_Token etc. for unattended renewal
```

## Provider status

| Status | Count | Description |
|--------|-------|-------------|
| **Implemented** | 196 | Full `add_txt`/`remove_txt` with real API calls |
| **Stub** | 5 | Require external CLI tools or have no API |
| **Total** | 201 | All acme.sh providers accounted for |

Stubs: `df`, `hetzner`, `knot`, `lexicon`, `nsd`.

Two providers do not use `http::` but are fully implemented: `nsupdate`
(DNS UPDATE over UDP/TCP) and `maradns` (zone file manipulation).

## CLI usage

```
acmers <command> [options]

Commands:
  issue           Issue a new certificate
  renew           Renew one or all certificates
  revoke          Revoke a certificate
  register        Register ACME account
  list-providers  List supported DNS providers
  cron            Renew all certificates (alias for renew --all)
  certificates    List stored certificates
  show            Show certificate details
  show-account    Show the active ACME account
  delete          Delete a stored certificate

Examples:
  acmers register --email user@example.com
  acmers register --email user@example.com --ca zerossl --eab-kid ... --eab-hmac-key ...
  acmers issue -d example.com -d '*.example.com' --dns cf --email user@example.com
  acmers issue -d example.com --dns cf --email user@example.com --ca google --eab-kid ... --eab-hmac-key ...
  acmers renew -d example.com
  acmers renew --all --dry-run --env-file /etc/acmers/env
  acmers revoke -d example.com --reason keycompromise
  acmers certificates
```

Issue/renew deployment flags: `--cert-path`, `--key-path`, `--fullchain-path`,
`--chain-path`, `--reloadcmd`, `--deploy-hook`, `--pre-hook`, `--post-hook`,
`--preferred-chain`, `--dns-servers`, `--dns-timeout`, `--env-file`.

## Completed work

- [x] ACME directory, nonce, account, order, challenge, finalize, download.
- [x] ECDSA P-256 account and certificate keys, ES256 JWS.
- [x] 196 DNS providers implemented from the acme.sh registry.
- [x] Certbot-compatible certificate and key files.
- [x] SAN-preserving renewal, bulk renewal, dry runs, and ARI.
- [x] Per-server accounts and staging isolation.
- [x] CA presets and External Account Binding for ZeroSSL, Google, SSL.com,
  and Sectigo.
- [x] Install paths, reload/deploy/pre/post hooks.
- [x] DNS propagation polling and Cloudflare migration guide.
- [x] Certificate management and revocation reasons.
- [x] Debian package with systemd timer, env file, and cron fallback.
- [x] Renewal locking and stale-lock recovery.

## Remaining gaps / non-goals

- Webroot, nginx, and apache plugins.
- TLS-ALPN-01.
- RSA and P-384 certificate keys.
- Importing certbot's ACME account or `/etc/letsencrypt` state.
- Notifications (email/webhook) and log rotation.
- User-level systemd timers.

## Future refactors

1. Generic REST provider base for the many identical REST implementations.
2. Provider plugin system for a smaller monolithic binary.
3. Authoritative-nameserver DNS propagation checks instead of relying on
   recursive resolvers.
4. Parallel multi-domain issuance.
5. Encrypted credential store.

## Verification

```sh
mise run check
mise run providers
cargo test
./target/debug/acmers list-providers | wc -l
```

`list-providers` prints a header plus 201 provider lines.

## License

MIT
