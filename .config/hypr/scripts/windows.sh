#!/usr/bin/env bash
# Window switcher across workspaces ("win " in the amane launcher,
# SUPER+SHIFT+Tab), replacing rofi's window mode.
#   --menu            rows: [workspace] class — title<TAB>address
#   --menu-pick ADDR  focus that window (dispatch is Lua here, hence eval)
case "${1:-}" in
    --menu)
        hyprctl clients -j | jq -r '
            sort_by(.focusHistoryID) | .[] | select(.mapped and .workspace.id > 0)
            | "[\(.workspace.name)]  \(.class)  —  \(.title | gsub("\t"; " "))\t\(.address)"' ;;
    --menu-pick)
        hyprctl eval "hl.dispatch(hl.dsp.focus({ window = 'address:$2' }))" >/dev/null ;;
    *)
        exec ~/.cargo/bin/amane ipc call auxiliary search win ;;
esac
