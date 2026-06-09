#!/usr/bin/env bash

chmod +x "$HOME/.config/hypr/scripts/"*.sh 2>/dev/null || true
chmod +x "$HOME/.config/waybar/scripts/"*.sh 2>/dev/null || true

if command -v hyprctl >/dev/null 2>&1; then
  hyprctl reload || true
fi

if command -v waybar >/dev/null 2>&1; then
  pkill waybar >/dev/null 2>&1 || true
  nohup waybar >/dev/null 2>/tmp/waybar.log & disown
fi
