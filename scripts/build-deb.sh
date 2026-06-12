#!/usr/bin/env bash
set -euo pipefail

BINARY=""
ARCH=""
OUTPUT_DIR="."

usage() {
    cat <<EOF
Usage: $0 --binary <path> [--arch <arch>] [--output-dir <dir>]

Packages a pre-built acmers binary into a .deb using dpkg-deb.
For source builds, use: dpkg-buildpackage -b

Options:
  --binary <path>      Path to pre-built acmers release binary
  --arch <arch>        Target architecture (amd64, arm64; default: amd64)
  --output-dir <dir>   Output directory (default: .)
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary) BINARY="$2"; shift 2 ;;
        --arch)   ARCH="$2"; shift 2 ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        *) usage ;;
    esac
done

[[ -n "$BINARY" ]] || { echo "error: --binary required"; usage; }
[[ -f "$BINARY" ]] || { echo "error: $BINARY not found"; exit 1; }

ARCH="${ARCH:-amd64}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

VERSION=$(dpkg-parsechangelog -l "$PROJECT_DIR/debian/changelog" -S Version 2>/dev/null \
    || grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

PKG="acmers"
PKG_DIR="$OUTPUT_DIR/${PKG}_${VERSION}_${ARCH}"

rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"

install -Dm755 "$BINARY" "$PKG_DIR/usr/bin/$PKG"

for f in "$PROJECT_DIR/debian/"*; do
    case "$(basename "$f")" in
        acmers.1)
            install -Dm644 "$f" "$PKG_DIR/usr/share/man/man1/acmers.1"
            gzip -n9 "$PKG_DIR/usr/share/man/man1/acmers.1"
            ;;
        acmers.service)
            install -Dm644 "$f" "$PKG_DIR/lib/systemd/system/acmers.service"
            ;;
        acmers.timer)
            install -Dm644 "$f" "$PKG_DIR/lib/systemd/system/acmers.timer"
            ;;
        postinst)
            install -Dm755 "$f" "$PKG_DIR/DEBIAN/postinst"
            ;;
        postrm)
            install -Dm755 "$f" "$PKG_DIR/DEBIAN/postrm"
            ;;
    esac
done

install -Dm644 "$PROJECT_DIR/README.md" "$PKG_DIR/usr/share/doc/$PKG/README.md"
install -Dm644 "$PROJECT_DIR/LICENSE" "$PKG_DIR/usr/share/doc/$PKG/copyright"

SIZE=$(du -sk "$PKG_DIR" | cut -f1)

cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: $PKG
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: José Carretero <josacar@users.noreply.github.com>
Installed-Size: $SIZE
Depends: libc6 (>= 2.35)
Description: Zero-dependency ACME client with DNS-01 challenge support
 acmers is a minimal-dependency Rust CLI for automatic SSL/TLS
 certificate issuance and renewal via the ACME protocol (RFC 8555).
 It supports 200+ DNS providers for DNS-01 challenge validation,
 ported from the acme.sh project.
EOF

dpkg-deb --build "$PKG_DIR" "$OUTPUT_DIR/${PKG}_${VERSION}_${ARCH}.deb"
rm -rf "$PKG_DIR"
echo "Built: $OUTPUT_DIR/${PKG}_${VERSION}_${ARCH}.deb"
