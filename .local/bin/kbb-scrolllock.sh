#!/usr/bin/bash
# ╔══════════════════════════════════════════════════════════════════╗
# ║  kbb-scrolllock.sh — RGB Keyboard Scroll Lock Controller        ║
# ║  USB Keyboard: vendor 2A7A, product 8A47                        ║
# ║  Works with xkb modifier_map Mod3 { Scroll_Lock } fix           ║
# ║  Uses ydotool to simulate Scroll_Lock keypress (Wayland-safe)    ║
# ╚══════════════════════════════════════════════════════════════════╝
#
# Usage:
#   kbb-scrolllock.sh on       — turn RGB on
#   kbb-scrolllock.sh off      — turn RGB off
#   kbb-scrolllock.sh toggle   — flip current state
#   kbb-scrolllock.sh status   — show current LED state
#   kbb-scrolllock.sh setup    — install xkb fix + pacman hook + ydotool
#
# Prerequisites:
#   1. xkb modifier_map Mod3 fix applied to /usr/share/X11/xkb/symbols/pc
#   2. ydotool installed and ydotool user service running
#   3. User in 'input' group
#
# Install location: ~/.local/bin/kbb-scrolllock.sh

set -euo pipefail

VENDOR="2a7a"
PRODUCT="8a47"
LED_NAME="scrolllock"
# Scroll_Lock scan code for ydotool
SCLK_CODE=70

# ── Helpers ──────────────────────────────────────────────────────────

find_led() {
    for led in /sys/class/leds/input*::${LED_NAME}; do
        [ -f "$led/device/id/vendor" ] || continue
        local v p
        v=$(cat "$led/device/id/vendor" 2>/dev/null)
        p=$(cat "$led/device/id/product" 2>/dev/null)
        if [ "$v" = "$VENDOR" ] && [ "$p" = "$PRODUCT" ]; then
            echo "$led"
            return 0
        fi
    done
    return 1
}

get_brightness() {
    local led
    led=$(find_led) || { echo "0"; return; }
    cat "$led/brightness"
}

send_scrolllock() {
    # Try user-level ydotool first, fall back to sudo
    if ydotool key "${SCLK_CODE}:1" "${SCLK_CODE}:0" 2>/dev/null; then
        return 0
    elif sudo ydotool key "${SCLK_CODE}:1" "${SCLK_CODE}:0" 2>/dev/null; then
        return 0
    else
        echo "ERROR: ydotool failed. Is ydotoold running?" >&2
        echo "  Try: systemctl --user start ydotool" >&2
        return 1
    fi
}

# ── Commands ─────────────────────────────────────────────────────────

cmd_on() {
    local current
    current=$(get_brightness)
    if [ "$current" = "0" ]; then
        send_scrolllock
        sleep 0.3
        local after
        after=$(get_brightness)
        if [ "$after" = "1" ]; then
            echo "RGB ON"
        else
            echo "Scroll Lock sent but LED did not toggle (keyboard may not be connected)"
        fi
    else
        echo "RGB already ON"
    fi
}

cmd_off() {
    local current
    current=$(get_brightness)
    if [ "$current" = "1" ]; then
        send_scrolllock
        sleep 0.3
        echo "RGB OFF"
    else
        echo "RGB already OFF"
    fi
}

cmd_toggle() {
    local current
    current=$(get_brightness)
    send_scrolllock
    sleep 0.3
    if [ "$current" = "0" ]; then
        echo "RGB ON"
    else
        echo "RGB OFF"
    fi
}

cmd_status() {
    local led
    if led=$(find_led); then
        local br
        br=$(cat "$led/brightness")
        echo "Keyboard: ${VENDOR}:${PRODUCT}"
        echo "LED path: $led"
        echo "Brightness: $br"
        if [ "$br" = "1" ]; then
            echo "Status: RGB ON"
        else
            echo "Status: RGB OFF"
        fi
    else
        echo "Keyboard ${VENDOR}:${PRODUCT} not found"
        echo "Status: DISCONNECTED"
    fi

    # Check ydotool status
    echo ""
    if systemctl --user is-active ydotool &>/dev/null; then
        echo "ydotool: running (user service)"
    elif pgrep -x ydotoold &>/dev/null; then
        echo "ydotool: running (manual)"
    else
        echo "ydotool: NOT RUNNING"
    fi

    # Check xkb fix
    echo ""
    if grep -q 'modifier_map Mod3 { Scroll_Lock }' /usr/share/X11/xkb/symbols/pc 2>/dev/null; then
        echo "xkb fix: APPLIED"
    else
        echo "xkb fix: NOT APPLIED"
    fi
}

