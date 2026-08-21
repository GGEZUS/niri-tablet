#!/bin/sh
# Rebuild the niri-tablet package from the current patchset state
# (regenerates patches/ from the gestures branch first). Works from any cwd.
#
# After it finishes:
#   sudo pacman -U pkg/niri-tablet-*.pkg.tar.zst     # then log out/in
set -eu
cd "$(dirname -- "$0")"
./regen-patches.sh
cd pkg
# No -f: reuse src/ and the shared cargo cache for incremental rebuilds.
makepkg
echo
echo "done — install with: sudo pacman -U niri-tablet-*.pkg.tar.zst   (then log out/in)"
