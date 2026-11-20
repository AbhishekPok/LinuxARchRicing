#!/usr/bin/env bash
# Ops chips for the amane menu bar. Idea from fiffeek/.dotfiles.v2
# (vpn_status.sh), extended for AWS and SSH.
#
#   ops.sh amane-watch  aws + vpn + ssh state as one tagged stream (island/ops.rs)
#   ops.sh aws-pick     open a kitty already set to a profile ("aws " in the
#                       amane launcher; --menu / --menu-pick PROFILE)
#   ops.sh aws|vpn|ssh  print one row now (debugging)
#
# Watchers: aws is event-driven off Hyprland's socket2, vpn off netlink link
# events, ssh polled but fork-free; each prints only on change.
#
# The AWS profile lives in each shell's environment, so the bar shows the one
# belonging to the focused window: the zsh title hook appends " · aws:<name>"
# and "[PROD]" (see ~/.zshrc). Nothing under ~/.aws/sso or credentials is read.

PROD_RE='(^|[^a-z0-9])(prod|prd|production|live)([^a-z0-9]|$)'
# The zsh title hook appends this; see the header note.
AWS_RE=' · aws:([^ ]+)'

# One row: class<TAB>text<TAB>tooltip. printf is a builtin, so no forks.
row() {
    printf '%s	%s	%s
' "$2" "$1" "${3//$'
'/ | }"
}

# Renders the module from a window title. Split from the title lookup so
# aws-watch can feed it the title straight out of a Hyprland event instead of
# forking hyprctl for it.
aws_render() {
    local title=$1 profile
    if [[ "$title" =~ $AWS_RE ]]; then
        profile=${BASH_REMATCH[1]}
        if [[ "$title" == *"[PROD]"* ]]; then
            row " $profile" prod "AWS profile: $profile (production)"
        else
            row " $profile" active "AWS profile: $profile"
        fi
    else
        row "" none "Focused window has no AWS profile
Click: open a terminal for a profile"
    fi
}

aws_title() { hyprctl activewindow -j 2>/dev/null | jq -r '.title // empty'; }

# Sets KEY to the state a title renders to, using only builtins. aws_watch
# compares this instead of the rendered JSON, so a window whose title churns
# (a browser tab, a progress bar) never reaches jq at all.
aws_key() {
    if [[ $1 =~ $AWS_RE ]]; then
        KEY=${BASH_REMATCH[1]}
        if [[ $1 == *"[PROD]"* ]]; then KEY="prod:$KEY"; else KEY="active:$KEY"; fi
    else
        KEY=none
    fi
}

aws_mod() { aws_render "$(aws_title)"; }

# Subscribe to Hyprland's event socket and reprint only when the row changes.
# The old 1 s poll cost ~7% of a core all day -- three forks a second
# (bash + hyprctl + jq) to re-derive a string that changes a few times an hour.
# Focus changes now also land instantly instead of up to a second late.
aws_watch() {
    local sock=$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock
    local init_title last_key
    init_title=$(aws_title)
    aws_render "$init_title"
    aws_key "$init_title"
    last_key=$KEY
    # No socket (Hyprland restarted -> new signature): exit; amane_watch's
    # parent (island/ops.rs) respawns the stream with the environment re-read.
    [[ -S "$sock" ]] || exit 0
    # The whole loop lives in one subshell, so focused/last_key persist.
    socat -U - UNIX-CONNECT:"$sock" | {
        local line ev data focused=
        emit() {
            aws_key "$1"
            [[ $KEY == "$last_key" ]] && return
            last_key=$KEY
            aws_render "$1"
        }
        while IFS= read -r line; do
            ev=${line%%>>*}
            data=${line#*>>}
            case "$ev" in
                # activewindow>>class,title -- the title is in the event itself.
                activewindow)   emit "${data#*,}" ;;
                activewindowv2) focused=$data ;;
                # windowtitlev2>>address,title -- only the focused window counts.
                windowtitlev2)  [[ "${data%%,*}" == "$focused" ]] && emit "${data#*,}" ;;
                closewindow|focusedmon) emit "$(aws_title)" ;;
            esac
        done
    }
}

