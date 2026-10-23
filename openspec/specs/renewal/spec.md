# renewal

## Purpose

Defines renewal configuration, SAN preservation, bulk renewal, renewal windows, and certificate lifecycle commands.

## Requirements


### Requirement: Complete renewal configuration
The system SHALL persist a renewal configuration per certificate that records every requested domain, the DNS provider, the account email, the ACME server, the challenge type, the key type, the DNS wait settings, the staging flag, the install paths, and the configured hooks.

#### Scenario: Multi-domain issuance is recorded
- **WHEN** a certificate is issued for `example.com`, `www.example.com`, and `*.example.com`
- **THEN** `renewal.json` contains all three names in a `domains` array

#### Scenario: Renewal records deployment settings
- **WHEN** a certificate is issued with install paths and a reload command
- **THEN** `renewal.json` contains those paths and the command

### Requirement: Renewal preserves all SANs
The `renew` command SHALL request a new certificate for every domain stored in the renewal configuration, preserving SANs and wildcards. It MUST NOT reduce the request to the primary domain.

#### Scenario: Renewing a multi-domain certificate
- **WHEN** `renew -d example.com` runs for a certificate stored with three domains
- **THEN** the new order includes all three identifiers

#### Scenario: Legacy configuration fallback
- **WHEN** a renewal configuration has no `domains` array
- **THEN** the system renews for the certificate directory name as a single domain

### Requirement: Bulk renewal
The `renew` command SHALL renew all stored certificates when no `-d` is supplied, and SHALL accept `--all` as an explicit equivalent. The `cron` command MUST behave as an alias for `renew --all`.

#### Scenario: Renew all certificates
- **WHEN** `acmers renew` runs with no domain argument and three certificate directories exist
- **THEN** every certificate due for renewal is processed

#### Scenario: Cron aliases bulk renewal
- **WHEN** `acmers cron` runs
- **THEN** it renews every certificate due for renewal using the same behavior as `renew --all`

### Requirement: Renewal decision
The system SHALL renew a certificate when its remaining validity is at or below the configured threshold, defaulting to 30 days. The threshold SHALL be configurable with `--days`. The system SHALL use an ARI suggested window instead of the fixed threshold when ARI data is available.

#### Scenario: Certificate is due
- **WHEN** a certificate expires in 20 days and the threshold is 30 days
- **THEN** the certificate is renewed

#### Scenario: Certificate is not due
- **WHEN** a certificate expires in 60 days and the threshold is 30 days
- **THEN** the certificate is skipped without contacting the ACME server for a new order

#### Scenario: ARI window is respected
- **WHEN** ARI reports a suggested renewal window that starts in the future
- **THEN** the certificate is not renewed before that window starts

### Requirement: Renewal control flags
The `renew` command SHALL support `--force` to renew regardless of expiry, `--dry-run` to perform a staging issuance without deploying or revoking, and `--keep-until-expiring` to skip certificates that are not due.

#### Scenario: Forced renewal
- **WHEN** `renew -d example.com --force` runs for a certificate valid for 80 days
- **THEN** a new order is created

#### Scenario: Dry run does not deploy
- **WHEN** `renew --all --dry-run` runs
- **THEN** issuance uses the staging server
- **AND** no configured install paths, reload command, or deploy hook is executed
- **AND** the stored production certificate remains unchanged

#### Scenario: Keep until expiring
- **WHEN** `renew --keep-until-expiring` runs for a certificate that is not due
- **THEN** the certificate is left unchanged

### Requirement: Renewal reporting and exit status
The `renew` command SHALL print one line per certificate indicating renewed, skipped, or failed, and SHALL exit non-zero if any certificate failed.

#### Scenario: Mixed results
- **WHEN** one certificate renews, one is skipped, and one fails
- **THEN** the summary reports one of each
- **AND** the process exits with a non-zero status

#### Scenario: All skipped
- **WHEN** every certificate is not due
- **THEN** all lines report skipped
- **AND** the process exits successfully

### Requirement: Certificate management commands
The system SHALL provide `certificates` to list stored certificate names, domains, and expiry, `show -d <domain>` to print a certificate's details, `show-account` to print the active account, and `delete -d <domain>` to remove a certificate directory.

#### Scenario: Listing certificates
- **WHEN** `acmers certificates` runs
- **THEN** each stored certificate is listed with its domains and expiry date

#### Scenario: Deleting a certificate
- **WHEN** `acmers delete -d example.com` runs
- **THEN** the certificate directory is removed

### Requirement: Revocation options
The `revoke` command SHALL accept `--reason` with the values `unspecified`, `keycompromise`, `affiliationchanged`, `superseded`, `cessationofoperation`, and `privilegewithdrawn`, mapping them to ACME reason codes. It SHALL accept `--delete-after-revoke` to remove local files after a successful revocation.

#### Scenario: Revoke with a reason
- **WHEN** `revoke -d example.com --reason keycompromise` runs
- **THEN** the revocation request uses ACME reason code 1

#### Scenario: Delete after revoke
- **WHEN** `revoke -d example.com --delete-after-revoke` succeeds
- **THEN** the local certificate directory is removed

### Requirement: Preferred chain
The `issue` and `renew` commands SHALL accept `--preferred-chain <issuer common name>` and SHALL select an alternate chain whose issuer matches when the ACME server offers one.

#### Scenario: Preferred chain is available
- **WHEN** a certificate is downloaded and `--preferred-chain "ISRG Root X1"` is set
- **THEN** the chain whose issuer common name matches is selected
- **AND** the preference is stored for future renewals

#### Scenario: Preferred chain is unavailable
- **WHEN** no alternate chain matches the preference
- **THEN** the default chain is used
- **AND** a warning is printed