cmd_setup() {
    echo "═══ RGB Keyboard Setup (${VENDOR}:${PRODUCT}) ═══"
    echo ""

    # Step 1: xkb modifier_map fix
    echo "── Step 1: xkb modifier_map Mod3 fix ──"
    if grep -q 'modifier_map Mod3 { Scroll_Lock }' /usr/share/X11/xkb/symbols/pc 2>/dev/null; then
        echo "  Already applied ✓"
    else
        echo "  Applying..."
        sudo sed -i '/key <SCLK>/a\    modifier_map Mod3 { Scroll_Lock };' /usr/share/X11/xkb/symbols/pc
        sudo rm -f /var/lib/xkb/server-*.xkm
        echo "  Applied ✓ (session restart required)"
    fi
    echo ""

    # Step 2: Pacman hook to survive xkeyboard-config updates
    echo "── Step 2: Pacman hook ──"
    local hook="/etc/pacman.d/hooks/xkb-scrolllock.hook"
    if [ -f "$hook" ]; then
        echo "  Already exists ✓"
    else
        echo "  Creating $hook..."
        sudo mkdir -p /etc/pacman.d/hooks
        sudo tee "$hook" > /dev/null << 'HOOKEOF'
[Trigger]
Operation = Install
Operation = Upgrade
Type = Package
Target = xkeyboard-config

[Action]
Description = Re-applying Scroll Lock modifier_map for RGB keyboard
When = PostTransaction
Exec = /bin/bash -c "grep -q 'modifier_map Mod3 { Scroll_Lock }' /usr/share/X11/xkb/symbols/pc || sed -i '/key <SCLK>/a\\    modifier_map Mod3 { Scroll_Lock };' /usr/share/X11/xkb/symbols/pc && rm -f /var/lib/xkb/server-*.xkm"
HOOKEOF
        echo "  Created ✓"
    fi
    echo ""

    # Step 3: ydotool
    echo "── Step 3: ydotool ──"
    if command -v ydotool &>/dev/null; then
        echo "  ydotool installed ✓"
    else
        echo "  Installing ydotool..."
        sudo pacman -S --noconfirm ydotool
        echo "  Installed ✓"
    fi
    echo ""

    # Step 4: input group
    echo "── Step 4: input group ──"
    if id -nG | grep -qw input; then
        echo "  User in input group ✓"
    else
        echo "  Adding user to input group..."
        sudo usermod -aG input "$USER"
        echo "  Added ✓ (logout required to take effect)"
    fi
    echo ""

    # Step 5: udev rule for /dev/uinput
    # Note: ydotool 1.0.4-2+ ships its own at /usr/lib/udev/rules.d/80-uinput.rules
    echo "── Step 5: uinput permissions ──"
    if [ -f /usr/lib/udev/rules.d/80-uinput.rules ]; then
        echo "  System udev rule exists (from ydotool package) ✓"
        # Remove our custom one if it conflicts
        if [ -f /etc/udev/rules.d/80-uinput.rules ]; then
            echo "  Removing conflicting custom rule at /etc/udev/rules.d/80-uinput.rules"
            sudo rm -f /etc/udev/rules.d/80-uinput.rules
        fi
    else
        if [ -f /etc/udev/rules.d/80-uinput.rules ]; then
            echo "  Custom udev rule exists ✓"
        else
            echo "  Creating udev rule..."
            echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput", TAG+="uaccess"' \
                | sudo tee /etc/udev/rules.d/80-uinput.rules > /dev/null
            sudo udevadm control --reload-rules
            sudo udevadm trigger
            echo "  Created ✓"
        fi
    fi
    echo ""

    # Step 6: Enable ydotool user service
    echo "── Step 6: ydotool user service ──"
    if systemctl --user is-enabled ydotool &>/dev/null; then
        echo "  Already enabled ✓"
    else
        echo "  Enabling..."
        systemctl --user enable ydotool
        echo "  Enabled ✓"
    fi
    if systemctl --user is-active ydotool &>/dev/null; then
        echo "  Currently running ✓"
    else
        echo "  Starting..."
        systemctl --user start ydotool 2>/dev/null || echo "  ⚠ Failed to start (may need logout/login for group change)"
    fi
    echo ""

    echo "═══ Setup Complete ═══"
    echo ""
    echo "Next steps:"
    echo "  1. Logout and login (for input group to take effect)"
    echo "  2. After login, run: kbb-scrolllock.sh status"
    echo "  3. Test: kbb-scrolllock.sh on"
    echo ""
    echo "Add to hyprland.lua autostart:"
    echo '  hl.exec_cmd("sleep 3 && ~/.local/bin/kbb-scrolllock.sh on")'
    echo ""
    echo "Add toggle keybind:"
    echo '  hl.bind("Scroll_Lock", hl.dsp.exec_cmd("~/.local/bin/kbb-scrolllock.sh toggle"))'
}

# ── Main ─────────────────────────────────────────────────────────────

case "${1:-toggle}" in
    on)     cmd_on     ;;
    off)    cmd_off    ;;
    toggle) cmd_toggle ;;
    status) cmd_status ;;
    setup)  cmd_setup  ;;
    *)
        echo "Usage: $(basename "$0") {on|off|toggle|status|setup}"
        exit 1
        ;;
esac
