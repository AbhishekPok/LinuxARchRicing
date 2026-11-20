#!/usr/bin/env bash
# The shell half of the workspace theme hook in hyprland.lua (WS_THEMES); the
# accent colour is set in Lua directly. Idea from Candys2000/Themeswitcher-hyprland.
#
#   workspace-theme.sh <file|-> <zen|->
#     <file>  show this wallpaper from ~/Pictures/Wallpapers
#     -       back to the rotator's current wallpaper
#     zen     Do Not Disturb on; anything else turns off a DND that zen set
#
# While a workspace wallpaper is up, OVERRIDE exists and wallpaper-rotate.sh
# keeps rotating underneath without touching hyprlax; leaving the workspace
# restores whatever the rotator last picked. hyprlax has one layer across all
# monitors, so the image changes on every screen.

WALLDIR="$HOME/Pictures/Wallpapers"
STATEFILE="$HOME/.local/state/wallpaper-rotate.current"
RUN="${XDG_RUNTIME_DIR:-/tmp}"
OVERRIDE="$RUN/workspace-wall.override"
WANT="$RUN/workspace-theme.want"
ZEN_DND="$RUN/zen-dnd"

# Switching monitors fires two events back to back, and flicking through
# workspaces many more. Each run records what it wants, waits a beat, and only
# the most recent one goes on, so they cannot race each other.
echo "$$ $1 $2" > "$WANT"
sleep 0.15
read -r pid wall_arg mode < "$WANT"
[ "$pid" = "$$" ] || exit 0

# Do Not Disturb: the marker records that zen turned it on, so leaving zen
# never switches off a DND you set yourself.
if [ "$mode" = "zen" ]; then
    if [ ! -f "$ZEN_DND" ] && [ "$("$HOME/.cargo/bin/amane" ipc call notifications dnd status)" = "off" ]; then
        "$HOME/.cargo/bin/amane" ipc call notifications dnd on > /dev/null && touch "$ZEN_DND"
    fi
elif [ -f "$ZEN_DND" ]; then
    "$HOME/.cargo/bin/amane" ipc call notifications dnd off > /dev/null
    rm -f "$ZEN_DND"
fi

# Wallpaper
if [ "$wall_arg" = "-" ]; then
    [ -f "$OVERRIDE" ] || exit 0
    rm -f "$OVERRIDE"
    wall=$(cat "$STATEFILE" 2>/dev/null)
else
    wall="$WALLDIR/$wall_arg"
    [ -f "$wall" ] || exit 0
    [ "$(cat "$OVERRIDE" 2>/dev/null)" = "$wall" ] && exit 0
    echo "$wall" > "$OVERRIDE"
fi
[ -f "$wall" ] || exit 0

id=$(hyprlax ctl list --json 2>/dev/null | jq -r '.[0].id // empty' 2>/dev/null)
[ -n "$id" ] && hyprlax ctl modify "$id" path "$wall" > /dev/null 2>&1
