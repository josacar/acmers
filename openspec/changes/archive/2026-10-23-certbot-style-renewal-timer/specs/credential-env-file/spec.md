## ADDED Requirements

### Requirement: Environment file option
The `issue` and `renew` commands SHALL accept `--env-file <path>` and load provider credentials from that file. The systemd service SHALL load the same file through `EnvironmentFile=-/etc/acmers/env`.

#### Scenario: Credentials from file
- **WHEN** `/etc/acmers/env` contains `CF_Token=abc`
- **AND** `acmers renew --all --env-file /etc/acmers/env` runs
- **THEN** the Cloudflare provider receives `CF_Token=abc`

#### Scenario: Missing file is an error
- **WHEN** `--env-file /nonexistent` is supplied
- **THEN** the command fails with a clear error before contacting the ACME server

#### Scenario: Service loads the same path
- **WHEN** `acmers.service` is inspected
- **THEN** it contains `EnvironmentFile=-/etc/acmers/env`

### Requirement: Environment file format
The environment file SHALL contain one `KEY=VALUE` pair per line. Blank lines and lines beginning with `#` SHALL be ignored. Surrounding single or double quotes around a value SHALL be removed.

#### Scenario: Comments and blank lines
- **WHEN** the file contains comments and blank lines
- **THEN** only `KEY=VALUE` lines are loaded

#### Scenario: Quoted value
- **WHEN** the file contains `CF_Token="abc def"`
- **THEN** the loaded value is `abc def`

#### Scenario: Malformed line
- **WHEN** a non-comment line has no `=`
- **THEN** the command fails with an error naming the line number

### Requirement: Precedence
Values from `-e KEY=VALUE` SHALL override `--env-file` values, which SHALL override the process environment.

#### Scenario: CLI override wins
- **WHEN** the env file sets `CF_Token=file`
- **AND** `-e CF_Token=cli` is passed
- **THEN** the provider receives `CF_Token=cli`

### Requirement: Secret values are not logged
The system SHALL NOT print environment file values in normal output or error messages.

#### Scenario: Parse error hides value
- **WHEN** an environment file line is malformed
- **THEN** the error reports the path and line number without the line contents
