#!/bin/bash

CARD="alsa_card.pci-0000_00_1f.3-platform-skl_hda_dsp_generic"

SPEAKER_PROFILE="HiFi (HDMI1, HDMI2, HDMI3, Mic1, Mic2, Speaker)"
HEADPHONE_PROFILE="HiFi (HDMI1, HDMI2, HDMI3, Headphones, Headset, Mic1)"

SPEAKER_SINK="alsa_output.pci-0000_00_1f.3-platform-skl_hda_dsp_generic.HiFi__Speaker__sink"
HEADPHONE_SINK="alsa_output.pci-0000_00_1f.3-platform-skl_hda_dsp_generic.HiFi__Headphones__sink"

LAST_STATE=""

get_bt_sink() {
    pactl list sinks short 2>/dev/null | command grep "bluez" | awk '{print $2}' | head -1
}

switch_audio() {
    BT_SINK=$(get_bt_sink)

    # Priority: Bluetooth > Headphones > Speaker
    if [ -n "$BT_SINK" ]; then
        CURRENT_STATE="bluetooth"

        if [ "$LAST_STATE" != "$CURRENT_STATE" ]; then
            pactl set-default-sink "$BT_SINK"
            pactl set-sink-volume "$BT_SINK" 80%
            pactl set-sink-mute "$BT_SINK" 0
            notify-send -h string:x-canonical-private-synchronous:audio "󰂱 Audio" "Switched to Bluetooth"
            
            LAST_STATE="$CURRENT_STATE"
        fi
        return
    fi

    HEADPHONE_STATUS=$(pactl list cards | awk '
        /\[Out\] Headphones:/ {found=1}
        found && /availability/ {print; exit}
    ')

    if echo "$HEADPHONE_STATUS" | /usr/bin/command grep -q "availability group: Headphone Mic, available"; then
        CURRENT_STATE="headphones"

        if [ "$LAST_STATE" != "$CURRENT_STATE" ]; then
            pactl set-card-profile "$CARD" "$HEADPHONE_PROFILE"
            sleep 1
            pactl set-default-sink "$HEADPHONE_SINK"
            pactl set-sink-volume "$HEADPHONE_SINK" 80%
            pactl set-sink-mute "$HEADPHONE_SINK" 0
            notify-send -h string:x-canonical-private-synchronous:audio "󰋋 Audio" "Switched to Headphones"
            
            LAST_STATE="$CURRENT_STATE"
        fi
    else
        CURRENT_STATE="speaker"

        if [ "$LAST_STATE" != "$CURRENT_STATE" ]; then
            pactl set-card-profile "$CARD" "$SPEAKER_PROFILE"
            sleep 1
            pactl set-default-sink "$SPEAKER_SINK"
            pactl set-sink-volume "$SPEAKER_SINK" 80%
            pactl set-sink-mute "$SPEAKER_SINK" 0
            notify-send -h string:x-canonical-private-synchronous:audio "󰕾 Audio" "Switched to Speaker"
            
            LAST_STATE="$CURRENT_STATE"
        fi
    fi
}

switch_audio

pactl subscribe | while read -r event; do
    if echo "$event" | /usr/bin/command grep -q -E "card|sink"; then
        sleep 0.5
        switch_audio
    fi
done
