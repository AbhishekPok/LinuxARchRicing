#!/usr/bin/env bash
# Jump to any running tmux window, with a live preview of what it shows.
# SUPER + SHIFT + O. Idea from fiffeek/.dotfiles.v2 (tmux-window-switch.sh).
#
# Companion to project.sh (SUPER + O): that one opens a project, this one
# jumps into one already running. The picker runs in a floating kitty (class
# kitty-tmuxswitch). After the pick, the kitty already attached to that
# session is focused; if there is none, a new one attaches.

if [[ "$1" != --inner ]]; then
    if ! tmux list-sessions > /dev/null 2>&1; then
        notify-send -u low "tmux" "No tmux sessions running"
        exit 0
    fi
    exec kitty --class kitty-tmuxswitch "$0" --inner
fi

pick=$(
    tmux list-windows -a -F $'#S:#I\t#{?window_active,●, } #S  #I:#W  · #{pane_current_command}  #{pane_current_path}' \
        | sed "s|$HOME|~|" \
        | fzf --delimiter=$'\t' --with-nth=2 --height=100% --layout=reverse \
              --prompt='tmux ❯ ' --border=none --info=inline-right \
              --color='hl:#00e5ff,hl+:#00e5ff,pointer:#00ff99,prompt:#00ff99,info:#00ff99,border:#333333,bg+:#1a1a1a' \
              --preview 'tmux capture-pane -ep -t {1}' \
              --preview-window='right,62%,border-left'
) || exit 0

target=${pick%%$'\t'*}
session=${target%%:*}
tmux select-window -t "$target"

# Find the kitty showing $session: tmux knows each client's pid, and walking
# up from it reaches the kitty that owns the pty (project.sh starts tmux
# directly under kitty; an attach typed in zsh has the shell in between).
clients=$(hyprctl clients -j)
addr=
while read -r cpid csess; do
    [[ "$csess" == "$session" ]] || continue
    pid=$cpid
    for _ in 1 2 3 4; do
        addr=$(jq -r --argjson p "$pid" '.[] | select(.pid == $p) | .address' <<< "$clients" | head -1)
        [[ -n "$addr" ]] && break 2
        pid=$(ps -o ppid= -p "$pid" | tr -d ' ')
        [[ -z "$pid" || "$pid" -le 1 ]] && break
    done
done < <(tmux list-clients -F '#{client_pid} #{client_session}')

if [[ -n "$addr" ]]; then
    hyprctl eval "hl.dispatch(hl.dsp.focus({ window = 'address:$addr' }))" > /dev/null
else
    setsid kitty tmux attach -t "=$session" > /dev/null 2>&1 < /dev/null &
fi
