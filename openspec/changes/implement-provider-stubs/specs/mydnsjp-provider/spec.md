## ADDED Requirements

### Requirement: MyDNS.JP authentication
The system SHALL authenticate with MyDNS.JP using HTTP Basic authentication with MYDNSJP_MasterID as username and MYDNSJP_MasterPassword as password, encoded in the Authorization header.

Env vars: MYDNSJP_MasterID, MYDNSJP_MasterPassword

#### Scenario: Valid credentials
- **WHEN** valid MasterID and MasterPassword are provided
- **THEN** the system SHALL include the Basic auth header in API requests

### Requirement: MyDNS.JP root domain discovery
The system SHALL discover the root domain by POSTing MENU=100&masterid={id}&masterpwd={password} to https://www.mydns.jp/members/ and extracting the value of DNSINFO[domainname] from the HTML response.

#### Scenario: Successful domain discovery
- **WHEN** valid credentials are provided
- **THEN** the system SHALL extract and return the root domain name

#### Scenario: Login failure
- **WHEN** invalid credentials are provided or domain name not found in response
- **THEN** the system SHALL return Error::Provider with an appropriate error message

### Requirement: MyDNS.JP add TXT record
The system SHALL add a TXT record by POSTing CERTBOT_DOMAIN={root_domain}&CERTBOT_VALIDATION={value}&EDIT_CMD=REGIST to https://www.mydns.jp/directedit.html with Basic auth. The system SHALL verify the response contains "OK.".

#### Scenario: Successful TXT record creation
- **WHEN** the API responds with "OK."
- **THEN** the system SHALL return Ok(())

#### Scenario: API error on add
- **WHEN** the API response does not contain "OK."
- **THEN** the system SHALL return Error::Provider with the response body

### Requirement: MyDNS.JP remove TXT record (idempotent)
The system SHALL remove a TXT record by POSTing CERTBOT_DOMAIN={root_domain}&CERTBOT_VALIDATION={value}&EDIT_CMD=DELETE to https://www.mydns.jp/directedit.html with Basic auth. If root domain discovery fails, the system SHALL return Ok(()) without error.

#### Scenario: Successful record removal
- **WHEN** the API responds with "OK."
- **THEN** the system SHALL return Ok(())

#### Scenario: Root domain discovery failure
- **WHEN** the root domain cannot be discovered
- **THEN** the system SHALL return Ok(()) (idempotent)
