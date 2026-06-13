## ADDED Requirements

### Requirement: Linode v3 API authentication
The system SHALL authenticate with Linode API v3 by including the API key as a query parameter: api_key={key} in all API requests to https://api.linode.com/.

Env vars: LINODE_API_KEY

#### Scenario: Valid API key
- **WHEN** a valid LINODE_API_KEY is provided
- **THEN** the system SHALL include it in all API request URLs

### Requirement: Linode v3 domain resolution
The system SHALL resolve the domain by calling api_action=domain.list and searching the response body for "DOMAIN":"{candidate}" where candidate is progressively shorter suffixes of the input domain. The system SHALL extract the DOMAINID from the matching entry and compute the sub_domain as the prefix before the matched domain.

#### Scenario: Domain found
- **WHEN** a matching domain is found in the domain list
- **THEN** the system SHALL return the domain ID and subdomain

#### Scenario: Domain not found
- **WHEN** no matching domain is found
- **THEN** the system SHALL return Error::Provider("Linode: domain not found for {domain}")

### Requirement: Linode v3 add TXT record
The system SHALL create a TXT record by calling api_action=domain.resource.create with parameters DomainID, Type=TXT, Name={sub_domain}, and Target={value}. The system SHALL verify the response contains "RESOURCEID" or "ResourceID".

#### Scenario: Successful TXT record creation
- **WHEN** the API response contains a resource ID
- **THEN** the system SHALL return Ok(())

#### Scenario: API error on add
- **WHEN** the API response does not contain a resource ID
- **THEN** the system SHALL return Error::Provider with the response body

### Requirement: Linode v3 remove TXT record (idempotent)
The system SHALL list resources via api_action=domain.resource.list with DomainID, find the resource matching NAME="{sub_domain}", extract its RESOURCEID, then call api_action=domain.resource.delete with DomainID and ResourceID. If any step fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the matching resource is found and deleted
- **THEN** the system SHALL return Ok(())

#### Scenario: Resource not found
- **WHEN** no matching resource is found
- **THEN** the system SHALL return Ok(()) (idempotent)
