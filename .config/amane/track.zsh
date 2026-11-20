# amane Live Activities for zsh: long-running subcommands show in the notch
# (ring, name, elapsed time, then a check or a cross with the duration).
# Everything else -- and every tool when amane isn't running -- behaves exactly
# as before. Enable by adding to ~/.zshrc:   source ~/.config/amane/track.zsh
# Wrapping happens only for tools that are installed; see ~/.local/bin/track.

(( $+commands[track] )) || return 0

(( $+commands[terraform] )) && terraform() {
  case $1 in
    apply|destroy|import) track -l "terraform $1" command terraform "$@" ;;
    *) command terraform "$@" ;;
  esac
}

(( $+commands[helm] )) && helm() {
  case $1 in
    install|upgrade|rollback|uninstall) track -l "helm $1 ${2:-}" command helm "$@" ;;
    *) command helm "$@" ;;
  esac
}

(( $+commands[kubectl] )) && kubectl() {
  if [[ $1 == rollout && $2 == status ]]; then
    track -l "rollout ${3:-}" command kubectl "$@"
  else
    command kubectl "$@"
  fi
}

(( $+commands[docker] )) && docker() {
  case $1 in
    build|push|pull) track -l "docker $1" command docker "$@" ;;
    *) command docker "$@" ;;
  esac
}

return 0
