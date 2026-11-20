#!/usr/bin/env bash
# Last-resort battery guard. The 20% and 10% warnings are amane's corner card
# (src/island/power.rs); this only handles 5%: a critical notification, then
# a suspend after 30 s unless the charger goes in, so a long terminal session
# never ends in a hard power-off. Idea from roscaa-stack/dotfiles.

BAT=$(find /sys/class/power_supply -maxdepth 1 -name 'BAT*' | head -1)
[[ -z "$BAT" ]] && exit 0

# Only one watcher, even after a hyprctl reload re-runs autostart
exec 9>"${XDG_RUNTIME_DIR:-/tmp}/battery-notify.lock"
flock -n 9 || exit 0

warned=100
while sleep 60; do
    cap=$(<"$BAT/capacity")
    status=$(<"$BAT/status")

    if [[ "$status" != "Discharging" ]]; then
        warned=100
        continue
    fi

    if (( cap <= 5 && warned > 5 )); then
        warned=5
        notify-send -u critical -i battery-empty "Battery at ${cap}%" "Suspending in 30 seconds. Plug in to cancel."
        sleep 30
        [[ $(<"$BAT/status") == "Discharging" ]] && systemctl suspend
    fi
done
