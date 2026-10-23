## 1. Storage Foundation

- [x] 1.1 Add `crypto::pkcs8_to_pem` and a PEM wrapping helper with 64-column output
- [x] 1.2 Add `order::split_cert_chain` returning leaf and chain PEM blocks
- [x] 1.3 Add `config` paths for `privkey.pem` and `chain.pem`
- [x] 1.4 Add an atomic-write helper with explicit Unix mode and use it for all certificate artifacts
- [x] 1.5 Write the complete file set (`cert.pem`, `chain.pem`, `fullchain.pem`, `privkey.pem`, `key.pem`) with `0644`/`0600` modes
- [x] 1.6 Set account file mode `0600` and account directory mode `0700`

## 2. Account Isolation

- [x] 2.1 Add a server-slug helper and `accounts/<slug>.json` storage in `config`
- [x] 2.2 Register and load accounts per server; reject a stored account whose server differs
- [x] 2.3 Import a legacy `account.json` into the per-server store without deleting it
- [x] 2.4 Make `--test` and `--dry-run` select staging regardless of stored account server
- [x] 2.5 Persist and honor the staging flag from `renewal.json` during renewal

## 3. Renewal Model

- [x] 3.1 Create `src/renewal.rs` with tolerant JSON parsing/serialization and defaults
- [x] 3.2 Store `domains`, provider, email, server, staging, challenge, key type, DNS settings, preferred chain, install paths, and hooks
- [x] 3.3 Fall back to the certificate directory name when `domains` is absent
- [x] 3.4 Refactor `cmd_issue` to accept a renewal config plus an options struct instead of positional arguments
- [x] 3.5 Renew every stored domain instead of only the primary domain
- [x] 3.6 Implement due-date logic with a configurable `--days` threshold
- [x] 3.7 Implement `renew` with no `-d` as bulk renewal and add `--all`, `--force`, `--dry-run`, and `--keep-until-expiring`
- [x] 3.8 Print a per-certificate summary and return non-zero when any renewal fails
- [x] 3.9 Make `cron` delegate to bulk renewal

## 4. CLI Lifecycle Commands

- [x] 4.1 Add `--server` to `issue` and `register`
- [x] 4.2 Add `certificates` to list names, domains, and expiry
- [x] 4.3 Add `show -d <domain>` and `show-account`
- [x] 4.4 Add `delete -d <domain>`
- [x] 4.5 Add `revoke --reason` with ACME reason mapping and `--delete-after-revoke`

## 5. Deployment and Hooks

- [x] 5.1 Add `--cert-path`, `--key-path`, `--fullchain-path`, `--chain-path`, and `--reloadcmd`
- [x] 5.2 Add `--deploy-hook`, `--pre-hook`, and `--post-hook`
- [x] 5.3 Persist install paths and hooks in `renewal.json`
- [x] 5.4 Implement atomic copy to install paths with correct modes
- [x] 5.5 Run deployment, reload, and deploy hooks only after an actual issuance/renewal
- [x] 5.6 Run the pre-hook once and the post-hook after all work, including failures
- [x] 5.7 Export `RENEWED_LINEAGE` and `RENEWED_DOMAINS` to hooks and report hook failures

## 6. DNS-01 Propagation

- [x] 6.1 Create `src/dns.rs` with a minimal UDP TXT query and response parser
- [x] 6.2 Follow a CNAME answer to the TXT owner
- [x] 6.3 Read nameservers from `/etc/resolv.conf` and accept `--dns-servers`
- [x] 6.4 Poll until the expected value is observed or `--dns-timeout` expires
- [x] 6.5 Fall back to `--dnssleep` when no resolver is usable
- [x] 6.6 Keep TXT cleanup on timeout and error paths

## 7. Chain Selection and ARI

- [x] 7.1 Add `--preferred-chain` and select a matching alternate chain from `Link: rel="alternate"`
- [x] 7.2 Persist the preferred chain in `renewal.json`
- [x] 7.3 Fetch ARI `renewalInfo` when the directory advertises it and use `suggestedWindow` for the due decision
- [x] 7.4 Treat ARI failures as non-fatal and fall back to the day threshold

## 8. Documentation and Packaging

- [x] 8.1 Update README with the new file layout, commands, flags, and breaking `key.pem` change
- [x] 8.2 Add a certbot-to-acmers migration guide
- [x] 8.3 Update `debian/acmers.1`
- [x] 8.4 Update the systemd service to run `renew --all`

## 9. Tests and Verification

- [x] 9.1 Unit-test PEM encoding, chain splitting, and atomic writes
- [x] 9.2 Unit-test renewal parsing, legacy fallback, and round-trip serialization
- [x] 9.3 Test account selection and staging isolation
- [x] 9.4 Test deployment copies, hook gating, and hook environment with a mock command
- [x] 9.5 Test the DNS TXT parser against crafted responses and a CNAME chain
- [x] 9.6 Add a mock-ACME end-to-end issuance test covering the new file set
- [x] 9.7 Run `cargo fmt --all -- --check`, clippy with `-D warnings`, and the full test suite
