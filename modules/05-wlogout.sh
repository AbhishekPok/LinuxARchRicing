#!/usr/bin/env bash

copy_file "$DOTFILES_DIR/files/wlogout/layout" "$HOME/.config/wlogout/layout"
copy_file "$DOTFILES_DIR/files/wlogout/style.css" "$HOME/.config/wlogout/style.css"
copy_dir "$DOTFILES_DIR/files/wlogout/icons" "$HOME/.config/wlogout/icons"
