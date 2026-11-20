#!/usr/bin/env bash
# keybinds.sh — searchable cheat sheet of every Hyprland bind (SUPER + /).
# ML4W and HyDE both ship a keybind viewer; this one reads hyprland.lua itself,
# so it can never drift out of date the way a hand-written list would.
#
#   --list    print to stdout
#   --menu    launcher rows ("keys " in the amane launcher, SUPER + /)
set -u

CONF="$HOME/.config/hypr/hyprland.lua"

render() {
python3 - "$CONF" "$@" <<'PY'
import re, sys

src = open(sys.argv[1], encoding="utf-8").read()
mod = (re.search(r'^\s*local\s+mod\s*=\s*"([^"]+)"', src, re.M) or [None, "SUPER"])[1]

def keys(raw):
    # 'mod .. " + SHIFT + V"' / '"SUPER + A"' / 'mod .. " + " .. tostring(i)'
    out = []
    for part in raw.split(".."):
        part = part.strip()
        if part == "mod":
            out.append(mod)
        elif part.startswith('"'):
            out.append(part.strip('"'))
        elif "tostring" in part:
            out.append("1-9")
    return re.sub(r"\s*\+\s*", " + ", "".join(out)).strip(" +")

# Resolve simple `local x = "..."` variables so binds show the real command
# (exec_cmd(terminal) -> kitty) instead of the variable name.
LOCALS = dict(re.findall(r'^\s*local\s+(\w+)\s*=\s*"([^"]+)"', src, re.M))

def action(rest):
    m = re.search(r'exec_cmd\(([A-Za-z_]\w*)\)', rest)
    if m and m.group(1) in LOCALS:
        return re.sub(r'^.*/scripts/', '', LOCALS[m.group(1)])
    m = re.search(r'exec_cmd\("([^"]+)"', rest)
    if m:
        cmd = m.group(1)
        cmd = re.sub(r'^~?/?(home/\w+/)?\.config/hypr/scripts/', '', cmd)
        return cmd
    m = re.search(r'hl\.dsp\.([\w.]+)\(([^)]*)\)', rest)
    if m:
        args = re.sub(r'[{}"]', '', m.group(2)).strip()
        return f"{m.group(1)}" + (f" ({args})" if args else "")
    return rest.strip().rstrip("),")[:60]

# Broad categories, matched against the action (and the key for a few cases).
# Deriving them here rather than from hyprland.lua's comment headers keeps the
# groups stable and few: comments produced 18 sections, several holding one bind.
# First match wins, so order matters.
CATEGORIES = [
    ("Launch & Apps",      r"kitty|auxiliary search|auxiliary utility|windows\.sh|nemo|dropdown\.sh"),
    ("Clipboard",          r"clipboard"),
    ("Screenshots",        r"screenshot"),
    ("Notifications",      r"settings history|notifications dnd"),
    ("Workspaces",         r"workspace"),
    ("Windows",            r"^window\.|^focus|^layout|fullscreen|float|drag|resize"),
    ("Display & Wallpaper",r"monitor-|wallpaper|hyprsunset|brightnessctl|MonBrightness"),
    ("Audio",              r"wpctl|audio-switch|Audio"),
    ("Hardware & Power",   r"bluetooth|blueman|power-profile|gamemode|kbb-scrolllock|Bluetooth"),
    ("Session & Lock",     r"hyprlock|hypridle|lock\.sh|dispatch exit"),
    ("Help",               r"keybinds\.sh"),
]

def category(k, a):
    hay = f"{a} {k}"
    for name, pat in CATEGORIES:
        if re.search(pat, hay, re.I):
            return name
    return "Other"

ORDER = [name for name, _ in CATEGORIES] + ["Other"]

rows = []
for line in src.splitlines():
    m = re.match(r'\s*hl\.(bind[\w_]*)\(\s*(.+?),\s*(hl\.dsp\..*)$', line)
    if not m:
        continue
    k, a = keys(m.group(2)), action(m.group(3))
    if k:
        rows.append((category(k, a), k, a))

seen, dedup = set(), []
for sec, k, a in rows:
    if (k, a) in seen:
        continue
    seen.add((k, a))
    dedup.append((sec, k, a))

# Group, preserving the order binds appear in the config within each category.
out = [r for name in ORDER for r in dedup if r[0] == name]

if "--markdown" in sys.argv:
    print("# Hyprland keybinds\n")
    print(f"Generated from `~/.config/hypr/hyprland.lua` — {len(out)} binds. "
          "Regenerate with `~/.config/hypr/scripts/keybinds.sh --write`.\n")
    last = None
    for sec, k, a in out:
        if sec != last:
            print(f"\n## {sec}\n")
            print("| Keys | Action |")
            print("|---|---|")
            last = sec
        print(f"| `{k}` | {a} |")
else:
    for sec, k, a in out:
        print(f"{k:<26} {a:<52} {sec}")
PY
}

OUTFILE="$HOME/.config/hypr/keybinds.md"

case "${1:-}" in
    --list) render; exit 0 ;;
    # nothing to run: the value is the line itself, picking just closes
    --menu) render | awk '{ print $0 "\t-" }'; exit 0 ;;
    --menu-pick) exit 0 ;;
    --markdown) render --markdown; exit 0 ;;
    --write)
        render --markdown > "${2:-$OUTFILE}"
        echo "wrote ${2:-$OUTFILE} ($(grep -c '^| `' "${2:-$OUTFILE}") binds)"
        exit 0 ;;
esac

exec ~/.cargo/bin/amane ipc call auxiliary search keys
