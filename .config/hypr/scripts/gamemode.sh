#!/usr/bin/env bash
# Game/Performance mode toggle
# Source: JaKooLit/Hyprland-Dots GameMode.sh (simplified)
# Lua config: `hyprctl keyword` is rejected, so settings are applied via `hyprctl eval`.

HYPRGAMEMODE=$(hyprctl getoption animations:enabled | awk 'NR==1{print $2}')

if [[ "$HYPRGAMEMODE" == "true" || "$HYPRGAMEMODE" == "1" ]]; then
    hyprctl eval 'hl.config({
        animations = { enabled = false },
        decoration = { rounding = 0, shadow = { enabled = false }, blur = { enabled = false } },
        general = { gaps_in = 0, gaps_out = 0, border_size = 1 },
    })' >/dev/null
    notify-send -u low "Game Mode" "Enabled — all effects disabled"
else
    hyprctl reload >/dev/null
    notify-send -u low "Game Mode" "Disabled — effects restored"
fi
