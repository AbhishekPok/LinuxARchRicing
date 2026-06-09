#!/usr/bin/env bash

gsettings set org.gnome.desktop.interface color-scheme 'prefer-dark' || true
gsettings set org.gnome.desktop.interface gtk-theme 'catppuccin-mocha-blue-standard+default' || true
gsettings set org.gnome.desktop.interface icon-theme 'Papirus-Dark' || true
gsettings set org.gnome.desktop.interface cursor-theme 'Bibata-Modern-Classic' || true
gsettings set org.gnome.desktop.interface font-name 'Adwaita Sans 11' || true
gsettings set org.gnome.desktop.interface document-font-name 'Adwaita Sans 11' || true
gsettings set org.gnome.desktop.interface monospace-font-name 'JetBrainsMono Nerd Font 11' || true

xdg-mime default org.gnome.Nautilus.desktop inode/directory || true
xdg-mime default org.gnome.Nautilus.desktop application/x-gnome-saved-search || true
