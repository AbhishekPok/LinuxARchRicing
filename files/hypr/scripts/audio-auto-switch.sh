#!/usr/bin/env bash

CARD="alsa_card.pci-0000_00_1f.3-platform-skl_hda_dsp_generic"

HEADPHONE_PROFILE="HiFi (HDMI1, HDMI2, HDMI3, Headphones, Headset, Mic1)"
SPEAKER_PROFILE="HiFi (HDMI1, HDMI2, HDMI3, Headset, Mic1, Speaker)"

HEADPHONE_SINK="alsa_output.pci-0000_00_1f.3-platform-skl_hda_dsp_generic.HiFi__Headphones__sink"
SPEAKER_SINK="alsa_output.pci-0000_00_1f.3-platform-skl_hda_dsp_generic.HiFi__Speaker__sink"

LAST_STATE=""

notify_audio() {
    notify-send -h string:x-canonical-private-synchronous:audio "$1" "$2" >/dev/null 2>&1 || true
}

sink_exists() {
    pactl list sinks short 2>/dev/null | awk '{print $2}' | command grep -Fxq "$1"
}

get_bt_sink() {
    pactl list sinks short 2>/dev/null | awk '/bluez/ {print $2; exit}'
}

headphones_available() {
    pactl list cards 2>/dev/null | awk '
        /\[Out\] Headphones:/ {found=1}
        found && /availability group: Headphone Mic, available/ {print "yes"; exit}
        found && /^\s*\[/ && !/\[Out\] Headphones:/ {found=0}
    '
}

switch_to_sink() {
    local sink="$1"
    local volume="${2:-80%}"

    if sink_exists "$sink"; then
        pactl set-default-sink "$sink" >/dev/null 2>&1 || true
        pactl set-sink-volume "$sink" "$volume" >/dev/null 2>&1 || true
        pactl set-sink-mute "$sink" 0 >/dev/null 2>&1 || true
        return 0
    fi

    return 1
}

switch_audio() {
    local bt_sink
    local hp_available
    local current_state

    bt_sink="$(get_bt_sink)"

    # Priority 1: Bluetooth audio
    if [ -n "$bt_sink" ]; then
        current_state="bluetooth"

        if [ "$LAST_STATE" != "$current_state" ]; then
            switch_to_sink "$bt_sink" "80%" && notify_audio "󰂱 Audio" "Switched to Bluetooth"
            LAST_STATE="$current_state"
        fi

        return
    fi

    hp_available="$(headphones_available)"

    # Priority 2: Wired headset/headphones
    if [ "$hp_available" = "yes" ]; then
        current_state="headphones"

        if [ "$LAST_STATE" != "$current_state" ]; then
            pactl set-card-profile "$CARD" "$HEADPHONE_PROFILE" >/dev/null 2>&1 || true
            sleep 0.6
            switch_to_sink "$HEADPHONE_SINK" "80%" && notify_audio "󰋋 Audio" "Switched to Headphones"
            LAST_STATE="$current_state"
        fi

        return
    fi

    # Priority 3: Laptop speaker
    current_state="speaker"

    if [ "$LAST_STATE" != "$current_state" ]; then
        pactl set-card-profile "$CARD" "$SPEAKER_PROFILE" >/dev/null 2>&1 || true
        sleep 0.6
        switch_to_sink "$SPEAKER_SINK" "80%" && notify_audio "󰕾 Audio" "Switched to Speaker"
        LAST_STATE="$current_state"
    fi
}

switch_audio

pactl subscribe 2>/dev/null | while read -r event; do
    if echo "$event" | command grep -qE "card|sink|server"; then
        sleep 0.5
        switch_audio
    fi
done
