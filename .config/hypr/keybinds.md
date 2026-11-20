# Hyprland keybinds

Generated from `~/.config/hypr/hyprland.lua` — 83 binds. Regenerate with `~/.config/hypr/scripts/keybinds.sh --write`.


## Launch & Apps

| Keys | Action |
|---|---|
| `SUPER + Return` | kitty |
| `SUPER + Space` | ~/.cargo/bin/amane ipc call auxiliary search |
| `SUPER + E` | nemo |
| `SUPER + grave` | dropdown.sh |
| `SUPER + SHIFT + Tab` | windows.sh |
| `SUPER + X` | ~/.cargo/bin/amane ipc call auxiliary utility |
| `SUPER + ALT + Return` | kitty-size.sh |

## Clipboard

| Keys | Action |
|---|---|
| `SUPER + SHIFT + V` | clipboard.sh |

## Screenshots

| Keys | Action |
|---|---|
| `Print` | screenshot.sh --now |
| `SUPER + SHIFT + S` | screenshot.sh --area |
| `SUPER + SHIFT + Print` | screenshot.sh --active |
| `SUPER + ALT + S` | screenshot.sh --swappy |

## Notifications

| Keys | Action |
|---|---|
| `SUPER + N` | ~/.cargo/bin/amane ipc call settings history |
| `SUPER + SHIFT + D` | ~/.cargo/bin/amane ipc call notifications dnd toggle |

## Workspaces

| Keys | Action |
|---|---|
| `SUPER + 1-9` | focus (workspace = i) |
| `SUPER + SHIFT + 1-9` | window.move (workspace = i) |
| `SUPER + mouse_down` | focus (workspace = e+1) |
| `SUPER + mouse_up` | focus (workspace = e-1) |
| `SUPER + S` | workspace.toggle_special (scratchpad) |
| `SUPER + CTRL + S` | window.move (workspace = special:scratchpad) |
| `SUPER + 0` | focus (workspace = 10) |
| `SUPER + SHIFT + 0` | window.move (workspace = 10) |
| `SUPER + CTRL + Left` | focus (workspace = e-1) |
| `SUPER + CTRL + Right` | focus (workspace = e+1) |
| `SUPER + Z` | focus (workspace = name:zen) |
| `SUPER + SHIFT + Z` | window.move (workspace = name:zen) |

## Windows

| Keys | Action |
|---|---|
| `SUPER + Q` | window.close |
| `SUPER + V` | window.float (action = toggle) |
| `SUPER + F` | window.fullscreen (mode = 1) |
| `SUPER + h` | focus (direction = left) |
| `SUPER + l` | focus (direction = right) |
| `SUPER + k` | focus (direction = up) |
| `SUPER + j` | focus (direction = down) |
| `SUPER + mouse:272` | window.drag |
| `SUPER + mouse:273` | window.resize |
| `SUPER + SHIFT + h` | window.move (direction = left) |
| `SUPER + SHIFT + l` | window.move (direction = right) |
| `SUPER + SHIFT + k` | window.move (direction = up) |
| `SUPER + SHIFT + j` | window.move (direction = down) |
| `SUPER + CTRL + h` | window.resize (x = -40, y = 0, relative = true) |
| `SUPER + CTRL + l` | window.resize (x = 40, y = 0, relative = true) |
| `SUPER + CTRL + k` | window.resize (x = 0, y = -40, relative = true) |
| `SUPER + CTRL + j` | window.resize (x = 0, y = 40, relative = true) |
| `SUPER + Tab` | focus (last = true) |
| `SUPER + SHIFT + F` | window.fullscreen (mode = fullscreen) |
| `SUPER + SHIFT + P` | window.pin |
| `SUPER + C` | window.center |
| `SUPER + T` | layout (togglesplit) |
| `SUPER + ALT + h` | window.move (into_group = left) |
| `SUPER + ALT + l` | window.move (into_group = right) |
| `SUPER + ALT + k` | window.move (into_group = up) |
| `SUPER + ALT + j` | window.move (into_group = down) |
| `SUPER + SHIFT + G` | window.move (out_of_group = true) |

## Display & Wallpaper

| Keys | Action |
|---|---|
| `SUPER + P` | monitor-switch.sh |
| `XF86MonBrightnessDown` | brightnessctl -e4 -n2 set 5%- |
| `XF86MonBrightnessUp` | brightnessctl -e4 -n2 set 5%+ |
| `SUPER + SHIFT + N` | hyprsunset.sh toggle |
| `SUPER + W` | wallpaper-picker.sh |

## Audio

| Keys | Action |
|---|---|
| `XF86AudioMute` | wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle |
| `XF86AudioLowerVolume` | wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%- |
| `XF86AudioRaiseVolume` | wpctl set-volume -l 2 @DEFAULT_AUDIO_SINK@ 5%+ |
| `XF86AudioMicMute` | wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle |
| `SUPER + A` | audio-switch.sh |

## Hardware & Power

| Keys | Action |
|---|---|
| `Scroll_Lock` | ~/.local/bin/kbb-scrolllock.sh toggle |
| `XF86Bluetooth` | bluetooth-toggle.sh |
| `F6` | bluetooth-toggle.sh |
| `SHIFT + F6` | blueman-manager |
| `F7` | power-profile.sh |
| `SUPER + F5` | gamemode.sh |

## Session & Lock

| Keys | Action |
|---|---|
| `SUPER + SHIFT + L` | lock.sh |

## Help

| Keys | Action |
|---|---|
| `SUPER + slash` | keybinds.sh |

## Other

| Keys | Action |
|---|---|
| `SUPER + M` | exit |
| `SUPER + ALT + I` | amane ipc call awake toggle |
| `SUPER + SHIFT + A` | ~/.cargo/bin/amane ipc call settings |
| `SUPER + SHIFT + B` | ~/.cargo/bin/amane ipc call bar toggle |
| `SUPER + backslash` | ~/.cargo/bin/amane ipc call media play-pause |
| `SUPER + G` | group.toggle |
| `SUPER + ALT + Tab` | group.next |
| `SUPER + period` | emoji.sh |
| `SUPER + O` | project.sh |
| `SUPER + ALT + W` | websearch.sh |
| `SUPER + ALT + C` | quickedit.sh |
| `SUPER + SHIFT + O` | tmux-switch.sh |
