#!/usr/bin/env bash
# Monitor management for the amane launcher ("display ", SUPER+P), applied
# with hyprctl eval (Lua config).
#   --menu [res|scale|rot]   launcher rows: label<TAB>value
#   --menu-pick VALUE        layout:<name> | dual:<name> | sub:<menu> |
#                            res:<mode> | scale:<n> | rot:<transform>
#   --apply                  re-apply the saved layout (monitor-restore.sh)
# sub:<menu> prints OPEN<TAB>display-<menu>, which makes the launcher reopen
# on that submenu: Resolution / Scale / Rotation are their own keyword modes.
#
# One external: .monitor-state (ext_left | laptop_left | mirror | laptop | external).
# Two or more: .monitor-state-dual (hdmi_left | hdmi_right | external) --
# HDMI goes on one side of the laptop, USB-C/DP on the other.
#
# Per-monitor memory: .monitor-profiles keeps each physical monitor's
# resolution, scale and rotation, keyed by its identity (make, model and serial
# from the EDID, i.e. hyprctl's "description"), not by the port it is on. A
# pick from the Resolution / Scale / Rotation menus is saved there and every
# --apply (login, hotplug, layout change) restores it. Lines are
# "description<TAB>mode<TAB>scale<TAB>transform".

LAPTOP="eDP-1"
LAPTOP_RES="1920x1200@60"
LAPTOP_SCALE="1.2"
STATE_FILE="$HOME/.config/hypr/.monitor-state"
DUAL_FILE="$HOME/.config/hypr/.monitor-state-dual"
PROFILE_FILE="$HOME/.config/hypr/.monitor-profiles"

hm() {
    hyprctl eval "hl.monitor({output='$1', mode='$2', position='$3', scale='$4' $5})"
}

transform_label() {
    case "$1" in
        0) echo "Normal" ;; 1) echo "90°" ;; 2) echo "180°" ;; 3) echo "270°" ;;
        4) echo "Flipped" ;; 5) echo "Flipped 90°" ;; 6) echo "Flipped 180°" ;; 7) echo "Flipped 270°" ;;
        *) echo "Normal" ;;
    esac
}

port_label() {
    case "$1" in HDMI-*) echo "HDMI" ;; DP-*) echo "USB-C" ;; *) echo "$1" ;; esac
}

# "mode scale transform" remembered for a monitor description (defaults:
# its preferred mode, scale 1, no rotation)
profile_get() {
    local found
    found=$(awk -F'\t' -v d="$1" '$1 == d { print $2, $3, $4; exit }' "$PROFILE_FILE" 2>/dev/null)
    echo "${found:-preferred 1.0 0}"
}

# profile_set DESCRIPTION FIELD VALUE   (FIELD: mode | scale | transform)
profile_set() {
    local mode scale transform
    read -r mode scale transform <<<"$(profile_get "$1")"
    case "$2" in
        mode) mode=$3 ;;
        scale) scale=$3 ;;
        transform) transform=$3 ;;
    esac
    local tmp="$PROFILE_FILE.tmp"
    { awk -F'\t' -v d="$1" '$1 != d' "$PROFILE_FILE" 2>/dev/null
      printf '%s\t%s\t%s\t%s\n' "$1" "$mode" "$scale" "$transform"; } > "$tmp"
    mv "$tmp" "$PROFILE_FILE"
}

desc_of() { jq -r --arg e "$1" '.[] | select(.name == $e) | .description' <<<"$MONJSON"; }

# An external at POSITION with its remembered mode / scale / rotation;
# EXTRA is appended to the Lua table (", mirror='eDP-1'").
apply_ext() {
    local mode scale transform
    read -r mode scale transform <<<"$(profile_get "$(desc_of "$1")")"
    hm "$1" "$mode" "$2" "$scale" ", transform=$transform${3:-}"
}

save_state() { echo "$1" > "$STATE_FILE"; }
save_dual() { echo "$1" > "$DUAL_FILE"; }
layout() { local s; s=$(cat "$STATE_FILE" 2>/dev/null); echo "${s:-laptop_left}"; }
dual_layout() { local s; s=$(cat "$DUAL_FILE" 2>/dev/null); echo "${s:-hdmi_left}"; }

