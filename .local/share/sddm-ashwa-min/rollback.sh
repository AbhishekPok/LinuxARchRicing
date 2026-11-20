#!/usr/bin/env bash
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "run with sudo" >&2; exit 1; }
sed -i 's|^Current=.*|Current=where_is_my_sddm_theme|' /etc/sddm.conf
grep -A1 '^\[Theme\]' /etc/sddm.conf
echo "back on the old theme; ashwa-min left installed at /usr/share/sddm/themes/ashwa-min"
