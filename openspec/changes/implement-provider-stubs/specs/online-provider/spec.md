## ADDED Requirements

### Requirement: Online.net API authentication
The system SHALL authenticate with Online.net API using Bearer token authentication with ONLINE_API_KEY in the Authorization header, plus X-Pretty-JSON: 1 header for all requests to https://api.online.net/api/v1/.

Env vars: ONLINE_API_KEY

#### Scenario: Valid API key
- **WHEN** a valid ONLINE_API_KEY is provided
- **THEN** the system SHALL include it as a Bearer token in all API requests

### Requirement: Online.net zone resolution
The system SHALL resolve the zone by trying progressively shorter suffixes and calling GET domain/{candidate}/version/active. The system SHALL extract the uuid_ref from the response as the real_dns_version and compute the sub_domain.

#### Scenario: Zone found
- **WHEN** a valid zone is found (response does not contain "Domain not found")
- **THEN** the system SHALL return the zone name, subdomain, and version UUID

#### Scenario: Zone not found
- **WHEN** no valid zone is found
- **THEN** the system SHALL return Error::Provider("Online.net: zone not found for {domain}")

### Requirement: Online.net zone versioning workflow for add
The system SHALL follow this workflow for adding TXT records:
1. Create a temporary zone version via POST domain/{zone}/version with name=acmers
2. Create a dummy TXT record in the temp version (Online.net requires non-empty versions)
3. Enable the temporary version via PATCH domain/{zone}/version/{temp}/enable
4. Create the actual TXT record in the real version via POST domain/{zone}/version/{real}/zone
5. Re-enable the real version via PATCH domain/{zone}/version/{real}/enable
6. Destroy the temporary version via DELETE domain/{zone}/version/{temp}

#### Scenario: Successful TXT record creation
- **WHEN** all steps in the versioning workflow succeed
- **THEN** the system SHALL return Ok(())

### Requirement: Online.net remove TXT record (idempotent)
The system SHALL find the record ID by searching the active version for a record matching the subdomain and value (with \u0022 quoting), then follow the zone versioning workflow to delete it. If any step fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the record is found and the versioning workflow completes
- **THEN** the system SHALL return Ok(())

#### Scenario: Record not found
- **WHEN** no matching record is found
- **THEN** the system SHALL return Ok(()) (idempotent)
