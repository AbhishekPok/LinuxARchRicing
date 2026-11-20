# ashwa-min — SDDM login theme

A single hairline input on black. Nothing else: no clock, no date, no greeting,
no user@host, no avatar, no buttons, no background image.

    sudo ~/.local/share/sddm-ashwa-min/install.sh    # install + activate
    sudo ~/.local/share/sddm-ashwa-min/rollback.sh   # back to where_is_my_sddm_theme

Takes effect at the next login; neither script restarts sddm.

## What this is

A stripped copy of `where_is_my_sddm_theme`, which was already the active theme
but had been hand-edited: 80 lines of `Main.qml` added a 72pt clock, a date
line, "Welcome back, abhishek" and "abhishek@ashwa". All four are gone here.
`passwordInput` anchors to the screen centre on its own, so nothing else had to
be re-anchored.

It lives in its own theme directory because `where-is-my-sddm-theme-git` has no
backup files registered with pacman (`Backup Files : None`), so a package
update silently overwrites both `Main.qml` and `theme.conf`. `pacman -Qkk`
already reports those two as altered. Keeping this separate means an update of
that package can no longer revert the design.

## Design

| Setting | Value | Why |
|---|---|---|
| `passwordInputBackground` | `#00000000` | fully transparent — just the hairline, no panel |
| `passwordInputBorderWidth` | `1` | was 2 |
| `passwordInputRadius` | `2` | was 8 |
| `passwordInputWidth` | `0.17` | was 0.25 |
| `passwordFontSize` | `22` | was 48 |
| `passwordCharacter` | `•` | was `*` |
| `showUserRealNameByDefault` | `false` | nothing should render but the box |
| `hideCursor` | `true` | see the caveat below |

`F1` still shows the keybind help, `F2`/`F3` cycle users/sessions, `F10`/`F11`/`F12`
suspend/poweroff/reboot. Those are upstream and untouched.

## metadata.desktop must keep `QtVersion=6`

`Main.qml` line 5 is `import Qt5Compat.GraphicalEffects` — versionless, which
is legal only under Qt6. SDDM chooses which greeter binary to launch from the
`QtVersion` key in `metadata.desktop`. With the key missing the daemon starts
the Qt5 `/usr/bin/sddm-greeter` and the theme fails at the login screen with:

    import needs a version

The journal shows which binary was used:

    journalctl -b -u sddm | grep 'Starting X11 session'

`install.sh` now refuses to run if `QtVersion=6` is absent, and prints the value
after installing. Do not drop the key when editing this file.

## Caveat: hideCursor

`hideCursor=true` is set, but upstream implements it as a `MouseArea` with
`enabled: false` and `cursorShape: Qt.BlankCursor`. A disabled MouseArea does
not reliably apply its cursor shape, so the pointer can still appear over parts
of the screen. This is unchanged from the previous theme and is cosmetic —
the password field takes focus on load, so logging in never needs the mouse.
Making it work would mean enabling that MouseArea, which would swallow clicks
to the user and session pickers, so it was left as upstream has it.

## Variant: with a clock

`Main.qml.with-clock` adds back one 20pt `HH:mm` above the input (see
`preview-with-clock.png`). To use it:

    cp Main.qml.with-clock ashwa-min/Main.qml && sudo ./install.sh

## Previewing a change without logging out

    sddm-greeter-qt6 --test-mode --theme ~/.local/share/sddm-ashwa-min/ashwa-min
