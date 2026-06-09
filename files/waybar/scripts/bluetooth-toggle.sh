#!/usr/bin/env bash

rfkill unblock bluetooth >/dev/null 2>&1

if ! systemctl is-active --quiet bluetooth; then
    systemctl start bluetooth >/dev/null 2>&1
fi

sleep 0.5

powered="$(bluetoothctl show 2>/dev/null | awk -F': ' '/Powered/ {print $2; exit}')"

if [ "$powered" = "yes" ]; then
    bluetoothctl power off >/dev/null 2>&1
    notify-send "Bluetooth" "Turned off" >/dev/null 2>&1
else
    bluetoothctl power on >/dev/null 2>&1
    notify-send "Bluetooth" "Turned on" >/dev/null 2>&1
fi
