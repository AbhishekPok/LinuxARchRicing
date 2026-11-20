#!/usr/bin/env bash
# Text for the hyprlock cards (caelestia-style layout, see hyprlock.conf).
# Each subcommand prints pango markup and always exits 0, so a missing tool
# shows a fallback instead of an empty card. Colours come from the palette
# lock.sh writes from the active theme right before locking.

cache=$HOME/.cache/hyprlock
# shellcheck source=/dev/null
[[ -r $cache/palette.sh ]] && . "$cache/palette.sh"
FG=${FG:-d4d4d4} DIM=${DIM:-8a8a8a} PRIMARY=${PRIMARY:-00b8cc}
SECONDARY=${SECONDARY:-00ff99} TERTIARY=${TERTIARY:-bc51ff}
ERROR=${ERROR:-ff3e3e} WARN=${WARN:-ff8c00}

c() { printf '<span foreground="#%s">%s</span>' "$1" "$2"; }
esc() { sed 's/&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g'; }

# 10-cell meter: bar <percent> <colour>
bar() {
    local p=$1 n i out=""
    (( p < 0 )) && p=0; (( p > 100 )) && p=100
    n=$(( (p + 5) / 10 ))
    for (( i = 0; i < 10; i++ )); do
        if (( i < n )); then out+="▰"; else out+="▱"; fi
    done
    c "$2" "${out:0:n}"; c "$DIM" "${out:n}"
}

row() { # row <label> <percent> <colour> <value>
    printf '%s  %s  %s\n' "$(c "$DIM" "$(printf '%-4s' "$1")")" "$(bar "$2" "$3")" "$(c "$FG" "$4")"
}

case "$1" in
clock)
    printf '%s%s' "$(c "$PRIMARY" "$(date +%H)")" "$(c "$SECONDARY" "$(date +%M)")"
    ;;
date)
    date '+%A  •  %d %b' | tr '[:lower:]' '[:upper:]'
    ;;
greeting)
    h=$(date +%-H)
    if (( h < 5 )); then g="Up late"; elif (( h < 12 )); then g="Good morning"
    elif (( h < 17 )); then g="Good afternoon"; else g="Good evening"; fi
    name=$(getent passwd "$USER" | cut -d: -f5 | cut -d, -f1)
    printf '%s, %s' "$g" "$(c "$PRIMARY" "${name:-$USER}")"
    ;;
weather-refresh)
    # at most every 30 minutes, never blocking a label
    f=$cache/weather
    if [[ ! -s $f ]] || (( $(date +%s) - $(stat -c %Y "$f") > 1800 )); then
        w=$(curl -sf --max-time 6 'https://wttr.in/?format=%t|%C|%l|%h|%w' 2>/dev/null)
        [[ $w == *"|"* && $w != *"Unknown"* ]] && printf '%s\n' "$w" > "$f"
    fi
    ;;
weather)
    IFS='|' read -r temp cond loc hum wind < "$cache/weather" 2>/dev/null
    if [[ -z $temp ]]; then
        printf '%s\n%s' "$(c "$PRIMARY" "<b>Weather</b>")" "$(c "$DIM" "Unavailable offline")"
        exit 0
    fi
    loc=${loc%%,*}
    # no emoji font here, so a Nerd Font glyph from the condition text
    case ${cond,,} in
        *thunder*) icon=󰖓 ;; *snow*|*sleet*|*ice*) icon=󰖘 ;;
        *rain*|*drizzle*|*shower*) icon=󰖗 ;; *fog*|*mist*|*haze*|*smok*|*dust*) icon=󰖑 ;;
        *partly*) icon=󰖕 ;; *cloud*|*overcast*) icon=󰖐 ;;
        *sun*|*clear*) [[ $(date +%-H) -ge 6 && $(date +%-H) -lt 18 ]] && icon=󰖙 || icon=󰖔 ;;
        *) icon=󰖐 ;;
    esac
    printf '%s  %s\n%s\n%s' \
        "$(c "$PRIMARY" "$icon")" "$(c "$PRIMARY" "<b>${temp#+}</b>")" \
        "$(c "$FG" "$(printf '%s' "$cond" | esc)")" \
        "$(c "$DIM" "<small>$(printf '%s · 󰖌 %s · %s' "$loc" "$hum" "${wind//[^0-9a-z\/]/}" | esc)</small>")"
    ;;
fetch)
    os=$(. /etc/os-release 2>/dev/null; echo "${PRETTY_NAME:-Linux}")
    up=$(uptime -p | sed 's/^up //; s/ hours\?/h/; s/ minutes\?/m/; s/ days\?/d/; s/ weeks\?/w/; s/,//g')
    pkgs=$(pacman -Qq 2>/dev/null | wc -l)
    k() { printf '%s %s\n' "$(c "$SECONDARY" "$(printf '%-7s' "$1")")" "$(c "$FG" "$2")"; }
    printf '%s %s\n' "$(c "$PRIMARY" "<b>$USER</b>")" "$(c "$DIM" "@ $(cat /etc/hostname 2>/dev/null || hostname)")"
    k os "$os"
    k kernel "$(uname -r)"
    k uptime "$up"
    k pkgs "$pkgs"
    k wm "Hyprland"
    printf '%s %s' "$(c "$SECONDARY" "$(printf '%-7s' shell)")" "$(c "$FG" "$(basename "${SHELL:-sh}")")"
    ;;
