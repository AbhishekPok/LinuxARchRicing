#!/usr/bin/env bash

if ! command -v swaync-client >/dev/null 2>&1; then
    echo "󰂚 NA"
    exit 0
fi

count=$(swaync-client -c 2>/dev/null)
dnd=$(swaync-client -D 2>/dev/null)

if [ "$dnd" = "true" ]; then
    echo "󰂛 DND"
elif [ "$count" -gt 0 ] 2>/dev/null; then
    echo "󰂚 $count"
else
    echo "󰂚"
fi