# Profile names and regions from ~/.aws/config. Only those two keys are read.
aws_profiles() {
    awk '
        /^\[profile / { if (p) print p, r; p = $2; sub(/\]$/, "", p); r = ""; next }
        /^\[/         { if (p) print p, r; p = ""; next }
        p && /^[ \t]*region[ \t]*=/ { r = $0; sub(/^[^=]*=[ \t]*/, "", r) }
        END           { if (p) print p, r }
    ' "$HOME/.aws/config" 2>/dev/null
}

aws_pick() {
    case "${1:-}" in
        --menu)
            aws_profiles | awk '{ printf "%s   %s\t%s\n", $1, ($2 ? $2 : "-"), $1 }'
            return ;;
        --menu-pick) profile=$2 ;;
        *) exec ~/.cargo/bin/amane ipc call auxiliary search aws ;;
    esac
    [[ -n "$profile" ]] || return
    region=$(aws_profiles | awk -v p="$profile" '$1 == p { print $2 }')
    # Same steps as efenv/tcpenv: log in only when the SSO session has expired.
    setsid kitty env AWS_PROFILE="$profile" ${region:+AWS_REGION="$region" AWS_DEFAULT_REGION="$region"} \
        zsh -ic 'aws sts get-caller-identity > /dev/null 2>&1 || aws sso login --no-browser; exec zsh' \
        > /dev/null 2>&1 < /dev/null &
}

vpn_mod() {
    names=$(nmcli -t -f TYPE,NAME connection show --active 2>/dev/null \
        | awk -F: '$1 ~ /vpn|wireguard/ { print $2 }')
    ifaces=$(ip -br link 2>/dev/null | awk '$1 ~ /^(wg|tun|tailscale)/ { print $1 }')
    up=$(printf '%s\n%s\n' "$names" "$ifaces" | sed '/^$/d' | sort -u)
    if [[ -n "$up" ]]; then
        row "󰦝" on "VPN up:
$up"
    else
        row "" off ""
    fi
}

