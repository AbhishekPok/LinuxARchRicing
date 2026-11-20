-- Environment
hl.env("LIBVA_DRIVER_NAME", "iHD")
hl.env("MOZ_ENABLE_WAYLAND", "1")
hl.env("QT_QPA_PLATFORM", "wayland")
hl.env("XDG_SESSION_TYPE", "wayland")
hl.env("XDG_CURRENT_DESKTOP", "Hyprland")
hl.env("XCURSOR_SIZE", "24")
hl.env("XCURSOR_THEME", "Bibata-Modern-Classic")
hl.env("GTK_THEME", "catppuccin-mocha-green-standard+default")
-- SDDM starts Hyprland with the bare system PATH (zsh adds these only for
-- terminals), so amane couldn't find theme-set and keybinds calling `amane`
-- failed after a reboot. Prepend once; a config reload must not stack them.
do
    local home = os.getenv("HOME")
    local path = os.getenv("PATH") or "/usr/local/bin:/usr/bin"
    for _, dir in ipairs({ home .. "/.cargo/bin", home .. "/.local/bin" }) do
        if not (":" .. path .. ":"):find(":" .. dir .. ":", 1, true) then
            path = dir .. ":" .. path
        end
    end
    hl.env("PATH", path)
end

-- Monitor layout
--
-- Placement is derived from ~/.config/hypr/.monitor-state, because `hyprctl
-- reload` re-evaluates this file. The previous version hardcoded position
-- "0x0" on BOTH outputs, so every reload stacked the external on top of the
-- laptop and any arrangement chosen from monitor-switch.sh was lost.
--
-- auto-left / auto-right let Hyprland compute the offset itself, so there is
-- no logical-width arithmetic to keep in sync with resolution or scale.
--
-- With two or more externals the dual layout applies instead, saved in
-- .monitor-state-dual: hdmi_left (default: HDMI left of the laptop, USB-C/DP
-- right), hdmi_right (swapped) or external (laptop off). Kept apart from
-- .monitor-state so unplugging one screen falls back to the single layout.
local LAPTOP       = "eDP-1"
local LAPTOP_MODE  = "1920x1200@60"
local LAPTOP_SCALE = "1.20"

local function read_state(file, default)
    local f = io.open(os.getenv("HOME") .. "/.config/hypr/" .. file, "r")
    if not f then return default end
    local s = f:read("*l")
    f:close()
    if s == nil or s == "" then return default end
    return (s:gsub("%s+$", ""))
end

-- Connected externals from sysfs: hyprctl can't be queried while the config
-- is being evaluated. Names match Hyprland's (card1-HDMI-A-1 -> HDMI-A-1).
local function connected_externals()
    local list = {}
    local p = io.popen("grep -l '^connected' /sys/class/drm/card*-*/status 2>/dev/null")
    if not p then return list end
    for path in p:lines() do
        local name = path:match("card%d+%-(.-)/status$")
        if name and not name:match("^eDP") and not name:match("^LVDS") then
            table.insert(list, name)
        end
    end
    p:close()
    return list
end

local externals = connected_externals()
local EXT = externals[1] or "HDMI-A-1"
local layout = read_state(".monitor-state", "laptop_left")

if #externals >= 2 then
    local dual = read_state(".monitor-state-dual", "hdmi_left")
    local hdmi_side, other_side = "auto-left", "auto-right"
    if dual == "hdmi_right" then hdmi_side, other_side = "auto-right", "auto-left" end

    if dual == "external" then
        hl.monitor({ output = LAPTOP, mode = "disable", position = "0x0", scale = "1.0" })
    else
        hl.monitor({ output = LAPTOP, mode = LAPTOP_MODE, position = "0x0", scale = LAPTOP_SCALE })
    end
    for _, out in ipairs(externals) do
        local side = out:match("^HDMI") and hdmi_side or other_side
        hl.monitor({ output = out, mode = "preferred", position = side, scale = "1.0" })
    end
elseif layout == "external" then
    hl.monitor({ output = LAPTOP, mode = "disable", position = "0x0", scale = "1.0" })
    hl.monitor({ output = EXT,    mode = "preferred", position = "0x0", scale = "1.0" })
else
    hl.monitor({ output = LAPTOP, mode = LAPTOP_MODE, position = "0x0", scale = LAPTOP_SCALE })

    if layout == "laptop" then
        hl.monitor({ output = EXT, mode = "disable", position = "0x0", scale = "1.0" })
    elseif layout == "mirror" then
        hl.monitor({ output = EXT, mode = "preferred", position = "0x0", scale = "1.0", mirror = LAPTOP })
    elseif layout == "ext_left" then
        hl.monitor({ output = EXT, mode = "preferred", position = "auto-left", scale = "1.0" })
    else -- laptop_left (default)
        hl.monitor({ output = EXT, mode = "preferred", position = "auto-right", scale = "1.0" })
    end
