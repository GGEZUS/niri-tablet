#!/bin/sh
# Auto-rotate the niri output from the device orientation sensor.
# Requires: iio-sensor-proxy (monitor-sensor). Runs as a systemd user service.

OUTPUT="${NIRI_ROTATE_OUTPUT:-eDP-1}"

# Resolve the running niri instance's IPC socket (newest first).
SOCKETS=$(ls -t "/run/user/$(id -u)"/niri.wayland-*.sock 2>/dev/null | head -1)
[ -n "$SOCKETS" ] && export NIRI_SOCKET="$SOCKETS"

monitor-sensor 2>/dev/null | while read -r line; do
    case "$line" in
        *"orientation changed: normal"*)
            niri msg output "$OUTPUT" transform normal ;;
        *"orientation changed: bottom-up"*)
            niri msg output "$OUTPUT" transform 180 ;;
        *"orientation changed: right-up"*)
            niri msg output "$OUTPUT" transform 270 ;;
        *"orientation changed: left-up"*)
            niri msg output "$OUTPUT" transform 90 ;;
        *)
            continue ;;
    esac

    # Resize the on-screen keyboard for the new orientation (if it's up).
    "$HOME/.local/bin/niri-osk.sh" resize
done
