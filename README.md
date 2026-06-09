# Abhi DevOps Hyprland Ricing Dotfiles

A modular Hyprland ricing setup for an Arch Linux DevOps workstation.

## Path

This repo is stored at:

    ~/Downloads/ricing

## Features

- Modular installer
- Hyprland DevOps keybinds
- Dynamic Waybar workspaces
- Waybar scripts for Arch updates, Docker, Kubernetes, VPN, public IP, Bluetooth, and notifications
- Random wallpaper script using awww
- Screenshot script using grim, slurp, and wl-copy
- Hyprlock lock screen
- Wlogout power menu
- Kitty terminal theme
- Wofi launcher theme
- SwayNC notification center theme
- GTK dark theme settings
- Optional laptop keyboard disable config
- Automatic backups before overwrite

## Structure

    ricing/
    ├── install.sh
    ├── README.md
    ├── .gitignore
    ├── dotfile.sh
    ├── modules/
    ├── files/
    └── reference/

## Install

    cd ~/Downloads/ricing
    chmod +x install.sh modules/*.sh
    ./install.sh

## What Gets Installed

The installer writes or updates:

    ~/.config/hypr/scripts/
    ~/.config/hypr/conf/
    ~/.config/hypr/hyprlock.conf
    ~/.config/waybar/
    ~/.config/wlogout/
    ~/.config/kitty/
    ~/.config/wofi/
    ~/.config/swaync/
    ~/.config/theme/

It also patches:

    ~/.config/hypr/hyprland.conf
    ~/.config/waybar/config.jsonc

## Backups

Backups are stored at:

    ~/.config/rice-backups/

## Waybar Dynamic Workspaces

Waybar is configured to show only active or used workspaces.

The installer removes persistent-workspaces from:

    ~/.config/waybar/config.jsonc

## Hyprland Modular Sources

The installer adds these source lines to hyprland.conf:

    source = ~/.config/hypr/conf/keybinds-devops.conf
    source = ~/.config/hypr/conf/windowrules-utility.conf
    source = ~/.config/hypr/conf/disable-laptop-keyboard.conf

The laptop keyboard source is added only if the file exists.

## Laptop Keyboard Disable

Temporary re-enable:

    hyprctl keyword 'device[at-translated-set-2-keyboard]:enabled' true

Permanent undo:

    rm ~/.config/hypr/conf/disable-laptop-keyboard.conf
    hyprctl reload

## Important Keybinds

| Keybind | Action |
|---|---|
| SUPER + SHIFT + W | Random wallpaper |
| SUPER + SHIFT + E | Open Thunar |
| SUPER + ALT + C | Open VS Code |
| SUPER + ALT + 1 | Open Cloudlaya folder |
| SUPER + ALT + 2 | Open Cloud folder |
| SUPER + ALT + 3 | Open HDD folder |
| SUPER + ALT + 4 | Open Downloads |
| SUPER + SHIFT + G | Open Lazygit |
| SUPER + SHIFT + K | Open K9s |
| SUPER + SHIFT + D | Docker status terminal |
| SUPER + SHIFT + T | Open btop |

## Screenshot Keybinds

| Keybind | Action |
|---|---|
| Print | Area screenshot to clipboard |
| SHIFT + Print | Full screenshot to clipboard |
| SUPER + Print | Area screenshot save and copy |
| SUPER + SHIFT + Print | Full screenshot save and copy |
| SUPER + ALT + Print | Open screenshots folder |

Screenshots are saved to:

    ~/Pictures/Screenshots

## Wallpaper

Wallpaper folder:

    ~/Pictures/Wallpapers

Wallpaper script:

    ~/.config/hypr/scripts/set-wallpaper.sh

## Waybar Logs

Check logs:

    tail -n 80 /tmp/waybar.log

## Recommended Packages

Pacman:

    sudo pacman -S hyprland waybar kitty wofi hyprlock wlogout grim slurp wl-clipboard swaync nautilus thunar btop jq curl brightnessctl bluez bluez-utils papirus-icon-theme gnome-themes-extra

AUR:

    yay -S catppuccin-gtk-theme-mocha lazygit k9s

## Git

    cd ~/Downloads/ricing
    git add .
    git commit -m "chore: modularize Hyprland ricing dotfiles"

## Notes

Before using on another machine, check:

- Monitor names
- Keyboard device names
- Folder paths
- Installed packages