end

-- Monitor: Fallback for any other display (hot-plug safe)
hl.monitor({
    output = "",
    mode = "preferred",
    position = "auto",
    scale = "1.0",
})

-- Programs
local terminal = "kitty"
-- amane's launcher replaced rofi: apps, plus keyword modes (clip, wall, emoji,
-- win, proj, edit, aws, audio, display, keys, kitty) and "s <query>" for the web
local menu = "~/.cargo/bin/amane ipc call auxiliary search"

-- Config
hl.config({
    general = {
        -- Geometry ported from adrsha/nix-dots mango.conf: gap_inner 4,
        -- gap_outer 20, border_px 0. The windows sit well inside the screen
        -- edge, which is what gives that layout its airy look.
        gaps_in = 4,
        gaps_out = 20,
        -- 0 to match mango. The old green gradient border is still defined
        -- below; set this back to 2 to bring it back.
        border_size = 0,
        resize_on_border = true,
        col = {
            active_border   = { colors = { "rgba(00ff99ff)", "rgba(00e5ffff)" }, angle = 45 },
            inactive_border = "rgba(1a1a1aff)",
        },
        layout = "dwindle",
    },
    -- Window "candies" borrowed from dave-eska/Nyxrn: unfocused windows fade a
    -- little, the background darkens behind a special workspace (scratchpad,
    -- dropterm, focus) and blur gets vibrancy. 0.92 rather than Nyxrn's 0.87
    -- because kitty is already translucent and the two multiply.
    decoration = {
        -- mango.conf border_radius=5
        rounding = 5,
        active_opacity   = 1.0,
        inactive_opacity = 0.92,
        dim_special      = 0.5,
        shadow = {
            enabled      = true,
            range        = 8,
            render_power = 3,
            color        = "rgba(000000aa)",
        },
        -- mango.conf: 3 passes, radius 5, noise 0.02, brightness 0.9,
        -- contrast 0.9, saturation 1.2. Hyprland spells saturation "vibrancy".
        blur = {
            enabled    = true,
            size       = 5,
            passes     = 3,
            noise      = 0.02,
            brightness = 0.9,
            contrast   = 0.9,
            vibrancy   = 0.2,
        },
    },
    -- Tabbed window groups (SUPER + G), coloured to match the green border
    group = {
        col = {
            border_active   = { colors = { "rgba(00ff99ff)", "rgba(00e5ffff)" }, angle = 45 },
            border_inactive = "rgba(1a1a1aff)",
        },
        groupbar = {
            font_size  = 11,
            height     = 16,
            gradients  = true,
            text_color = "rgba(0a0a0aff)",
            col = {
                active   = "rgba(00ff99dd)",
                inactive = "rgba(1a1a1add)",
            },
        },
    },
    animations = {
        enabled = true,
    },
    input = {
        kb_layout = "us",
        follow_mouse = 1,
        touchpad = {
            natural_scroll = true,
        },
    },
    dwindle = {
        preserve_split = true,
    },
    misc = {
        disable_hyprland_logo = true,
        -- if the lock screen (amane's, see ~/.config/amane/src/lock.rs) ever
        -- dies while locked, another locker may take over instead of leaving
        -- the red "lockdead" screen: from a TTY run
        --   hyprctl --instance 0 dispatch 'hl.dsp.exec_cmd("hyprlock")'
        allow_session_lock_restore = true,
    },
    cursor = {
        no_hardware_cursors = true,
    },
})

------------------------------
---- ANIMATIONS / CURVES ----
------------------------------

hl.curve("easeOutQuint", {
	type = "bezier",
	points = {
		{ 0.23, 1 },
		{ 0.32, 1 },
	},
})

hl.curve("linear", {
	type = "bezier",
	points = {
		{ 0, 0 },
		{ 1, 1 },
	},
})

hl.curve("almostLinear", {
	type = "bezier",
	points = {
		{ 0.5, 0.5 },
		{ 0.75, 1 },
	},
})

hl.curve("quick", {
	type = "bezier",
	points = {
		{ 0.15, 0 },
		{ 0.1, 1 },
	},
})

hl.curve("easy", {
	type = "spring",
	mass = 1,
	stiffness = 238.1191,
	dampening = 24.21279333,
})

-- Curves and durations from adrsha/nix-dots mango.conf. Hyprland's "speed" is
-- in deciseconds, so mango's milliseconds map directly: open 400 -> 4,
-- move 500 -> 5, close 800 -> 8, tag 350 -> 3.5.
hl.curve("mangoOpen", {
	type = "bezier",
	points = {
		{ 0.46, 1.0 },
		{ 0.29, 1.1 },
	},
})

hl.curve("mangoMove", {
	type = "bezier",
	points = {
		{ 0.46, 1.0 },
		{ 0.29, 1 },
	},
})

hl.curve("mangoClose", {
	type = "bezier",
	points = {
		{ 0.08, 0.92 },
		{ 0, 1 },
	},
})

hl.animation({ leaf = "global",        enabled = true, speed = 10,   bezier = "default" })
hl.animation({ leaf = "border",        enabled = true, speed = 5.39, bezier = "easeOutQuint" })
hl.animation({ leaf = "windows",       enabled = true, speed = 5,    bezier = "mangoMove" })
hl.animation({ leaf = "windowsIn",     enabled = true, speed = 4,    bezier = "mangoOpen",  style = "slide" })
hl.animation({ leaf = "windowsOut",    enabled = true, speed = 8,    bezier = "mangoClose", style = "slide" })
hl.animation({ leaf = "fadeIn",        enabled = true, speed = 1.73, bezier = "almostLinear" })
hl.animation({ leaf = "fadeOut",       enabled = true, speed = 1.46, bezier = "almostLinear" })
hl.animation({ leaf = "fade",          enabled = true, speed = 3.03, bezier = "quick" })
hl.animation({ leaf = "layers",        enabled = true, speed = 3.81, bezier = "easeOutQuint" })
hl.animation({ leaf = "layersIn",      enabled = true, speed = 4,    bezier = "easeOutQuint", style = "fade" })
hl.animation({ leaf = "layersOut",     enabled = true, speed = 1.5,  bezier = "linear", style = "fade" })
hl.animation({ leaf = "fadeLayersIn",  enabled = true, speed = 1.79, bezier = "almostLinear" })
hl.animation({ leaf = "fadeLayersOut", enabled = true, speed = 1.39, bezier = "almostLinear" })
-- Workspaces slide instead of fade, on Nyxrn's "wind" curve (overshoots a
-- touch, so the new workspace settles rather than stops). Special workspaces
-- drop in vertically, and the gradient border sweeps once on focus change.
hl.curve("wind", {
	type = "bezier",
	points = {
		{ 0.05, 0.9 },
		{ 0.1, 1.05 },
	},
})
hl.animation({ leaf = "workspaces",       enabled = true, speed = 3.5, bezier = "mangoMove", style = "slide" })
hl.animation({ leaf = "specialWorkspace", enabled = true, speed = 5,  bezier = "wind", style = "slidevert" })
hl.animation({ leaf = "borderangle",      enabled = true, speed = 30, bezier = "linear", style = "once" })

