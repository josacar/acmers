## ADDED Requirements

### Requirement: Yandex 360 API authentication
The system SHALL authenticate with Yandex 360 API using OAuth access token in the Authorization header as "OAuth {token}" for all requests to https://api360.yandex.net/directory/v1/.

Env vars: YANDEX360_ACCESS_TOKEN, YANDEX360_ORG_ID (optional)

#### Scenario: Valid access token
- **WHEN** a valid YANDEX360_ACCESS_TOKEN is provided
- **THEN** the system SHALL include it in the Authorization header for all API requests

### Requirement: Yandex 360 organization resolution
The system SHALL resolve the organization ID by either using the provided YANDEX360_ORG_ID or calling GET /org to list organizations and extracting the id from the first organization in the response.

#### Scenario: Explicit org ID provided
- **WHEN** YANDEX360_ORG_ID is set
- **THEN** the system SHALL use it directly

#### Scenario: Auto-discover org ID
- **WHEN** YANDEX360_ORG_ID is not set
- **THEN** the system SHALL call /org and use the first organization's ID

### Requirement: Yandex 360 domain resolution
The system SHALL resolve the root domain by calling GET /org/{org_id}/domains and matching the input domain against the name field of each domain in the response.

#### Scenario: Domain found
- **WHEN** a matching domain is found in the organization
- **THEN** the system SHALL return the root domain name

#### Scenario: Domain not found
- **WHEN** no matching domain is found
- **THEN** the system SHALL return Error::Provider("Yandex360: domain not found for {domain}")

### Requirement: Yandex 360 add TXT record
The system SHALL create a TXT record by POSTing JSON to /org/{org_id}/domains/{root_domain}/dns with fields: name (subdomain), type ("TXT"), ttl (60), and text (TXT value). The system SHALL verify the response contains "recordId".

#### Scenario: Successful TXT record creation
- **WHEN** the API response contains recordId
- **THEN** the system SHALL return Ok(())

### Requirement: Yandex 360 remove TXT record (idempotent)
The system SHALL list records via GET /org/{org_id}/domains/{root_domain}/dns?perPage=100, find the record matching the text value, then DELETE /org/{org_id}/domains/{root_domain}/dns/{recordId}. If any step fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the matching record is found and deleted
- **THEN** the system SHALL return Ok(())

#### Scenario: Record not found
- **WHEN** the response does not contain the value
- **THEN** the system SHALL return Ok(()) (idempotent)
