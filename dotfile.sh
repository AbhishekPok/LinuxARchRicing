#!/usr/bin/env bash

set -euo pipefail

# ============================================================
# Abhi DevOps Hyprland Rice Installer
# Arch Linux / Hyprland / Waybar / Kitty / Wlogout / Hyprlock
# ============================================================

BACKUP_DIR="$HOME/.config/rice-backups/$(date +%F-%H%M%S)"

backup_file() {
    local file="$1"

    if [ -f "$file" ]; then
        mkdir -p "$BACKUP_DIR$(dirname "$file")"
        cp "$file" "$BACKUP_DIR$file"
        echo "Backup: $file"
    fi
}

ensure_line() {
    local file="$1"
    local line="$2"

    touch "$file"

    if ! command grep -Fxq "$line" "$file"; then
        echo "$line" >> "$file"
    fi
}

section() {
    echo
    echo "============================================================"
    echo "$1"
    echo "============================================================"
}

section "Creating directories"

mkdir -p \
    "$HOME/.config/hypr/scripts" \
    "$HOME/.config/hypr/conf" \
    "$HOME/.config/waybar/scripts" \
    "$HOME/.config/wlogout/icons" \
    "$HOME/.config/kitty" \
    "$HOME/.config/theme" \
    "$HOME/Pictures/Screenshots"

# ============================================================
# Shared theme colors
# ============================================================

section "Writing shared theme colors"

cat <<'THEME_EOF' > "$HOME/.config/theme/colors.sh"
#!/usr/bin/env bash

# DevOps Dark Command Center Theme
export BG="#1e1e2e"
export BG_DARK="#11111b"
export BG_ALT="#313244"
export FG="#cdd6f4"
export MUTED="#a6adc8"

export BLUE="#89b4fa"
export CYAN="#74c7ec"
export GREEN="#a6e3a1"
export YELLOW="#f9e2af"
export ORANGE="#fab387"
export RED="#f38ba8"
export PURPLE="#cba6f7"

export TRANSPARENT_BG="rgba(17, 17, 27, 0.78)"
export BORDER="rgba(69, 71, 90, 0.65)"
THEME_EOF

chmod +x "$HOME/.config/theme/colors.sh"

# ============================================================
# Hyprland helper scripts
# ============================================================

section "Writing Hyprland scripts"

cat <<'WALL_EOF' > "$HOME/.config/hypr/scripts/set-wallpaper.sh"
#!/usr/bin/env bash

WALL_DIR="$HOME/Pictures/Wallpapers"
LOG_FILE="/tmp/awww.log"

pick_wallpaper() {
    command find "$WALL_DIR" -type f \( \
        -iname "*.jpg" -o \
        -iname "*.jpeg" -o \
        -iname "*.png" -o \
        -iname "*.webp" -o \
        -iname "*.gif" \
    \) 2>/dev/null | shuf -n 1
}

if ! pgrep -x awww-daemon >/dev/null 2>&1; then
    nohup awww-daemon > "$LOG_FILE" 2>&1 &
    sleep 0.8
fi

LAPTOP_WALL="$(pick_wallpaper)"
HDMI_WALL="$(pick_wallpaper)"

if [ -z "$LAPTOP_WALL" ]; then
    notify-send "Wallpaper" "No image found in $WALL_DIR" >/dev/null 2>&1
    exit 1
fi

outputs="$(awww query 2>/dev/null)"

if echo "$outputs" | command grep -q "eDP-1"; then
    awww img -o eDP-1 "$LAPTOP_WALL" >/dev/null 2>&1
fi

if echo "$outputs" | command grep -q "HDMI-A-1"; then
    [ -n "$HDMI_WALL" ] && awww img -o HDMI-A-1 "$HDMI_WALL" >/dev/null 2>&1
fi

notify-send "Wallpaper Updated" "$(basename "$LAPTOP_WALL")" >/dev/null 2>&1
WALL_EOF

cat <<'SHOT_EOF' > "$HOME/.config/hypr/scripts/screenshot.sh"
#!/usr/bin/env bash

SCREENSHOT_DIR="$HOME/Pictures/Screenshots"
mkdir -p "$SCREENSHOT_DIR"