hl.gesture({
    fingers = 3,
    direction = "horizontal",
    action = "workspace",
})

-- Autostart
hl.on("hyprland.start", function()
    -- The desktop shell is amane (~/.config/amane, git). `amane run` starts it
    -- under a crash supervisor; its stderr goes to ~/.cache/amane/amane.log
    -- (previous run: amane.log.1). Waybar is retired; its config is in git
    -- history and the trash.
    hl.exec_cmd("~/.cargo/bin/amane run")
    -- Absolute path so this does not depend on ~/.local/bin being on the
    -- session PATH. It is there today, so the bare name works too.
    hl.exec_cmd("~/.local/bin/theme-daemon")
    hl.exec_cmd("~/.config/hypr/scripts/clipboard-watch.sh")
    -- Refresh the static keybind map (~/.config/hypr/keybinds.md) each login
    hl.exec_cmd("~/.config/hypr/scripts/keybinds.sh --write")
    hl.exec_cmd("/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1")
    hl.exec_cmd("nm-applet --indicator")
    hl.exec_cmd("blueman-applet")
    -- Wallpaper (static layer; see ~/.config/hyprlax/hyprlax.toml)
    hl.exec_cmd("hyprlax -c ~/.config/hyprlax/hyprlax.toml")
    hl.exec_cmd("hypridle")
    hl.exec_cmd("~/.config/hypr/scripts/monitor-restore.sh")
    hl.exec_cmd("~/.config/hypr/scripts/wallpaper-rotate.sh")
    hl.exec_cmd("~/.config/hypr/scripts/monitor-watch.sh")
    hl.exec_cmd("sleep 3 && ~/.local/bin/kbb-scrolllock.sh on")
    hl.exec_cmd("gnome-keyring-daemon --start --components=secrets")
    hl.exec_cmd("~/.config/hypr/scripts/hyprsunset.sh init")
    -- Low-battery notifications at 20% / 10%, suspend at 5% (from roscaa-stack)
    hl.exec_cmd("~/.config/hypr/scripts/battery-notify.sh")
    -- battery: power-saver profile, no blur/shadows, brightness <= 50% (AC restores)
    hl.exec_cmd("~/.config/hypr/scripts/power-mode.sh")
end)

