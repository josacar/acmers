## ADDED Requirements

### Requirement: OpenProvider XML API authentication
The system SHALL authenticate with OpenProvider XML API by including username and password hash in the XML request body as <credentials><username>{user}</username><hash>{hash}</hash></credentials> wrapped in <openXML> tags. All requests SHALL be POSTed to https://api.openprovider.eu/ with Content-Type application/xml.

Env vars: OPENPROVIDER_USER, OPENPROVIDER_PASSWORD_HASH

#### Scenario: Valid credentials
- **WHEN** valid OPENPROVIDER_USER and OPENPROVIDER_PASSWORD_HASH are provided
- **THEN** the system SHALL include them in the XML credentials block for all API requests

#### Scenario: API error
- **WHEN** the API response does not contain <code>0</code>
- **THEN** the system SHALL return Error::Provider with the response body

### Requirement: OpenProvider domain resolution
The system SHALL resolve the domain by trying progressively shorter suffixes and calling searchDomainRequest with the first part of the domain as domainNamePattern. The system SHALL extract the name and extension from the matching <domain> element in the response.

#### Scenario: Domain found
- **WHEN** a matching domain is found in the search results
- **THEN** the system SHALL return the domain name and extension

#### Scenario: Domain not found
- **WHEN** no matching domain is found
- **THEN** the system SHALL return Error::Provider("OpenProvider: domain not found for {domain}")

### Requirement: OpenProvider add TXT record
The system SHALL add a TXT record by:
1. Calling searchZoneRecordDnsRequest to get all existing records
2. Extracting and simplifying existing record items (A, AAAA, CNAME, MX, SPF, SRV, TXT, TLSA, SSHFP, CAA)
3. Appending a new TXT item with the subdomain, value, and ttl 600
4. Calling modifyZoneDnsRequest with all records (existing + new)

#### Scenario: Successful TXT record creation
- **WHEN** the modifyZoneDnsRequest succeeds (response contains <code>0</code>)
- **THEN** the system SHALL return Ok(())

### Requirement: OpenProvider remove TXT record (idempotent)
The system SHALL remove a TXT record by:
1. Calling searchZoneRecordDnsRequest to get all existing records
2. Filtering out records whose name contains the target domain
3. Calling modifyZoneDnsRequest with the filtered records (excluding the target)

If any step fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the modifyZoneDnsRequest succeeds with filtered records
- **THEN** the system SHALL return Ok(())

#### Scenario: Domain resolution failure
- **WHEN** the domain cannot be resolved
- **THEN** the system SHALL return Ok(()) (idempotent)
