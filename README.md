# niri-tablet

Tablet support for [niri](https://github.com/niri-wm/niri): multi-finger
touchscreen gestures, an on-screen keyboard that adapts to screen orientation,
and automatic display rotation — everything needed to run a niri session with
the keyboard detached.

![demo](https://raw.githubusercontent.com/GGEZUS/niri-tablet/main/demo/demo.webp)

[![CI](https://github.com/GGEZUS/niri-tablet/actions/workflows/ci.yml/badge.svg)](https://github.com/GGEZUS/niri-tablet/actions/workflows/ci.yml)
[![License: GPL-3.0-only](https://img.shields.io/badge/license-GPL--3.0--only-blue)](LICENSE)
[![niri base](https://img.shields.io/badge/niri-v26.04%20%2B%208%20patches-blueviolet)](#status--credits)
[![Last commit](https://img.shields.io/github/last-commit/GGEZUS/niri-tablet)](https://github.com/GGEZUS/niri-tablet/commits/main)

## Gestures

| Gesture | Action |
|---|---|
| 3-finger drag, horizontal | scroll the view across columns (animated) |
| 3-finger drag, vertical | workspace carousel (animated) |
| 3-finger tap | maximize / restore the focused column |
| 4-finger tap | window overview (built-in; any launcher binds in its place) |
| 4-finger flick down | close the focused window |
| 4-finger flick up | toggle the on-screen keyboard (needs the OSK script) |

All actions are configurable — every node in the `gestures` block takes any
niri action, same as keybinds. The example `config/gestures.kdl` binds 4-tap
to the built-in `toggle-overview`; Noctalia, fuzzel, wofi and rofi swaps are
one uncomment away. A single-finger bottom-edge swipe is also
implemented but unbound by default. Touchpad behavior is untouched.

### Touch point visualization

`show-touch-points` renders translucent dots that follow your fingers —
drawn by the compositor itself, so any screen recording captures them and
input is never touched. Meant for demo videos and for debugging gestures:

```kdl
gestures {
    show-touch-points "gestures"   // dots only for 3+/4-finger gesture touches
    // show-touch-points "all"     // every finger (debugging)
}
```

Off by default. In `gestures` mode dots appear only while enough fingers
are down concurrently for a gesture (`touchscreen-swipe.fingers`), so
ordinary taps and scrolling stay clean.

## How it works

niri has no native touchscreen gestures (see the upstream
[discussion](https://github.com/niri-wm/niri/discussions/463)). This repo
maintains a small patch series on top of a current niri release:

- **`pkg/`** — the Arch PKGBUILD plus the 8 patches (`git am`-able, authorship
  preserved) sitting next to it, as makepkg requires: animated 3-finger swipes
  reusing niri's touchpad gesture pipeline, 3/4-finger taps, discrete 4-finger
  flicks, the optional edge swipe, and touch point visualization. The
  animated swipes run through the same spring-physics pipeline as touchpad
  gestures — rotation-proof logical coordinates, live follow, inertia.
- **`config/gestures.kdl`** — the gesture config block (see above).
- **`scripts/`** — OSK toggle + auto-rotate helpers (work on stock niri too).

The base swipe implementation comes from
[niri-touch-gestures](https://github.com/pir0c0pter0/niri-touch-gestures) by
Mario St Jr (author of the Noctalia shell); this repo extends it with taps,
discrete flicks, finger-count and deadlock fixes, and packaging.

Upstream is working on [configurable gesture binds](https://github.com/niri-wm/niri/pull/3771);
when that lands, this patchset can be retired.

## Install (Arch Linux)

```bash
git clone https://github.com/GGEZUS/niri-tablet.git
cd niri-tablet/pkg
makepkg -si        # builds upstream niri + patches; replaces stock niri
```

Then protect it from repo updates in `/etc/pacman.conf`:

```ini
IgnorePkg = niri
```

pacman will ask to remove stock niri (`Remove niri? [y/N]` — the default is
N) — answer **y**; with N the install simply aborts. `niri-tablet` provides
and conflicts with `niri`, so the swap itself is automatic.
Needs `rust`, `cargo`, `clang` and `git` to build. First build takes a while;
the PKGBUILD shares a cargo target dir (`~/.cache/niri-tablet-target`) so
rebuilds are incremental.

After a re-login, `niri --version` prints `26.04 (v26.04-modified)`: the
patchset applies uncommitted on top of the release tag, so git-describe
reports that rather than the pkgver's `26.04.6…`. The positive check that
the patched binary is running: `niri validate` accepts a `gestures {}`
block — stock niri rejects the unknown node.

## Other distros

The patchset is plain `git`/`patch` + cargo — nothing Arch-specific in it:

```bash
git clone https://github.com/niri-wm/niri
cd niri
git checkout v26.04        # the release named by _tag in pkg/PKGBUILD
for p in /path/to/niri-tablet/pkg/*.patch; do patch -Np1 < "$p"; done
cargo build --release      # binary lands in target/release/niri
```

You need `rust`, `cargo` and `clang` plus the system libraries niri links
against — the PKGBUILD's `depends` list doubles as the checklist (pipewire,
libinput, libudev, libdisplay-info, libxkbcommon, seatd, …). `cargo test
touch_` runs the unit tests. The helpers in `scripts/` and the config
fragment in `config/` are distro-agnostic (POSIX sh + python3); off-Arch,
build [wvkbd](https://git.sr.ht/~proycon/wvkbd) from source instead of using
the AUR package.

## Updating

The patchset is rebased onto the niri release named by `_tag` in the PKGBUILD;
when this repo follows a new one:

```bash
git pull
cd pkg
makepkg -si        # incremental: the shared target dir is reused
```

`update.sh` and `install.sh` at the repo root are maintainer scripts (they
rebase and regenerate the patch series against a dev clone of niri); plain
`makepkg` is all a user install ever needs. After a big Rust toolchain jump,
deleting `~/.cache/niri-tablet-target` is harmless — the next build is just a
slow cold one again.

## On-screen keyboard + auto-rotation (optional, works on stock niri too)

Two independent helpers — take either, both, or neither:

```bash
# OSK — any touchscreen device, no sensors involved
cp scripts/niri-osk.sh ~/.local/bin/

# auto-rotate — only for devices with an accelerometer
cp scripts/niri-rotate.sh ~/.local/bin/
cp scripts/niri-rotate.service ~/.config/systemd/user/
systemctl --user enable --now niri-rotate.service
```

- **`niri-osk.sh`** — toggles [wvkbd](https://git.sr.ht/~proycon/wvkbd); install
  the AUR package **`wvkbd-git`**, which provides the `wvkbd-deskintl` binary
  (`python3` parses the output transform). Reads the transform from niri
  itself (not from sensors): doubles the keyboard height in portrait and
  flips the xkb layout group if you use a two-group layout like `"pt,us"`
  (the OSK is US-labelled; with a single-group layout the flip is a no-op,
  so a US-only machine needs nothing special).
- **`niri-rotate.sh`** — follows the accelerometer via
  [iio-sensor-proxy](https://github.com/hadess/iio-sensor-proxy) and sets
  `niri msg output <name> transform` as the tablet turns; resizes a running
  keyboard. Only useful with a real accelerometer — check yours with
  `monitor-sensor` (after starting `iio-sensor-proxy.service`); many touch
  laptops have none, in which case skip it: nothing else depends on it.

Edit `OUTPUT`/heights at the top of the scripts for your device.

## Repo layout

```
pkg/       Arch PKGBUILD + the patch series against upstream niri
scripts/   OSK toggle + auto-rotate helpers
config/    example niri config fragments
extras/    optional extras (wvkbd build with mobile layouts)
test.kdl   minimal config for nested (in-window) testing
niri/      maintainer's dev clone for rebasing — not part of the repo
```

## Tested hardware

- **Microsoft Surface Go 2** — built and daily-driven here.
- **ThinkPad T480** — verified from a fresh install (the portability test).

## Status & credits

- Patchset: `v26.04 + 8 patches`, unit-tested (full suite runs in CI).
- One design note for anyone hacking on the gesture code: never run a niri
  action from inside a smithay touch-grab callback (seat touch mutex
  deadlock) — actions are deferred via `Niri::pending_touch_action`.

License: **GPL-3.0-only** (the patches modify niri, also GPL-3.0-only).
Swipe foundation by Mario St Jr; taps, flicks, fixes, scripts and packaging
by [GGEZUS](https://github.com/GGEZUS).
