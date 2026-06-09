#!/usr/bin/env bash

if ! command -v docker >/dev/null 2>&1; then
    echo "󰡨 NA"
    exit 0
fi

if systemctl is-active --quiet docker 2>/dev/null; then
    running=$(docker ps -q 2>/dev/null | wc -l)
    total=$(docker ps -aq 2>/dev/null | wc -l)

    if [ "$running" -gt 0 ]; then
        echo "󰡨 $running/$total"
    else
        echo "󰡨 0"
    fi
else
    echo "󰡨 OFF"
fi