timestamp="$(date +'%Y-%m-%d_%H-%M-%S')"
file="$SCREENSHOT_DIR/screenshot_$timestamp.png"
mode="$1"

area_geometry() {
    slurp 2>/dev/null
}

case "$mode" in
    area-copy)
        geo="$(area_geometry)" || exit 0
        [ -z "$geo" ] && exit 0
        grim -g "$geo" - | wl-copy
        notify-send "Screenshot" "Area copied to clipboard" >/dev/null 2>&1
        ;;

    full-copy)
        grim - | wl-copy
        notify-send "Screenshot" "Full screen copied to clipboard" >/dev/null 2>&1
        ;;

    area-save)
        geo="$(area_geometry)" || exit 0
        [ -z "$geo" ] && exit 0
        grim -g "$geo" "$file"
        wl-copy < "$file"
        notify-send "Screenshot Saved" "$(basename "$file")" >/dev/null 2>&1
        ;;

    full-save)
        grim "$file"
        wl-copy < "$file"
        notify-send "Screenshot Saved" "$(basename "$file")" >/dev/null 2>&1
        ;;

    open-folder)
        nautilus "$SCREENSHOT_DIR" >/dev/null 2>&1 &
        ;;

    *)
        notify-send "Screenshot" "Unknown mode: $mode" >/dev/null 2>&1
        exit 1
        ;;
esac
SHOT_EOF

chmod +x "$HOME/.config/hypr/scripts/set-wallpaper.sh"
chmod +x "$HOME/.config/hypr/scripts/screenshot.sh"

# ============================================================
# Waybar scripts
# ============================================================

section "Writing Waybar scripts"

cat <<'BT_STATUS_EOF' > "$HOME/.config/waybar/scripts/bluetooth-status.sh"
#!/usr/bin/env bash

if ! command -v bluetoothctl >/dev/null 2>&1; then
    echo "󰂲 NA"
    exit 0
fi

if rfkill list bluetooth 2>/dev/null | command grep -q "Soft blocked: yes"; then
    echo "󰂲 Blocked"
    exit 0
fi

if ! systemctl is-active --quiet bluetooth; then
    echo "󰂲 OFF"
    exit 0
fi

powered="$(bluetoothctl show 2>/dev/null | awk -F': ' '/Powered/ {print $2; exit}')"

if [ "$powered" != "yes" ]; then
    echo "󰂲 Off"
    exit 0
fi

connected="$(bluetoothctl devices Connected 2>/dev/null | wc -l)"

if [ "$connected" -gt 0 ]; then
    device="$(bluetoothctl devices Connected 2>/dev/null | sed 's/^Device [A-F0-9:]* //' | head -n 1)"
    short="$(echo "$device" | cut -c1-14)"
    echo "󰂱 $short"
else
    echo "󰂯 On"
fi
BT_STATUS_EOF

cat <<'BT_TOGGLE_EOF' > "$HOME/.config/waybar/scripts/bluetooth-toggle.sh"
#!/usr/bin/env bash

rfkill unblock bluetooth >/dev/null 2>&1

if ! systemctl is-active --quiet bluetooth; then
    systemctl start bluetooth >/dev/null 2>&1
fi

sleep 0.5

powered="$(bluetoothctl show 2>/dev/null | awk -F': ' '/Powered/ {print $2; exit}')"

if [ "$powered" = "yes" ]; then
    bluetoothctl power off >/dev/null 2>&1
    notify-send "Bluetooth" "Turned off" >/dev/null 2>&1
else
    bluetoothctl power on >/dev/null 2>&1
    notify-send "Bluetooth" "Turned on" >/dev/null 2>&1
fi
BT_TOGGLE_EOF

cat <<'PUBLIC_IP_EOF' > "$HOME/.config/waybar/scripts/public-ip.sh"
#!/usr/bin/env bash

CACHE_FILE="/tmp/waybar-public-ip"
CACHE_TTL=300

now="$(date +%s)"

if [ -f "$CACHE_FILE" ]; then
    file_time="$(stat -c %Y "$CACHE_FILE" 2>/dev/null || echo 0)"
    age=$((now - file_time))

    if [ "$age" -lt "$CACHE_TTL" ]; then
        cached="$(cat "$CACHE_FILE" 2>/dev/null)"
        [ -n "$cached" ] && echo "󰩟 $cached" && exit 0
    fi
