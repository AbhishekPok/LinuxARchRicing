#!/usr/bin/env bash

SCREENSHOT_DIR="$HOME/Pictures/Screenshots"
mkdir -p "$SCREENSHOT_DIR"

timestamp="$(date +'%Y-%m-%d_%H-%M-%S')"
file="$SCREENSHOT_DIR/screenshot_$timestamp.png"
mode="$1"

area_geometry() {
    slurp 2>/dev/null
}

case "$mode" in
    area-copy)
        geo="$(area_geometry)" || exit 0
        [ -z "$geo" ] && exit 0
        grim -g "$geo" - | wl-copy
        notify-send "Screenshot" "Area copied to clipboard" >/dev/null 2>&1
        ;;

    full-copy)
        grim - | wl-copy
        notify-send "Screenshot" "Full screen copied to clipboard" >/dev/null 2>&1
        ;;

    area-save)
        geo="$(area_geometry)" || exit 0
        [ -z "$geo" ] && exit 0
        grim -g "$geo" "$file"
        wl-copy < "$file"
        notify-send "Screenshot Saved" "$(basename "$file")" >/dev/null 2>&1
        ;;

    full-save)
        grim "$file"
        wl-copy < "$file"
        notify-send "Screenshot Saved" "$(basename "$file")" >/dev/null 2>&1
        ;;

    open-folder)
        nautilus "$SCREENSHOT_DIR" >/dev/null 2>&1 &
        ;;

    *)
        notify-send "Screenshot" "Unknown mode: $mode" >/dev/null 2>&1
        exit 1
        ;;
esac
