#!/bin/sh
# update-niri-tablet.sh — the user-facing updater for the niri-tablet package.
#
# Fetches the latest release tag from GitHub, shows what changed since the
# installed version, builds the package with makepkg, installs it, and pins
# stock niri in /etc/pacman.conf (IgnorePkg) if that pin is missing. Works
# for first installs too. Run it from anywhere inside your clone:
#
#   ./update-niri-tablet.sh              update to the latest release
#   ./update-niri-tablet.sh --check      report only, build nothing
#   ./update-niri-tablet.sh --force      rebuild even when up to date
#   ./update-niri-tablet.sh --tag v26.04.11   install a specific release
#   ./update-niri-tablet.sh --main       track the repo's main branch
#   ./update-niri-tablet.sh --yes        no prompts (needs stdin TTY otherwise)
#
# The other root scripts are maintainer-only (they need a dev clone of niri):
# update.sh rebases onto new upstream releases, install.sh rebuilds as-is.
set -eu

usage() {
    sed -n '/^#   \.\//{s/^#   //;p}' "$0" 2>/dev/null || true
    cat <<'EOF'
  --check        report installed/latest/changelog, then exit
  --force        rebuild and reinstall even when up to date
  --tag <vX.Y.Z> pick a specific release tag instead of the latest
  --main         build origin/main instead of a release tag
  -y, --yes      skip the confirmation prompt
  -h, --help     this help
EOF
}

# ── pretty output ──────────────────────────────────────────────────
if [ -t 1 ]; then
    B=$(printf '\033[1m'); DIM=$(printf '\033[2m'); N=$(printf '\033[0m')
    G=$(printf '\033[32m'); Y=$(printf '\033[33m'); R=$(printf '\033[31m')
else B=''; DIM=''; N=''; G=''; Y=''; R=''; fi
say()  { printf '%s\n' "$*"; }
hdr()  { printf '\n%s%s%s\n' "$B" "$*" "$N"; }
ok()   { printf '  %s✓%s %s\n' "$G" "$N" "$*"; }
warn() { printf '  %s!%s %s\n' "$Y" "$N" "$*"; }
die()  { printf '  %s✗%s %s\n' "$R" "$N" "$*" >&2; exit 1; }

# ── args ───────────────────────────────────────────────────────────
CHECK=0; FORCE=0; ASSUME_YES=0; DO_MAIN=0; TARGET=''
while [ $# -gt 0 ]; do
    case $1 in
        --check) CHECK=1 ;;
        --force) FORCE=1 ;;
        -y|--yes) ASSUME_YES=1 ;;
        --main)  DO_MAIN=1 ;;
        --tag)   [ $# -ge 2 ] || die "--tag needs a version, e.g. --tag v26.04.12"
                 TARGET=$2; shift ;;
        --tag=*) TARGET=${1#--tag=} ;;
        -h|--help) usage; exit 0 ;;
        *) printf 'unknown option: %s\n\n' "$1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

# ── preflight ──────────────────────────────────────────────────────
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$ROOT"
[ -f pkg/PKGBUILD ] || die "pkg/PKGBUILD not found — run me from a niri-tablet clone"
command -v git     >/dev/null 2>&1 || die "git not found"
command -v makepkg >/dev/null 2>&1 || die "makepkg not found — install base-devel first (pacman -S base-devel)"
command -v pacman  >/dev/null 2>&1 || die "pacman not found — this updater is Arch-only"

hdr "niri-tablet updater"

git fetch origin --tags --quiet ||
    die "git fetch failed — check your network and the clone's origin remote"

if [ "$DO_MAIN" = 1 ]; then
    TARGET=main
else
    if [ -z "$TARGET" ]; then
        TARGET=$(git tag -l 'v*' --sort=-v:refname | grep -v -- '-' | head -1)
    fi
    case $TARGET in
        v*) ;;
        *) TARGET="v$TARGET" ;;
    esac
    git rev-parse -q --verify "refs/tags/$TARGET" >/dev/null ||
        die "release tag $TARGET not found in this clone"
fi

# ── versions ───────────────────────────────────────────────────────
BASE_TAG=$(sed -n 's/^_tag=\(v[^[:space:]]*\).*/\1/p' pkg/PKGBUILD)
INSTALLED=$(pacman -Q niri-tablet 2>/dev/null || true)
# pacman -Q niri resolves to whichever package provides it — only flag a
# literal stock-niri install (the fresh-migration case).
NIRI_PROVIDER=$(pacman -Qq niri 2>/dev/null || true)
STOCK_NIRI=''
[ "$NIRI_PROVIDER" = niri ] && STOCK_NIRI=$(pacman -Q niri 2>/dev/null || true)

if [ -n "$INSTALLED" ]; then
    INST_VER=$(printf '%s' "$INSTALLED" | awk '{print $2}')      # 26.04.12.g2e96de30-3
    INST_REL=$(printf '%s' "$INST_VER" | cut -d. -f1-3)          # 26.04.12
    UP_TO_DATE=0
    [ "$TARGET" != main ] && [ "v$INST_REL" = "$TARGET" ] && UP_TO_DATE=1
else
    INST_VER='(not installed)'; INST_REL=''; UP_TO_DATE=0
