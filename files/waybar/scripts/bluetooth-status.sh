#!/usr/bin/env bash

if ! command -v bluetoothctl >/dev/null 2>&1; then
    echo "󰂲 NA"
    exit 0
fi

if rfkill list bluetooth 2>/dev/null | command grep -q "Soft blocked: yes"; then
    echo "󰂲 Blocked"
    exit 0
fi

if ! systemctl is-active --quiet bluetooth; then
    echo "󰂲 OFF"
    exit 0
fi

powered="$(bluetoothctl show 2>/dev/null | awk -F': ' '/Powered/ {print $2; exit}')"

if [ "$powered" != "yes" ]; then
    echo "󰂲 Off"
    exit 0
fi

connected="$(bluetoothctl devices Connected 2>/dev/null | wc -l)"

if [ "$connected" -gt 0 ]; then
    device="$(bluetoothctl devices Connected 2>/dev/null | sed 's/^Device [A-F0-9:]* //' | head -n 1)"
    short="$(echo "$device" | cut -c1-14)"
    echo "󰂱 $short"
else
    echo "󰂯 On"
fi
