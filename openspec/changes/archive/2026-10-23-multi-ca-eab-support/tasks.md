## 1. CA Registry

- [x] 1.1 Add `src/ca.rs` with the CA preset table and directory/EAB metadata
- [x] 1.2 Add lookup, default resolution, and supported-name listing
- [x] 1.3 Unit-test preset resolution, unknown names, and server override

## 2. EAB Signing

- [x] 2.1 Add HMAC-SHA256 EAB JWS generation to `src/crypto.rs`
- [x] 2.2 Decode base64url and standard-base64 EAB keys without logging them
- [x] 2.3 Add unit tests for the EAB protected header, payload, and signature

## 3. Account Registration

- [x] 3.1 Extend `acme::account::load_or_register` to accept optional EAB credentials
- [x] 3.2 Include `externalAccountBinding` in the new-account payload when supplied
- [x] 3.3 Reject missing EAB for EAB-required CAs before creating an order
- [x] 3.4 Keep existing accounts usable without EAB

## 4. CLI and Renewal

- [x] 4.1 Add `--ca`, `--eab-kid`, and `--eab-hmac-key` to `register` and `issue`
- [x] 4.2 Add `ACME_EAB_KID` and `ACME_EAB_HMAC_KEY` environment fallbacks
- [x] 4.3 Reject `--test` with non-Let's Encrypt CA presets
- [x] 4.4 Persist `ca` in `renewal.json` and display it in `show`
- [x] 4.5 Use the stored server/ca during renewal without requiring EAB

## 5. Tests

- [x] 5.1 Add a mock ACME registration test asserting `externalAccountBinding`
- [x] 5.2 Add parser tests for `--ca` and EAB flags
- [x] 5.3 Run `cargo fmt --all -- --check`, clippy with `-D warnings`, and the full test suite

## 6. Documentation and OpenSpec

- [x] 6.1 Update the README supported-CA table and examples
- [x] 6.2 Update `debian/acmers.1`
- [x] 6.3 Update `docs/MIGRATION.md` with an EAB registration example
- [x] 6.4 Sync the `acme-eab` and `ca-presets` specs and archive the change
