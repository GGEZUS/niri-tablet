#!/bin/bash
# Regenerate patches/ from the gestures branch and refresh pkg/PKGBUILD
# (_tag, pkgver, source list, checksums). Run after any rebase or patchset
# edit, before makepkg — update.sh and install.sh call this automatically.
# Requires the dev clone in niri/ (maintainer-only; users build from the
# committed patches). Works from any cwd.
set -euo pipefail
cd "$(dirname -- "$0")"

BASE=$(git -C niri describe --tags --abbrev=0)
PKGVER=$(git -C niri describe --tags --long | sed 's/^v//;s/-/./g')
echo "regen: base $BASE, pkgver $PKGVER"

rm -f pkg/*.patch
git -C niri format-patch "$BASE..gestures" -o "$(pwd)/pkg/" >/dev/null

shopt -s nullglob
NL=$'\n'
SRC="source=(${NL}    \"git+https://github.com/niri-wm/niri#tag=\${_tag}\""
SUMS="sha256sums=(${NL}    'SKIP'"
for p in pkg/*.patch; do
    h=$(sha256sum "$p" | awk '{print $1}')
    SRC+="${NL}    '$(basename "$p")'"
    SUMS+="${NL}    '$h'"
done
SRC+="${NL})"
SUMS+="${NL})"

awk -v tag="$BASE" -v ver="$PKGVER" -v src="$SRC" -v sums="$SUMS" '
    /^_tag=/   { print "_tag=" tag "  # upstream release the patchset is rebased onto"; next }
    /^pkgver=/ { print "pkgver=" ver; next }
    $0 == "source=("     { printf "%s\n", src;  in_src=1;  next }
    in_src  && $0 == ")" { in_src=0;  next }
    in_src                { next }
    $0 == "sha256sums=(" { printf "%s\n", sums; in_sums=1; next }
    in_sums && $0 == ")" { in_sums=0; next }
    in_sums               { next }
    { print }
' pkg/PKGBUILD > pkg/PKGBUILD.new && mv pkg/PKGBUILD.new pkg/PKGBUILD

echo "regen: patches/ and pkg/PKGBUILD updated"