read_monitors() {
    MONJSON=$(hyprctl monitors all -j)
    mapfile -t EXTS < <(jq -r --arg l "$LAPTOP" '.[] | select(.name != $l) | .name' <<<"$MONJSON")
    [ ${#EXTS[@]} -eq 0 ] && return 1
    DUAL=0; [ ${#EXTS[@]} -ge 2 ] && DUAL=1

    # Resolution/scale/rotation act on the focused external, else the first.
    EXT=$(jq -r --arg l "$LAPTOP" '.[] | select(.focused and .name != $l) | .name' <<<"$MONJSON")
    [ -z "$EXT" ] && EXT=${EXTS[0]}

    local m
    m=$(jq -c --arg e "$EXT" '.[] | select(.name == $e)' <<<"$MONJSON")
    EXT_MODES=$(jq -r '.availableModes | join(" ")' <<<"$m")
    EXT_CURRENT_RES=$(jq -r '"\(.width)x\(.height)@\(.refreshRate)"' <<<"$m")
    EXT_CURRENT_SCALE=$(jq -r '.scale' <<<"$m")
    EXT_CURRENT_TRANSFORM=$(jq -r '.transform' <<<"$m")
    EXT_DESC=$(jq -r '.model // empty' <<<"$m")
    EXT_DESC="${EXT_DESC:-$EXT} ($(port_label "$EXT"))"
    return 0
}

# Position an external takes in the current layout.
ext_pos() {
    if [ "$DUAL" = 1 ]; then
        local hdmi=left other=right
        [ "$(dual_layout)" = hdmi_right ] && { hdmi=right; other=left; }
        case "$1" in HDMI-*) echo "auto-$hdmi" ;; *) echo "auto-$other" ;; esac
        return
    fi
    case "$(layout)" in
        ext_left)    echo "auto-left" ;;
        laptop_left) echo "auto-right" ;;
        mirror)      echo "0x0" ;;
        *)           echo "auto" ;;
    esac
}

apply_dual() {
    if [ "$(dual_layout)" = external ]; then
        hm "$LAPTOP" "disabled" "0x0" "1.0"
    else
        hm "$LAPTOP" "$LAPTOP_RES" "0x0" "$LAPTOP_SCALE"
    fi
    # First pass: every external gets its remembered mode / scale / rotation
    # (positions are fixed in the second pass, once the real sizes are known).
    local e
    for e in "${EXTS[@]}"; do apply_ext "$e" "$(ext_pos "$e")"; done
    place_in_port_order
}

# Two or more externals (a dock can add several): lay them out at explicit
# positions, each side in port order (DP-1 before DP-2), laptop at 0x0.
# Hyprland's auto-left / auto-right place relative to wherever the other
# screens currently are, so the order used to depend on the previous layout.
place_in_port_order() {
    local mons e side w x
    mons=$(hyprctl monitors -j)
    # logical width: pixels / scale, with width and height swapped when rotated
    lw() { jq -r --arg e "$1" '.[] | select(.name == $e)
        | (if (.transform % 2) == 1 then .height else .width end) / .scale | floor' <<<"$mons"; }
    local -a sorted left=() right=()
    mapfile -t sorted < <(printf '%s\n' "${EXTS[@]}" | sort -V)
    for e in "${sorted[@]}"; do
        side=$(ext_pos "$e")
        [ "$side" = auto-left ] && left+=("$e") || right+=("$e")
    done
    if [ "$(dual_layout)" = external ]; then
        # laptop off: everything in one row from 0
        x=0
        for e in "${left[@]}" "${right[@]}"; do
            apply_ext "$e" "${x}x0"; w=$(lw "$e"); x=$((x + ${w:-0}))
        done
        return
    fi
    # right of the laptop, in order
    x=$(lw "$LAPTOP"); x=${x:-0}
    for e in "${right[@]}"; do
        apply_ext "$e" "${x}x0"; w=$(lw "$e"); x=$((x + ${w:-0}))
    done
    # left of the laptop: the first in port order is the leftmost
    local total=0
    for e in "${left[@]}"; do w=$(lw "$e"); total=$((total + ${w:-0})); done
    x=$((-total))
    for e in "${left[@]}"; do
        apply_ext "$e" "${x}x0"; w=$(lw "$e"); x=$((x + ${w:-0}))
    done
}

apply_single() {
    case "$(layout)" in
        laptop)
            hm "$EXT" "disabled" "0x0" "1.0" ;;
        external)
            hm "$LAPTOP" "disabled" "0x0" "1.0" ;;
        ext_left)
            apply_ext "$EXT" "auto-left"
            hm "$LAPTOP" "$LAPTOP_RES" "0x0" "$LAPTOP_SCALE" ;;
        mirror)
            hm "$LAPTOP" "$LAPTOP_RES" "0x0" "$LAPTOP_SCALE"
            apply_ext "$EXT" "0x0" ", mirror='$LAPTOP'" ;;
        *)
            hm "$LAPTOP" "$LAPTOP_RES" "0x0" "$LAPTOP_SCALE"
            apply_ext "$EXT" "auto-right" ;;
    esac
}

apply() { [ "$DUAL" = 1 ] && apply_dual || apply_single; }

