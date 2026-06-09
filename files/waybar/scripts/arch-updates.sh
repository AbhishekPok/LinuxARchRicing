#!/usr/bin/env bash

updates=0
aur_updates=0

if command -v checkupdates >/dev/null 2>&1; then
    updates=$(checkupdates 2>/dev/null | wc -l)
fi

if command -v paru >/dev/null 2>&1; then
    aur_updates=$(paru -Qua 2>/dev/null | wc -l)
elif command -v yay >/dev/null 2>&1; then
    aur_updates=$(yay -Qua 2>/dev/null | wc -l)
fi

total=$((updates + aur_updates))

if [ "$total" -gt 0 ]; then
    echo " $total"
else
    echo " 0"
fi