fi

ip="$(curl -4 -s --max-time 3 ifconfig.me 2>/dev/null)"

if [ -z "$ip" ]; then
    ip="$(curl -s --max-time 3 ifconfig.me 2>/dev/null)"
fi

if [ -z "$ip" ]; then
    echo "󰩟 no ip"
else
    echo "$ip" > "$CACHE_FILE"
    echo "󰩟 $ip"
fi
PUBLIC_IP_EOF

cat <<'DOCKER_EOF' > "$HOME/.config/waybar/scripts/docker-status.sh"
#!/usr/bin/env bash

if ! command -v docker >/dev/null 2>&1; then
    echo "󰡨 NA"
    exit 0
fi

if systemctl is-active --quiet docker 2>/dev/null; then
    running="$(docker ps -q 2>/dev/null | wc -l)"
    total="$(docker ps -aq 2>/dev/null | wc -l)"

    if [ "$running" -gt 0 ]; then
        echo "󰡨 $running/$total"
    else
        echo "󰡨 0"
    fi
else
    echo "󰡨 OFF"
fi
DOCKER_EOF

cat <<'KUBE_EOF' > "$HOME/.config/waybar/scripts/kube-context.sh"
#!/usr/bin/env bash

if ! command -v kubectl >/dev/null 2>&1; then
    echo "󱃾 NA"
    exit 0
fi

context="$(kubectl config current-context 2>/dev/null)"

if [ -z "$context" ]; then
    echo "󱃾 none"
    exit 0
fi

case "$context" in
    *prod*|*production*) icon="󱃾 PROD" ;;
    *stage*|*staging*) icon="󱃾 STG" ;;
    *dev*|*development*) icon="󱃾 DEV" ;;
    *minikube*) icon="󱃾 mini" ;;
    *kind*) icon="󱃾 kind" ;;
    *) icon="󱃾" ;;
esac

short="$(echo "$context" | sed 's/^arn:aws:eks:[^:]*:[^:]*:cluster\///' | cut -c1-18)"
echo "$icon $short"
KUBE_EOF

cat <<'VPN_EOF' > "$HOME/.config/waybar/scripts/vpn-status.sh"
#!/usr/bin/env bash

if ip link show tun0 >/dev/null 2>&1; then
    echo " VPN"
elif ip link show wg0 >/dev/null 2>&1; then
    echo " WG"
elif ip link show tailscale0 >/dev/null 2>&1; then
    echo " TS"
elif ip link show ppp0 >/dev/null 2>&1; then
    echo " VPN"
else
    echo " No VPN"
fi
VPN_EOF

cat <<'NOTIFY_EOF' > "$HOME/.config/waybar/scripts/notifications-status.sh"
#!/usr/bin/env bash

if ! command -v swaync-client >/dev/null 2>&1; then
    echo "󰂚 NA"
    exit 0
fi

count="$(swaync-client -c 2>/dev/null)"
dnd="$(swaync-client -D 2>/dev/null)"

if [ "$dnd" = "true" ]; then
    echo "󰂛 DND"
elif [ "$count" -gt 0 ] 2>/dev/null; then
    echo "󰂚 $count"
else
    echo "󰂚"
fi
NOTIFY_EOF

cat <<'UPDATES_EOF' > "$HOME/.config/waybar/scripts/arch-updates.sh"
#!/usr/bin/env bash

updates=0
aur_updates=0

if command -v checkupdates >/dev/null 2>&1; then
    updates="$(checkupdates 2>/dev/null | wc -l)"
fi

if command -v yay >/dev/null 2>&1; then
    aur_updates="$(yay -Qua 2>/dev/null | wc -l)"
elif command -v paru >/dev/null 2>&1; then
    aur_updates="$(paru -Qua 2>/dev/null | wc -l)"
fi

total=$((updates + aur_updates))
echo " $total"
UPDATES_EOF

chmod +x "$HOME/.config/waybar/scripts/"*.sh


# ============================================================
# Waybar dynamic workspaces
# ============================================================

section "Configuring Waybar dynamic workspaces"

WAYBAR_CONFIG="$HOME/.config/waybar/config.jsonc"

if [ -f "$WAYBAR_CONFIG" ]; then
    backup_file "$WAYBAR_CONFIG"

    python - <<'WAYBAR_WS_PY'
