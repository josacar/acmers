# Cross-Compile CI

## Purpose

GitHub Actions workflows for cross-compiling acmers to amd64 and arm64 targets, building `.deb` packages, and publishing them to GitHub Releases on tag push.

## Requirements

### Requirement: Cross-compile release workflow
A GitHub Actions workflow SHALL exist that, on tag push matching `v*`, builds release binaries for amd64 and arm64, assembles `.deb` packages, and attaches them to the GitHub Release.

#### Scenario: Workflow triggers on version tag
- **WHEN** a git tag matching `v*` is pushed (e.g., `v0.2.0`)
- **THEN** the release workflow SHALL execute

#### Scenario: Workflow builds both architectures
- **WHEN** the release workflow runs
- **THEN** it SHALL produce a release binary for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`

#### Scenario: Workflow produces .deb artifacts
- **WHEN** the release workflow completes successfully
- **THEN** `.deb` files for amd64 and arm64 SHALL be attached to the GitHub Release

### Requirement: Cross-compile CI testing
The CI workflow SHALL verify that both amd64 and arm64 targets compile successfully on every PR and push to main.

#### Scenario: PR triggers cross-compile check
- **WHEN** a pull request is opened or updated
- **THEN** the CI workflow SHALL attempt to build for both `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`

#### Scenario: Build failure blocks merge
- **WHEN** cross-compilation for any target fails
- **THEN** the CI check SHALL report failure

### Requirement: ARM cross-compilation toolchain
The CI workflow SHALL install the `gcc-aarch64-linux-gnu` package to support cross-compilation of the `ring` crate for arm64.

#### Scenario: ARM toolchain configured
- **WHEN** the CI job for arm64 runs
- **THEN** the `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER` environment variable SHALL point to the `aarch64-linux-gnu-gcc` linker
