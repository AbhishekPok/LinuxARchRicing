#!/usr/bin/env bash
# Re-applies the saved monitor layout whenever a display is plugged in or out.
#
# Hyprland runs the autostart hook only at session start, so monitor-restore.sh
# never fires on hotplug. Without this the external can come back disabled or
# stacked at 0x0 while focus still lands on it -- which makes the launcher (and
# its display menu) open on an output that shows nothing, looking like a dead key.

LOCKFILE="/tmp/monitor-watch.lock"
RESTORE="$HOME/.config/hypr/scripts/monitor-restore.sh"
SOCK="${XDG_RUNTIME_DIR}/hypr/${HYPRLAND_INSTANCE_SIGNATURE}/.socket2.sock"

# Single instance.
if [ -f "$LOCKFILE" ]; then
    OLD_PID=$(cat "$LOCKFILE" 2>/dev/null)
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        kill "$OLD_PID" 2>/dev/null
        sleep 0.5
    fi
fi
echo $$ > "$LOCKFILE"
trap 'rm -f "$LOCKFILE"; exit 0' EXIT INT TERM

command -v socat > /dev/null 2>&1 || exit 1
[ -x "$RESTORE" ] || exit 1

# The socket only exists once Hyprland is up; we start from the same hook.
for _ in $(seq 1 30); do
    [ -S "$SOCK" ] && break
    sleep 1
done
[ -S "$SOCK" ] || exit 1

socat -U - UNIX-CONNECT:"$SOCK" 2>/dev/null | while read -r line; do
    case "$line" in
        monitoradded*|monitorremoved*)
            # Let Hyprland finish enumerating the output before re-applying,
            # and swallow the burst of events a single plug event produces.
            sleep 1
            "$RESTORE" > /dev/null 2>&1
            ;;
    esac
done