from pathlib import Path
import re

path = Path.home() / ".config/waybar/config.jsonc"
text = path.read_text()

# Remove persistent workspace block so Waybar only shows active/used workspaces.
text = re.sub(
    r',?\n\s*"persistent-workspaces"\s*:\s*\{\s*"\*"\s*:\s*\d+\s*\}',
    '',
    text
)

path.write_text(text)
WAYBAR_WS_PY
fi


# ============================================================
# Hyprland modular config
# ============================================================

section "Writing Hyprland modular config"

cat <<'KEYBINDS_EOF' > "$HOME/.config/hypr/conf/keybinds-devops.conf"
# ============================================================
# DevOps Workspace Shortcuts
# ============================================================

# Reload wallpaper
bind = $mainMod SHIFT, W, exec, ~/.config/hypr/scripts/set-wallpaper.sh

# File managers
bind = $mainMod SHIFT, E, exec, thunar

# Code editor
bind = $mainMod ALT, C, exec, code

# Open important work folders
bind = $mainMod ALT, 1, exec, nautilus ~/Documents/Cloudlaya
bind = $mainMod ALT, 2, exec, nautilus ~/Documents/Cloud
bind = $mainMod ALT, 3, exec, nautilus ~/HDD/abhi
bind = $mainMod ALT, 4, exec, nautilus ~/Downloads

# DevOps terminal tools
bind = $mainMod SHIFT, G, exec, kitty -e lazygit
bind = $mainMod SHIFT, K, exec, kitty -e k9s
bind = $mainMod SHIFT, D, exec, kitty -e bash -lc 'docker ps; echo; read -p "Press Enter to close..."'

# System monitor
bind = $mainMod SHIFT, T, exec, kitty -e btop
KEYBINDS_EOF

cat <<'RULES_EOF' > "$HOME/.config/hypr/conf/windowrules-utility.conf"
# ============================================================
# Utility Floating Window Rules
# ============================================================

windowrule {
    name = float-blueman
    match:class = blueman-manager
    float = yes
    center = yes
    size = 900 600
}

windowrule {
    name = float-nwg-look
    match:class = nwg-look
    float = yes
    center = yes
    size = 900 650
}

windowrule {
    name = float-pavucontrol
    match:class = pavucontrol
    float = yes
    center = yes
    size = 850 600
}

windowrule {
    name = float-file-dialogs
    match:title = ^(Open File|Save File|Select a File|Choose File|Open Folder).*
    float = yes
    center = yes
}

windowrule {
    name = float-pip
    match:title = ^Picture-in-Picture$
    float = yes
    pin = yes
    move = 70% 60%
    size = 480 270
}
RULES_EOF

if [ -f "$HOME/.config/hypr/hyprland.conf" ]; then
    backup_file "$HOME/.config/hypr/hyprland.conf"

    ensure_line "$HOME/.config/hypr/hyprland.conf" ""
    ensure_line "$HOME/.config/hypr/hyprland.conf" "# ============================================================"
    ensure_line "$HOME/.config/hypr/hyprland.conf" "# Modular user config"
    ensure_line "$HOME/.config/hypr/hyprland.conf" "# ============================================================"
    ensure_line "$HOME/.config/hypr/hyprland.conf" "source = ~/.config/hypr/conf/keybinds-devops.conf"
    ensure_line "$HOME/.config/hypr/hyprland.conf" "source = ~/.config/hypr/conf/windowrules-utility.conf"

    # Quiet Waybar restart bind if present.
    perl -0pi -e 's|bind = \$mainMod, Z, exec, .*waybar.*|bind = \$mainMod, Z, exec, pkill waybar; nohup waybar >/dev/null 2>/tmp/waybar.log \&|' "$HOME/.config/hypr/hyprland.conf"

    # Remove unsupported rule if copied from old snippets.
    perl -0pi -e 's/^\s*keepaspectratio = yes\n//mg' "$HOME/.config/hypr/hyprland.conf"
fi

# ============================================================
# Hyprlock
# ============================================================

section "Writing Hyprlock config"

backup_file "$HOME/.config/hypr/hyprlock.conf"

