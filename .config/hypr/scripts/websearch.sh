#!/usr/bin/env bash
# Web search, run by the amane launcher for "s <query>" (SUPER+ALT+W opens it).
# Idea from SherLock707/hyprland_dot_yadm (RofiSearch.sh). Prefix the query to
# pick an engine, e.g. "s gh hyprland lua" or "s aw ec2 ssm"; a URL or a bare
# domain opens directly; anything else searches Google.

declare -A engines=(
    [g]="https://www.google.com/search?q="
    [gh]="https://github.com/search?type=repositories&q="
    [aw]="https://wiki.archlinux.org/index.php?search="
    [aur]="https://aur.archlinux.org/packages?K="
    [yt]="https://www.youtube.com/results?search_query="
    [mdn]="https://developer.mozilla.org/en-US/search?q="
    [ghc]="https://github.com/search?type=code&q="
    [tf]="https://registry.terraform.io/search/providers?q="
    [tfm]="https://registry.terraform.io/search/modules?q="
)

query=${1:-}
[[ -z "$query" ]] && exec ~/.cargo/bin/amane ipc call auxiliary search s

key=${query%% *}
if [[ "$query" =~ ^https?:// ]]; then
    target=$query
elif [[ "$query" == *.* && "$query" != *" "* ]]; then
    target="https://$query"
elif [[ -n "${engines[$key]}" && "$query" == *" "* ]]; then
    target=${engines[$key]}$(jq -rn --arg q "${query#* }" '$q|@uri')
else
    target=${engines[g]}$(jq -rn --arg q "$query" '$q|@uri')
fi

xdg-open "$target" >/dev/null 2>&1 &

# Bring the browser forward instead of leaving the tab to be hunted down
# (idea from dhrruvsharma/shell smart-chrome.sh). Chrome opens the tab in the
# window it used last, which is the one Hyprland focused most recently. A
# browser that wasn't running opens a fresh window that takes focus anyway.
sleep 0.4
addr=$(hyprctl clients -j | jq -r '
    [.[] | select(.class | test("^(google-chrome|chromium|brave-browser|firefox|zen)"; "i"))]
    | min_by(.focusHistoryID) | .address // empty')
# Best effort: a browser that is only starting has no window yet, and that is
# not a failure -- the search still opens -- so never let it set the exit code.
if [[ -n "$addr" ]]; then
    hyprctl eval "hl.dispatch(hl.dsp.focus({ window = 'address:$addr' }))" >/dev/null
fi
exit 0
