//! AWS profile, VPN and open SSH/SSM sessions, from the same
//! ~/.config/hypr/scripts/ops.sh, for the menu bar's ops chips.
//!
//! The script's `amane-watch` mode runs the three existing event-driven
//! watchers on one stream and prints `<kind>\t<class>\t<text>\t<tooltip>`.
//! Nothing polls: aws follows Hyprland's window-title events, vpn follows
//! netlink, ssh is a fork-free /proc scan every 3s inside the script.
use super::*;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

#[derive(Clone, Default, PartialEq)]
struct Entry {
    class: String,
    text: String,
}

impl Entry {
    /// Nothing to show: the script reports "none"/"off" when a slot is empty.
    fn idle(&self) -> bool {
        self.class.is_empty() || self.class == "none" || self.class == "off"
    }
}

/// One slot's live entry. The menu bar draws it as a chip; nothing animates
/// here any more, so the slot is just the data.
struct Slot {
    entry: Entry,
}

impl Slot {
    fn new() -> Self {
        Self { entry: Entry::default() }
    }

    fn set(&mut self, entry: Entry) {
        self.entry = entry;
    }
}

pub(super) struct Ops {
    aws: Slot,
    vpn: Slot,
    ssh: Slot,
}

impl Service for Ops {
    fn new() -> Self {
        Self {
            aws: Slot::new(),
            vpn: Slot::new(),
            ssh: Slot::new(),
        }
    }
    fn listen() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let script = std::path::PathBuf::from(home).join(".config/hypr/scripts/ops.sh");
        loop {
            let Ok(mut child) = Command::new(&script)
                .arg("amane-watch")
                // its own group, so ops.sh can end every watcher it started
                .process_group(0)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            else {
                return;
            };
            if let Some(stdout) = child.stdout.take() {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let mut fields = line.split('\t');
                    let (Some(kind), Some(class), Some(text)) =
                        (fields.next(), fields.next(), fields.next())
                    else {
                        continue;
                    };
                    let entry = Entry {
                        class: class.to_string(),
                        text: label(text),
                    };
                    let changed = {
                        let state = Self::read();
                        match kind {
                            "aws" => state.aws.entry != entry,
                            "vpn" => state.vpn.entry != entry,
                            "ssh" => state.ssh.entry != entry,
                            _ => false,
                        }
                    };
                    if changed {
                        let mut state = Self::write();
                        match kind {
                            "aws" => state.aws.set(entry),
                            "vpn" => state.vpn.set(entry),
                            "ssh" => state.ssh.set(entry),
                            _ => {}
                        }
                    }
                }
            }
            let _ = child.wait();
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

/// The script's text is written for waybar, so it starts with a Nerd Font
/// glyph the island's UI font has no coverage for. Keep the words, draw the
/// icon here instead.
fn label(text: &str) -> String {
    text.trim_start_matches(|c: char| !c.is_ascii())
        .trim()
        .to_string()
}




/// The live, non-idle slots as (kind, class, text), for the menu bar chips.
pub(super) fn chips() -> Vec<(&'static str, String, String)> {
    let ops = Ops::read();
    [("aws", &ops.aws), ("vpn", &ops.vpn), ("ssh", &ops.ssh)]
        .into_iter()
        .filter(|(_, slot)| !slot.entry.idle())
        .map(|(kind, slot)| (kind, slot.entry.class.clone(), slot.entry.text.clone()))
        .collect()
}