if ! read_monitors; then
    # Unplugged everything: make sure the laptop panel isn't left disabled.
    if [ "${1:-}" = --apply ]; then
        hm "$LAPTOP" "$LAPTOP_RES" "0x0" "$LAPTOP_SCALE"
        exit 0
    fi
    # The launcher shows this row instead of an empty list; picking it does
    # nothing. Run directly (keybind), it says so in a notification.
    case "${1:-}" in
        --menu) printf '%s\t%s\n' "No external monitor connected · laptop screen only" "-" ;;
        --menu-pick) ;;
        *) notify-send -u low -t 3000 "Display" "No external monitor connected" ;;
    esac
    exit 0
fi

mark() { [ "$1" = "$2" ] && echo "  ✓" || echo ""; }
row() { printf '%s\t%s\n' "$1" "$2"; }

menu() {
    case "${1:-}" in
        "")
            if [ "$DUAL" = 1 ]; then
                local cur; cur=$(dual_layout)
                row "HDMI left · USB-C right$(mark "$cur" hdmi_left)" dual:hdmi_left
                row "USB-C left · HDMI right$(mark "$cur" hdmi_right)" dual:hdmi_right
                row "Externals only (laptop off)$(mark "$cur" external)" dual:external
            else
                local cur; cur=$(layout)
                row "External left$(mark "$cur" ext_left)" layout:ext_left
                row "Laptop left$(mark "$cur" laptop_left)" layout:laptop_left
                row "Mirror$(mark "$cur" mirror)" layout:mirror
            fi
            row "Resolution…   $EXT_DESC  $EXT_CURRENT_RES" sub:res
            row "Scale…   $EXT_DESC  $EXT_CURRENT_SCALE" sub:scale
            row "Rotation…   $EXT_DESC  $(transform_label "$EXT_CURRENT_TRANSFORM")" sub:rot
            ;;
        res)
            tr ' ' '\n' <<<"$EXT_MODES" | command grep -v '^$' | while IFS= read -r mode; do
                clean=$(sed 's/Hz$//; s/\.[0-9]*$//' <<<"$mode")
                [[ "$clean" == "${EXT_CURRENT_RES%%.*}" ]] && row "$mode  ✓" "res:$clean" || row "$mode" "res:$clean"
            done
            ;;
        scale)
            for v in 0.5 0.75 1.0 1.25 1.5 2.0; do
                awk -v a="$v" -v b="$EXT_CURRENT_SCALE" 'BEGIN { exit !(a + 0 == b + 0) }' \
                    && row "$v  ✓" "scale:$v" || row "$v" "scale:$v"
            done
            ;;
        rot)
            for t in 0 1 2 3 4 5; do
                row "$(transform_label $t)$(mark "$EXT_CURRENT_TRANSFORM" $t)" "rot:$t"
            done
            ;;
    esac
}

pick() {
    case "$1" in
        sub:*) printf 'OPEN\tdisplay-%s\n' "${1#sub:}" ;;
        dual:*)
            save_dual "${1#dual:}"
            apply_dual
            case "${1#dual:}" in
                hdmi_left)  notify-send -u low -t 2000 "🖵💻🖵 Extended" "HDMI left, Laptop, USB-C right" ;;
                hdmi_right) notify-send -u low -t 2000 "🖵💻🖵 Extended" "USB-C left, Laptop, HDMI right" ;;
                external)   notify-send -u low -t 2000 "🖵🖵 Externals only" "Laptop panel off" ;;
            esac
            ;;
        layout:ext_left)
            save_state "ext_left"
            apply_single
            notify-send -u low -t 2000 "🖵💻 Extended" "$EXT_DESC left, Laptop right"
            ;;
        layout:laptop_left)
            save_state "laptop_left"
            apply_single
            notify-send -u low -t 2000 "💻🖵 Extended" "Laptop left, $EXT_DESC right"
            ;;
        layout:mirror)
            save_state "mirror"
            apply_single
            notify-send -u low -t 2000 "🪞 Mirror" "Both showing same content"
            ;;
        # each pick is remembered for this physical monitor and re-applied
        # with the others, so a scale change no longer resets the resolution
        res:*)
            profile_set "$(desc_of "$EXT")" mode "${1#res:}"
            apply
            notify-send -u low -t 2000 "Resolution" "$EXT_DESC → ${1#res:} (remembered for this monitor)"
            ;;
        scale:*)
            profile_set "$(desc_of "$EXT")" scale "${1#scale:}"
            apply
            notify-send -u low -t 2000 "Scale" "$EXT_DESC → ${1#scale:}x (remembered for this monitor)"
            ;;
        rot:*)
            profile_set "$(desc_of "$EXT")" transform "${1#rot:}"
            apply
            notify-send -u low -t 2000 "Rotation" "$EXT_DESC → $(transform_label "${1#rot:}") (remembered for this monitor)"
            ;;
    esac
}

case "${1:-}" in
    --apply) apply ;;
    --menu) menu "${2:-}" ;;
    --menu-pick) pick "$2" ;;
    *) exec ~/.cargo/bin/amane ipc call auxiliary search display ;;
esac
