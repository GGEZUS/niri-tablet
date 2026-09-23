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
# After installing, it also checks ~/.config/niri for nodes the new build
# rejects (the v26.04.20 gesture rename) and offers to migrate them, and
# rebuilds niri-tablet-easysetup (the GUI configurator) when its sources
# changed — a plain cargo build, no sudo. It also installs the app's
# launcher entry (niri logo icon) and a ~/.local/bin symlink, so the GUI
# shows up in app launchers by itself.
#
# A plain release run leaves the clone detached on the last release tag,
# so the script itself can be older than origin/main. Before doing
# anything it therefore refreshes itself from origin/main and re-execs
# (guarded, no loop), keeping updater fixes and new sub-steps from
# waiting on the next release tag.
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

# ── v26.04.20 gesture rename ───────────────────────────────────────
# tap-4 / swipe-4-up / swipe-4-down became tap-more / swipe-more-up /
# swipe-more-down, and `fingers` is now restricted to 3 or 4. Configs
# from v26.04.18 or earlier that still use the old names fail to parse
# on the new build: without migration the next login starts on niri's
# default config (with an error notification) until the file is fixed.
# Offer the rename here: opt-in, one dated .bak per touched file, kept
# only if `niri validate` still passes on the whole include chain.
check_config_rename() {
    command -v niri >/dev/null 2>&1 || return 0

    # Only act when the installed build speaks the new names; probing
    # the binary rather than the version leaves --tag installs of older
    # releases alone.
    CFG_PROBE=$(mktemp) || return 0
    printf 'gestures {\n    touchscreen-swipe {\n        tap-more { toggle-overview; }\n    }\n}\n' >"$CFG_PROBE"
    if ! niri validate -c "$CFG_PROBE" >/dev/null 2>&1; then
        rm -f "$CFG_PROBE"
        return 0
    fi
    rm -f "$CFG_PROBE"

    CFG_DIR=${XDG_CONFIG_HOME:-$HOME/.config}/niri

    # The documented layout: config.kdl plus the cfg/ fragments it
    # includes. Files included from elsewhere are not touched.
    CFG_HITS=0; CFG_FINGERS=0
    for CFG_F in "$CFG_DIR/config.kdl" "$CFG_DIR"/cfg/*.kdl; do
        [ -f "$CFG_F" ] || continue
        grep -qE '\b(tap-4|swipe-4-up|swipe-4-down)\b' "$CFG_F" && CFG_HITS=1
        for CFG_N in $(sed -n 's/^[[:space:]]*fingers[[:space:]]\{1,\}\([0-9]\{1,\}\).*/\1/p' "$CFG_F"); do
            case $CFG_N in 3|4) ;; *) CFG_FINGERS=1 ;; esac
        done
    done
    [ "$CFG_HITS" = 0 ] && [ "$CFG_FINGERS" = 0 ] && return 0

    hdr "config check: the v26.04.20 gesture changes"

    if [ "$CFG_HITS" = 1 ]; then
        say "  The second-tier gesture nodes were renamed:"
        say "    tap-4        → tap-more"
        say "    swipe-4-up   → swipe-more-up"
        say "    swipe-4-down → swipe-more-down"
        say "  They now fire at one finger more than the base count (fingers)."
        say "  The old names no longer parse on this build: the next login"
        say "  would start on niri's default config, with an error"
        say "  notification, until they are renamed."
        for CFG_F in "$CFG_DIR/config.kdl" "$CFG_DIR"/cfg/*.kdl; do
            [ -f "$CFG_F" ] || continue
            grep -qE '\b(tap-4|swipe-4-up|swipe-4-down)\b' "$CFG_F" &&
                say "  ${Y}old names:$N $CFG_F"
        done
        CFG_MIGRATE=0
        if [ "$ASSUME_YES" = 1 ] || [ ! -t 0 ]; then
            warn "not renaming anything without a prompt (--yes); edit the files above,"
            say "      or open niri-tablet-easysetup and save, before your next login"
        else
            printf '  %s' "rename them now? (per-file .bak backup; undone if niri validate complains) [Y/n] "
            read -r CFG_REPLY || CFG_REPLY=n
            case $CFG_REPLY in
                n*|N*) say "  Leaving the old names $DIM(niri-tablet-easysetup renames them on save too)$N" ;;
                *) CFG_MIGRATE=1 ;;
            esac
        fi
        if [ "$CFG_MIGRATE" = 1 ]; then
            CFG_STAMP=$(date +%Y%m%d-%H%M%S)
            CFG_LIST=''
            for CFG_F in "$CFG_DIR/config.kdl" "$CFG_DIR"/cfg/*.kdl; do
                [ -f "$CFG_F" ] || continue
                grep -qE '\b(tap-4|swipe-4-up|swipe-4-down)\b' "$CFG_F" || continue
                cp -p "$CFG_F" "$CFG_F.bak-$CFG_STAMP"
                sed -i -e 's/\btap-4\b/tap-more/g' \
                       -e 's/\bswipe-4-up\b/swipe-more-up/g' \
                       -e 's/\bswipe-4-down\b/swipe-more-down/g' "$CFG_F"
                ok "renamed in $CFG_F $DIM(backup: $CFG_F.bak-$CFG_STAMP)$N"
                CFG_LIST="$CFG_LIST$CFG_F
"
            done
            if [ -f "$CFG_DIR/config.kdl" ]; then
                if CFG_ERR=$(niri validate -c "$CFG_DIR/config.kdl" 2>&1); then
                    ok "niri validate passes on the migrated config"
                    say "    $DIM(the still-running old build may show one config-error notification until re-login)$N"
                else
                    printf '%s\n' "$CFG_LIST" | while IFS= read -r CFG_F; do
                        [ -n "$CFG_F" ] || continue
                        cp -p "$CFG_F.bak-$CFG_STAMP" "$CFG_F"
                        rm -f "$CFG_F.bak-$CFG_STAMP"
                    done
                    warn "niri validate rejected the result; the files were restored unchanged"
                    printf '%s\n' "$CFG_ERR" | sed 's/^/    /'
                    say "    $DIM(fix the error above, or open niri-tablet-easysetup and save)$N"
                fi
            else
                say "    $DIM(no config.kdl found to validate; run niri validate yourself before login)$N"
            fi
        fi
    fi

    if [ "$CFG_FINGERS" = 1 ]; then
        warn "a fingers value outside 3-4 is now a hard config error (\"fingers must be 3 or 4\")"
        for CFG_F in "$CFG_DIR/config.kdl" "$CFG_DIR"/cfg/*.kdl; do
            [ -f "$CFG_F" ] || continue
            for CFG_N in $(sed -n 's/^[[:space:]]*fingers[[:space:]]\{1,\}\([0-9]\{1,\}\).*/\1/p' "$CFG_F"); do
                case $CFG_N in 3|4) ;; *) say "  ${Y}fingers $CFG_N:$N $CFG_F" ;; esac
            done
        done
        say "      Set it to 3 or 4 by hand, then re-run me (the rename is offered again);"
        say "      niri-tablet-easysetup resets it on save"
    fi
}

