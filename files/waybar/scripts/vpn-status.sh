#!/usr/bin/env bash

if ip link show tun0 >/dev/null 2>&1; then
    echo " VPN"
elif ip link show wg0 >/dev/null 2>&1; then
    echo " WG"
elif ip link show tailscale0 >/dev/null 2>&1; then
    echo " TS"
elif ip link show ppp0 >/dev/null 2>&1; then
    echo " VPN"
else
    echo " No VPN"
fi
