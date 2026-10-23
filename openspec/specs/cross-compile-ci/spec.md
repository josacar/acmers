# Multi-Arch CI

## Purpose

GitHub Actions workflows that build and test acmers on native amd64 and arm64
runners, assemble `.deb` packages, and publish them to GitHub Releases on tag
push. No cross-compilation toolchain is required.

The capability retains its historical `cross-compile-ci` directory name; the
workflows now use native arm64 runners instead of cross-compilation.

## Requirements

### Requirement: CI test job
The CI workflow SHALL run on every push and pull request and SHALL build the release binary and run the full test suite.

#### Scenario: Push triggers tests
- **WHEN** a commit is pushed to any branch
- **THEN** `cargo build --release` and `cargo test` SHALL run on `ubuntu-latest`

#### Scenario: Pull request triggers tests
- **WHEN** a pull request is opened or updated
- **THEN** the same test job SHALL run

### Requirement: Native multi-arch package builds
The CI and release workflows SHALL build `.deb` packages on native amd64 and arm64 runners using a matrix.

#### Scenario: amd64 package
- **WHEN** the build matrix runs on `ubuntu-latest`
- **THEN** `dpkg-buildpackage` SHALL produce an amd64 `.deb`

#### Scenario: arm64 package
- **WHEN** the build matrix runs on `ubuntu-24.04-arm`
- **THEN** `dpkg-buildpackage` SHALL produce an arm64 `.deb`

#### Scenario: No cross toolchain
- **WHEN** the workflow is inspected
- **THEN** it SHALL NOT require `gcc-aarch64-linux-gnu` or a cross linker

### Requirement: Build artifacts
Each build job SHALL upload its `.deb` as a workflow artifact.

#### Scenario: Artifacts uploaded
- **WHEN** a build job completes successfully
- **THEN** its `.deb` SHALL be available as an `acmers-<arch>` artifact

### Requirement: Release on tag
The release workflow SHALL trigger on tags matching `v*`, download both architecture artifacts, and attach them to the GitHub Release.

#### Scenario: Version tag
- **WHEN** a tag such as `v0.4.0` is pushed
- **THEN** the release workflow SHALL build both architectures
- **AND** attach `acmers_<version>_amd64.deb` and `acmers_<version>_arm64.deb` to the release

#### Scenario: Release requires both builds
- **WHEN** either architecture build fails
- **THEN** the release job SHALL NOT publish
