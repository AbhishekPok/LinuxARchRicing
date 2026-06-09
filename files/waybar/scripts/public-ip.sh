#!/usr/bin/env bash

CACHE_FILE="/tmp/waybar-public-ip"
CACHE_TTL=300

now="$(date +%s)"

if [ -f "$CACHE_FILE" ]; then
    file_time="$(stat -c %Y "$CACHE_FILE" 2>/dev/null || echo 0)"
    age=$((now - file_time))

    if [ "$age" -lt "$CACHE_TTL" ]; then
        cached="$(cat "$CACHE_FILE" 2>/dev/null)"
        [ -n "$cached" ] && echo "󰩟 $cached" && exit 0
    fi
fi

ip="$(curl -4 -s --max-time 3 ifconfig.me 2>/dev/null)"

if [ -z "$ip" ]; then
    ip="$(curl -s --max-time 3 ifconfig.me 2>/dev/null)"
fi

if [ -z "$ip" ]; then
    echo "󰩟 no ip"
else
    echo "$ip" > "$CACHE_FILE"
    echo "󰩟 $ip"
fi
