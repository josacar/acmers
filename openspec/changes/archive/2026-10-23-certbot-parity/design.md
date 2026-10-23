## Context

`acmers` currently issues certificates with a single EC P-256 account and
certificate key, writes `cert.pem`/`fullchain.pem` from the same body and
`key.pem` as raw PKCS#8 DER, and stores a minimal `renewal.json`. The renewal
path re-enters `cmd_issue` with a single domain, so SANs are lost, and it
hardcodes `dnssleep = 10`. Account selection is global and a stored production
account silently overrides `--test`.

The agreed migration scope is DNS-01 only, ECDSA P-256 keys, Let's Encrypt,
and deployment by copying files to configured paths plus running a reload
command. The project constraints remain: synchronous code, no async runtime,
no external crypto/HTTP/DNS tools, and no new dependencies unless Debian
packaged.

## Goals / Non-Goals

**Goals:**
- Produce a certbot-compatible certificate file set with PEM private keys.
- Renew every SAN and wildcard, with correct staging isolation.
- Deploy certificates to service paths and reload services only after renewal.
- Provide certbot-style bulk renewal and lifecycle commands.
- Verify DNS-01 propagation without fixed sleeps and without external tools.

**Non-Goals:**
- Webroot, nginx, apache, or TLS-ALPN-01 challenges.
- RSA or P-384 certificate keys.
- EAB and non-Let's Encrypt CAs.
- `/etc/letsencrypt/live` symlink layout or importing certbot state.
- Parallel issuance.

## Decisions

### File layout and formats
Each certificate directory keeps its current name under `ACMERS_HOME` and gains
`chain.pem` and `privkey.pem`. `key.pem` remains as a PEM copy to avoid breaking
existing scripts, but its format changes from DER to PEM.

The ACME download body is split at certificate boundaries. The first PEM block
is the leaf; the rest is the chain.

Alternatives considered:
- Writing only `privkey.pem` and deleting `key.pem`: cleaner, but breaks the
  current documented layout for existing users.
- Keeping DER for `key.pem`: avoids a breaking change but keeps a format that
  most services cannot consume directly. The breaking change is accepted and
  documented.

### No new dependencies
PEM encoding is a short base64-and-wrap routine using the existing `base64`
module. Renewal documents stay as JSON read through the existing `json::get_*`
helpers instead of introducing `serde` derive. DNS polling uses
`std::net::UdpSocket`. Hook execution uses `std::process::Command` with
`/bin/sh -c`; this is the only intentional subprocess execution and does not
replace any internal crypto, HTTP, or DNS work.

### Atomic writes
Add a shared `write_atomic(path, bytes, mode)` helper that creates a temporary
file in the same directory, writes, sets permissions, and renames. Certificate
directories are created with `0755`; private material is written `0600`.
Deployment uses a separate helper that copies to a temporary file next to the
destination before renaming.

### Renewal model
A `RenewalConfig` type in a new `src/renewal.rs` owns parsing, defaults, and
serialization. Fields: `domains`, `provider`, `email`, `server`, `staging`,
`challenge`, `key_type`, `dnssleep`, `dns_servers`, `dns_timeout`,
`preferred_chain`, install paths, and hook commands. Missing `domains` falls
back to the directory name. Unknown or missing fields take conservative
defaults so old files keep working.

`cmd_issue` becomes the single issuance engine and accepts a `RenewalConfig`
plus an `IssueOptions` struct, avoiding the current positional argument list.
`renew` loads configuration, checks due status, and calls the engine.

### Renewal decision
Use the existing X.509 expiry parser for the default 30-day window. ARI is
fetched from the directory's `renewalInfo` endpoint with the certificate's
serial number and Authority Key Identifier; when a `suggestedWindow` is
present it overrides the fixed threshold. ARI failures are non-fatal and fall
back to the threshold.

### Account storage
Accounts move to `accounts/<slug>.json`, where the slug is derived from the
directory host and path (for example `acme-v02.api.letsencrypt.org`). The
loader only accepts an account whose `server` equals the requested server.
A legacy `account.json` is imported once into its matching slug. Staging is
therefore a separate account, and `--test`/`--dry-run` cannot be overridden by
a production account.

### Hooks and deployment
Install paths and hook commands are stored in `renewal.json`. Deployment order:
write local artifacts, copy to install paths, then run `--reloadcmd`, then
`--deploy-hook`. `--pre-hook` runs once lazily before the first order in a
command; `--post-hook` runs in all exit paths after work starts. Hooks receive
`RENEWED_LINEAGE` and `RENEWED_DOMAINS`. Dry runs and skipped certificates run
no deployment hooks. A hook failure is reported after the certificate is saved
and makes the command exit non-zero.

### CLI shape
Extend the hand-written parser. `renew` with no `-d` means `--all`. `cron`
delegates to the bulk renewal function. Add subcommands `certificates`, `show`,
`show-account`, and `delete`. Keep all existing flags working.

### DNS propagation polling
Implement a minimal DNS client in `src/dns.rs`: build a TXT query with a
random ID, send over UDP to each resolver with a short per-attempt timeout,
parse the answer section, and compare TXT strings. Support a CNAME hop by
following the target. Poll every few seconds until `--dns-timeout`; fall back
to `--dnssleep` when no resolver is usable. Cleanup stays in the existing
`Drop` guard, so the record is removed on success, failure, or timeout.

Alternatives considered:
- DNS-over-HTTPS: simpler parsing, but adds a hard dependency on an external
  resolver service and ignores local configuration.
- Adding a DNS crate: violates the no-new-dependency constraint.

## Risks / Trade-offs

- **`key.pem` format change breaks existing consumers** → Document as breaking,
  write `privkey.pem` alongside, and mention it in the migration guide.
- **Hand-rolled DNS parser is a correctness risk** → Restrict support to TXT
  and CNAME answers, ignore unknown record types, and fall back to sleep.
- **Hook execution is a new subprocess surface** → Commands come only from
  user-supplied flags or the user-owned renewal file; run them unchanged
  through `/bin/sh -c`.
- **ARI adds an extra request per certificate** → Treat failures as non-fatal
  and cache the result for the current run.
- **Renewal config schema drift** → Keep parsing tolerant, never deserialize
  into a fixed struct without defaults, and add a back-compat test.
- **Staging certificate renewal** → Store `staging` explicitly and never infer
  it from the account file.

## Migration Plan

1. Existing installations keep their certificate directories. On first run,
   `renew` reads old `renewal.json` and defaults `domains` to `[directory]`.
2. Legacy `account.json` is imported into `accounts/<slug>.json`; the original
   file is kept.
3. Existing `key.pem` DER files are replaced with PEM on the next issuance;
   users are told to reissue or switch consumers to `privkey.pem`.
4. `cron` keeps working and now performs a real bulk renewal; the systemd unit
   is updated to call `renew --all`.
5. Rollback is a binary downgrade: the new files are additive, and the old
   binary ignores `chain.pem`, `privkey.pem`, and the extra renewal fields.

## Open Questions

- Should `delete` refuse when install paths outside the certificate directory
  still point at the files? Default decision: warn and proceed; revocation is
  explicit.
