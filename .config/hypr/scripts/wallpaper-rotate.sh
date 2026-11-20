#!/usr/bin/env bash
# Wallpaper rotator — picks a new wallpaper every hour.
# Started from the hyprland.start hook. Replaces the SUPER+W picker menu.
#
# Rotate immediately:  kill -USR1 "$(cat /tmp/wallpaper-rotate.lock)"
#
# The wallpaper is drawn by hyprlax (parallax on workspace switch), which
# replaced hyprpaper on 2026-10-15. hyprlax has one image layer covering every
# monitor; swapping it is `hyprlax ctl modify <id> path <file>`. Its config
# (~/.config/hyprlax/hyprlax.toml) starts on ~/.config/hypr/lock-bg, which this
# script keeps pointed at the current wallpaper, so reboots restore it too.

WALLDIR="$HOME/Pictures/Wallpapers"
INTERVAL=3600          # seconds between rotations
TICK=30                # poll granularity, also how fast a hotplug is noticed
LOCKFILE="/tmp/wallpaper-rotate.lock"
# Must outlive a reboot, so not /tmp.
STATEFILE="$HOME/.local/state/wallpaper-rotate.current"

# Single instance: replace any previous rotator rather than stacking timers.
if [ -f "$LOCKFILE" ]; then
    OLD_PID=$(cat "$LOCKFILE" 2>/dev/null)
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        kill "$OLD_PID" 2>/dev/null
        sleep 0.5
    fi
fi
echo $$ > "$LOCKFILE"
trap 'rm -f "$LOCKFILE"; exit 0' EXIT INT TERM

# Trap only sets a flag. Calling rotate() from the handler would fire once here
# and again when the interrupted loop resumed, double-stepping the wallpaper.
ROTATE_NOW=0
trap 'ROTATE_NOW=1' USR1

for dep in hyprctl jq; do
    command -v "$dep" > /dev/null 2>&1 || exit 1
done
[ -d "$WALLDIR" ] || { notify-send -u normal -t 3000 "🖼 Wallpaper" "No such directory: $WALLDIR"; exit 1; }

monitors() {
    hyprctl monitors -j 2>/dev/null | jq -r '.[].name' 2>/dev/null
}

list_walls() {
    find "$WALLDIR" -maxdepth 1 -type f \
        \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' -o -iname '*.bmp' \) \
        | sort
}

# Hyprland's IPC and hyprlax are both starting from the same hook we are, so
# nothing can be applied until they answer. Without this the first apply is
# silently lost and the desktop stays blank until the next tick.
wait_ready() {
    for _ in $(seq 1 90); do
        if hyprlax ctl status > /dev/null 2>&1 && [ -n "$(monitors)" ]; then
            return 0
        fi
        sleep 1
    done
    return 1
}

apply_wall() {
    wall="$1"
    [ -f "$wall" ] || return 1

    mons=$(monitors)
    [ -z "$mons" ] && return 1

    # A workspace with its own wallpaper is up (workspace-wall.sh): keep the
    # rotation going underneath and let that script restore it on leave.
    if [ ! -f "${XDG_RUNTIME_DIR:-/tmp}/workspace-wall.override" ]; then
        hyprlax_set "$wall" || return 1
    fi

    echo "$wall" > "$STATEFILE"

    # Keep the lock screen on the same image. hyprlock cannot read a rotating
    # path, and it fails outright if its background is missing -- which is what
    # broke it when dark.png was deleted.
    ln -sfn "$wall" "$HOME/.config/hypr/lock-bg"

    LAST_MONITORS="$mons"
    return 0
}

# Swap hyprlax's image layer, or add one if it has none. Verified against
# `ctl list` because a request sent while hyprlax starts up can be lost.
hyprlax_set() {
    id=$(hyprlax ctl list --json 2>/dev/null | jq -r '.[0].id // empty' 2>/dev/null)
    if [ -n "$id" ]; then
        hyprlax ctl modify "$id" path "$1" > /dev/null 2>&1
    else
        hyprlax ctl add "$1" > /dev/null 2>&1
    fi
    hyprlax ctl list --json 2>/dev/null | jq -e --arg w "$1" 'any(.[]; .path == $w)' > /dev/null
}

apply_with_retry() {
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        apply_wall "$1" && return 0
        sleep 2
    done
    return 1
}

# Random pick, avoiding an immediate repeat when more than one exists.
pick_next() {
    mapfile -t walls < <(list_walls)
    count=${#walls[@]}
    [ "$count" -eq 0 ] && return 1
    if [ "$count" -eq 1 ]; then
        printf '%s\n' "${walls[0]}"
        return 0
    fi
    current=$(cat "$STATEFILE" 2>/dev/null)
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        candidate="${walls[RANDOM % count]}"
        [ "$candidate" != "$current" ] && { printf '%s\n' "$candidate"; return 0; }
    done
    printf '%s\n' "$candidate"
}

rotate() {
    next=$(pick_next) || {
        notify-send -u normal -t 3000 "🖼 Wallpaper" "No images found in $WALLDIR"
        return 1
    }
    apply_with_retry "$next"
}

wait_ready || {
    notify-send -u normal -t 5000 "🖼 Wallpaper" "hyprlax never became ready; rotation disabled"
    exit 1
}

LAST_MONITORS=$(monitors)

# Restore the wallpaper that was showing before the reboot rather than jumping
# to a random one. (hyprlax already starts on it via lock-bg; this re-applies
# the real path so `ctl list` shows which file is up.)
SAVED=$(cat "$STATEFILE" 2>/dev/null)
if [ -n "$SAVED" ] && [ -f "$SAVED" ]; then
    apply_with_retry "$SAVED" || rotate
else
    rotate
fi
elapsed=0

while true; do
    sleep "$TICK" &
    SLEEP_PID=$!
    wait "$SLEEP_PID"                 # returns early when SIGUSR1 arrives
    kill "$SLEEP_PID" 2>/dev/null     # tidy the timer if we were interrupted

    if [ "$ROTATE_NOW" = "1" ]; then
        ROTATE_NOW=0
        elapsed=0
        rotate
        continue
    fi

    elapsed=$((elapsed + TICK))

    # A display plugged in after startup has no wallpaper until something sets
    # one; re-apply the current image instead of leaving it blank for an hour.
    NOW_MONITORS=$(monitors)
    if [ -n "$NOW_MONITORS" ] && [ "$NOW_MONITORS" != "$LAST_MONITORS" ]; then
        LAST_MONITORS="$NOW_MONITORS"
        CURRENT=$(cat "$STATEFILE" 2>/dev/null)
        [ -n "$CURRENT" ] && apply_with_retry "$CURRENT"
    fi

    if [ "$elapsed" -ge "$INTERVAL" ]; then
        elapsed=0
        rotate
    fi
done
