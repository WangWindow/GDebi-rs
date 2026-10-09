#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:?Debian architecture is required}"
VERSION="${2:?Package version is required}"
OUTPUT_DIR="${3:?Output directory is required}"

PACKAGE="gdebi-rs"
ROOT="$(mktemp -d)"
trap 'rm -rf "$ROOT"' EXIT

mkdir -p "$OUTPUT_DIR"

install -Dm755 "target/release/gdebi-rs" \
    "$ROOT/usr/bin/gdebi-rs"
install -Dm644 "data/rs.GDebi.desktop" \
    "$ROOT/usr/share/applications/rs.GDebi.desktop"

while IFS= read -r language; do
    language="${language%%#*}"
    language="${language//[[:space:]]/}"
    [ -n "$language" ] || continue
    catalog="$(find target/release/build \
        -path "*/out/locale/$language/LC_MESSAGES/gdebi-rs.mo" \
        -type f -print -quit)"
    [ -n "$catalog" ] || continue
    install -Dm644 "$catalog" \
        "$ROOT/usr/share/locale/$language/LC_MESSAGES/gdebi-rs.mo"
done < po/LINGUAS

mkdir -p "$ROOT/DEBIAN"
cat > "$ROOT/DEBIAN/control" <<EOF
Package: $PACKAGE
Version: $VERSION
Section: admin
Priority: optional
Architecture: $ARCH
Maintainer: WangWindow <1598593280@qq.com>
Depends: apt, dpkg, libgtk-4-1 (>= 4.0) | libgtk-4-1t64 (>= 4.0)
Recommends: pkexec | policykit-1
Description: GTK4 Debian package installer
 A lightweight GTK4 installer for local Debian packages.
Homepage: https://github.com/WangWindow/GDebi-rs
EOF

OUTPUT="$OUTPUT_DIR/${PACKAGE}_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "$ROOT" "$OUTPUT" >/dev/null
echo "Created $OUTPUT"
