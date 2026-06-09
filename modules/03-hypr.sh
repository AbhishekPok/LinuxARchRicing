#!/usr/bin/env bash

copy_dir "$DOTFILES_DIR/files/hypr/scripts" "$HOME/.config/hypr/scripts"
copy_dir "$DOTFILES_DIR/files/hypr/conf" "$HOME/.config/hypr/conf"
copy_file "$DOTFILES_DIR/files/hypr/hyprlock.conf" "$HOME/.config/hypr/hyprlock.conf"

chmod +x "$HOME/.config/hypr/scripts/"*.sh 2>/dev/null || true

HYPRLAND_CONF="$HOME/.config/hypr/hyprland.conf"

if [ -f "$HYPRLAND_CONF" ]; then
  backup_file "$HYPRLAND_CONF"

  ensure_line "$HYPRLAND_CONF" ""
  ensure_line "$HYPRLAND_CONF" "# ============================================================"
  ensure_line "$HYPRLAND_CONF" "# Modular user config"
  ensure_line "$HYPRLAND_CONF" "# ============================================================"
  ensure_line "$HYPRLAND_CONF" "source = ~/.config/hypr/conf/keybinds-devops.conf"
  ensure_line "$HYPRLAND_CONF" "source = ~/.config/hypr/conf/windowrules-utility.conf"

  if [ -f "$HOME/.config/hypr/conf/disable-laptop-keyboard.conf" ]; then
    ensure_line "$HYPRLAND_CONF" "source = ~/.config/hypr/conf/disable-laptop-keyboard.conf"
  fi

  perl -0pi -e 's|bind = \$mainMod, Z, exec, .*waybar.*|bind = \$mainMod, Z, exec, pkill waybar; nohup waybar >/dev/null 2>/tmp/waybar.log \&|' "$HYPRLAND_CONF"
  perl -0pi -e 's/^\s*keepaspectratio = yes\n//mg' "$HYPRLAND_CONF"
else
  echo "Hyprland config not found: $HYPRLAND_CONF"
fi