-- Keybinds
local mod = "SUPER"

hl.bind(mod .. " + Return", hl.dsp.exec_cmd(terminal))
hl.bind(mod .. " + Space", hl.dsp.exec_cmd(menu))
hl.bind(mod .. " + Q", hl.dsp.window.close())
hl.bind(mod .. " + M", hl.dsp.exit())
hl.bind(mod .. " + V", hl.dsp.window.float({ action = "toggle" }))
hl.bind(mod .. " + F", hl.dsp.window.fullscreen({ mode = 1 }))
hl.bind("Scroll_Lock", hl.dsp.exec_cmd("~/.local/bin/kbb-scrolllock.sh toggle"))


-- Focus with vim keys
hl.bind(mod .. " + h", hl.dsp.focus({ direction = "left" }))
hl.bind(mod .. " + l", hl.dsp.focus({ direction = "right" }))
hl.bind(mod .. " + k", hl.dsp.focus({ direction = "up" }))
hl.bind(mod .. " + j", hl.dsp.focus({ direction = "down" }))

-- Workspaces 1-9
for i = 1, 9 do
    hl.bind(mod .. " + " .. tostring(i), hl.dsp.focus({ workspace = i }))
    hl.bind(mod .. " + SHIFT + " .. tostring(i), hl.dsp.window.move({ workspace = i }))
end

-- Scroll workspaces
hl.bind(mod .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }))
hl.bind(mod .. " + mouse_up", hl.dsp.focus({ workspace = "e-1" }))

-- Mouse move/resize
hl.bind(mod .. " + mouse:272", hl.dsp.window.drag(), { mouse = true })
hl.bind(mod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })

