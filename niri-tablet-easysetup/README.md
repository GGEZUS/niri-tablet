# niri-tablet-easysetup

Friendly GUI configurator for [niri-tablet](https://github.com/GGEZUS/niri-tablet)
touchscreen gestures: pick what each tap, flick, hold-swipe, and edge or
corner swipe does, without hand-editing KDL.

- Shows your current scheme on launch, wherever it lives
- Picker with installed apps, ~65 curated niri actions, commands, and raw KDL
- Test buttons fire actions live via `niri msg action`
- Saves only what `niri validate` accepts; niri reloads within ~500 ms
- Export and import schemes as JSON or `.kdl`

System deps: `gtk4`, `libadwaita`, Rust 1.85+.

```bash
cargo build --release
./target/release/niri-tablet-easysetup
```

`--check` prints a headless report (niri support, managed file, current
scheme as JSON). Docs: the [EasySetup wiki page](https://github.com/GGEZUS/niri-tablet/wiki/EasySetup).

`data/` holds the desktop entry and icon for app launchers;
`update-niri-tablet.sh` installs both for you. The icon is niri's logo,
[CC BY-SA 4.0](https://github.com/niri-wm/niri/wiki/Name-and-Logo).

License: GPL-3.0-only (the icon file is CC BY-SA 4.0).