# Continuous. Bringing a VPN up or down always adds or removes
# a wg/tun interface -- including for NM-managed connections -- so a single
# netlink stream covers both halves of vpn_mod with no polling of nmcli.
vpn_watch() {
    local last out
    last=$(vpn_mod)
    printf '%s\n' "$last"
    # exec: the coproc subshell becomes ip itself, so $IPMON_PID is the real
    # producer and the trap reaps it instead of orphaning it on a restart.
    coproc IPMON { exec ip -o monitor link 2>/dev/null; }
    trap 'kill "$IPMON_PID" 2>/dev/null' EXIT TERM INT

    # Interfaces that exist now and are not VPN-ish are "boring": wifi flaps a
    # couple of times a minute, and state churn on an already-present physical
    # link cannot change what vpn_mod reports -- but recomputing it costs an
    # nmcli + ip + jq round trip. Any name *not* in this set is new, which is
    # exactly what a tun/wg/ppp/tailscale device appearing looks like, so it
    # always triggers. Deletions always trigger too.
    local -A boring=()
    local ifn
    while read -r _ ifn _; do
        ifn=${ifn%:}; ifn=${ifn%%@*}
        [[ $ifn =~ ^(wg|tun|tailscale|ppp) ]] || boring[$ifn]=1
    done < <(ip -o link show 2>/dev/null)

    local line
    while IFS= read -r -u "${IPMON[0]}" line; do
        if [[ $line != Deleted* ]]; then
            # "3: wlo1: <BROADCAST,...>" -> wlo1
            ifn=${line#*: }; ifn=${ifn%%:*}; ifn=${ifn%%@*}
            [[ -v boring[$ifn] ]] && continue
        fi
        # One connect emits a burst of link events; collapse the burst into a
        # single recompute once the stream has been quiet for 300ms.
        while IFS= read -r -t 0.3 -u "${IPMON[0]}" _; do :; done
        out=$(vpn_mod)
        [[ "$out" == "$last" ]] && continue
        last=$out
        printf '%s\n' "$out"
    done
}

SSH_POLL_BATTERY=10
AC_ONLINE=$(echo /sys/class/power_supply/A*/online | cut -d' ' -f1)
SSH_POLL=3          # seconds. Measured cost of the fork-free scan below, as a
                    # share of one core: 1.70% at 2 s, 0.90% at 3 s, 0.65% at
                    # 5 s. The old pgrep+awk scan cost 3.25% at 5 s, so 3 s is
                    # 3.6x cheaper than that and still 1.7x fresher.

# Fills SSH_TARGETS with the open ssh / SSM session targets using only shell
# builtins: no pgrep, no awk, no forks at all. That is what makes polling cheap.
#
# It also fixes a parsing bug in the old pgrep version: "pgrep -a" flattens
# argv into one space-separated line, so an option value containing a space
# (e.g. -o ProxyCommand="sleep 40") shifted the fields and the bar showed "40"
# as the host. /proc/<pid>/cmdline is NUL-separated, so argv survives intact.
ssh_targets() {
    local p comm i a target ssm=0
    local -a argv
    SSH_TARGETS=()
    for p in /proc/[0-9]*; do
        # A process can exit mid-scan. The failing redirect is reported by the
        # shell, not by read, so the group is what needs its stderr silenced.
        { read -r comm < "$p/comm"; } 2>/dev/null || continue
        case $comm in
            ssh) ;;
            # comm is truncated to 15 chars, so session-manager-plugin arrives
            # as "session-manager".
            session-manager*) ssm=1; continue ;;
            *) continue ;;
        esac
        { mapfile -d '' -t argv < "$p/cmdline"; } 2>/dev/null || continue
        [[ " ${argv[*]} " == *git-upload-pack* || " ${argv[*]} " == *git-receive-pack* ]] && continue
        target=
        # First word that is neither an option nor the value of one.
        for (( i = 1; i < ${#argv[@]}; i++ )); do
            a=${argv[i]}
            if [[ $a =~ ^-[bcDEeFIiJLlmOoPpQRSWw]$ ]]; then (( i++ )); continue; fi
            [[ $a == -* ]] && continue
            target=$a
            break
        done
        [[ -n $target ]] && SSH_TARGETS+=("$target")
    done
    (( ssm )) && SSH_TARGETS+=("ssm session")
    return 0
}

ssh_render() {
    local class=active joined
    (( ${#SSH_TARGETS[@]} == 0 )) && { row "" none ""; return; }
    printf -v joined '%s\n' "${SSH_TARGETS[@]}"
    joined=${joined%$'\n'}
    [[ ${joined,,} =~ $PROD_RE ]] && class=prod
    row "󰣀 ${#SSH_TARGETS[@]}" "$class" "Open sessions:
$joined"
}

ssh_mod() { ssh_targets; ssh_render; }

# Continuous. There is no unprivileged event for "a process
# started or exited", so this still polls -- but a pass costs ~9 ms of CPU and
# zero forks (the old pgrep+awk pass cost ~83 ms), and it prints only when the
# set of sessions actually changes.
ssh_watch() {
    local fp last_fp=__start__
    while :; do
        ssh_targets
        fp="${SSH_TARGETS[*]}"
        if [[ "$fp" != "$last_fp" ]]; then
            last_fp=$fp
            ssh_render
        fi
        # on battery the /proc scan runs a third as often (0.9% -> ~0.3% of a core)
        if [[ $(<"$AC_ONLINE") == 1 ]] 2>/dev/null; then
            sleep "$SSH_POLL"
        else
            sleep "$SSH_POLL_BATTERY"
        fi
    done
}

# All three watchers on one stream for the amane island, tagged by kind:
#   <kind>\t<class>\t<text>\t<tooltip>
# Each child is the existing watcher, so the event-driven behaviour and the
# dedupe are unchanged; only the rendering differs.
amane_watch() {
    local self=$0 parent=$PPID
    for kind in aws vpn ssh; do
        # printf is a builtin, so tagging costs no forks per line.
        "$self" "$kind-watch" | while IFS= read -r line; do
            printf '%s\t%s\n' "$kind" "$line"
        done &
    done
    # amane-shell starts this as its own process group. When the shell goes
    # (restart, crash) nothing reads our output, but the watchers only notice
    # on their next write, which for vpn may be never; every restart used to
    # leave a tree behind. So follow the parent and take the group down with it.
    while kill -0 "$parent" 2>/dev/null; do sleep 5; done
    kill -- -"$$" 2>/dev/null
}

case "$1" in
    amane-watch) amane_watch ;;
    aws)       aws_mod ;;
    aws-watch) aws_watch ;;
    aws-pick)  aws_pick "${@:2}" ;;
    vpn)       vpn_mod ;;
    vpn-watch) vpn_watch ;;
    ssh)       ssh_mod ;;
    ssh-watch) ssh_watch ;;
    *)        echo "Usage: $0 {aws|aws-watch|aws-pick|vpn|vpn-watch|ssh|ssh-watch|amane-watch}" >&2; exit 1 ;;
esac
