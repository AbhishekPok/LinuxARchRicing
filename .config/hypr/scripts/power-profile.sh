#!/bin/bash
# Cycle the power profile: balanced -> performance -> power-saver -> balanced
# (Spotlight "Power profile (cycle)", F7). Takes no arguments; anything else
# only prints usage, so a probe can never change the profile by accident.
if [ $# -gt 0 ]; then
    echo "usage: power-profile.sh   (cycles balanced -> performance -> power-saver)" >&2
    exit 2
fi
CURRENT=$(powerprofilesctl get)

case "$CURRENT" in
    balanced)
        powerprofilesctl set performance
        notify-send -u low -t 2000 "⚡ Performance" "Mode: Performance" 
        ;;
    performance)
        powerprofilesctl set power-saver
        notify-send -u low -t 2000 "🔋 Power Saver" "Mode: Power Saver"
        ;;
    power-saver)
        powerprofilesctl set balanced
        notify-send -u low -t 2000 "⚖️ Balanced" "Mode: Balanced"
        ;;
esac
