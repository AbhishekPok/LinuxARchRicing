#!/usr/bin/env bash
# Battery / AC switcher. Event-driven: waits on udev power_supply events, so
# it costs nothing between plug and unplug.
#
# On battery:  power-saver profile, no blur, no shadows, brightness capped at
#              BATTERY_BRIGHTNESS (only lowered, never raised).
# On AC:       balanced profile, blur and shadows back, brightness restored to
#              what it was when the charger came out (unless you changed it).
#
#   power-mode.sh            watch (started from hyprland.lua)
#   power-mode.sh battery    apply battery mode once
#   power-mode.sh ac         apply AC mode once

BATTERY_BRIGHTNESS=50
AC=$(echo /sys/class/power_supply/A*/online | cut -d' ' -f1)
STATE="${XDG_RUNTIME_DIR:-/tmp}/power-mode"
mkdir -p "$STATE"

hypr() { hyprctl dispatch "function() hl.config($1) end" >/dev/null 2>&1; }

battery_mode() {
    powerprofilesctl set power-saver 2>/dev/null
    hypr '{decoration = {blur = {enabled = false}, shadow = {enabled = false}}}'

    local now
    now=$(brightnessctl -m | cut -d, -f4 | tr -d %)
    if (( now > BATTERY_BRIGHTNESS )); then
        echo "$now" >"$STATE/brightness"
        brightnessctl -q set "${BATTERY_BRIGHTNESS}%"
        echo "$BATTERY_BRIGHTNESS" >"$STATE/set"
    else
        rm -f "$STATE/brightness" "$STATE/set"
    fi
}

ac_mode() {
    powerprofilesctl set balanced 2>/dev/null
    hypr '{decoration = {blur = {enabled = true}, shadow = {enabled = true}}}'

    # restore only if the user didn't pick a brightness themselves meanwhile
    if [[ -f "$STATE/brightness" && -f "$STATE/set" ]]; then
        local now
        now=$(brightnessctl -m | cut -d, -f4 | tr -d %)
        (( now == $(<"$STATE/set") )) && brightnessctl -q set "$(<"$STATE/brightness")%"
    fi
    rm -f "$STATE/brightness" "$STATE/set"
}

apply() {
    local online
    online=$(<"$AC")
    [[ "$online" == "$(<"$STATE/last" 2>/dev/null)" ]] && return
    echo "$online" >"$STATE/last"
    if [[ "$online" == 1 ]]; then ac_mode; else battery_mode; fi
}

case "${1:-watch}" in
    battery) battery_mode; exit ;;
    ac)      ac_mode; exit ;;
esac

# one watcher only, even after a config reload re-runs autostart
exec 9>"$STATE/lock"
flock -n 9 || exit 0

rm -f "$STATE/last"
apply
udevadm monitor --udev --subsystem-match=power_supply 2>/dev/null |
    while read -r line; do
        [[ "$line" == *change* ]] && apply
    done
