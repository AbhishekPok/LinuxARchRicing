#!/usr/bin/env bash
# Clipboard history for the amane launcher ("clip ", SUPER+SHIFT+V).
#
#   --menu               one line per entry: preview<TAB>cliphist id
#   --menu-pick ID       copy that entry back to the clipboard
#   --menu-delete ID     drop it from history (Ctrl+X in the launcher)
#   --wipe               clear the whole history
#
# cliphist wants "<id><TAB>" on stdin, not the bare id. wl-copy forks a server
# that would hold the launcher's pipe open, so its output goes to /dev/null.
case "${1:-}" in
    --menu)
        cliphist list | awk -F'\t' '{ id = $1; sub(/^[^\t]*\t/, ""); gsub(/\t/, " "); print $0 "\t" id }' ;;
    --menu-pick)
        printf '%s\t\n' "$2" | cliphist decode | wl-copy >/dev/null 2>&1 ;;
    --menu-delete)
        printf '%s\t\n' "$2" | cliphist delete ;;
    --wipe)
        cliphist wipe && notify-send -a Clipboard "Clipboard" "History wiped" ;;
    *)
        exec ~/.cargo/bin/amane ipc call auxiliary search clip ;;
esac
