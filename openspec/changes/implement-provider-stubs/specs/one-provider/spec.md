## ADDED Requirements

### Requirement: One.com session authentication
The system SHALL authenticate with One.com by POSTing loginDomain=true&displayUsername={user}&username={user}&targetDomain=&password1={pass}&loginTarget= to https://www.one.com/admin/login.do with Content-Type application/x-www-form-urlencoded. The system SHALL extract the OneSIDCrmAdmin cookie from the Set-Cookie response header and use it as the Cookie header for subsequent requests.

Env vars: ONE_Username, ONE_Password

#### Scenario: Successful login
- **WHEN** valid username and password are provided
- **THEN** the system SHALL obtain a session cookie and use it for API requests

#### Scenario: Login failure
- **WHEN** the response does not contain a OneSIDCrmAdmin cookie
- **THEN** the system SHALL return Error::Provider("One.com login: session cookie not found")

### Requirement: One.com zone resolution
The system SHALL resolve the zone by trying progressively shorter suffixes and calling GET https://www.one.com/admin/api/domains/{candidate}/dns/custom_records with the session cookie. The first suffix whose response does not contain "CRMRST_000302" is the zone.

#### Scenario: Zone found
- **WHEN** a suffix returns a valid response (no CRMRST_000302 error)
- **THEN** the system SHALL return that suffix as the main domain and the prefix as the subdomain

#### Scenario: Zone not found
- **WHEN** all suffixes return CRMRST_000302
- **THEN** the system SHALL return Error::Provider("One.com: zone not found for {domain}")

### Requirement: One.com add TXT record
The system SHALL create a TXT record by POSTing JSON to /admin/api/domains/{main}/dns/custom_records with type "dns_custom_records" and attributes: priority (0), ttl (600), type ("TXT"), prefix (subdomain), and content (value).

#### Scenario: Successful TXT record creation
- **WHEN** the API accepts the record (HTTP < 400)
- **THEN** the system SHALL return Ok(())

### Requirement: One.com remove TXT record (idempotent)
The system SHALL list records via GET /admin/api/domains/{main}/dns/custom_records, find the record in result.data matching type "TXT", prefix (subdomain), and content (value), then DELETE /admin/api/domains/{main}/dns/custom_records/{id}. If any step fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the matching record is found and deleted
- **THEN** the system SHALL return Ok(())

#### Scenario: Record not found
- **WHEN** no matching record is found
- **THEN** the system SHALL return Ok(()) (idempotent)