cat <<'HYPRLOCK_EOF' > "$HOME/.config/hypr/hyprlock.conf"
# ============================================================
# Hyprlock DevOps Dark Command Center
# ============================================================

general {
    hide_cursor = false
    ignore_empty_input = true
    immediate_render = true
    fail_timeout = 1500
}

background {
    monitor =
    path = /home/abhi/Pictures/Wallpapers/wallpaper_caven.jpg
    color = rgba(17, 17, 27, 1.0)
    blur_passes = 3
    blur_size = 8
    brightness = 0.42
    contrast = 1.05
    vibrancy = 0.18
    vibrancy_darkness = 0.25
}

label {
    monitor =
    text = $TIME
    color = rgba(205, 214, 244, 0.98)
    font_size = 86
    font_family = JetBrainsMono Nerd Font
    position = 0, 150
    halign = center
    valign = center
}

label {
    monitor =
    text = cmd[update:1000] date +"%A, %d %B %Y"
    color = rgba(166, 173, 200, 0.92)
    font_size = 20
    font_family = JetBrainsMono Nerd Font
    position = 0, 78
    halign = center
    valign = center
}

label {
    monitor =
    text = Welcome back, $USER
    color = rgba(137, 180, 250, 0.95)
    font_size = 16
    font_family = JetBrainsMono Nerd Font
    position = 0, 34
    halign = center
    valign = center
}

input-field {
    monitor =
    size = 420, 58
    outline_thickness = 2
    dots_size = 0.25
    dots_spacing = 0.25
    dots_center = true
    outer_color = rgba(137, 180, 250, 0.78)
    inner_color = rgba(30, 30, 46, 0.78)
    font_color = rgba(205, 214, 244, 0.95)
    check_color = rgba(166, 227, 161, 0.95)
    fail_color = rgba(243, 139, 168, 0.95)
    placeholder_text = <span foreground="##a6adc8">Enter Password</span>
    fail_text = <span foreground="##f38ba8">Authentication Failed</span>
    font_family = JetBrainsMono Nerd Font
    fade_on_empty = false
    rounding = 18
    position = 0, -55
    halign = center
    valign = center
}

label {
    monitor =
    text =   Ruby  •  Hyprland
    color = rgba(249, 226, 175, 0.88)
    font_size = 13
    font_family = JetBrainsMono Nerd Font
    position = 0, -28
    halign = center
    valign = top
}

shape {
    monitor =
    size = 760, 38
    color = rgba(17, 17, 27, 0.58)
    rounding = 14
    border_size = 1
    border_color = rgba(137, 180, 250, 0.35)
    position = 0, 24
    halign = center
    valign = bottom
}

label {
    monitor =
    text = cmd[update:5000] bash -lc 'BAT=$(cat /sys/class/power_supply/BAT*/capacity 2>/dev/null | head -n1); BAT=${BAT:-AC}; WIFI=$(iwgetid -r 2>/dev/null); WIFI=${WIFI:-Offline}; VOL=$(wpctl get-volume @DEFAULT_AUDIO_SINK@ 2>/dev/null | awk "{print int(\$2*100)}"); VOL=${VOL:-0}; LAYOUT=$(hyprctl devices -j 2>/dev/null | jq -r ".keyboards[0].active_keymap" 2>/dev/null); LAYOUT=${LAYOUT:-Default}; echo "󰁹 ${BAT}%    ${WIFI}   󰕾 ${VOL}%    ${LAYOUT}"'
    color = rgba(205, 214, 244, 0.86)
    font_size = 12
    font_family = JetBrainsMono Nerd Font
    position = 0, 35
    halign = center
    valign = bottom
}

label {
    monitor =
    text = Press ESC to cancel
    color = rgba(108, 112, 134, 0.75)
    font_size = 11
    font_family = JetBrainsMono Nerd Font
    position = 0, 8
    halign = center
    valign = bottom
}
HYPRLOCK_EOF

# ============================================================
# Wlogout
# ============================================================

section "Writing Wlogout config"

backup_file "$HOME/.config/wlogout/layout"
backup_file "$HOME/.config/wlogout/style.css"

