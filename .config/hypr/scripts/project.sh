#!/usr/bin/env bash
# Project launcher ("proj " in the amane launcher, SUPER+O): pick a git repo,
# open it as a tmux session in kitty.
# Idea from dave-eska/Nyxrn (tmux_session.sh), rewritten to find repos instead
# of hardcoding a language per folder. Re-picking a project reattaches to the
# session that is already running.
#
# Session layout: 1 nvim | 2 git | 3 shell

ROOTS=(~/Projects ~/Documents/Technozlife ~/Downloads/project)

case "${1:-}" in
    --menu)
        find "${ROOTS[@]}" -maxdepth 4 -type d -name .git -prune 2>/dev/null \
            | sed 's|/\.git$||' | sort | while IFS= read -r d; do
                printf '%s\t%s\n' "${d/#$HOME/\~}" "$d"
            done
        exit 0 ;;
    --menu-pick) dir=$2 ;;
    *) exec ~/.cargo/bin/amane ipc call auxiliary search proj ;;
esac
[[ -d "$dir" ]] || exit 0

# tmux session names can't contain dots or colons
name=$(basename "$dir" | tr '.:' '__')

if ! tmux has-session -t "=$name" 2>/dev/null; then
    tmux new-session -d -s "$name" -n nvim -c "$dir" nvim
    tmux new-window -t "$name:" -n git -c "$dir"
    tmux send-keys -t "$name:git" 'git status -sb' C-m
    tmux new-window -t "$name:" -n shell -c "$dir"
    tmux select-window -t "$name:nvim"
fi

setsid kitty --title "tmux: $name" tmux attach -t "=$name" >/dev/null 2>&1 < /dev/null &
