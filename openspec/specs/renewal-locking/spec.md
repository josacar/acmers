# renewal locking

## Purpose

Defines the exclusive lock that prevents overlapping issuance and renewal runs.

## Requirements


### Requirement: Exclusive renewal lock
The `issue` and `renew` commands SHALL acquire an exclusive lock in the acmers home directory before performing any ACME or file operation, and SHALL release it when the command ends.

#### Scenario: Timer and manual run overlap
- **WHEN** the timer is running `renew --all`
- **AND** a manual `acmers renew -d example.com` starts
- **THEN** the manual run fails with a clear "another acmers process is running" error
- **AND** it does not create an ACME order

#### Scenario: Lock released after success
- **WHEN** a renewal completes successfully
- **THEN** the lock file is removed

#### Scenario: Lock released after failure
- **WHEN** a renewal fails
- **THEN** the lock file is removed

### Requirement: Stale lock recovery
If the lock file exists but its recorded process ID is no longer running, the system SHALL remove the stale lock and acquire a new one.

#### Scenario: Previous process crashed
- **WHEN** the lock file references a PID that does not exist under `/proc`
- **THEN** the next run removes the stale lock and proceeds

### Requirement: Lock location
The lock file SHALL be stored as `.lock` inside the resolved acmers home directory, so per-home and system-wide installations do not block each other.

#### Scenario: Separate homes
- **WHEN** two processes use different `ACMERS_HOME` directories
- **THEN** each can hold its own lock concurrently
