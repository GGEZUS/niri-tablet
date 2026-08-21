# niri-tablet

Tablet support for [niri](https://github.com/niri-wm/niri): multi-finger
touchscreen gestures, an on-screen keyboard that adapts to screen orientation,
and automatic display rotation — everything needed to run a niri session with
the keyboard detached.

Built and daily-driven on a Microsoft Surface Go 2 running Arch Linux.

## Gestures

| Gesture | Action |
|---|---|
| 3-finger drag, horizontal | scroll the view across columns (animated) |
| 3-finger drag, vertical | workspace carousel (animated) |
| 3-finger tap | maximize / restore the focused column |
| 4-finger tap | app launcher (`noctalia msg panel-toggle launcher` by default) |
| 4-finger flick down | close the focused window |
| 4-finger flick up | toggle the on-screen keyboard |

All actions are configurable — every node in the `gestures` block takes any
niri action, same as keybinds. A single-finger bottom-edge swipe is also
implemented but unbound by default. Touchpad behavior is untouched.

## How it works

niri has no native touchscreen gestures (see the upstream
[discussion](https://github.com/niri-wm/niri/discussions/463)). This repo
maintains a small patch series on top of a current niri release:

- **`patches/`** — 6 patches (`git am`-able, authorship preserved): animated
  3-finger swipes reusing niri's touchpad gesture pipeline, 3/4-finger taps,
  discrete 4-finger flicks, and the optional edge swipe. The animated swipes
  run through the same spring-physics pipeline as touchpad gestures —
  rotation-proof logical coordinates, live follow, inertia.
- **`config/gestures.kdl`** — the gesture config block (see above).
- **`scripts/`** — OSK toggle + auto-rotate helpers (work on stock niri too).
- **`pkg/PKGBUILD`** — Arch package: upstream niri release + the patches.

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

`niri-tablet` provides and conflicts with `niri`, so the swap is automatic.
Needs `rust`, `cargo`, `clang` and `git` to build. First build takes a while;
the PKGBUILD shares a cargo target dir (`~/.cache/niri-tablet-target`) so
rebuilds are incremental.

On other distros: apply `patches/` onto the matching niri release
(`_tag` in the PKGBUILD names it) and build with `cargo build --release`.

## On-screen keyboard + auto-rotation (optional, works on stock niri too)

```bash
cp scripts/niri-osk.sh scripts/niri-rotate.sh ~/.local/bin/
cp scripts/niri-rotate.service ~/.config/systemd/user/
systemctl --user enable --now niri-rotate.service
```

- **`niri-osk.sh`** — toggles [wvkbd](https://git.sr.ht/~proycon/wvkbd)
  (`wvkbd-deskintl`); doubles its height in portrait orientation and flips the
  xkb layout group if you use a two-group layout like `"pt,us"` (the OSK is
  US-labelled).
- **`niri-rotate.sh`** — follows the accelerometer via
  [iio-sensor-proxy](https://github.com/hadess/iio-sensor-proxy) and sets
  `niri msg output <name> transform` as the tablet turns; resizes a running
  keyboard. Edit `OUTPUT`/heights at the top of the scripts for your device.

## Repo layout

```
patches/   the patch series against upstream niri
pkg/       Arch PKGBUILD (upstream release + patches)
scripts/   OSK toggle + auto-rotate helpers
config/    example niri config fragments
extras/    optional extras (wvkbd build with mobile layouts)
test.kdl   minimal config for nested (in-window) testing
niri/      maintainer's dev clone for rebasing — not part of the repo
```

## Status & credits

- Patchset: `v26.04 + 6 patches`, unit-tested (`cargo test touch_`) and
  hardware-validated on a Surface Go 2.
- One design note for anyone hacking on the gesture code: never run a niri
  action from inside a smithay touch-grab callback (seat touch mutex
  deadlock) — actions are deferred via `Niri::pending_touch_action`.

License: **GPL-3.0-only** (the patches modify niri, also GPL-3.0-only).
Swipe foundation by Mario St Jr; taps, flicks, fixes, scripts and packaging
by [GGEZUS](https://github.com/GGEZUS).
