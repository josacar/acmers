## ADDED Requirements

### Requirement: Curanet OAuth2 authentication
The system SHALL authenticate with Curanet API using OAuth2 client credentials flow by POSTing to https://apiauth.dk.team.blue/auth/realms/Curanet/protocol/openid-connect/token with grant_type=client_credentials, client_id, client_secret, and scope=dns. The system SHALL extract the access_token from the JSON response and use it as a Bearer token in subsequent API calls.

Env vars: CURANET_AUTH_CLIENT_ID, CURANET_AUTH_CLIENT_SECRET

#### Scenario: Successful OAuth2 token retrieval
- **WHEN** valid CURANET_AUTH_CLIENT_ID and CURANET_AUTH_CLIENT_SECRET are provided
- **THEN** the system SHALL obtain an access_token and use it for API requests

#### Scenario: Invalid credentials
- **WHEN** invalid client_id or client_secret are provided
- **THEN** the system SHALL return Error::Provider with the authentication error message

### Requirement: Curanet zone resolution
The system SHALL resolve the DNS zone for a given domain by trying progressively shorter suffixes (e.g., for _acme-challenge.www.example.com: try www.example.com, then example.com) and checking if GET https://api.curanet.dk/dns/v1/Domains/{zone}/Records returns a valid response (not containing "Entity not found" or "Bad Request").

#### Scenario: Zone found at domain level
- **WHEN** domain is _acme-challenge.example.com and example.com is a valid zone
- **THEN** the system SHALL identify example.com as the zone

#### Scenario: Zone not found
- **WHEN** no suffix matches a valid zone
- **THEN** the system SHALL return Error::Provider("Curanet: zone not found for {domain}")

### Requirement: Curanet add TXT record
The system SHALL create a TXT record by POSTing JSON to https://api.curanet.dk/dns/v1/Domains/{zone}/Records with fields: name (full domain name), type ("TXT"), ttl (60), priority (0), and data (TXT value). The system SHALL verify the response contains the TXT value.

#### Scenario: Successful TXT record creation
- **WHEN** a valid zone is found and the API accepts the record
- **THEN** the system SHALL return Ok(())

#### Scenario: API error on add
- **WHEN** the API returns HTTP 400+ or response does not contain the value
- **THEN** the system SHALL return Error::Provider with the error details

### Requirement: Curanet remove TXT record (idempotent)
The system SHALL list records via GET https://api.curanet.dk/dns/v1/Domains/{zone}/Records, find the record matching the name, type "TXT", and data value, then DELETE https://api.curanet.dk/dns/v1/Domains/{zone}/Records/{id}. If any step fails (zone lookup, record list, record not found), the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the record exists and is successfully deleted
- **THEN** the system SHALL return Ok(())

#### Scenario: Record not found
- **WHEN** no matching TXT record is found in the list
- **THEN** the system SHALL return Ok(()) (idempotent)

#### Scenario: Zone lookup failure
- **WHEN** the zone cannot be resolved
- **THEN** the system SHALL return Ok(()) (idempotent)
