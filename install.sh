#!/usr/bin/env bash
# Copy this rice into $HOME. Any file it would replace is moved to
# ~/.rice-backup-<time>/ first, at the same relative path.
set -euo pipefail

src=$(cd "$(dirname "$0")" && pwd)
backup="$HOME/.rice-backup-$(date +%Y%m%d-%H%M%S)"

paths=(
    .config/amane
    .config/hypr
    .config/theme-manager/themes
    .config/kitty
    .config/fontconfig
    .config/fastfetch
    .config/gtk-3.0/settings.ini
    .config/gtk-4.0/settings.ini
    .local/bin/track
    .local/bin/chrome-theme-sync
    .local/bin/kbb-scrolllock.sh
    .local/share/sddm-ashwa-min
    .local/share/sddm-ashwa-mac
    Pictures/Wallpapers
)

for p in "${paths[@]}"; do
    while IFS= read -r -d '' f; do
        rel=${f#"$src"/}
        dest="$HOME/$rel"
        if [[ -e $dest ]] && ! cmp -s "$f" "$dest"; then
            mkdir -p "$backup/$(dirname "$rel")"
            mv "$dest" "$backup/$rel"
        fi
        mkdir -p "$(dirname "$dest")"
        cp -a "$f" "$dest"
    done < <(find "$src/$p" -type f -print0)
done

if [[ -d $backup ]]; then
    echo "Replaced files were moved to $backup"
fi
echo "Done. Next: build amane, run theme-set, see README.md."
