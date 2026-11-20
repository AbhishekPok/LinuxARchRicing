#!/usr/bin/env bash
# Installs the ashwa-mac SDDM theme (macOS-style, matches amane's lock screen)
# and points SDDM at it. Does NOT restart sddm; takes effect at next login.
#
#   sudo ./install.sh              install with the theme's own background.jpg
#   sudo ./install.sh <image>      copy <image> in as the new background first
#
# The greeter runs as the sddm user and cannot read /home, so the login
# background lives in the theme (ashwa-mac/background.jpg) and is independent
# of the desktop wallpaper: changing wallpapers never changes the login screen.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "run with sudo" >&2; exit 1; }
SRC="$(cd "$(dirname "$0")" && pwd)/ashwa-mac"
DEST=/usr/share/sddm/themes/ashwa-mac
OWNER_HOME=$(getent passwd "${SUDO_USER:-abhishek}" | cut -d: -f6)
STAMP=$(date +%F-%H%M)

grep -qx 'QtVersion=6' "$SRC/metadata.desktop" || { echo "refusing: no QtVersion=6" >&2; exit 1; }

if [ $# -ge 1 ]; then
    [ -f "$1" ] || { echo "no such image: $1" >&2; exit 1; }
    cp "$1" "$SRC/background.jpg" && echo "==> background: $1"
fi
[ -f "$SRC/background.jpg" ] || { echo "missing $SRC/background.jpg" >&2; exit 1; }
[ -f "$OWNER_HOME/.face" ] && cp "$OWNER_HOME/.face" "$SRC/face.png" && echo "==> avatar: ~/.face"
chown -R "${SUDO_USER:-abhishek}": "$SRC"

cp -a /etc/sddm.conf "/etc/sddm.conf.bak.$STAMP"
rm -rf "$DEST"; cp -r "$SRC" "$DEST"
chown -R root:root "$DEST"
find "$DEST" -type d -exec chmod 755 {} +
find "$DEST" -type f -exec chmod 644 {} +
sed -i 's|^Current=.*|Current=ashwa-mac|' /etc/sddm.conf

grep -A1 '^\[Theme\]' /etc/sddm.conf
echo "greeter can read it: $(sudo -u sddm test -r "$DEST/background.jpg" && echo yes || echo NO)"
echo "Done. Next login. Rollback: $(dirname "$0")/rollback.sh"
