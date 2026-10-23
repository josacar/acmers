# dns01 validation

## Purpose

Defines DNS TXT propagation polling for DNS-01 challenges using only the Rust standard library.

## Requirements


### Requirement: DNS TXT propagation polling
After creating a DNS-01 TXT record, the system SHALL poll DNS resolvers until the expected TXT value is observed or a timeout expires, instead of relying only on a fixed sleep.

#### Scenario: Value becomes visible
- **WHEN** the TXT record has propagated to a configured resolver
- **THEN** polling succeeds and challenge validation proceeds without waiting the full timeout

#### Scenario: Value never appears
- **WHEN** the expected TXT value is not observed before the timeout
- **THEN** the command fails with an error naming the domain and the expected value
- **AND** the TXT record is still removed

### Requirement: Resolver configuration
The system SHALL read default nameservers from `/etc/resolv.conf` and SHALL accept `--dns-servers <ip[,ip...]>` to override them.

#### Scenario: Custom resolvers
- **WHEN** `--dns-servers 1.1.1.1,8.8.8.8` is supplied
- **THEN** both addresses are queried

#### Scenario: Default resolvers
- **WHEN** no `--dns-servers` value is supplied
- **THEN** the nameservers in `/etc/resolv.conf` are used

### Requirement: Poll timeout and fallback
The system SHALL accept `--dns-timeout <seconds>` for the propagation poll. When no resolver can be reached or the poll cannot run, the system SHALL fall back to sleeping for `--dnssleep` seconds if a sleep value is configured.

#### Scenario: Timeout is honored
- **WHEN** `--dns-timeout 60` is set and the value does not appear
- **THEN** polling stops after approximately 60 seconds

#### Scenario: Fallback to sleep
- **WHEN** no resolver responds and `--dnssleep 30` is configured
- **THEN** the system waits 30 seconds before signaling the challenge

### Requirement: Pure standard library DNS
The propagation check SHALL use `std::net::UdpSocket` and MUST NOT call external tools such as `dig`, `nslookup`, or `host`.

#### Scenario: No external process
- **WHEN** a propagation check runs
- **THEN** no external DNS utility is executed

### Requirement: Multiple TXT values
The propagation check SHALL treat success as observing the expected value among any TXT values returned for the challenge name.

#### Scenario: Coexisting TXT records
- **WHEN** the challenge name has other TXT records in addition to the expected value
- **THEN** polling succeeds once the expected value is present
