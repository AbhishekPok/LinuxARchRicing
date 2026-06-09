#!/usr/bin/env bash

set -euo pipefail

DOTFILES_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODULES_DIR="$DOTFILES_DIR/modules"

source "$MODULES_DIR/00-utils.sh"

section "Abhi DevOps Hyprland Ricing Installer"

run_module "01-dirs.sh"
run_module "02-theme.sh"
run_module "03-hypr.sh"
run_module "04-waybar.sh"
run_module "05-wlogout.sh"
run_module "06-kitty-wofi-swaync.sh"
run_module "07-gtk.sh"
run_module "08-finalize.sh"

section "Installation complete"

echo "Ricing applied successfully."
echo "Backups saved in: $BACKUP_DIR"
