## Context

The ACME client already stores accounts per server and supports an arbitrary
`--server` directory URL. Let's Encrypt works because it does not require EAB.
The other major free CAs either require EAB or are not discoverable through the
current CLI. Users currently have to know the directory URL and hand-roll the
account registration flow, which acmers does not expose.

## Goals / Non-Goals

**Goals:**
- Register accounts at EAB-required CAs.
- Provide named presets for the common CAs.
- Keep `--server` as an escape hatch for any ACME directory.
- Keep EAB secrets out of renewal state.
- Preserve existing Let's Encrypt behavior and flags.

**Non-Goals:**
- RSA certificate keys.
- Changing DNS-01 challenge handling.
- Automatically fetching EAB credentials from CA dashboards/APIs.
- CAs with proprietary non-ACME APIs.

## Decisions

### EAB is a new-account concern only
EAB is included in the `newAccount` payload. Once the account exists, renewals
authenticate with the stored account key and do not need EAB. Therefore EAB
credentials are supplied only to `register` and `issue` when an account is
created; they are never written to `renewal.json` or the account file.

Alternative: persist EAB secrets alongside the account. Rejected because the
renewal file is world-readable and the account file does not need them after
registration.

### HS256 JWS over the account JWK
Per RFC 8555 §7.3.4, the EAB object is a JWS with:
- protected header `{"alg":"HS256","kid":<eab-kid>,"url":<newAccount URL>}`,
- payload = the account public JWK,
- signature = HMAC-SHA256 over `base64url(protected) + "." + base64url(payload)`.

Use `ring::hmac` (already a dependency). Decode the HMAC key with the existing
base64url decoder, falling back to standard base64 for CAs that print padded
standard encoding. Reject empty or invalid keys with a config error.

### CA presets are a static registry
Add `src/ca.rs` with a `CaPreset` table:

| Name | Directory | EAB |
|---|---|---|
| `letsencrypt` | `https://acme-v02.api.letsencrypt.org/directory` | no |
| `letsencrypt-staging` | `https://acme-staging-v02.api.letsencrypt.org/directory` | no |
| `zerossl` | `https://acme.zerossl.com/v2/DV90` | yes |
| `google` | `https://dv.acme-v02.api.pki.goog/directory` | yes |
| `sslcom` | `https://acme.ssl.com/sslcom-dv-ecc` | yes |
| `sslcom-rsa` | `https://acme.ssl.com/sslcom-dv-rsa` | yes |
| `buypass` | `https://api.buypass.com/acme/directory` | no |
| `buypass-test` | `https://api.test4.buypass.no/acme/directory` | no |
| `sectigo` | `https://acme.sectigo.com/v2/DV` | yes |

Buypass discontinued TLS/SSL certificate issuance in October 2025; its presets
are retained for existing accounts and test endpoints but cannot issue new
certificates. Buypass also issued RSA-only certificates, which acmers does not
support yet.

`--server URL` overrides the preset URL while retaining the preset's EAB
expectation. `--test` keeps its current meaning (Let's Encrypt staging) and is
rejected when combined with a non-Let's Encrypt `--ca`. With no `--ca` and no
`--server`, the default remains Let's Encrypt production.

### Renewal stores the CA name, not credentials
`renewal.json` gains `ca`. Renewal uses the stored server; if an account is
missing, the command fails with guidance to run `register`/`issue` with EAB
rather than silently registering a new account.

## Risks / Trade-offs

- **CA directory URLs change** → They are centralized in `ca.rs` and covered by
  tests; `--server` remains the escape hatch.
- **EAB key encoding varies** → Try base64url then standard base64, and report a
  clear error without echoing the key.
- **Wrong EAB credentials are hard to diagnose** → Surface the ACME error body
  from newAccount, which usually names the EAB failure.
- **Secret leakage via process arguments** → Support env vars so keys do not
  have to appear in shell history; never print them.
- **`--test` ambiguity with presets** → Reject `--test` with a non-LE preset and
  document `--ca letsencrypt-staging`.

## Migration Plan

Existing Let's Encrypt renewals are unaffected: `ca` is absent, the stored
server is used, and the account already exists. New CA users pass `--ca` and EAB
flags/env once during registration; later renewals need neither.

## Open Questions

None. The CA set and EAB behavior were confirmed by the user.
