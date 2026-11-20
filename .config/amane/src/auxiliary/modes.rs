//! The launcher's keyword modes and built-in actions. Typing `<keyword> `
//! switches the search pill from applications to that mode's list; this is
//! what replaced rofi. Each mode is a script under ~/.config/hypr/scripts that
//! speaks one small protocol:
//!
//!   <script> [list args] --menu        rows: label<TAB>value[<TAB>image]
//!   <script> [prefix] --menu-pick V    act on a row; may print OPEN<TAB>query
//!                                      to reopen the launcher on a submenu
//!   <script> [prefix] --menu-delete V  drop a row (Ctrl+X, modes that allow it)
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub(super) struct Mode {
    pub keyword: &'static str,
    pub title: &'static str,
    pub glyph: &'static str,
    script: &'static str,
    /// argv between the script and `--menu` / `--menu-pick`
    list: &'static [&'static str],
    prefix: &'static [&'static str],
    /// submenus are reached through OPEN only, never suggested
    pub hidden: bool,
    pub delete: bool,
}

const fn mode(
    keyword: &'static str,
    title: &'static str,
    glyph: &'static str,
    script: &'static str,
) -> Mode {
    Mode {
        keyword,
        title,
        glyph,
        script,
        list: &[],
        prefix: &[],
        hidden: false,
        delete: false,
    }
}

const fn display(keyword: &'static str, list: &'static [&'static str]) -> Mode {
    Mode {
        list,
        hidden: true,
        ..mode(keyword, "Display", "\u{e30c}", "monitor-switch.sh")
    }
}

pub(super) const MODES: &[Mode] = &[
    Mode {
        delete: true,
        ..mode("clip", "Clipboard history", "\u{e14f}", "clipboard.sh")
    },
    mode("emoji", "Emoji", "\u{e24e}", "emoji.sh"),
    mode("wall", "Wallpapers", "\u{e3f4}", "wallpaper-picker.sh"),
    mode("win", "Open windows", "\u{e8d8}", "windows.sh"),
    mode("proj", "Projects (tmux)", "\u{e2c7}", "project.sh"),
    mode("edit", "Edit a config file", "\u{e3c9}", "quickedit.sh"),
    Mode {
        list: &["aws-pick"],
        prefix: &["aws-pick"],
        ..mode("aws", "AWS profile terminal", "\u{e2bd}", "ops.sh")
    },
    Mode {
        list: &["kube"],
        prefix: &["kube"],
        ..mode("kube", "Kubernetes context", "\u{e9f4}", "devops.sh")
    },
    Mode {
        list: &["ssm"],
        prefix: &["ssm"],
        ..mode("ssm", "SSM session (AWS profile)", "\u{eb8e}", "devops.sh")
    },
    Mode {
        list: &["gh"],
        prefix: &["gh"],
        ..mode("gh", "GitHub", "\u{e86f}", "devops.sh")
    },
    mode("audio", "Audio output", "\u{e32d}", "audio-switch.sh"),
    mode("display", "Displays", "\u{e30c}", "monitor-switch.sh"),
    display("display-res", &["--menu", "res"]),
    display("display-scale", &["--menu", "scale"]),
    display("display-rot", &["--menu", "rot"]),
    mode("keys", "Keybinds", "\u{e312}", "keybinds.sh"),
    mode("kitty", "Kitty at a size", "\u{eb8e}", "kitty-size.sh"),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Act {
    Shot(&'static str),
    /// runs at once
    Session(&'static str),
    /// opens the utility panel with Confirm armed
    Guarded(&'static str),
    Script(&'static str, &'static [&'static str]),
}

pub(super) struct Action {
    pub label: &'static str,
    pub glyph: &'static str,
    pub act: Act,
}

pub(super) const ACTIONS: &[Action] = &[
    Action { label: "Screenshot region", glyph: "\u{e3be}", act: Act::Shot("region") },
    Action { label: "Screenshot window", glyph: "\u{e1b0}", act: Act::Shot("window") },
    Action { label: "Screenshot screen", glyph: "\u{e30a}", act: Act::Shot("full") },
    Action { label: "Lock", glyph: "\u{e897}", act: Act::Session("lock") },
    Action { label: "Sleep", glyph: "\u{ef44}", act: Act::Session("sleep") },
    Action { label: "Log out", glyph: "\u{e9ba}", act: Act::Guarded("logout") },
    Action { label: "Restart", glyph: "\u{e5d5}", act: Act::Guarded("restart") },
    Action { label: "Shut down", glyph: "\u{e8ac}", act: Act::Guarded("shutdown") },
    Action { label: "Power profile (cycle)", glyph: "\u{ea0b}", act: Act::Script("power-profile.sh", &[]) },
    Action { label: "Wipe clipboard history", glyph: "\u{e14f}", act: Act::Script("clipboard.sh", &["--wipe"]) },
];

#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub label: String,
    lower: String,
    pub value: String,
    pub image: Option<PathBuf>,
}

pub(super) fn script(name: &str) -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".config/hypr/scripts")
        .join(name)
}

/// The mode a query is in, and the filter text after its keyword.
pub(super) fn active(query: &str) -> Option<(&'static Mode, &str)> {
    let (word, rest) = query.split_once(' ')?;
    MODES.iter().find(|m| m.keyword == word).map(|m| (m, rest))
}

pub(super) fn find(keyword: &str) -> Option<&'static Mode> {
    MODES.iter().find(|m| m.keyword == keyword)
}

/// Runs the mode's `--menu`. Blocking: called from a loader thread.
pub(super) fn load(mode: &Mode) -> Vec<Entry> {
    let mut args: Vec<&str> = mode.list.to_vec();
    if !args.contains(&"--menu") {
        args.push("--menu");
    }
    let Ok(out) = Command::new(script(mode.script))
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return vec![];
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let label = f.next()?.trim();
            let value = f.next().unwrap_or(label);
            (!label.is_empty()).then(|| Entry {
                label: label.to_string(),
                lower: label.to_lowercase(),
                value: value.to_string(),
                image: f.next().filter(|p| !p.is_empty()).map(PathBuf::from),
            })
        })
        .collect()
}

/// Every word of the filter must appear in the label, in any order.
pub(super) fn filter<'a>(entries: &'a [Entry], text: &str) -> Vec<&'a Entry> {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    entries
        .iter()
        .filter(|e| words.iter().all(|w| e.lower.contains(w.as_str())))
        .collect()
}