-- Fn keys
hl.bind("XF86AudioMute", hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"), { locked = true })
hl.bind("XF86AudioLowerVolume", hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"), { locked = true, repeating = true })
-- -l 2 allows software boost up to 200%; -l 1 previously clamped every
-- raise back down to 100% even if volume had been set higher elsewhere.
hl.bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd("wpctl set-volume -l 2 @DEFAULT_AUDIO_SINK@ 5%+"), { locked = true, repeating = true })
hl.bind("XF86AudioMicMute", hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"), { locked = true })

-- Monitor switcher
hl.bind(mod .. " + P", hl.dsp.exec_cmd("~/.config/hypr/scripts/monitor-switch.sh"))

-- Wallpaper switcher
hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("brightnessctl -e4 -n2 set 5%-"), { locked = true, repeating = true })
hl.bind("XF86MonBrightnessUp", hl.dsp.exec_cmd("brightnessctl -e4 -n2 set 5%+"), { locked = true, repeating = true })
hl.bind("Print", hl.dsp.exec_cmd("~/.config/hypr/scripts/screenshot.sh --now"))

-- Window rules
hl.window_rule({
    name = "suppress-maximize",
    match = { class = ".*" },
    suppress_event = "maximize",
})


-- Hardware keys
hl.bind("XF86Bluetooth", hl.dsp.exec_cmd("~/.config/hypr/scripts/bluetooth-toggle.sh"))
hl.bind("F6", hl.dsp.exec_cmd("~/.config/hypr/scripts/bluetooth-toggle.sh"))
hl.bind("SHIFT + F6", hl.dsp.exec_cmd("blueman-manager"))
hl.bind("F7", hl.dsp.exec_cmd("~/.config/hypr/scripts/power-profile.sh"))
hl.bind("SUPER + F5", hl.dsp.exec_cmd("~/.config/hypr/scripts/gamemode.sh"))

-- Extra binds
hl.bind("SUPER + SHIFT + L", hl.dsp.exec_cmd("~/.config/hypr/scripts/lock.sh"))
hl.bind("SUPER + ALT + I", hl.dsp.exec_cmd("amane ipc call awake toggle"))  -- Keep Awake

-- Audio output switcher
hl.bind("SUPER + A", hl.dsp.exec_cmd("~/.config/hypr/scripts/audio-switch.sh"))
-- amane. SUPER+SHIFT+A opens the Control Center (SUPER+A stays the audio
-- switcher above; both used to fire on one key). SUPER+SHIFT+B flips the
-- menu bar between auto-hide and always shown. SUPER+X is the utility
-- toolbar (screenshots + power), SUPER+Space is Spotlight.
hl.bind("SUPER + SHIFT + A", hl.dsp.exec_cmd("~/.cargo/bin/amane ipc call settings"))
hl.bind("SUPER + SHIFT + B", hl.dsp.exec_cmd("~/.cargo/bin/amane ipc call bar toggle"))
hl.bind("SUPER + backslash", hl.dsp.exec_cmd("~/.cargo/bin/amane ipc call media play-pause"))
hl.bind("SUPER + SHIFT + S", hl.dsp.exec_cmd("~/.config/hypr/scripts/screenshot.sh --area"))
hl.bind("SUPER + SHIFT + Print", hl.dsp.exec_cmd("~/.config/hypr/scripts/screenshot.sh --active"))
hl.bind("SUPER + ALT + S", hl.dsp.exec_cmd("~/.config/hypr/scripts/screenshot.sh --swappy"))

-- Scratchpad workspace
hl.bind("SUPER + S", hl.dsp.workspace.toggle_special("scratchpad"))
hl.bind("SUPER + CTRL + S", hl.dsp.window.move({ workspace = "special:scratchpad" }))


-- Clipboard history
-- Started from the autostart block below (see hyprland.start): run here at
-- parse time the watchers came up before the Wayland socket and died.
hl.bind(mod .. " + SHIFT + V", hl.dsp.exec_cmd("/home/abhishek/.config/hypr/scripts/clipboard.sh"))

-- Notification center toggle
hl.bind("SUPER + N", hl.dsp.exec_cmd("~/.cargo/bin/amane ipc call settings history"))
hl.bind("SUPER + SHIFT + N", hl.dsp.exec_cmd("~/.config/hypr/scripts/hyprsunset.sh toggle"))

-- File manager
hl.bind("SUPER + E", hl.dsp.exec_cmd("nemo"))

-- amane menu bar: frosted glass behind its translucent fill
hl.layer_rule({
	name = "amane-bar-blur",
	match = {
		namespace = "^amane-bar$",
	},
	blur = true,
	ignore_alpha = 0.3,
})

-- amane notch, Control Center, banners and HUD: the same frosted glass
hl.layer_rule({
	name = "amane-island-blur",
	match = {
		namespace = "^island$",
	},
	blur = true,
	ignore_alpha = 0.5,
})

-- amane Spotlight + toolbar: frosted glass behind the cards only
hl.layer_rule({
	name = "amane-auxiliary-blur",
	match = {
		namespace = "^amane-auxiliary$",
	},
	blur = true,
	ignore_alpha = 0.5,
})

-- Dropdown terminal (SUPER + grave)
hl.window_rule({
    name      = "dropterm",
    match     = { class = "^dropterm$" },
    float     = true,
    size      = "(monitor_w*0.65) (monitor_h*0.65)",
    move      = "(monitor_w*0.175) (monitor_h*0.10)",
    workspace = "special:dropterm",
})
hl.workspace_rule({ workspace = "special:dropterm", animation = "slidevert" })
hl.bind("SUPER + grave", hl.dsp.exec_cmd("~/.config/hypr/scripts/dropdown.sh"))



-------------------------
---- COMMUNITY-RICE EXTRAS
-------------------------
-- Features common to the public Hyprland rices (HyDE, JaKooLit, ML4W, Omarchy)
-- that this config was missing. All additive: no existing bind was changed.

-- Window switcher across workspaces (the launcher's "win" mode). On SHIFT + Tab
-- so plain SUPER + Tab can alternate between the last two windows, which is
-- what Tab does in every other WM and is the faster move for two terminals.
hl.bind("SUPER + SHIFT + Tab", hl.dsp.exec_cmd("~/.config/hypr/scripts/windows.sh"))

-- Utility panel, bottom right: screenshots + power in one row. Keyboard:
-- w r f (window/region/screen), l z o b p (lock/sleep/logout/reboot/poweroff),
-- arrows + Enter, Esc. Pressing it again closes it.
hl.bind("SUPER + X", hl.dsp.exec_cmd("~/.cargo/bin/amane ipc call auxiliary utility"))

-- Wallpaper picker, the launcher's "wall" mode (rotator keeps running; this just sets one)
hl.bind("SUPER + W", hl.dsp.exec_cmd("~/.config/hypr/scripts/wallpaper-picker.sh"))

-- Searchable keybind cheat sheet ("keys" mode), generated from this file
hl.bind("SUPER + slash", hl.dsp.exec_cmd("~/.config/hypr/scripts/keybinds.sh"))

-- Do-not-disturb toggle for notifications
hl.bind("SUPER + SHIFT + D", hl.dsp.exec_cmd("~/.cargo/bin/amane ipc call notifications dnd toggle"))

-- Picture-in-Picture: float, keep on top and out of the tiling layout, so a
-- video follows you across workspaces instead of being tiled away.
hl.window_rule({
    name  = "pip-float",
    match = { title = "^(Picture[- ]in[- ][Pp]icture)$" },
    float = true,
    pin   = true,
    size  = "(monitor_w*0.25) (monitor_h*0.25)",
    move  = "(monitor_w*0.73) (monitor_h*0.70)",
})


-------------------------
---- KEYBOARD WINDOW CONTROL
-------------------------
-- Gaps found by auditing keybinds.md against the config: the window could only
-- be moved or resized with the mouse (SUPER + mouse:272 / :273), workspaces
-- could only be stepped through by scrolling, and there was no way back to the
-- previous window. All of that matters here because the shell history is almost
-- entirely terminal work (aws, ssh, git), so a workspace holds several kitty
-- windows that get rearranged by hand.
--
-- Key scheme, so it stays guessable:
--   SUPER            + hjkl  focus   (already present)
--   SUPER + SHIFT    + hjkl  move the window   (SHIFT = "bring it along",
--                                               same as SUPER + SHIFT + 1-9)
--   SUPER + CTRL     + hjkl  resize
hl.bind(mod .. " + SHIFT + h", hl.dsp.window.move({ direction = "left" }))
hl.bind(mod .. " + SHIFT + l", hl.dsp.window.move({ direction = "right" }))
hl.bind(mod .. " + SHIFT + k", hl.dsp.window.move({ direction = "up" }))
hl.bind(mod .. " + SHIFT + j", hl.dsp.window.move({ direction = "down" }))

-- Layout is dwindle with preserve_split, so these shift the split ratio.
-- repeating = true lets the key be held down instead of tapped.
hl.bind(mod .. " + CTRL + h", hl.dsp.window.resize({ x = -40, y = 0, relative = true }), { repeating = true })
hl.bind(mod .. " + CTRL + l", hl.dsp.window.resize({ x = 40, y = 0, relative = true }), { repeating = true })
hl.bind(mod .. " + CTRL + k", hl.dsp.window.resize({ x = 0, y = -40, relative = true }), { repeating = true })
hl.bind(mod .. " + CTRL + j", hl.dsp.window.resize({ x = 0, y = 40, relative = true }), { repeating = true })

-- Alternate between the last two windows (the window switcher is SHIFT + Tab)
hl.bind(mod .. " + Tab", hl.dsp.focus({ last = true }))

-- SUPER + F is fullscreen mode 1 (maximized, keeps gaps and the bar); this is
-- the real edge-to-edge one.
hl.bind(mod .. " + SHIFT + F", hl.dsp.window.fullscreen({ mode = "fullscreen" }))

-- Pin a floating window above the others on every workspace. Same effect the
-- pip-float rule applies automatically, available by hand for any window.
hl.bind(mod .. " + SHIFT + P", hl.dsp.window.pin())

-- Recenter a floating window that was dragged half off-screen
hl.bind(mod .. " + C", hl.dsp.window.center())

-- Flip the next dwindle split between vertical and horizontal
hl.bind(mod .. " + T", hl.dsp.layout("togglesplit"))

-- Workspace 10: the loop above only covers 1-9, leaving the 0 key unused
hl.bind(mod .. " + 0", hl.dsp.focus({ workspace = 10 }))
hl.bind(mod .. " + SHIFT + 0", hl.dsp.window.move({ workspace = 10 }))

-- Keyboard equivalent of SUPER + scroll
hl.bind(mod .. " + CTRL + Left", hl.dsp.focus({ workspace = "e-1" }), { repeating = true })
hl.bind(mod .. " + CTRL + Right", hl.dsp.focus({ workspace = "e+1" }), { repeating = true })


-------------------------
---- BORROWED FROM dave-eska/Nyxrn
-------------------------

-- Tabbed groups. SUPER + G turns the focused window into a group (or breaks
-- it up); SUPER + ALT + hjkl pushes the window into the neighbouring group, or
-- moves it like SUPER + SHIFT + hjkl when there is none.
hl.bind(mod .. " + G", hl.dsp.group.toggle())
hl.bind(mod .. " + ALT + Tab", hl.dsp.group.next())
hl.bind(mod .. " + ALT + h", hl.dsp.window.move({ into_group = "left" }))
hl.bind(mod .. " + ALT + l", hl.dsp.window.move({ into_group = "right" }))
hl.bind(mod .. " + ALT + k", hl.dsp.window.move({ into_group = "up" }))
hl.bind(mod .. " + ALT + j", hl.dsp.window.move({ into_group = "down" }))
hl.bind(mod .. " + SHIFT + G", hl.dsp.window.move({ out_of_group = true }))

-- Focus workspace (Z for "zen"): wide margins turn a lone browser or editor
-- into a centred card. Nyxrn does this on workspace 1; here it is its own
-- workspace so 1 keeps its normal tiling.
hl.workspace_rule({
    workspace = "name:zen",
    gaps_in   = 0,
    gaps_out  = { top = 60, right = 220, bottom = 60, left = 220 },
})
hl.bind(mod .. " + Z", hl.dsp.focus({ workspace = "name:zen" }))
hl.bind(mod .. " + SHIFT + Z", hl.dsp.window.move({ workspace = "name:zen" }))

-- Emoji picker, copies to the clipboard
hl.bind(mod .. " + period", hl.dsp.exec_cmd("~/.config/hypr/scripts/emoji.sh"))

-- Project launcher: git repo -> tmux session (nvim | git | shell) in kitty
hl.bind(mod .. " + O", hl.dsp.exec_cmd("~/.config/hypr/scripts/project.sh"))

-- Floating kitty at a chosen size
hl.bind(mod .. " + ALT + Return", hl.dsp.exec_cmd("~/.config/hypr/scripts/kitty-size.sh"))
local kitty_sizes = {
    box    = "(monitor_w*0.30) (monitor_h*0.45)",
    small  = "(monitor_w*0.45) (monitor_h*0.40)",
    medium = "(monitor_w*0.75) (monitor_h*0.70)",
    large  = "(monitor_w*0.87) (monitor_h*0.87)",
}
for name, size in pairs(kitty_sizes) do
    hl.window_rule({
        name   = "kitty-" .. name,
        match  = { class = "^kitty-" .. name .. "$" },
        float  = true,
        size   = size,
        center = true,
    })
end


-------------------------
---- BORROWED FROM THE hyprland-arch TOP 15
-------------------------

-- Web search with engine prefixes: gh, ghc, tf, tfm, aw, aur, yt, mdn
-- (SherLock707); focuses the browser afterwards
hl.bind(mod .. " + ALT + W", hl.dsp.exec_cmd("~/.config/hypr/scripts/websearch.sh"))

-- Open a rice config file in nvim (JaKooLit QuickEdit)
hl.bind(mod .. " + ALT + C", hl.dsp.exec_cmd("~/.config/hypr/scripts/quickedit.sh"))


-------------------------
---- FROM THE hyprland-dotfiles TOPIC
-------------------------

-- Production guard: a window whose title carries [PROD] gets a red border.
-- The zsh title hook (~/.zshrc, "Window title + production marker") adds the
-- tag when a command, directory or AWS profile looks like production, and
-- tmux passes pane titles up (set-titles in ~/.tmux.conf). Rules re-evaluate
-- on every title change, so the border turns red when `ssh my-prod-host`
-- starts and back at the next local prompt.
hl.window_rule({
    name         = "prod-border",
    match        = { title = ".*\\[PROD\\].*" },
    border_color = "rgba(ff3e3eff) rgba(ff8c00ff) 45deg",
})

-- Workspace identity (idea from Candys2000/Themeswitcher-hyprland): some
-- workspaces get their own accent, and zen also turns on Do Not Disturb.
-- Unlisted ones keep the green/cyan accent.
--
-- The per-workspace *wallpaper* was removed on 2026-10-26: hyprlax draws one
-- layer across all monitors, so a workspace wallpaper changed the image on
-- every screen at once. The hourly rotation (wallpaper-rotate.sh) now owns the
-- wallpaper outright. To bring it back, add `wall = "file.png"` to an entry;
-- workspace-theme.sh still understands it.
local WS_ACCENT_DEFAULT = { "rgba(00ff99ff)", "rgba(00e5ffff)" }
local WS_THEMES = {
    ["2"] = { accent = { "rgba(4da6ffff)", "rgba(00e5ffff)" } },
    ["3"] = { accent = { "rgba(ffb000ff)", "rgba(ff6a00ff)" } },
    ["4"] = { accent = { "rgba(bc51ffff)", "rgba(00e5ffff)" } },
    zen   = { accent = { "rgba(d4d4d4ff)", "rgba(6a6a6aff)" } },
}

-- The neon per-workspace accents belong to the kali-neon theme; any other
-- theme keeps the border colour theme-manager wrote into colors.lua.
local function theme_name()
    local f = io.open(os.getenv("HOME") .. "/.config/hypr/theme_env.lua")
    if not f then return "" end
    local text = f:read("*a")
    f:close()
    return text:match('THEME_NAME", "([^"]+)"') or ""
end

local ws_theme_current = nil
local function apply_ws_theme(ws)
    if not ws or ws.name == ws_theme_current then return end
    if theme_name() ~= "kali-neon" then
        ws_theme_current = ws.name
        hl.exec_cmd("~/.config/hypr/scripts/workspace-theme.sh - " .. (ws.name == "zen" and "zen" or "-"))
        return
    end
    ws_theme_current = ws.name
    local theme = WS_THEMES[ws.name] or {}
    local grad  = { colors = theme.accent or WS_ACCENT_DEFAULT, angle = 45 }
    hl.config({
        general = { col = { active_border = grad } },
        group   = { col = { border_active = grad } },
    })
    hl.exec_cmd("~/.config/hypr/scripts/workspace-theme.sh "
        .. (theme.wall or "-") .. " " .. (ws.name == "zen" and "zen" or "-"))
end

-- A workspace already showing on the other monitor only fires monitor.focused
-- when you switch to it, so both events are needed. A reload resets the
-- accent from hl.config above, hence the re-apply.
hl.on("workspace.active", apply_ws_theme)
hl.on("monitor.focused", function(mon)
    apply_ws_theme(mon and mon.active_workspace)
end)
hl.on("config.reloaded", function()
    ws_theme_current = nil
    apply_ws_theme(hl.get_active_workspace())
end)

-- tmux window switcher with live previews (idea from fiffeek/.dotfiles.v2).
-- SUPER + O opens a project, SUPER + SHIFT + O jumps into a running one.
hl.window_rule({
    name   = "kitty-tmuxswitch",
    match  = { class = "^kitty-tmuxswitch$" },
    float  = true,
    size   = "(monitor_w*0.80) (monitor_h*0.70)",
    center = true,
})
hl.bind(mod .. " + SHIFT + O", hl.dsp.exec_cmd("~/.config/hypr/scripts/tmux-switch.sh"))

-- theme-manager. Both files are generated; they must load *after* the
-- hl.config above so the active theme's colours win. colors.lua sets
-- border_size, rounding and the border colours; theme_env.lua exports
-- THEME_* and GTK_THEME to apps launched from the compositor.
require("colors")
require("theme_env")

-- ---------------------------------------------------------------------------
-- mac desktop (amane "Graphite Glass", 12 Nov 2026). Loads after colors.lua
-- so it survives theme switches; the per-workspace accent still colours the
-- focused border. To roll back, delete this block (or restore
-- hyprland.lua.bak.2026-11-12-macdesk) and `hyprctl reload`.
-- the theme's accent (THEME_BLUE from theme_env.lua), e.g. macOS blue on
-- graphite; theme-manager itself writes the same dark grey for both borders
local function theme_accent()
    local f = io.open(os.getenv("HOME") .. "/.config/hypr/theme_env.lua")
    if not f then return "0a84ff" end
    local text = f:read("*a")
    f:close()
    return (text:match('THEME_BLUE", "#(%x%x%x%x%x%x)"') or "0a84ff"):lower()
end

hl.config({
    general = {
        gaps_in     = 5,     -- 10px between windows
        gaps_out    = 10,    -- 10px to the screen edge (the bar auto-hides)
        border_size = 2,     -- the focused window gets the accent ring
        col = {
            active_border   = "rgba(" .. theme_accent() .. "ee)",
            inactive_border = "rgba(ffffff14)",
        },
    },
    decoration = {
        rounding       = 12,
        rounding_power = 3.0, -- squircle-ish, closer to macOS' continuous corners
        shadow = {
            enabled      = true,
            range        = 30,
            render_power = 3,
            offset       = "0 12",
            color        = "rgba(00000073)",
            color_inactive = "rgba(00000040)",
        },
    },
})