cat <<'WLOGOUT_LAYOUT_EOF' > "$HOME/.config/wlogout/layout"
{
    "label": "lock",
    "action": "hyprlock",
    "text": "Lock",
    "keybind": "l"
}
{
    "label": "logout",
    "action": "hyprctl dispatch exit",
    "text": "Logout",
    "keybind": "e"
}
{
    "label": "suspend",
    "action": "systemctl suspend",
    "text": "Suspend",
    "keybind": "s"
}
{
    "label": "hibernate",
    "action": "systemctl hibernate",
    "text": "Hibernate",
    "keybind": "h"
}
{
    "label": "reboot",
    "action": "systemctl reboot",
    "text": "Reboot",
    "keybind": "r"
}
{
    "label": "shutdown",
    "action": "systemctl poweroff",
    "text": "Shutdown",
    "keybind": "p"
}
WLOGOUT_LAYOUT_EOF

cat <<'SVG_EOF' > "$HOME/.config/wlogout/icons/lock.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#89b4fa" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="10" width="16" height="10" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>
SVG_EOF

cat <<'SVG_EOF' > "$HOME/.config/wlogout/icons/logout.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#cba6f7" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 17l5-5-5-5"/><path d="M15 12H3"/><path d="M21 19V5a2 2 0 0 0-2-2h-6"/></svg>
SVG_EOF

cat <<'SVG_EOF' > "$HOME/.config/wlogout/icons/suspend.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#f9e2af" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 4v16"/><path d="M14 4v16"/><circle cx="12" cy="12" r="9"/></svg>
SVG_EOF

cat <<'SVG_EOF' > "$HOME/.config/wlogout/icons/hibernate.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#fab387" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.8A8.5 8.5 0 1 1 11.2 3 6.8 6.8 0 0 0 21 12.8z"/></svg>
SVG_EOF

cat <<'SVG_EOF' > "$HOME/.config/wlogout/icons/reboot.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#74c7ec" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-3-6.7"/><path d="M21 3v6h-6"/></svg>
SVG_EOF

cat <<'SVG_EOF' > "$HOME/.config/wlogout/icons/shutdown.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#f38ba8" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.8 0"/></svg>
SVG_EOF

cat <<'WLOGOUT_STYLE_EOF' > "$HOME/.config/wlogout/style.css"
* {
    font-family: "JetBrainsMono Nerd Font", "JetBrains Mono", sans-serif;
    background-image: none;
    box-shadow: none;
}

window {
    background-color: rgba(17, 17, 27, 0.52);
}

button {
    color: #cdd6f4;
    background-color: rgba(30, 30, 46, 0.96);
    border: 1px solid rgba(69, 71, 90, 0.9);
    border-radius: 18px;
    margin: 8px;
    padding: 78px 18px 18px 18px;
    min-width: 145px;
    min-height: 105px;
    font-size: 13px;
    font-weight: 800;
    background-repeat: no-repeat;
    background-position: center 24px;
    background-size: 48px;
    transition: all 150ms ease;
}

button:hover,
button:focus {
    background-color: rgba(49, 50, 68, 0.98);
    border: 1px solid #89b4fa;
    color: #ffffff;
}

#lock {
    color: #89b4fa;
    background-image: url("/home/abhi/.config/wlogout/icons/lock.svg");
}

#logout {
    color: #cba6f7;
    background-image: url("/home/abhi/.config/wlogout/icons/logout.svg");
}

#suspend {
    color: #f9e2af;
    background-image: url("/home/abhi/.config/wlogout/icons/suspend.svg");
}

#hibernate {
    color: #fab387;
    background-image: url("/home/abhi/.config/wlogout/icons/hibernate.svg");
}

#reboot {
    color: #74c7ec;
    background-image: url("/home/abhi/.config/wlogout/icons/reboot.svg");
}

#shutdown {
    color: #f38ba8;
    background-image: url("/home/abhi/.config/wlogout/icons/shutdown.svg");
}
WLOGOUT_STYLE_EOF

# ============================================================
# Kitty
# ============================================================

section "Writing Kitty config"

backup_file "$HOME/.config/kitty/kitty.conf"

cat <<'KITTY_EOF' > "$HOME/.config/kitty/kitty.conf"
# ============================================================
# Kitty DevOps Dark Command Center - Vibrant
# ============================================================

font_family      JetBrains Mono Nerd Font
bold_font        auto
italic_font      auto
bold_italic_font auto
font_size        12.0

disable_ligatures never

