## 1. Debian Packaging Files

- [x] 1.1 Create `debian/control` with package metadata (Source, Package, Version, Section, Priority, Maintainer, Architecture, Depends, Description)
- [x] 1.2 Create `debian/compat` with debhelper compatibility level
- [x] 1.3 Create `debian/rules` as a no-op build rules file (binary is pre-built)
- [x] 1.4 Create `debian/changelog` with initial entry matching Cargo.toml version
- [x] 1.5 Create `debian/install` with file path mappings for binary and manpage

## 2. Build Script

- [x] 2.1 Create `scripts/build-deb.sh` that extracts version from Cargo.toml, arranges files in DPKG layout, and calls `dpkg-deb --build`
- [x] 2.2 Script SHALL accept `--arch` flag to set the Architecture field and filename suffix
- [x] 2.3 Script SHALL accept `--binary` flag to point to the pre-built release binary
- [x] 2.4 Script SHALL accept `--output-dir` flag for destination of the `.deb` file
- [x] 2.5 Verify `dpkg-deb --build` succeeds and produces a valid `.deb` on local system

## 3. Manpage

- [x] 3.1 Write `debian/acmers.1` manpage documenting CLI usage, options, and DNS provider setup
- [x] 3.2 Verify `man ./debian/acmers.1` renders correctly

## 4. CI: Cross-Compile Testing

- [x] 4.1 Modify `.github/workflows/ci.yml` to add a cross-compile matrix job building for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`
- [x] 4.2 Install `gcc-aarch64-linux-gnu` for arm64 target in CI
- [x] 4.3 Set `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc` for arm64 builds
- [x] 4.4 Verify both targets compile without errors in CI

## 5. CI: Release Workflow

- [x] 5.1 Create `.github/workflows/release.yml` triggered on tag push matching `v*`
- [x] 5.2 Add matrix job building release binaries for amd64 and arm64 with `--release`
- [x] 5.3 Add job to run `scripts/build-deb.sh` for each architecture using the release binary
- [x] 5.4 Upload `.deb` files as workflow artifacts
- [x] 5.5 Add job to publish `.deb` files to GitHub Release using `softprops/action-gh-release`
- [x] 5.6 Verify the full workflow end-to-end on a test tag

## 6. Documentation

- [x] 6.1 Update `README.md` with Debian package installation instructions (`dpkg -i acmers_*.deb`)
- [x] 6.2 Document minimum Debian/Ubuntu version requirements (glibc 2.35+)
