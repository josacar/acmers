## ADDED Requirements

### Requirement: UnoEuro API authentication
The system SHALL authenticate with UnoEuro/Simply.com API by including the user and key in the URL path: https://api.simply.com/1/{user}/{key}/. The system SHALL accept both UNOEURO_User/UNOEURO_Password and UNO_User/UNO_Key env var names.

Env vars: UNOEURO_User (or UNO_User), UNOEURO_Password (or UNO_Key)

#### Scenario: Valid credentials
- **WHEN** valid user and key are provided via either env var naming convention
- **THEN** the system SHALL include them in the URL path for all API requests

### Requirement: UnoEuro zone resolution
The system SHALL resolve the zone by trying progressively shorter suffixes and calling GET https://api.simply.com/1/{user}/{key}/my/products/{candidate}/dns/records. The first suffix whose response contains "\"status\": 200" is the zone.

#### Scenario: Zone found
- **WHEN** a suffix returns a response with status 200
- **THEN** the system SHALL return that suffix as the zone

#### Scenario: Zone not found
- **WHEN** no suffix returns a response with status 200
- **THEN** the system SHALL return Error::Provider("UnoEuro: zone not found for {domain}")

### Requirement: UnoEuro add TXT record
The system SHALL create a TXT record by POSTing JSON to /my/products/{zone}/dns/records with fields: name (full domain), type ("TXT"), data (value), ttl (120), and priority (0). The system SHALL verify the response contains "\"status\": 200".

#### Scenario: Successful TXT record creation
- **WHEN** the API response contains status 200
- **THEN** the system SHALL return Ok(())

#### Scenario: API error on add
- **WHEN** the API response does not contain status 200
- **THEN** the system SHALL return Error::Provider with the response body

### Requirement: UnoEuro remove TXT record (idempotent)
The system SHALL list records via GET /my/products/{zone}/dns/records, find the record matching name, type "TXT", and data value, then DELETE /my/products/{zone}/dns/records/{record_id}. If any step fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the matching record is found and deleted
- **THEN** the system SHALL return Ok(())

#### Scenario: Record not found
- **WHEN** no matching record is found or response doesn't contain status 200
- **THEN** the system SHALL return Ok(()) (idempotent)
