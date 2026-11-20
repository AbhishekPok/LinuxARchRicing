#!/usr/bin/env bash
# Installs the ashwa-min SDDM theme and points SDDM at it.
# Does NOT restart sddm — that would kill the running session. Next login.
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)/ashwa-min"
DEST=/usr/share/sddm/themes/ashwa-min
STAMP=$(date +%F-%H%M)

[ -d "$SRC" ] || { echo "missing source: $SRC" >&2; exit 1; }
[ "$(id -u)" -eq 0 ] || { echo "run with sudo" >&2; exit 1; }

# The theme's .qml uses versionless imports (Qt6-only). SDDM picks the greeter
# binary from QtVersion in metadata.desktop; without it the daemon falls back to
# the Qt5 /usr/bin/sddm-greeter and the theme dies with "import needs a version".
grep -qx 'QtVersion=6' "$SRC/metadata.desktop" || {
    echo "refusing: $SRC/metadata.desktop has no 'QtVersion=6'" >&2; exit 1; }

echo "==> backing up /etc/sddm.conf -> /etc/sddm.conf.bak.$STAMP"
cp -a /etc/sddm.conf "/etc/sddm.conf.bak.$STAMP"

echo "==> installing theme to $DEST"
rm -rf "$DEST"; cp -r "$SRC" "$DEST"
chown -R root:root "$DEST"
find "$DEST" -type d -exec chmod 755 {} +
find "$DEST" -type f -exec chmod 644 {} +

echo "==> pointing [Theme] Current at ashwa-min"
if grep -q '^Current=' /etc/sddm.conf; then
    sed -i 's|^Current=.*|Current=ashwa-min|' /etc/sddm.conf
else
    printf '\n[Theme]\nCurrent=ashwa-min\n' >> /etc/sddm.conf
fi

echo "==> removing the dead 'corners' stub"
# No Main.qml, no metadata.desktop, and its dark.png is a 14-byte file holding
# the text "404: Not Found" from a failed download.
if [ -d /usr/share/sddm/themes/corners ] && [ ! -f /usr/share/sddm/themes/corners/Main.qml ]; then
    rm -rf /usr/share/sddm/themes/corners && echo "    removed"
else
    echo "    skipped"
fi

echo
grep -A1 '^\[Theme\]' /etc/sddm.conf
echo "    QtVersion:        $(grep -oP '(?<=^QtVersion=).*' "$DEST/metadata.desktop") (6 = sddm-greeter-qt6)"
echo "    greeter-readable: $(sudo -u sddm test -r "$DEST/Main.qml" && echo yes || echo NO)"
echo
echo "Done. Takes effect at next login. Rollback: $(dirname "$0")/rollback.sh"
