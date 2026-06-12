## ADDED Requirements

### Requirement: Debian package directory structure
The project SHALL contain a `debian/` directory with valid Debian packaging metadata that produces an installable `.deb` package.

#### Scenario: Package control file exists
- **WHEN** the `debian/control` file is inspected
- **THEN** it SHALL contain `Source`, `Package`, `Version`, `Section`, `Priority`, `Maintainer`, `Architecture`, and `Description` fields

#### Scenario: Package architecture field
- **WHEN** the `.deb` is built
- **THEN** the `Architecture` field SHALL match the target CPU architecture (amd64 or arm64) as set by the build script

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

### Requirement: Package build script
A build script SHALL exist that arranges files into the DPKG layout and invokes `dpkg-deb --build` to produce the `.deb` file, without requiring `cargo-deb` or any Rust-only build tool.

#### Scenario: Build produces .deb file
- **WHEN** the build script is executed on a system with `dpkg-deb` available
- **THEN** a `.deb` file named `acmers_<version>_<arch>.deb` SHALL be produced

#### Scenario: Build script accepts architecture argument
- **WHEN** the build script is invoked with an architecture parameter (e.g., `amd64` or `arm64`)
- **THEN** the resulting `.deb` SHALL be tagged with that architecture

### Requirement: Package version from Cargo.toml
The `.deb` package version SHALL be derived from the `version` field in `Cargo.toml`.

#### Scenario: Version matches Cargo.toml
- **WHEN** `Cargo.toml` version is `0.1.0`
- **THEN** the built `.deb` SHALL have version `0.1.0`
