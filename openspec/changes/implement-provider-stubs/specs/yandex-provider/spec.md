## ADDED Requirements

### Requirement: Yandex PDD API authentication
The system SHALL authenticate with Yandex PDD API using a Token header with format "Token {token}" for all requests to https://pddimpex2.yandex.net/.

Env vars: YANDEX_Token

#### Scenario: Valid token
- **WHEN** a valid YANDEX_Token is provided
- **THEN** the system SHALL include it in the Authorization header for all API requests

### Requirement: Yandex PDD domain resolution
The system SHALL resolve the root domain by calling GET /get_domain_list and matching the input domain against the name field of each domain in the "domains" array of the JSON response.

#### Scenario: Domain found
- **WHEN** a matching domain is found in the list
- **THEN** the system SHALL return the root domain name

#### Scenario: Domain not found
- **WHEN** no matching domain is found
- **THEN** the system SHALL return Error::Provider("Yandex PDD: domain not found for {domain}")

### Requirement: Yandex PDD add TXT record
The system SHALL create a TXT record by POSTing domain={root}&type=TXT&subdomain={sub}&content={value}&ttl=60 to /add with Token auth. The system SHALL verify the JSON response has "success": "ok".

#### Scenario: Successful TXT record creation
- **WHEN** the API response indicates success
- **THEN** the system SHALL return Ok(())

#### Scenario: API error on add
- **WHEN** the API response does not indicate success or returns HTTP 400+
- **THEN** the system SHALL return Error::Provider with the error details

### Requirement: Yandex PDD remove TXT record (idempotent)
The system SHALL list records via GET /list?domain={root}, find the record matching type "TXT", subdomain, and content, extract its record_id, then POST domain={root}&record_id={id} to /del. If any step fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the matching record is found and deleted
- **THEN** the system SHALL return Ok(())

#### Scenario: Record not found
- **WHEN** no matching record is found in the list
- **THEN** the system SHALL return Ok(()) (idempotent)
