# CA Presets

## Purpose

Defines built-in ACME CA presets, server overrides, staging compatibility, and CA persistence for renewals.

## Requirements


### Requirement: CA preset selection
The `register` and `issue` commands SHALL accept `--ca <name>` and resolve it to an ACME directory URL and EAB requirement.

#### Scenario: Let's Encrypt preset
- **WHEN** `--ca letsencrypt` is supplied
- **THEN** the Let's Encrypt production directory is used
- **AND** EAB is not required

#### Scenario: ZeroSSL preset
- **WHEN** `--ca zerossl` is supplied
- **THEN** `https://acme.zerossl.com/v2/DV90` is used
- **AND** EAB is required

#### Scenario: SSL.com RSA preset
- **WHEN** `--ca sslcom-rsa` is supplied
- **THEN** the SSL.com RSA directory is used

#### Scenario: Buypass preset
- **WHEN** `--ca buypass` is supplied
- **THEN** the Buypass production directory is used
- **AND** EAB is not required

#### Scenario: Unknown preset
- **WHEN** an unknown `--ca` name is supplied
- **THEN** the command fails and lists the supported names

### Requirement: Server override
The `issue` and `register` commands SHALL continue to accept `--server <url>`, which overrides the preset directory URL while retaining the preset's EAB requirement.

#### Scenario: Custom URL with preset
- **WHEN** `--ca zerossl --server https://acme.example/directory` is supplied
- **THEN** the custom URL is used
- **AND** EAB is still required

### Requirement: Staging compatibility
The existing `--test` flag SHALL continue to select Let's Encrypt staging and SHALL be rejected when combined with a non-Let's Encrypt `--ca`.

#### Scenario: Test without CA
- **WHEN** `--test` is supplied without `--ca`
- **THEN** Let's Encrypt staging is used

#### Scenario: Test with ZeroSSL
- **WHEN** `--test --ca zerossl` is supplied
- **THEN** the command fails with a clear conflict error

### Requirement: CA persistence and display
The system SHALL store the CA name in `renewal.json` when one is selected and SHALL display it in `show`.

#### Scenario: Renewal reuses the server
- **WHEN** a certificate was issued with `--ca sectigo`
- **THEN** renewal uses the stored Sectigo directory
- **AND** `show` reports `ca: sectigo`

#### Scenario: Legacy renewal
- **WHEN** `renewal.json` has no `ca` field
- **THEN** renewal uses the stored `server` and remains compatible with existing certificates
