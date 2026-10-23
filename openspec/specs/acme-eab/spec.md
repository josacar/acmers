# ACME EAB

## Purpose

Defines External Account Binding for ACME account registration, including JWS construction, credential input, key decoding, and first-registration-only behavior.

## Requirements


### Requirement: External Account Binding JWS
The system SHALL build an EAB JWS for ACME `newAccount` requests using HS256, the configured EAB key identifier as `kid`, the new-account URL as `url`, and the account public JWK as the payload.

#### Scenario: Registration includes EAB
- **WHEN** an account is registered with an EAB key identifier and HMAC key
- **THEN** the `newAccount` payload contains an `externalAccountBinding` object
- **AND** the object contains `protected`, `payload`, and `signature`

#### Scenario: Signature covers the encoded parts
- **WHEN** the EAB object is built
- **THEN** the HMAC-SHA256 signature is computed over `base64url(protected) + "." + base64url(payload)`
- **AND** the protected header contains `alg=HS256`, the EAB `kid`, and the new-account URL

### Requirement: EAB credential input
The `register` and `issue` commands SHALL accept `--eab-kid` and `--eab-hmac-key`, and SHALL fall back to the `ACME_EAB_KID` and `ACME_EAB_HMAC_KEY` environment variables.

#### Scenario: CLI credentials
- **WHEN** `--eab-kid` and `--eab-hmac-key` are supplied
- **THEN** they are used for account registration

#### Scenario: Environment fallback
- **WHEN** the CLI flags are absent and the environment variables are set
- **THEN** the environment values are used

#### Scenario: Missing credentials for EAB CA
- **WHEN** the selected CA requires EAB and no credentials are supplied
- **THEN** the command fails before creating an order with a clear error

#### Scenario: Existing account
- **WHEN** an account already exists for the selected server
- **THEN** EAB credentials are not required

### Requirement: EAB key decoding
The EAB HMAC key SHALL be decoded as base64url, with a fallback to standard base64, and MUST NOT be printed in errors or logs.

#### Scenario: Base64url key
- **WHEN** the key uses the URL-safe alphabet without padding
- **THEN** it is decoded correctly

#### Scenario: Standard base64 key
- **WHEN** the key uses `+`/`/` or padding
- **THEN** it is decoded correctly

#### Scenario: Invalid key
- **WHEN** the key cannot be decoded
- **THEN** the command fails with an error that does not include the key
