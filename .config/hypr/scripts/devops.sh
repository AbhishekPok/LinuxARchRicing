#!/usr/bin/env bash
# DevOps keyword modes for the amane launcher (SUPER+Space, then the keyword):
#
#   kube   switch kubectl context            devops.sh kube [--menu | --menu-pick CTX]
#   ssm    SSM session for an AWS profile    devops.sh ssm  [--menu | --menu-pick PROFILE]
#   gh     GitHub pages                      devops.sh gh   [--menu | --menu-pick URL]
#
# Rows are "label<TAB>value" (amane modes.rs protocol). Picks run under the
# launcher, which waits on stdout, so anything started here is detached with
# its output sent to /dev/null. Nothing here polls; each list is built only
# when its mode is opened.
set -u

open_mode() { exec ~/.cargo/bin/amane ipc call auxiliary search "$1"; }

# ---------------------------------------------------------------- kube
kubeconfig() { printf '%s' "${KUBECONFIG:-$HOME/.kube/config}" | cut -d: -f1; }

kube_contexts() {
    local cfg; cfg=$(kubeconfig)
    [[ -r $cfg ]] || return 1
    local current
    current=$(awk '/^current-context:/ { gsub(/"/, "", $2); print $2 }' "$cfg")
    # names under the contexts: list, in file order
    awk '/^contexts:/ { inside = 1; next }
         /^[^ -]/    { inside = 0 }
         inside && /^[ -]+name:/ { sub(/^[ -]+name:[ ]*/, ""); gsub(/"/, ""); print }' "$cfg" |
    while read -r ctx; do
        short=${ctx##*/}
        if [[ $ctx == "$current" ]]; then
            printf '%s   (current)\t%s\n' "$short" "$ctx"
        else
            printf '%s\t%s\n' "$short" "$ctx"
        fi
    done
}

kube() {
    case "${1:-}" in
        --menu)
            if ! kube_contexts; then
                printf 'No kubeconfig at %s (run aws eks update-kubeconfig)\t-\n' "$(kubeconfig)"
            fi ;;
        --menu-pick)
            [[ ${2:-} == - || -z ${2:-} ]] && return
            if command -v kubectl > /dev/null; then
                kubectl config use-context "$2" > /dev/null 2>&1
            else
                # no kubectl: edit current-context in place (what use-context does)
                sed -i "s|^current-context:.*|current-context: $2|" "$(kubeconfig)"
            fi ;;
        *) open_mode kube ;;
    esac
}

# ----------------------------------------------------------------- ssm
aws_profiles() {
    awk '/^\[profile / { gsub(/^\[profile |\]$/, ""); print }' "$HOME/.aws/config" 2>/dev/null
}

ssm() {
    case "${1:-}" in
        --menu)
            aws_profiles | awk '{ printf "%s\t%s\n", $1, $1 }' ;;
        --menu-pick)
            [[ -n ${2:-} ]] || return
            # the user's own `ssmc` picker (~/.zshrc) does the instance list and
            # SSO login; this only hands it a terminal with the profile set
            setsid kitty env AWS_PROFILE="$2" zsh -ic 'ssmc; exec zsh' \
                > /dev/null 2>&1 < /dev/null & ;;
        *) open_mode ssm ;;
    esac
}

# ------------------------------------------------------------------ gh
gh_rows() {
    printf '%s\t%s\n' \
        "Pull requests (yours)"   "https://github.com/pulls" \
        "Review requests"         "https://github.com/pulls/review-requested" \
        "Notifications"           "https://github.com/notifications" \
        "Issues (yours)"          "https://github.com/issues" \
        "Actions: recent runs"    "https://github.com/notifications?query=reason%3Aci-activity"
}

gh() {
    case "${1:-}" in
        --menu) gh_rows ;;
        --menu-pick)
            [[ ${2:-} == https://* ]] || return
            setsid xdg-open "$2" > /dev/null 2>&1 < /dev/null & ;;
        *) open_mode gh ;;
    esac
}

case "${1:-}" in
    kube) kube "${@:2}" ;;
    ssm)  ssm "${@:2}" ;;
    gh)   gh "${@:2}" ;;
    *) echo "Usage: $0 {kube|ssm|gh} [--menu | --menu-pick VALUE]" >&2; exit 1 ;;
esac
