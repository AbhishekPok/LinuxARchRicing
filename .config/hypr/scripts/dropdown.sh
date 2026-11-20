#!/usr/bin/env bash
# Quake-style dropdown terminal on its own special workspace (special:dropterm).
# Placement, float and slide animation come from rules in hyprland.lua.

if hyprctl clients -j | jq -e 'any(.[]; .class == "dropterm")' >/dev/null; then
    hyprctl dispatch 'hl.dsp.workspace.toggle_special("dropterm")' >/dev/null
else
    setsid kitty --class dropterm >/dev/null 2>&1 < /dev/null &
fi