background_opacity 0.92
dynamic_background_opacity yes

window_padding_width 6
single_window_margin_width 0
window_margin_width 0

confirm_os_window_close 0
remember_window_size yes
initial_window_width  1100
initial_window_height 680

wayland_titlebar_color background
hide_window_decorations yes

cursor_shape beam
cursor_blink_interval 0.5
cursor_stop_blinking_after 15.0
cursor #00ff99
cursor_text_color #0b1020

scrollback_lines 20000
wheel_scroll_multiplier 3.0
touch_scroll_multiplier 3.0

mouse_hide_wait 3.0
url_color #33ccff
url_style curly
open_url_with default
copy_on_select yes
strip_trailing_spaces smart

enable_audio_bell no
visual_bell_duration 0.0
window_alert_on_bell no
bell_on_tab no

tab_bar_edge bottom
tab_bar_style powerline
tab_powerline_style slanted
tab_bar_min_tabs 2
tab_title_template " {index}: {title} "
active_tab_foreground #0b1020
active_tab_background #00ff99
inactive_tab_foreground #e6f1ff
inactive_tab_background #1b2433

enabled_layouts splits,stack,tall,fat,grid

map ctrl+shift+enter new_window
map ctrl+shift+w close_window
map ctrl+shift+h neighboring_window left
map ctrl+shift+l neighboring_window right
map ctrl+shift+k neighboring_window up
map ctrl+shift+j neighboring_window down
map ctrl+shift+left resize_window narrower 5
map ctrl+shift+right resize_window wider 5
map ctrl+shift+up resize_window taller 5
map ctrl+shift+down resize_window shorter 5
map ctrl+shift+space next_layout

map ctrl+shift+t new_tab
map ctrl+shift+q close_tab
map ctrl+shift+right next_tab
map ctrl+shift+left previous_tab

map ctrl+shift+c copy_to_clipboard
map ctrl+shift+v paste_from_clipboard
map ctrl+shift+plus change_font_size all +1.0
map ctrl+shift+minus change_font_size all -1.0
map ctrl+shift+backspace change_font_size all 0

foreground #e6f1ff
background #0b1020
selection_foreground #0b1020
selection_background #33ccff

color0 #1b2433
color8 #4b5b73
color1 #ff4d6d
color9 #ff6b8a
color2 #00ff99
color10 #5cffc3
color3 #ffd166
color11 #ffe08a
color4 #33ccff
color12 #66d9ff
color5 #c77dff
color13 #d9a3ff
color6 #00e5ff
color14 #7df9ff
color7 #d8e2f3
color15 #ffffff
KITTY_EOF

# ============================================================
# GTK settings
# ============================================================

section "Applying GTK settings"

gsettings set org.gnome.desktop.interface color-scheme 'prefer-dark' || true
gsettings set org.gnome.desktop.interface gtk-theme 'catppuccin-mocha-blue-standard+default' || true
gsettings set org.gnome.desktop.interface icon-theme 'Papirus-Dark' || true
gsettings set org.gnome.desktop.interface cursor-theme 'Bibata-Modern-Classic' || true
gsettings set org.gnome.desktop.interface font-name 'Adwaita Sans 11' || true
gsettings set org.gnome.desktop.interface document-font-name 'Adwaita Sans 11' || true
gsettings set org.gnome.desktop.interface monospace-font-name 'JetBrainsMono Nerd Font 11' || true

xdg-mime default org.gnome.Nautilus.desktop inode/directory || true
xdg-mime default org.gnome.Nautilus.desktop application/x-gnome-saved-search || true

# ============================================================
# Final checks
# ============================================================

section "Final checks"

if command -v hyprctl >/dev/null 2>&1; then
    hyprctl reload || true
fi

if command -v waybar >/dev/null 2>&1; then
    pkill waybar >/dev/null 2>&1 || true
    nohup waybar >/dev/null 2>/tmp/waybar.log & disown
fi

echo
echo "Done."
echo "Backups saved in: $BACKUP_DIR"
echo
echo "Recommended checks:"
echo "  hyprctl reload"
echo "  tail -n 80 /tmp/waybar.log"
echo "  ~/.config/hypr/scripts/screenshot.sh full-save"
echo "  ~/.config/waybar/scripts/bluetooth-status.sh"
