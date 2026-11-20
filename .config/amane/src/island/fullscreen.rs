//! Event-driven fullscreen state, not a window-size heuristic.
//!
//! Ported from Mango to Hyprland. fullscreen.py follows Hyprland's event
//! stream and prints 1 or 0; decoding the client list needs real JSON, which
//! this crate has no dependency for, exactly as in clients.rs. Hyprland's
//! `fullscreen` is an enum (0 none, 1 maximized, 2 fullscreen) and only 2
//! hides the island.
use amane::Service;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

struct Fullscreen {
    hidden: bool,
}
impl Service for Fullscreen {
    fn new() -> Self {
        Self { hidden: false }
    }
    fn listen() {
        loop {
            let Ok(mut child) = Command::new("python3")
                .args(["-u", "-c", include_str!("fullscreen.py")])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            else {
                return;
            };
            if let Some(stdout) = child.stdout.take() {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    match line.trim() {
                        "1" => set(true),
                        "0" => set(false),
                        _ => {}
                    }
                }
            }
            let _ = child.wait();
            set(false);
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
fn set(hidden: bool) {
    if Fullscreen::read().hidden != hidden {
        Fullscreen::write().hidden = hidden;
    }
}
pub(crate) fn blocked() -> bool {
    Fullscreen::read().hidden && !super::visibility::over_fullscreen()
}
pub(super) fn hidden() -> bool {
    blocked() || super::exposure::tucked()
}
