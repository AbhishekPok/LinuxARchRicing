#!/usr/bin/env bash
# Back to the previous minimal theme; ashwa-mac stays installed.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "run with sudo" >&2; exit 1; }
sed -i 's|^Current=.*|Current=ashwa-min|' /etc/sddm.conf
grep -A1 '^\[Theme\]' /etc/sddm.conf
