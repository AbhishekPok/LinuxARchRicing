#!/usr/bin/env bash

if pgrep -x waybar >/dev/null 2>&1; then
    pkill waybar
else
    nohup waybar >/dev/null 2>/tmp/waybar.log & disown
fi