# ── niri-tablet-easysetup (the GUI configurator) ───────────────────
# Not part of the package: rebuild it whenever its sources are newer
# than the binary (or there is no binary yet), so updates keep the GUI
# current without anyone remembering to run cargo by hand. No-op when
# already current (cargo's own staleness check is cheap).
build_easysetup() {
    ES_DIR=$ROOT/niri-tablet-easysetup
    [ -f "$ES_DIR/Cargo.toml" ] || return 0
    ES_BIN=$ES_DIR/target/release/niri-tablet-easysetup
    if ! [ -x "$ES_BIN" ] ||
       [ -n "$(find "$ES_DIR/src" "$ES_DIR/Cargo.toml" "$ES_DIR/Cargo.lock" \
              -newer "$ES_BIN" -print -quit 2>/dev/null)" ]; then
        if command -v cargo >/dev/null 2>&1; then
            hdr "building niri-tablet-easysetup (the gesture GUI)"
            say "  $DIM(a plain cargo build, no sudo; the first build takes a couple of minutes)$N"
            if cargo build --release --manifest-path "$ES_DIR/Cargo.toml"; then
                ok "niri-tablet-easysetup built: $ES_BIN"
            else
                warn "the easysetup build failed — the niri package above is unaffected; retry with"
                say "      $DIM(cargo build --release --manifest-path niri-tablet-easysetup/Cargo.toml)$N"
            fi
        else
            warn "cargo not found — skipped niri-tablet-easysetup (the gesture GUI)"
            say "      $DIM(build it later: cargo build --release --manifest-path niri-tablet-easysetup/Cargo.toml)$N"
        fi
    fi

    # Launcher entry (niri logo icon) + a ~/.local/bin symlink, all
    # user-local, no sudo. Runs on every invocation so first installs and
    # already-up-to-date runs get it too.
    ES_DATA=$ES_DIR/data
    ES_DESKTOP=$ES_DATA/com.github.ggezus.NiriTabletEasySetup.desktop
    ES_ICON=$ES_DATA/icons/hicolor/scalable/apps/com.github.ggezus.NiriTabletEasySetup.svg
    XDG_DATA=${XDG_DATA_HOME:-$HOME/.local/share}
    # The installed entry gets ABSOLUTE Exec/TryExec paths. Launchers are
    # often started by the systemd user manager, whose PATH has no
    # ~/.local/bin: bare names resolve as dead there (Surface bug,
    # 2026-09-23). The repo file keeps bare names; only the installed
    # copy is rewritten.
    if [ -x "$ES_BIN" ] && [ -f "$ES_DESKTOP" ] && [ -f "$ES_ICON" ] &&
       mkdir -p "$HOME/.local/bin" &&
       ln -sfn "$ES_BIN" "$HOME/.local/bin/niri-tablet-easysetup" &&
       mkdir -p "$XDG_DATA/applications" &&
       sed -e "s|^Exec=niri-tablet-easysetup\$|Exec=$HOME/.local/bin/niri-tablet-easysetup|" \
           -e "s|^TryExec=niri-tablet-easysetup\$|TryExec=$HOME/.local/bin/niri-tablet-easysetup|" \
           "$ES_DESKTOP" > "$XDG_DATA/applications/com.github.ggezus.NiriTabletEasySetup.desktop" &&
       install -Dm644 "$ES_ICON" "$XDG_DATA/icons/hicolor/scalable/apps/com.github.ggezus.NiriTabletEasySetup.svg"; then
        ok "EasySetup launcher entry installed (niri logo icon): $XDG_DATA/applications"
        case ":$PATH:" in
            *":$HOME/.local/bin:"*) ;;
            *) say "    $DIM(launcher entry uses absolute paths; re-login once to also launch by name in a shell)$N" ;;
        esac
    else
        warn "could not install the EasySetup launcher entry — see the EasySetup wiki page for the manual steps"
    fi
}

