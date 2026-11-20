#!/bin/bash
# Off only powers the adapter down: an rfkill block would persist across reboots.
if bluetoothctl show | grep -q "Powered: yes"; then
    bluetoothctl power off
    notify-send -u low -t 2000 "Bluetooth" "Disabled"
else
    rfkill unblock bluetooth
    sleep 1
    bluetoothctl power on
    notify-send -u low -t 2000 "Bluetooth" "Enabled"
fi