media)
    st=$(playerctl status 2>/dev/null)
    if [[ -z $st || $st == Stopped ]]; then
        printf '%s\n%s' "$(c "$PRIMARY" "<b>Now playing</b>")" "$(c "$DIM" "Nothing playing")"
        exit 0
    fi
    title=$(playerctl metadata title 2>/dev/null | cut -c1-34 | esc)
    artist=$(playerctl metadata artist 2>/dev/null | cut -c1-34 | esc)
    icon=$([[ $st == Playing ]] && echo "󰐊" || echo "󰏤")
    printf '%s  %s\n%s\n%s' "$(c "$PRIMARY" "$icon")" "$(c "$FG" "<b>${title:-Unknown}</b>")" \
        "$(c "$DIM" "${artist:-Unknown artist}")" "$(c "$SECONDARY" "$st")"
    ;;
resources)
    read -r _ a1 b1 c1 d1 e1 f1 g1 _ < /proc/stat
    sleep 0.25
    read -r _ a2 b2 c2 d2 e2 f2 g2 _ < /proc/stat
    busy=$(( (a2+b2+c2+f2+g2) - (a1+b1+c1+f1+g1) )); idle=$(( (d2+e2) - (d1+e1) ))
    cpu=$(( busy + idle > 0 ? 100 * busy / (busy + idle) : 0 ))
    temp=$(sensors 2>/dev/null | awk '/^Package id 0:/{gsub(/[+°C]/,"",$4); printf "%d", $4; exit}')
    read -r mt ma < <(awk '/^MemTotal/{t=$2} /^MemAvailable/{a=$2} END{print t, a}' /proc/meminfo)
    mem=$(( 100 * (mt - ma) / mt ))
    memv=$(awk -v u=$((mt - ma)) -v t="$mt" 'BEGIN{printf "%.1f/%.0fG", u/1048576, t/1048576}')
    disk=$(df --output=pcent / | tail -1 | tr -dc 0-9)
    printf '%s\n\n' "$(c "$PRIMARY" "<b>Resources</b>")"
    row cpu "$cpu" "$( ((cpu > 85)) && echo "$ERROR" || echo "$PRIMARY")" "$cpu%  ${temp:-?}°"
    row mem "$mem" "$( ((mem > 85)) && echo "$ERROR" || echo "$SECONDARY")" "$memv"
    row disk "$disk" "$( ((disk > 90)) && echo "$ERROR" || echo "$TERTIARY")" "$disk%"
    bat=/sys/class/power_supply/BAT1
    if [[ -r $bat/capacity ]]; then
        cap=$(<"$bat/capacity"); st=$(<"$bat/status")
        printf '%s' "$(row bat "$cap" "$( ((cap < 20)) && echo "$WARN" || echo "$SECONDARY")" "$cap%$([[ $st == Charging ]] && echo ' 󱐋')")"
    fi
    ;;
notifications)
    # newest last in amane's history: key, time, then hex app/title/body/...
    f=$HOME/.local/state/amane/notifications.v1
    printf '%s\n\n' "$(c "$PRIMARY" "<b>Notifications</b>")"
    lines=$(tail -n 4 "$f" 2>/dev/null | tac | python3 -I -c '
import sys, html, time
for line in sys.stdin:
    p = line.rstrip("\n").split("\t")
    if len(p) < 4:
        continue
    try:
        app = bytes.fromhex(p[2]).decode(errors="replace")[:18]
        title = bytes.fromhex(p[3]).decode(errors="replace")[:30]
        ago = int(time.time()) - int(p[1])
    except ValueError:
        continue
    when = "now" if ago < 60 else f"{ago//60}m" if ago < 3600 else f"{ago//3600}h" if ago < 86400 else f"{ago//86400}d"
    print(html.escape(app) + "\t" + html.escape(title) + "\t" + when)
')
    if [[ -z $lines ]]; then
        c "$DIM" "All caught up"
        exit 0
    fi
    while IFS=$'\t' read -r app title when; do
        printf '%s  %s\n%s\n\n' "$(c "$SECONDARY" "<b>$app</b>")" "$(c "$DIM" "$when")" "$(c "$FG" "$title")"
    done <<< "$lines"
    ;;
esac
exit 0