# ── preflight ──────────────────────────────────────────────────────
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$ROOT"
[ -f pkg/PKGBUILD ] || die "pkg/PKGBUILD not found — run me from a niri-tablet clone"
command -v git     >/dev/null 2>&1 || die "git not found"
command -v makepkg >/dev/null 2>&1 || die "makepkg not found — install base-devel first (pacman -S base-devel)"
command -v pacman  >/dev/null 2>&1 || die "pacman not found — this updater is Arch-only"

# Help must not need the network: answer it before the fetch below.
case " $* " in
    *' -h '*|*' --help '*) usage; exit 0 ;;
esac

hdr "niri-tablet updater"

git fetch origin --tags --quiet ||
    die "git fetch failed — check your network and the clone's origin remote"

# ── self-update ─────────────────────────────────────────────────────
# See the header comment: the clone usually sits detached on the last
# release tag, so this running copy can be older than origin/main. Swap
# in the newest script and re-exec before anything else happens. Once
# the checkout below succeeds the file on disk is main's version, so
# exec is safe; the env var stops a re-exec loop when the local copy
# intentionally differs from origin/main (e.g. maintainer edits).
# This runs before the arg loop on purpose: re-exec needs "$@" still
# holding the original command line.
if [ "${NIRI_TABLET_UPDATER_SELF:-}" != 1 ]; then
    UPD_SELF=$(git hash-object -- "$ROOT/update-niri-tablet.sh")
    UPD_MAIN=$(git rev-parse -q --verify 'origin/main:update-niri-tablet.sh' 2>/dev/null || true)
    if [ -n "$UPD_MAIN" ] && [ "$UPD_SELF" != "$UPD_MAIN" ]; then
        if git checkout -q main 2>/dev/null ||
           git checkout -q -b main --track origin/main 2>/dev/null; then
            git pull --ff-only --quiet 2>/dev/null || true
            say "  ${DIM}refreshed the updater from origin/main$N"
            NIRI_TABLET_UPDATER_SELF=1 exec "$ROOT/update-niri-tablet.sh" "$@"
        fi
        say "  ${DIM}could not refresh the updater (local changes?) — continuing with this copy$N"
    fi
fi

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
        if [ "$CHECK" != 1 ]; then
            check_config_rename
            build_easysetup
        fi
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

# ── easysetup + config rename check ─────────────────────────────────
build_easysetup
check_config_rename

# ── done ───────────────────────────────────────────────────────────
say ""
ok "done — log out and back in to start the new build"
say "  $DIM afterwards: niri validate accepts a gestures {} block → the patched build runs$N"
