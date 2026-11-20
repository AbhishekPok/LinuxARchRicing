#!/usr/bin/env bash
# clipboard-watch.sh — keep cliphist's wl-paste watchers alive.
#
# The two watchers used to be plain top-level exec lines in hyprland.lua, so
# Hyprland ran them while parsing the config -- before the Wayland socket was
# ready. They exited immediately and clipboard history silently stopped being
# recorded (db last written two days before this was found). This waits for the
# socket, starts both watchers, and restarts either one if it dies.
set -u

LOCK=/tmp/clipboard-watch.$UID.lock
INTERVAL=10

# Single supervisor, even if hyprland re-runs this on reload.
exec 9>"$LOCK"
# -w 3, not -n: if hyprland restarts this the instant an old supervisor is
# killed, the dying process can still hold the lock for a moment.
flock -w 3 9 || exit 0

# Wait for the compositor's clipboard to answer (up to ~15s).
for _ in $(seq 1 60); do
    wl-paste --list-types >/dev/null 2>&1 && break
    sleep 0.25
done

# Anchored: an unanchored -f pattern also matches any shell whose command line
# merely contains this text (a test shell, a grep), which would make us think
# the watcher is up when it is not.
watcher_up() { pgrep -f "^wl-paste --type $1 --watch cliphist store$" >/dev/null 2>&1; }

# wl-clip-persist: on Wayland the clipboard dies with the program you copied
# from. This reads the selection into memory and re-offers it, so copy-then-close
# keeps working. cliphist is documented as compatible with it.
#   --clipboard regular      upstream advises against primary (app side effects)
#   --ignore-event-on-error  do not re-offer half-read selections
#   --selection-size-limit   cap held data so a huge image cannot balloon RSS
PERSIST_ARGS=(--clipboard regular --ignore-event-on-error --selection-size-limit 67108864)

persist_up() { pgrep -x wl-clip-persist >/dev/null 2>&1; }

start_persist() {
    persist_up && return 0
    setsid wl-clip-persist "${PERSIST_ARGS[@]}" >/dev/null 2>&1 < /dev/null 9>&- &
}

start() {
    watcher_up "$1" && return 0
    # 9>&- : without closing the lock fd the watchers inherit it and keep the
    # flock held after this supervisor exits, which blocks every later one.
    setsid wl-paste --type "$1" --watch cliphist store >/dev/null 2>&1 < /dev/null 9>&- &
}

while :; do
    start_persist
    start text
    start image
    sleep "$INTERVAL"
done
