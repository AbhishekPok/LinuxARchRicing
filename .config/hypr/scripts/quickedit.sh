#!/usr/bin/env bash
# Open one of the rice's config files in nvim ("edit " in the launcher,
# SUPER+ALT+C). Idea from JaKooLit's QuickEdit.sh.
files=(
    ~/.config/hypr/hyprland.lua
    ~/.config/hypr/hyprlock.conf
    ~/.config/hypr/hypridle.conf
    ~/.config/amane/src/island/modules.rs
    ~/.config/amane/src/auxiliary/modes.rs
    ~/.config/kitty/kitty.conf
    ~/.config/starship.toml
    ~/.config/fastfetch/config.jsonc
    ~/.zshrc
)
case "${1:-}" in
    --menu)
        for f in "${files[@]}"; do [[ -e "$f" ]] && printf '%s\t%s\n' "${f/#$HOME/\~}" "$f"; done ;;
    --menu-pick)
        setsid kitty --title "edit: $(basename "$2")" nvim "$2" >/dev/null 2>&1 < /dev/null & ;;
    *)
        exec ~/.cargo/bin/amane ipc call auxiliary search edit ;;
esac
