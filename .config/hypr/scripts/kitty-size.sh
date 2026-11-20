#!/usr/bin/env bash
# Floating kitty at a chosen size ("kitty " in the launcher, SUPER+ALT+Return).
# Idea from dave-eska/Nyxrn (open_kitty.sh). Sizes come from the
# kitty-small/medium/large/box rules in hyprland.lua.
case "${1:-}" in
    --menu)
        printf '%s\t%s\n' "Small" small "Medium" medium "Large" large "Box" box ;;
    --menu-pick)
        setsid kitty --class "kitty-$2" >/dev/null 2>&1 < /dev/null & ;;
    *)
        exec ~/.cargo/bin/amane ipc call auxiliary search kitty ;;
esac
