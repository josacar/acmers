# Debian Packaging

## Purpose

Builds installable `.deb` packages for the acmers binary with `debhelper` and
`dpkg-buildpackage`. The package ships the systemd renewal timer and service,
the `/etc/acmers/env.example` credential template, and a non-systemd cron
fallback.

## Requirements

### Requirement: Debian package directory structure
The project SHALL contain a `debian/` directory with valid Debian packaging metadata that produces an installable `.deb` package.

#### Scenario: Package control file exists
- **WHEN** the `debian/control` file is inspected
- **THEN** it SHALL contain `Source`, `Package`, `Version`, `Section`, `Priority`, `Maintainer`, `Architecture`, and `Description` fields

#### Scenario: Package architecture field
- **WHEN** the `.deb` is built on an amd64 or arm64 host
- **THEN** the `Architecture` field SHALL match the build host architecture

### Requirement: Debhelper build
The package SHALL be built with `dpkg-buildpackage` and `debhelper`; it MUST NOT require `cargo-deb` or a custom `.deb` assembly script.

#### Scenario: Build produces .deb file
- **WHEN** `dpkg-buildpackage -d -b -us -uc` runs on a supported host
- **THEN** a `.deb` named `acmers_<version>_<arch>.deb` SHALL be produced

#### Scenario: Build runs the test suite
- **WHEN** the package build runs
- **THEN** `cargo test` SHALL execute as part of `dh_auto_test`

### Requirement: Binary installation path
The `.deb` package SHALL install the `acmers` binary to `/usr/bin/acmers` with executable permissions.

#### Scenario: Binary present after install
- **WHEN** the `.deb` is installed with `dpkg -i`
- **THEN** `/usr/bin/acmers` SHALL exist and be executable

### Requirement: Manpage installation
The `.deb` package SHALL install a manpage to `/usr/share/man/man1/acmers.1`.

#### Scenario: Manpage accessible
- **WHEN** the `.deb` is installed
- **THEN** `man acmers` SHALL display the manpage

### Requirement: Renewal units
The package SHALL install `acmers.service` and `acmers.timer` to `/usr/lib/systemd/system`, SHALL enable and start the timer on first install, and SHALL NOT enable the static service separately.

#### Scenario: Timer is enabled on install
- **WHEN** the package is configured on a systemd host
- **THEN** `acmers.timer` SHALL be enabled
- **AND** the generated `postinst` SHALL start only `acmers.timer`

#### Scenario: Service is static
- **WHEN** `acmers.service` is inspected
- **THEN** it SHALL have no `[Install]` section

#### Scenario: Service can deploy certificates
- **WHEN** `acmers.service` is inspected
- **THEN** it SHALL NOT set `ProtectSystem=strict`
- **AND** it SHALL load `EnvironmentFile=-/etc/acmers/env`

### Requirement: Cron fallback
The package SHALL install `/etc/cron.d/acmers`, which runs renewal twice daily with a random sleep only when `/run/systemd/system` does not exist.

#### Scenario: Systemd host skips cron
- **WHEN** `/run/systemd/system` exists
- **THEN** the cron entry does not run renewal

### Requirement: Credential template and state directories
The package SHALL create `/etc/acmers` with mode `0700` and `/var/lib/acmers` with mode `0750`, and SHALL install `/etc/acmers/env.example` as a credential template.

#### Scenario: Directories exist
- **WHEN** the package is configured
- **THEN** `/etc/acmers` and `/var/lib/acmers` exist with restrictive ownership

#### Scenario: No secret shipped
- **WHEN** the package contents are inspected
- **THEN** only `env.example` is present, not a real credential file

### Requirement: Package version from Cargo.toml
The `.deb` package version SHALL be derived from the `version` field in `Cargo.toml`, and `debian/changelog` SHALL contain a matching entry at release time.

#### Scenario: Version matches Cargo.toml
- **WHEN** `Cargo.toml` version is `0.4.0`
- **THEN** the built `.deb` SHALL have version `0.4.0`
