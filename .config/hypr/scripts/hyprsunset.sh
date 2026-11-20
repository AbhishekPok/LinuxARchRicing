#!/usr/bin/env bash
# Night light (SUPER+SHIFT+N): hyprsunset on/off with the state kept across
# logins (init). Source: JaKooLit/Hyprland-Dots Hyprsunset.sh (simplified)

STATE_FILE="$HOME/.cache/.hyprsunset_state"
# Target temperature; `up` / `down` move it in 250 K steps
TEMP_FILE="$HOME/.cache/.hyprsunset_temp"
TARGET_TEMP="$(cat "$TEMP_FILE" 2>/dev/null || echo "${HYPRSUNSET_TEMP:-4500}")"
STEP=250
MIN_TEMP=2500
MAX_TEMP=6500

[[ -f "$STATE_FILE" ]] || echo "off" > "$STATE_FILE"

cmd_toggle() {
    command -v hyprsunset >/dev/null || { notify-send -u low "Night Light" "hyprsunset not installed"; exit 1; }
    state="$(cat "$STATE_FILE" 2>/dev/null || echo off)"

    # Kill any running instance first
    pgrep -x hyprsunset >/dev/null 2>&1 && { pkill -x hyprsunset; sleep 0.2; }

    if [[ "$state" == "on" ]]; then
        hyprsunset -i >/dev/null 2>&1 &
        sleep 0.3 && pkill -x hyprsunset 2>/dev/null
        echo "off" > "$STATE_FILE"
        notify-send -u low "Night Light" "Disabled"
    else
        setsid hyprsunset -t "$TARGET_TEMP" >/dev/null 2>&1 < /dev/null &
        echo "on" > "$STATE_FILE"
        notify-send -u low "Night Light" "Enabled — ${TARGET_TEMP}K"
    fi
}

# up = warmer (lower K), down = cooler. While it is off only the stored
# target moves, so the next toggle starts there.
cmd_adjust() {
    t=$((TARGET_TEMP + $1))
    (( t < MIN_TEMP )) && t=$MIN_TEMP
    (( t > MAX_TEMP )) && t=$MAX_TEMP
    echo "$t" > "$TEMP_FILE"
    pgrep -x hyprsunset >/dev/null 2>&1 && hyprctl hyprsunset temperature "$t" >/dev/null 2>&1
}

cmd_init() {
    command -v hyprsunset >/dev/null || exit 0
    state="$(cat "$STATE_FILE" 2>/dev/null || echo off)"
    if [[ "$state" == "on" ]]; then
        setsid hyprsunset -t "$TARGET_TEMP" >/dev/null 2>&1 < /dev/null &
    fi
}

case "${1:-}" in
    toggle) cmd_toggle ;;
    init)   cmd_init ;;
    up)     cmd_adjust "-$STEP" ;;
    down)   cmd_adjust "$STEP" ;;
    *)      echo "Usage: $0 {toggle|init|up|down}"; exit 1 ;;
esac
