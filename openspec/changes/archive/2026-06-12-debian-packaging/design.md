## Context

acmers is a pure-Rust CLI with 8 dependencies, targeting Debian/Ubuntu systems. It currently builds via `cargo build --release` and is distributed through `cargo install`. There is no OS-level packaging. Users on Debian/Ubuntu either need Rust installed or must download the binary manually from GitHub Releases.

The project already uses GitHub Actions (`ci.yml`) for CI. The binary is self-contained with zero runtime dependencies beyond glibc.

## Goals / Non-Goals

**Goals:**
- Produce `.deb` packages installable via `dpkg -i` on Debian 12+ and Ubuntu 22.04+
- Cross-compile for `amd64` (x86_64-unknown-linux-gnu) and `arm64` (aarch64-unknown-linux-gnu)
- Automate builds via GitHub Actions on tag push
- Attach `.deb` artifacts to GitHub Releases

**Non-Goals:**
- PPA or apt repository hosting (manual `.deb` download only)
- Other distros (RPM, Arch, Alpine)
- Other architectures beyond amd64/arm64
- Cross-compilation for macOS or Windows targets
- Signing packages with GPG
- `cargo-deb` dependency (manual `dpkg-deb` to keep zero build deps)

## Decisions

### Decision 1: Manual `dpkg-deb` over `cargo-deb`

**Rationale:** `cargo-deb` adds a build dependency and wraps `dpkg-deb` anyway. Since the CI runs on Ubuntu, `dpkg-deb` is already available. We create a minimal `debian/control` and a shell script that arranges files into the correct DPKG layout, then calls `dpkg-deb --build`. This keeps the packaging self-contained and avoids another toolchain.

**Alternatives considered:**
- `cargo-deb`: adds a Rust dependency, more features than needed (systemd service detection, etc.)
- `debbuild`/`checkinstall`: too heavy, designed for autotools/cmake projects

### Decision 2: GitHub Actions matrix for cross-compilation

**Rationale:** Use `dtolnay/rust-toolchain` action with `target` parameter. For arm64, install `gcc-aarch64-linux-gnu` via `apt` and set `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`. Build each target in separate matrix jobs, upload artifacts, then assemble `.deb` in a final combine job.

**Alternatives considered:**
- `cross` tool (rust-embedded/cross): requires Docker, heavier but handles all cross-compilation details. Rejected to keep CI simple and fast.
- Single job with multiple `cargo build --target`: harder to debug failures, slower due to lack of parallelism.

### Decision 3: Package structure layout

**Rationale:** Install to `/usr/bin/acmers`, `/usr/share/man/man1/acmers.1`, `/usr/share/doc/acmers/`. Standard Debian paths.

```
debian/
  control        # Package metadata
  changelog      # Debian changelog
  compat         # Debhelper compat level
  rules          # Build rules (no-op, binary is pre-built)
  install        # File mapping
  acmers.1       # Manpage (generated or placeholder)
```

### Decision 4: Release trigger

**Rationale:** Build `.deb` packages on tag push matching `v*` (e.g., `v0.2.0`). This separates release packaging from CI test builds. PR/commit CI (`ci.yml`) continues to do `cargo build --release` and `cargo test` on amd64 only for speed.

## Risks / Trade-offs

- **arm64 ring crate compilation** → The `ring` 0.17 crate requires a C toolchain and cross-compilation headers for the target. Mitigation: install `gcc-aarch64-linux-gnu` and set `CC_aarch64_unknown_linux_gnu` in CI.
- **glibc version mismatch** → Building on `ubuntu-latest` (22.04, glibc 2.35) means packages require glibc >= 2.35. Mitigation: document minimum Ubuntu/Debian version; use `ubuntu-22.04` runner explicitly for older glibc (2.35).
- **No package signing** → Users must trust the `.deb` download. Mitigation: document installing via `dpkg -i` directly; GitHub provides HTTPS integrity.
- **Release CI breakage** → If the cross-compile workflow breaks, releases stall. Mitigation: run cross-compile also on PRs to catch breakage early.
