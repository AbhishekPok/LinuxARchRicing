#!/usr/bin/env bash

WALL_DIR="$HOME/Pictures/Wallpapers"
LOG_FILE="/tmp/awww.log"

pick_wallpaper() {
    command find "$WALL_DIR" -type f \( \
        -iname "*.jpg" -o \
        -iname "*.jpeg" -o \
        -iname "*.png" -o \
        -iname "*.webp" -o \
        -iname "*.gif" \
    \) 2>/dev/null | shuf -n 1
}

if ! pgrep -x awww-daemon >/dev/null 2>&1; then
    nohup awww-daemon > "$LOG_FILE" 2>&1 &
    sleep 0.8
fi

LAPTOP_WALL="$(pick_wallpaper)"
HDMI_WALL="$(pick_wallpaper)"

if [ -z "$LAPTOP_WALL" ]; then
    notify-send "Wallpaper" "No image found in $WALL_DIR" >/dev/null 2>&1
    exit 1
fi

outputs="$(awww query 2>/dev/null)"

if echo "$outputs" | command grep -q "eDP-1"; then
    awww img -o eDP-1 "$LAPTOP_WALL" >/dev/null 2>&1
fi

if echo "$outputs" | command grep -q "HDMI-A-1"; then
    [ -n "$HDMI_WALL" ] && awww img -o HDMI-A-1 "$HDMI_WALL" >/dev/null 2>&1
fi

notify-send "Wallpaper Updated" "$(basename "$LAPTOP_WALL")" >/dev/null 2>&1
