## ADDED Requirements

### Requirement: Google Domains API authentication
The system SHALL authenticate with Google Domains ACME DNS API using OAuth2 Bearer token in the Authorization header for all requests to https://acmedns.googleapis.com/v1/.

Env vars: GOOGLEDOMAINS_ACCESS_TOKEN

#### Scenario: Valid access token
- **WHEN** a valid GOOGLEDOMAINS_ACCESS_TOKEN is provided
- **THEN** the system SHALL include it as a Bearer token in all API requests

### Requirement: Google Domains zone resolution
The system SHALL resolve the zone by trying progressively shorter suffixes and POSTing to /acmeChallenges/{candidate}:start with a test record. The first suffix that returns HTTP < 400 is the zone. The system SHALL clean up the test record by calling :clear afterward.

#### Scenario: Zone found
- **WHEN** a suffix returns a successful response from the start endpoint
- **THEN** the system SHALL return that suffix as the zone

#### Scenario: Zone not found
- **WHEN** no suffix returns a successful response
- **THEN** the system SHALL return Error::Provider("Google Domains: zone not found for {domain}")

### Requirement: Google Domains add TXT record
The system SHALL create a TXT record by POSTing JSON with recordName and digest fields to /acmeChallenges/{zone}:start.

#### Scenario: Successful TXT record creation
- **WHEN** the API accepts the challenge record
- **THEN** the system SHALL return Ok(())

#### Scenario: API error on add
- **WHEN** the API returns HTTP 400+
- **THEN** the system SHALL return Error::Provider with the error details

### Requirement: Google Domains remove TXT record (idempotent)
The system SHALL remove a TXT record by POSTing JSON with recordName and digest fields to /acmeChallenges/{zone}:clear. If zone resolution fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the API clears the challenge record
- **THEN** the system SHALL return Ok(())

#### Scenario: Zone resolution failure
- **WHEN** the zone cannot be resolved
- **THEN** the system SHALL return Ok(()) (idempotent)
