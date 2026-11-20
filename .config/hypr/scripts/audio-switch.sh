#!/usr/bin/env bash
# Audio output switcher ("audio " in the amane launcher).
#   --menu            rows: label<TAB>speaker|headphones|bluetooth
#   --menu-pick DEV   switch to it

CARD="alsa_card.pci-0000_00_1f.3-platform-skl_hda_dsp_generic"
SPEAKER_PROFILE="HiFi (HDMI1, HDMI2, HDMI3, Mic1, Mic2, Speaker)"
HEADPHONE_PROFILE="HiFi (HDMI1, HDMI2, HDMI3, Headphones, Mic1, Mic2)"
SPEAKER_SINK="alsa_output.pci-0000_00_1f.3-platform-skl_hda_dsp_generic.HiFi__Speaker__sink"
HEADPHONE_SINK="alsa_output.pci-0000_00_1f.3-platform-skl_hda_dsp_generic.HiFi__Headphones__sink"

ACTIVE_PROFILE=$(pactl list cards | awk -v c="$CARD" '
    /^Card #/ { in_card = 0 }
    $1 == "Name:" && $2 == c { in_card = 1 }
    in_card && /Active Profile:/ { sub(/.*Active Profile: /, ""); print; exit }
')
DEFAULT_SINK=$(pactl get-default-sink)

BT_SINK=$(pactl list sinks short 2>/dev/null | command grep "bluez" | awk '{print $2}' | head -1)
BT_NAME="__no_bluetooth_sink__"   # sentinel: keeps *"$BT_NAME"* from becoming a catch-all glob
if [ -n "$BT_SINK" ]; then
    BT_NAME=$(pactl list sinks | awk -v sink="$BT_SINK" '
        /Name: / { found=($2==sink) }
        found && /Description:/ { sub(/.*Description: /,""); print; exit }
    ')
    [ -z "$BT_NAME" ] && BT_NAME="Bluetooth"
fi

if [ -n "$BT_SINK" ] && [ "$DEFAULT_SINK" = "$BT_SINK" ]; then
    CURRENT="bluetooth"
    CURRENT_LABEL="$BT_NAME"
elif echo "$ACTIVE_PROFILE" | command grep -q "Speaker"; then
    CURRENT="speaker"
    CURRENT_LABEL="Speaker"
else
    CURRENT="headphones"
    CURRENT_LABEL="Headphones"
fi

mark() { [ "$CURRENT" = "$1" ] && echo "  ✓" || echo ""; }

case "${1:-}" in
    --menu)
        printf '%s\t%s\n' "Speaker$(mark speaker)" speaker "Headphones$(mark headphones)" headphones
        [ -n "$BT_SINK" ] && printf '%s\t%s\n' "$BT_NAME$(mark bluetooth)" bluetooth
        exit 0 ;;
    --menu-pick) CHOICE=$2 ;;
    *) exec ~/.cargo/bin/amane ipc call auxiliary search audio ;;
esac

move_streams() {
    pactl list sink-inputs short | while read -r input_idx rest; do
        pactl move-sink-input "$input_idx" "$1" 2>/dev/null
    done
}

case "$CHOICE" in
    speaker)
        pactl set-card-profile "$CARD" "$SPEAKER_PROFILE"
        sleep 0.5
        pactl set-default-sink "$SPEAKER_SINK"
        move_streams "$SPEAKER_SINK"
        notify-send -u low -t 2000 "🔊 Audio" "Switched to: Speaker"
        ;;
    headphones)
        pactl set-card-profile "$CARD" "$HEADPHONE_PROFILE"
        sleep 0.5
        pactl set-default-sink "$HEADPHONE_SINK"
        move_streams "$HEADPHONE_SINK"
        notify-send -u low -t 2000 "🎧 Audio" "Switched to: Headphones"
        ;;
    bluetooth)
        if [ -n "$BT_SINK" ]; then
            pactl set-default-sink "$BT_SINK"
            move_streams "$BT_SINK"
            notify-send -u low -t 2000 "🎙 Audio" "Switched to: $BT_NAME"
        fi
        ;;
esac
