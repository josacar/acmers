## ADDED Requirements

### Requirement: Account per ACME server
The system SHALL store ACME accounts keyed by server, under `accounts/<server-slug>.json`, and SHALL load an account only when its stored server matches the requested server. A mismatch MUST trigger registration of a new account for the requested server instead of reusing the existing account URL or key.

#### Scenario: Staging and production are isolated
- **WHEN** a production account exists and `issue --test` runs
- **THEN** the staging account is used for the order
- **AND** the production account is not modified

#### Scenario: Server mismatch creates an account
- **WHEN** an account exists for one server and a command requests a different server
- **THEN** a new account is registered for the requested server

### Requirement: Staging flag is honored
A `--test` or `--dry-run` request SHALL use the Let's Encrypt staging directory regardless of any previously stored account server. A certificate issued against staging SHALL store the staging setting and renew against staging until reissued against production.

#### Scenario: Test overrides stored server
- **WHEN** `issue --test` runs while a production account exists
- **THEN** the order is created against the staging directory

#### Scenario: Staging certificate renews on staging
- **WHEN** a certificate stored with `staging: true` is renewed without `--test`
- **THEN** the renewal uses the staging directory

### Requirement: Server selection
The `issue` and `register` commands SHALL accept `--server <url>` and use it as the ACME directory URL. The default SHALL remain the Let's Encrypt production directory.

#### Scenario: Custom server URL
- **WHEN** `issue --server https://acme.example/directory` runs
- **THEN** the directory and all subsequent ACME requests use that URL

### Requirement: Account migration
On first use, if a legacy `account.json` exists and no account for its server is stored, the system SHALL import it into the new per-server location without requiring re-registration.

#### Scenario: Legacy account is imported
- **WHEN** `~/.acmers/account.json` exists and the per-server store is empty
- **THEN** the account is available for its stored server
- **AND** the legacy file is left intact

### Requirement: Account key protection
The system SHALL write account files with mode `0600` in a directory with mode `0700`.

#### Scenario: Account file permissions
- **WHEN** an account is registered or imported
- **THEN** the account file mode is `0600`

### Requirement: Account inspection
The system SHALL provide `show-account` to print the active account URL, server, and contact email when stored.

#### Scenario: Showing the active account
- **WHEN** `acmers show-account` runs
- **THEN** the account URL and server are printed
