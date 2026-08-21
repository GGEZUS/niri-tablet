#!/bin/sh
# wvkbd on-screen keyboard toggle for niri, orientation-aware.
#
#   niri-osk.sh          toggle the keyboard (switching the keymap pt <-> us)
#   niri-osk.sh resize   if the keyboard is up, restart it at the height
#                        matching the current output orientation
#
# Called from the 4-finger swipe-up gesture and from niri-rotate.sh.

BIN=wvkbd-deskintl
OUTPUT=eDP-1
H_LANDSCAPE=260
H_PORTRAIT=520

# Resolve the running niri instance's IPC socket (newest first).
SOCKET=$(ls -t "/run/user/$(id -u)"/niri.wayland-*.sock 2>/dev/null | head -1)
[ -n "$SOCKET" ] && export NIRI_SOCKET="$SOCKET"

height_for_orientation() {
    T=$(niri msg -j outputs 2>/dev/null | python3 -c \
        "import json,sys; print(json.load(sys.stdin)['$OUTPUT']['logical']['transform'])" 2>/dev/null)
    case "$T" in
        90 | 270) echo "$H_PORTRAIT" ;;
        *) echo "$H_LANDSCAPE" ;;
    esac
}

case "${1:-toggle}" in
    toggle)
        if pgrep -x "$BIN" >/dev/null; then
            pkill -x "$BIN"
            niri msg action switch-layout next
        else
            "$BIN" -H "$(height_for_orientation)" >/dev/null 2>&1 &
            niri msg action switch-layout next
        fi
        ;;
    resize)
        if pgrep -x "$BIN" >/dev/null; then
            pkill -x "$BIN"
            sleep 0.3
            "$BIN" -H "$(height_for_orientation)" >/dev/null 2>&1 &
        fi
        ;;
esac