fi

say ""
if [ -n "$INSTALLED" ]; then
    say "  installed : $INSTALLED ${DIM}(release v$INST_REL)$N"
else
    say "  installed : ${DIM}none$N"
fi
[ -n "$STOCK_NIRI" ] && say "  stock niri : $STOCK_NIRI $DIM(will be replaced)$N"
say "  target    : $TARGET $DIM(niri $BASE_TAG + patches)$N"

# ── changelog since the installed release ──────────────────────────
# Only when something is actually pending: an update or a fresh install.
CHANGELOG=''
if [ "$UP_TO_DATE" != 1 ]; then
    if [ -n "$INST_REL" ] && [ "v$INST_REL" != "$TARGET" ] && \
       git rev-parse -q --verify "refs/tags/v$INST_REL" >/dev/null; then
        CHANGELOG=$(git log --oneline --no-decorate "v$INST_REL..$TARGET" 2>/dev/null || true)
    fi
    [ -n "$CHANGELOG" ] || CHANGELOG=$(git log --oneline --no-decorate -5 "$TARGET" 2>/dev/null || true)
fi
if [ -n "$CHANGELOG" ]; then
    say ""
    say "  ${B}what changed:${N}"
    printf '%s\n' "$CHANGELOG" | sed 's/^/    /'
fi

# ── status / exit paths ────────────────────────────────────────────
say ""
if [ -z "$INSTALLED" ]; then
    warn "niri-tablet is not installed — this will be a fresh install"
elif [ "$UP_TO_DATE" = 1 ]; then
    if [ "$FORCE" = 1 ]; then
        warn "already up to date — rebuilding anyway (--force)"
    else
        ok "already up to date ($TARGET) — nothing to do"
        say "    $DIM--force rebuilds anyway; --main tracks the development branch$N"
        exit 0
    fi
else
    ok "update available: ${INST_REL:+v$INST_REL → }$TARGET"
fi

if [ "$CHECK" = 1 ]; then
    say ""
    say "  ${DIM}--check: stopping here, nothing was built or installed$N"
    exit 0
fi

# ── confirm ────────────────────────────────────────────────────────
if [ "$ASSUME_YES" != 1 ]; then
    [ -t 0 ] || die "stdin is not a terminal — pass --yes to run non-interactively"
    printf '\n  %s' "fetch, build and install $TARGET now? (sudo prompt comes from makepkg) [Y/n] "
    read -r REPLY || REPLY=n
    case $REPLY in n*|N*) die "aborted — nothing was changed" ;; esac
fi

# ── checkout the target ────────────────────────────────────────────
[ -z "$(git status --porcelain --untracked-files=no)" ] ||
    die "working tree has local changes — stash or commit them first (git stash)"

hdr "checking out $TARGET"
if [ "$TARGET" = main ]; then
    git checkout -q main 2>/dev/null || git checkout -q -b main --track origin/main
    git pull --ff-only --quiet ||
        die "origin/main diverged from your main — resolve manually (git pull)"
else
    git checkout -q --detach "$TARGET"
    ok "clone now sits on the release tag $TARGET $DIM(re-run me, or git checkout main, to move)$N"
fi

# ── build + install ────────────────────────────────────────────────
hdr "building niri-tablet $TARGET"
say "  $DIM(first build takes a while; later builds reuse ~/.cache/niri-tablet-target)$N"
cd pkg
makepkg -si || die "makepkg failed — read the output above; missing deps are installed by -s"

NEW_VER=$(pacman -Q niri-tablet 2>/dev/null | awk '{print $2}')
[ -n "$NEW_VER" ] && ok "installed: niri-tablet $NEW_VER"

# ── keep stock niri pinned ─────────────────────────────────────────
cd "$ROOT"
if ! grep -E '^[[:space:]]*IgnorePkg([[:space:]]|=)' /etc/pacman.conf | grep -qw niri; then
    warn "/etc/pacman.conf does not pin niri — the next pacman -Syu swaps back to stock niri"
    FIX=0
    if [ "$ASSUME_YES" = 1 ]; then FIX=1; else
        printf '  %s' "add IgnorePkg = niri now? [Y/n] "
        read -r FIXREPLY || FIXREPLY=n
        case $FIXREPLY in n*|N*) FIX=0 ;; *) FIX=1 ;; esac
    fi
    if [ "$FIX" = 1 ]; then
        sudo cp /etc/pacman.conf /etc/pacman.conf.bak-niri-tablet
        if grep -qE '^[[:space:]]*IgnorePkg([[:space:]]|=)' /etc/pacman.conf; then
            sudo sed -i 's/^\([[:space:]]*IgnorePkg[[:space:]]*=[[:space:]]*.*\)$/\1 niri/' /etc/pacman.conf
        else
            printf '\nIgnorePkg = niri\n' | sudo tee -a /etc/pacman.conf >/dev/null
        fi
        ok "niri pinned $DIM(backup: /etc/pacman.conf.bak-niri-tablet)$N"
    fi
fi

# ── done ───────────────────────────────────────────────────────────
say ""
ok "done — log out and back in to start the new build"
say "  $DIM afterwards: niri validate accepts a gestures {} block → the patched build runs$N"
