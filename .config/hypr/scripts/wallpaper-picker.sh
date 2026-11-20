#!/usr/bin/env bash
# wallpaper-picker.sh — pick a wallpaper ("wall " in the amane launcher,
# SUPER + W), each row showing the image itself as its thumbnail.
#
# Applying mirrors apply_wall() in wallpaper-rotate.sh on purpose: the rotator
# is a long-running process, so its function cannot be sourced. The steps have
# to stay in sync -- hyprlax layer swap, then the state file (so the rotator
# keeps the same idea of "current"), then the lock-bg symlink (hyprlock fails outright if its background is missing).
#
#   --list          print the wallpapers
#   --set <file>    apply that file directly
#   --menu          launcher rows: name<TAB>file<TAB>file (thumbnail)
#   --menu-pick F   same as --set
set -u

WALLDIR="$HOME/Pictures/Wallpapers"
STATEFILE="$HOME/.local/state/wallpaper-rotate.current"

list_walls() {
    find "$WALLDIR" -maxdepth 1 -type f \
        \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' -o -iname '*.bmp' \) |
        sort
}

monitors() { hyprctl monitors -j 2>/dev/null | jq -r '.[].name' 2>/dev/null; }

apply_wall() {
    local wall="$1" mons
    [ -f "$wall" ] || { notify-send -u critical "🖼 Wallpaper" "No such file"; return 1; }
    mons=$(monitors)
    [ -z "$mons" ] && return 1

    # hyprlax draws the wallpaper; swap its image layer (or add one if empty)
    local id
    id=$(hyprlax ctl list --json 2>/dev/null | jq -r '.[0].id // empty' 2>/dev/null)
    if [ -n "$id" ]; then
        hyprlax ctl modify "$id" path "$wall" > /dev/null 2>&1
    else
        hyprlax ctl add "$wall" > /dev/null 2>&1
    fi
    hyprlax ctl list --json 2>/dev/null | jq -e --arg w "$wall" 'any(.[]; .path == $w)' > /dev/null || {
        notify-send -u critical "🖼 Wallpaper" "hyprlax did not apply it"; return 1; }

    mkdir -p "$(dirname "$STATEFILE")"
    echo "$wall" > "$STATEFILE"
    ln -sfn "$wall" "$HOME/.config/hypr/lock-bg"
    notify-send -t 2500 "🖼 Wallpaper" "$(basename "$wall")"
}

case "${1:-}" in
    --list) list_walls; exit 0 ;;
    --set | --menu-pick) apply_wall "${2:-}"; exit $? ;;
    --menu)
        list_walls | while IFS= read -r f; do
            printf '%s\t%s\t%s\n' "$(basename "${f%.*}")" "$f" "$f"
        done
        exit 0 ;;
esac

exec ~/.cargo/bin/amane ipc call auxiliary search wall
