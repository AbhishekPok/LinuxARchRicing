//! Keep Awake (waybar's idle_inhibitor): while on, a systemd `idle`
//! inhibitor is held, which hypridle honours, so the screen does not
//! auto-lock, blank or suspend. Closing the lid and manual lock still work.
//!
//! The inhibitor is its own process (own process group, pid in
//! $XDG_RUNTIME_DIR), so it outlives an amane restart and the state is read
//! back from it; it never outlives the login session.
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

const WHO: &str = "amane-keep-awake";
static ON: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

fn pid_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    dir.join("amane-keep-awake.pid")
}

/// The inhibitor's pid, if it is still running (checked against its
/// command line so a reused pid is not mistaken for it).
fn running() -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(pid_path()).ok()?.trim().parse().ok()?;
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    String::from_utf8_lossy(&cmdline).contains(WHO).then_some(pid)
}

pub(crate) fn on() -> bool {
    INIT.call_once(|| ON.store(running().is_some(), Ordering::Relaxed));
    ON.load(Ordering::Relaxed)
}

pub(crate) fn set(want: bool) {
    on(); // make sure the state was read once
    let current = running();
    if want && current.is_none() {
        let child = Command::new("systemd-inhibit")
            .args(["--what=idle", &format!("--who={WHO}"), "--why=Keep Awake", "--mode=block"])
            .args(["sleep", "infinity"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
        match child {
            Ok(mut child) => {
                let _ = std::fs::write(pid_path(), format!("{}\n", child.id()));
                // reap it when it is stopped, so no zombie is left behind
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(_) => return,
        }
    } else if !want {
        if let Some(pid) = current {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        let _ = std::fs::remove_file(pid_path());
    }
    ON.store(want, Ordering::Relaxed);
    amane::request_frame();
}

fn announce() {
    let (title, body) = if on() {
        ("Keep Awake on", "The screen won't auto-lock, turn off or sleep")
    } else {
        ("Keep Awake off", "Auto-lock is back on")
    };
    let _ = Command::new("notify-send").args(["-a", "Keep Awake", "-u", "low", title, body]).spawn();
}

/// `awake on|off|toggle|status`
pub fn command(args: &[String]) -> String {
    match args.first().map(String::as_str) {
        Some("on") => set(true),
        Some("off") => set(false),
        Some("toggle") => set(!on()),
        Some("status") | None => return if on() { "on".into() } else { "off".into() },
        _ => return "usage: awake on|off|toggle|status".into(),
    }
    announce();
    if on() { "on".into() } else { "off".into() }
}
