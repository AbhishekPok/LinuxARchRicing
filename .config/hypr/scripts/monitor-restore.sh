#!/usr/bin/env bash
# Re-applies the saved layout (single or dual external). The logic lives in
# monitor-switch.sh so the menu and hotplug can't drift apart.
sleep 1
exec "$HOME/.config/hypr/scripts/monitor-switch.sh" --apply
