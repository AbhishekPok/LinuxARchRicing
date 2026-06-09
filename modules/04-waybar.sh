#!/usr/bin/env bash

copy_file "$DOTFILES_DIR/files/waybar/config.jsonc" "$HOME/.config/waybar/config.jsonc"
copy_file "$DOTFILES_DIR/files/waybar/style.css" "$HOME/.config/waybar/style.css"
copy_dir "$DOTFILES_DIR/files/waybar/scripts" "$HOME/.config/waybar/scripts"

chmod +x "$HOME/.config/waybar/scripts/"*.sh 2>/dev/null || true

WAYBAR_CONFIG="$HOME/.config/waybar/config.jsonc"

if [ -f "$WAYBAR_CONFIG" ]; then
  python - <<'WAYBAR_WS_PY'
from pathlib import Path
import re

path = Path.home() / ".config/waybar/config.jsonc"
text = path.read_text()

text = re.sub(
    r',?\n\s*"persistent-workspaces"\s*:\s*\{\s*"\*"\s*:\s*\d+\s*\}',
    '',
    text
)

path.write_text(text)
WAYBAR_WS_PY
fi
