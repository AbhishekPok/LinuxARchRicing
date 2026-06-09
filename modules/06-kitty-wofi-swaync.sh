#!/usr/bin/env bash

copy_file "$DOTFILES_DIR/files/kitty/kitty.conf" "$HOME/.config/kitty/kitty.conf"
copy_file "$DOTFILES_DIR/files/wofi/config" "$HOME/.config/wofi/config"
copy_file "$DOTFILES_DIR/files/wofi/style.css" "$HOME/.config/wofi/style.css"
copy_file "$DOTFILES_DIR/files/swaync/config.json" "$HOME/.config/swaync/config.json"
copy_file "$DOTFILES_DIR/files/swaync/style.css" "$HOME/.config/swaync/style.css"
