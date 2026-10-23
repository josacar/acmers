## Why

acmers currently works with Let's Encrypt but cannot register accounts at the
other ACME CAs users ask for. ZeroSSL, Google Trust Services, SSL.com, and
Sectigo require External Account Binding (EAB), and users have no way to select
a known CA or supply EAB credentials. Buypass Go has presets for existing
accounts and test endpoints, though Buypass discontinued new TLS/SSL issuance in
October 2025. This change adds EAB and CA presets while keeping the existing
`--server` escape hatch.

## What Changes

- Implement EAB JWS generation (HS256) for ACME new-account requests.
- Add `--eab-kid` and `--eab-hmac-key` to `register` and `issue`, with
  `ACME_EAB_KID` and `ACME_EAB_HMAC_KEY` environment fallbacks.
- Add `--ca <name>` presets for Let's Encrypt, Let's Encrypt staging, ZeroSSL,
  Google Trust Services, SSL.com (ECC/RSA), Buypass Go (production/test), and
  Sectigo.
- Resolve the ACME directory URL from the preset, with `--server` overriding the
  URL. `--test` remains an alias for Let's Encrypt staging and conflicts with
  `--ca` other than Let's Encrypt.
- Persist the CA name alongside the server in `renewal.json` and show it in
  `show`.
- Require EAB when the selected CA is marked as EAB-required, and fail before
  creating an order with a clear message.
- Do not persist EAB secrets; they are only needed when an account is first
  registered. Existing accounts renew without EAB.
- Update README, man page, docs, and openspec.

## Capabilities

### New Capabilities
- `acme-eab`: External Account Binding JWS generation, credential input, and account registration behavior.
- `ca-presets`: Built-in ACME CA registry and command-line selection.

### Modified Capabilities

None.

## Impact

- **Code**: `src/crypto.rs`, `src/acme/account.rs`, new `src/ca.rs`,
  `src/cli.rs`, `src/main.rs`, `src/renewal.rs`.
- **Docs**: README, `debian/acmers.1`, `docs/MIGRATION.md`.
- **Tests**: EAB signing/verification, CA resolution, CLI parsing, and a mock
  ACME registration that asserts `externalAccountBinding`.
- **Dependencies**: none added; HMAC uses the existing `ring` crate.
