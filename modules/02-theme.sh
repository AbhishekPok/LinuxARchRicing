#!/usr/bin/env bash

copy_file "$DOTFILES_DIR/files/theme/colors.sh" "$HOME/.config/theme/colors.sh"

if [ -f "$HOME/.config/theme/colors.sh" ]; then
  chmod +x "$HOME/.config/theme/colors.sh"
fi
