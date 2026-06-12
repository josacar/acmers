#!/usr/bin/env bash
set -euo pipefail

VERSION=""
ARCH=""
BINARY=""
OUTPUT_DIR="."

usage() {
    cat <<EOF
Usage: $0 --binary <path> [--arch <arch>] [--output-dir <dir>]

Options:
  --binary <path>      Path to pre-built acmers release binary
  --arch <arch>        Target architecture (amd64 or arm64, default: amd64)
  --output-dir <dir>   Output directory for the .deb file (default: .)

Examples:
  $0 --binary ./target/release/acmers
  $0 --binary ./target/aarch64-unknown-linux-gnu/release/acmers --arch arm64
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary)
            BINARY="$2"; shift 2 ;;
        --arch)
            ARCH="$2"; shift 2 ;;
        --output-dir)
            OUTPUT_DIR="$2"; shift 2 ;;
        *)
            usage ;;
    esac
done

if [[ -z "$BINARY" ]]; then
    echo "error: --binary is required"
    usage
fi
if [[ ! -f "$BINARY" ]]; then
    echo "error: binary not found: $BINARY"
    exit 1
fi

ARCH="${ARCH:-amd64}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
if [[ -z "$VERSION" ]]; then
    echo "error: could not extract version from Cargo.toml"
    exit 1
fi

PKG_NAME="acmers"
PKG_DIR="$OUTPUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}"

rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/man/man1"
mkdir -p "$PKG_DIR/usr/share/doc/$PKG_NAME"
mkdir -p "$PKG_DIR/lib/systemd/system"
mkdir -p "$PKG_DIR/var/lib/$PKG_NAME"

cp "$BINARY" "$PKG_DIR/usr/bin/$PKG_NAME"
chmod 755 "$PKG_DIR/usr/bin/$PKG_NAME"

if [[ -f "$PROJECT_DIR/debian/acmers.1" ]]; then
    cp "$PROJECT_DIR/debian/acmers.1" "$PKG_DIR/usr/share/man/man1/"
    gzip -n -9 "$PKG_DIR/usr/share/man/man1/acmers.1"
fi

cp "$PROJECT_DIR/README.md" "$PKG_DIR/usr/share/doc/$PKG_NAME/"
cp "$PROJECT_DIR/LICENSE" "$PKG_DIR/usr/share/doc/$PKG_NAME/copyright"

if [[ -f "$PROJECT_DIR/debian/acmers.service" ]]; then
    cp "$PROJECT_DIR/debian/acmers.service" "$PKG_DIR/lib/systemd/system/"
fi
if [[ -f "$PROJECT_DIR/debian/acmers.timer" ]]; then
    cp "$PROJECT_DIR/debian/acmers.timer" "$PKG_DIR/lib/systemd/system/"
fi
if [[ -f "$PROJECT_DIR/debian/postinst" ]]; then
    cp "$PROJECT_DIR/debian/postinst" "$PKG_DIR/DEBIAN/postinst"
    chmod 755 "$PKG_DIR/DEBIAN/postinst"
fi
if [[ -f "$PROJECT_DIR/debian/postrm" ]]; then
    cp "$PROJECT_DIR/debian/postrm" "$PKG_DIR/DEBIAN/postrm"
    chmod 755 "$PKG_DIR/DEBIAN/postrm"
fi

INSTALLED_SIZE=$(du -sk "$PKG_DIR" | cut -f1)

cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: $PKG_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: José Carretero <josacar@users.noreply.github.com>
Installed-Size: $INSTALLED_SIZE
Depends: libc6 (>= 2.35)
Description: Zero-dependency ACME client with DNS-01 challenge support
 acmers is a minimal-dependency Rust CLI for automatic SSL/TLS
 certificate issuance and renewal via the ACME protocol (RFC 8555).
 It supports 200+ DNS providers for DNS-01 challenge validation,
 ported from the acme.sh project.
EOF

dpkg-deb --build "$PKG_DIR" "$OUTPUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"

rm -rf "$PKG_DIR"

echo "Built: $OUTPUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