/// argv for `--menu-pick` / `--menu-delete` on a row.
pub(super) fn argv(mode: &Mode, flag: &str, value: &str) -> Vec<String> {
    let mut argv = vec![script(mode.script).to_string_lossy().into_owned()];
    argv.extend(mode.prefix.iter().map(|s| s.to_string()));
    argv.push(flag.into());
    argv.push(value.into());
    argv
}

pub(super) fn delete(mode: &Mode, value: &str) {
    let argv = argv(mode, "--menu-delete", value);
    let _ = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Built-in actions matching a plain query (two letters or more, so a single
/// keystroke never puts Shut down at the top).
pub(super) fn actions(query: &str) -> Vec<&'static Action> {
    let needle = query.trim().to_lowercase();
    if needle.len() < 2 {
        return vec![];
    }
    ACTIONS
        .iter()
        .filter(|a| {
            let label = a.label.to_lowercase();
            label.starts_with(&needle) || label.split(' ').any(|w| w.starts_with(&needle))
        })
        .collect()
}

/// Modes whose keyword starts with the query, or whose title contains it.
pub(super) fn suggest(query: &str) -> Vec<&'static Mode> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return vec![];
    }
    MODES
        .iter()
        .filter(|m| {
            !m.hidden
                && (m.keyword.starts_with(&needle) || m.title.to_lowercase().contains(&needle))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyword_needs_its_space() {
        assert!(active("clip").is_none());
        assert_eq!(active("clip foo").map(|(m, r)| (m.keyword, r)), Some(("clip", "foo")));
        assert_eq!(active("display-res ").map(|(m, _)| m.keyword), Some("display-res"));
        assert!(active("spotify music").is_none());
    }
    #[test]
    fn one_letter_never_offers_power_actions() {
        assert!(actions("s").is_empty());
        assert_eq!(actions("shut")[0].label, "Shut down");
        assert_eq!(actions("reg")[0].label, "Screenshot region");
    }
}
