#!/usr/bin/env bash
# Emoji picker for the amane launcher ("emoji ", SUPER+.): search by name,
# copies the emoji. Source: JaKooLit/Hyprland-Dots via dave-eska/Nyxrn. The
# list lives in ~/.local/share/emoji/emoji.tsv (emoji<TAB>name<TAB>keywords).
LIST="$HOME/.local/share/emoji/emoji.tsv"
case "${1:-}" in
    --menu)
        awk -F'\t' '{ print $1 "  " $2 "  · " $3 "\t" $1 }' "$LIST" ;;
    --menu-pick)
        printf '%s' "$2" | wl-copy >/dev/null 2>&1 ;;
    *)
        exec ~/.cargo/bin/amane ipc call auxiliary search emoji ;;
esac
