#!/usr/bin/env bash
# Multi-mode screenshot tool
# Source: JaKooLit/Hyprland-Dots ScreenShot.sh (simplified)

time=$(date "+%d-%b_%H-%M-%S")
PICTURES_DIR="$(xdg-user-dir PICTURES 2>/dev/null)"
[[ -z "$PICTURES_DIR" || "$PICTURES_DIR" == "$HOME" ]] && PICTURES_DIR="$HOME/Pictures"
dir="$PICTURES_DIR/Screenshots"
file="Screenshot_${time}_${RANDOM}.png"

mkdir -p "$dir"

notify_view() {
    local check_file="$1"
    if [[ -s "$check_file" ]]; then
        local action
        action=$(notify-send -t 10000 \
            -A "open=Open" \
            -A "delete=Delete" \
            "Screenshot" "Saved: $(basename "$check_file")")
        case "$action" in
            open) xdg-open "$check_file" & ;;
            delete) rm "$check_file" ;;
        esac
    else
        rm -f "$check_file"
        notify-send -u low "Screenshot" "Failed to save"
    fi
}

countdown() {
    for sec in $(seq "$1" -1 1); do
        notify-send -t 1000 "Screenshot" "Taking in: $sec"
        sleep 1
    done
}

# "Screen" is the monitor you are on (as on macOS), not every monitor
# stitched into one image; with a single screen that is the same thing
shot_full() {
    local output
    output=$(hyprctl monitors -j | jq -r '.[] | select(.focused) | .name')
    cd "$dir" && grim ${output:+-o "$output"} - | tee "$file" | wl-copy
    notify_view "$dir/$file"
}

case "${1:-}" in
    --now)
        shot_full
        ;;
    --area)
        geom=$(slurp) || exit 0
        cd "$dir" && grim -g "$geom" - | tee "$file" | wl-copy
        notify_view "$dir/$file"
        ;;
    --active)
        active_file="Screenshot_${time}_$(hyprctl -j activewindow | jq -r '.class').png"
        hyprctl -j activewindow | jq -r '"\(.at[0]),\(.at[1]) \(.size[0])x\(.size[1])"' | \
            grim -g - "$dir/$active_file"
        wl-copy < "$dir/$active_file"
        notify_view "$dir/$active_file"
        ;;
    --in5)
        countdown 5
        sleep 1 && shot_full
        ;;
    --in10)
        countdown 10
        sleep 1 && shot_full
        ;;
    --swappy)
        command -v swappy >/dev/null || { notify-send -u low "Screenshot" "swappy not installed"; exit 1; }
        geom=$(slurp) || exit 0
        grim -g "$geom" - | swappy -f -
        ;;
    *)
        echo "Usage: $0 {--now|--area|--active|--in5|--in10|--swappy}"
        exit 1
        ;;
esac
