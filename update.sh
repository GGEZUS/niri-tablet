#!/bin/sh
# Rebase the touch-gesture patchset onto the latest upstream niri release,
# regenerate the patch series, and rebuild the niri-tablet package.
# Maintainer script (needs the dev clone in niri/). Works from any cwd.
#
# After it finishes:
#   sudo pacman -U pkg/niri-tablet-*.pkg.tar.zst     # then log out/in
set -eu
cd "$(dirname -- "$0")/niri"

git fetch https://github.com/niri-wm/niri '+refs/tags/*:refs/tags/*'
LATEST=$(git tag -l 'v*' --sort=-v:refname | head -1)
BASE=$(git describe --tags --abbrev=0)
echo "patchset base: $BASE  ->  latest upstream: $LATEST"

if [ "$BASE" != "$LATEST" ]; then
    git rebase "$LATEST"
    echo "rebased onto $LATEST — running the gesture unit tests"
    cargo test --release touch_
fi

cd ..
./regen-patches.sh
cd pkg
# No -f: reuse src/ and the shared cargo cache for incremental rebuilds.
makepkg
echo
echo "done — install with: sudo pacman -U niri-tablet-*.pkg.tar.zst   (then log out/in)"
