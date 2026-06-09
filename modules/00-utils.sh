#!/usr/bin/env bash

BACKUP_DIR="$HOME/.config/rice-backups/$(date +%F-%H%M%S)"

section() {
  echo
  echo "============================================================"
  echo "$1"
  echo "============================================================"
}

backup_file() {
  local file="$1"

  if [ -f "$file" ]; then
    mkdir -p "$BACKUP_DIR$(dirname "$file")"
    cp "$file" "$BACKUP_DIR$file"
    echo "Backup: $file"
  fi
}

copy_file() {
  local src="$1"
  local dst="$2"

  if [ -f "$src" ]; then
    backup_file "$dst"
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
    echo "Installed: $dst"
  else
    echo "Skipped missing file: $src"
  fi
}

copy_dir() {
  local src="$1"
  local dst="$2"

  if [ -d "$src" ]; then
    mkdir -p "$dst"
    cp -a "$src"/. "$dst"/
    echo "Installed directory: $dst"
  else
    echo "Skipped missing directory: $src"
  fi
}

ensure_line() {
  local file="$1"
  local line="$2"

  touch "$file"

  if ! command grep -Fxq "$line" "$file"; then
    echo "$line" >> "$file"
    echo "Added line to $file: $line"
  fi
}

run_module() {
  local module="$1"
  local path="$MODULES_DIR/$module"

  if [ ! -f "$path" ]; then
    echo "Missing module: $path"
    exit 1
  fi

  section "Running $module"
  source "$path"
}
