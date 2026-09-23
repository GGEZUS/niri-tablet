#!/bin/sh
# Auto-rotate the niri output from the device orientation sensor.
# Requires: iio-sensor-proxy (monitor-sensor). Runs as a systemd user service.
#
# On 2-in-1s, rotation is additionally gated on the tablet-mode switch:
# nothing rotates while the device is in laptop mode, folding back returns
# the output to the normal orientation, and opening it up again catches up
# to the current angle. See TABLET_MODE_ONLY / TABLET_MODE_SYSFS below;
# environment overrides: NIRI_ROTATE_OUTPUT, NIRI_ROTATE_TABLET_MODE_ONLY,
# NIRI_ROTATE_TABLET_MODE_SYSFS.

OUTPUT="${NIRI_ROTATE_OUTPUT:-eDP-1}"

# auto (default): gate only when a tablet-mode switch is found; yes: insist
# on the gate (warn and rotate always if none is readable); no: never gate.
TABLET_MODE_ONLY="${NIRI_ROTATE_TABLET_MODE_ONLY:-auto}"
# A file printing 0 in laptop mode, 1 in tablet mode. Probed from known
# vendors when empty; set it for devices the probe does not cover.
TABLET_MODE_SYSFS="${NIRI_ROTATE_TABLET_MODE_SYSFS:-}"

tablet_state() {
    # Leaves 0 or 1 in TABLET_NOW, or fails.
    [ -n "$TABLET_MODE_SYSFS" ] || return 1
    TABLET_NOW=$(cat "$TABLET_MODE_SYSFS" 2>/dev/null) || return 1
    case $TABLET_NOW in 0|1) return 0 ;; esac
    return 1
}

if [ "$TABLET_MODE_ONLY" != no ]; then
    if [ -z "$TABLET_MODE_SYSFS" ]; then
        for p in /sys/devices/platform/thinkpad_acpi/hotkey_tablet_mode \
                 /sys/devices/platform/hp-wmi/tablet; do
            TABLET_MODE_SYSFS=$p
            tablet_state && break
            TABLET_MODE_SYSFS=
        done
    fi
    if tablet_state; then
        TABLET_MODE=$TABLET_NOW
    elif [ "$TABLET_MODE_ONLY" = yes ]; then
        echo "niri-rotate: TABLET_MODE_ONLY=yes but no tablet-mode switch found; rotating always" >&2
    fi
fi
GATE=0
[ -n "$TABLET_MODE" ] && GATE=1

# Resolve the running niri instance's IPC socket (newest first).
SOCKETS=$(ls -t "/run/user/$(id -u)"/niri.wayland-*.sock 2>/dev/null | head -1)
[ -n "$SOCKETS" ] && export NIRI_SOCKET="$SOCKETS"

ORIENT=normal

apply_orientation() {
    case "$ORIENT" in
        bottom-up) niri msg output "$OUTPUT" transform 180 ;;
        right-up)  niri msg output "$OUTPUT" transform 270 ;;
        left-up)   niri msg output "$OUTPUT" transform 90 ;;
        *)         niri msg output "$OUTPUT" transform normal ;;
    esac

    # Resize the on-screen keyboard for the new orientation (if it's up).
    "$HOME/.local/bin/niri-osk.sh" resize
}

# Poll the tablet-mode switch and echo "tablet-mode 0|1" on every change.
tablet_watch() {
    last=$TABLET_MODE
    while :; do
        tablet_state || TABLET_NOW=$last
        if [ -n "$TABLET_NOW" ] && [ "$TABLET_NOW" != "$last" ]; then
            last=$TABLET_NOW
            printf 'tablet-mode %s\n' "$TABLET_NOW"
        fi
        sleep 1
    done
}

{
    monitor-sensor 2>/dev/null &
    if [ "$GATE" = 1 ]; then
        tablet_watch &
    fi
    wait
} | while read -r line; do
    SW=
    case "$line" in
        # monitor-sensor reports the current orientation at startup and on
        # every change; both update ORIENT (the startup line keeps it right
        # while the gate is closed, so reopening applies the true angle).
        *"orientation changed: normal"*|"=== Has accelerometer (orientation: normal"*)
            ORIENT=normal ;;
        *"orientation changed: bottom-up"*|"=== Has accelerometer (orientation: bottom-up"*)
            ORIENT=bottom-up ;;
        *"orientation changed: right-up"*|"=== Has accelerometer (orientation: right-up"*)
            ORIENT=right-up ;;
        *"orientation changed: left-up"*|"=== Has accelerometer (orientation: left-up"*)
            ORIENT=left-up ;;
        *"tablet-mode 0"*|*"tablet-mode 1"*)
            SW=${line#*tablet-mode } ;;
        *)
            continue ;;
    esac

    if [ -n "$SW" ]; then
        # The watcher echoes the current state once at startup: no-op then.
        [ "$SW" = "$TABLET_MODE" ] && continue
        TABLET_MODE=$SW
        [ "$SW" = 0 ] && ORIENT=normal    # the clamshell sits landscape
        apply_orientation
    elif [ "$GATE" != 1 ] || [ "$TABLET_MODE" = 1 ]; then
        apply_orientation
    fi
done
