## Why

acmers currently has no distribution packaging — users must install via `cargo install` (requiring a Rust toolchain) or build from source. Debian packaging makes the binary available via `apt` on Debian/Ubuntu systems, lowering the barrier for adoption and enabling automated deployment workflows. Cross-compilation support extends coverage to arm64 servers, Raspberry Pi, and other non-amd64 targets commonly used in self-hosted infrastructure.

## What Changes

- Add `debian/` packaging directory with control files for building `.deb` packages
- Add CI workflow to build release binaries for `amd64` and `arm64` architectures
- Add CI workflow to assemble `.deb` packages from release binaries
- Configure cross-compilation toolchain for ARM targets in CI
- Add a publish job to GitHub Releases that attaches `.deb` artifacts

## Capabilities

### New Capabilities
- `debian-packaging`: Generate installable `.deb` packages containing the acmers binary, manpage, and shell completions for amd64 and arm64 architectures
- `cross-compile-ci`: GitHub Actions workflow to cross-compile and build Debian packages on push/PR, with release publishing

### Modified Capabilities
<!-- No existing capabilities are changing -->

## Impact

- New `debian/` directory with packaging metadata (`control`, `rules`, `changelog`, `install`, `compat`)
- New `.github/workflows/release.yml` for release builds and package assembly
- Modified `.github/workflows/ci.yml` to add cross-compilation test targets
- Zero Rust code changes — packaging is entirely external to the binary
- Depends on GitHub Actions (already in use), `cargo-deb` or manual `dpkg-deb` invocation
