//! Charger plugged in / pulled out, like macOS: a small glass card in the
//! top-right corner ("Charging · 67%" / "On Battery · 66%") and, on plug-in
//! only, the system's power-plug chime.
//!
//! A kernel uevent socket (what udev listens on) wakes this the moment a power
//! supply changes; firmware that sends no such event is still caught by a
//! re-check every second (one tiny sysfs read). No extra process.
use super::*;
use std::os::fd::{FromRawFd, OwnedFd};

const SHOW_FOR: Duration = Duration::from_millis(2500);
const FADE: f32 = 0.26;
const SIZE: (f32, f32) = (240.0, 52.0);
const CHIME: &str = "/usr/share/sounds/freedesktop/stereo/power-plug.oga";

// power-supply uevents seen, for `amane ipc call power status`
static EVENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Power {
    on_ac: Option<bool>,
    changed: Option<Instant>,
    // low battery: the level last warned at (100 = none since the charger
    // was last in) and the warning being shown
    warned: u8,
    low: Option<(u8, Instant)>,
}

// macOS-style low battery warnings while discharging; 5% (suspend) stays with
// ~/.config/hypr/scripts/battery-notify.sh
const LOW_AT: [u8; 2] = [20, 10];
const LOW_FOR: Duration = Duration::from_secs(6);
const WARNING: &str = "/usr/share/sounds/freedesktop/stereo/dialog-warning.oga";

impl Service for Power {
    fn new() -> Self {
        Self { on_ac: on_ac(), changed: None, warned: 100, low: None }
    }
    // only keeps the card's fade-out drawing; detection is in listen()
    fn interval() -> Duration {
        Duration::from_millis(250)
    }
    fn update(&mut self) -> bool {
        self.check_low();
        self.changed.is_some_and(|t| t.elapsed() < SHOW_FOR + Duration::from_secs(1))
            || self.low.is_some_and(|(_, t)| t.elapsed() < LOW_FOR + Duration::from_secs(1))
    }
    fn listen() {
        let socket = uevents();
        loop {
            match &socket {
                // returns as soon as a power event arrives, or after 1 s
                Some(fd) => wait_for_power_event(fd),
                None => std::thread::sleep(Duration::from_secs(1)),
            }
            check();
        }
    }
}

impl Power {
    /// Warns once at each of LOW_AT while discharging; the charger resets it.
    fn check_low(&mut self) {
        let (present, percent, charging) = {
            let b = amane::Battery::read();
            (b.present(), b.percent(), b.charging() || b.full())
        };
        if !present {
            return;
        }
        if charging || self.on_ac == Some(true) {
            self.warned = 100;
            self.low = None;
            return;
        }
        let Some(level) = LOW_AT.iter().copied().find(|l| percent <= *l && self.warned > *l) else {
            return;
        };
        self.warned = level;
        self.low = Some((percent, Instant::now()));
        if level <= 10 && std::path::Path::new(WARNING).exists() {
            amane::spawn(&format!("pw-play {WARNING} >/dev/null 2>&1"));
        }
    }
}

/// Re-reads the supplies and, on a change, shows the card (and chimes on plug).
fn check() {
    let Some(now) = on_ac() else { return };
    let was = Power::read().on_ac;
    if was == Some(now) {
        return;
    }
    // the chime first: it is what tells you the plug is seated
    if was.is_some() && now && std::path::Path::new(CHIME).exists() {
        amane::spawn(&format!("pw-play {CHIME} >/dev/null 2>&1"));
    }
    let mut p = Power::write();
    if p.on_ac.is_some() {
        p.changed = Some(Instant::now());
    }
    p.on_ac = Some(now);
}

const AF_NETLINK: i32 = 16;
const SOCK_DGRAM: i32 = 2;
const SOCK_CLOEXEC: i32 = 0o2000000;
const NETLINK_KOBJECT_UEVENT: i32 = 15;
const POLLIN: i16 = 1;

#[repr(C)]
struct SockaddrNl {
    nl_family: u16,
    nl_pad: u16,
    nl_pid: u32,
    nl_groups: u32,
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn socket(domain: i32, kind: i32, protocol: i32) -> i32;
    fn bind(fd: i32, addr: *const SockaddrNl, len: u32) -> i32;
    fn recv(fd: i32, buf: *mut u8, len: usize, flags: i32) -> isize;
    fn poll(fds: *mut PollFd, n: u64, timeout_ms: i32) -> i32;
}

/// The kernel's uevent broadcast (group 1), the same feed udev reads.
fn uevents() -> Option<OwnedFd> {
    // SAFETY: plain socket/bind calls; the fd is owned from here on
    unsafe {
        let fd = socket(AF_NETLINK, SOCK_DGRAM | SOCK_CLOEXEC, NETLINK_KOBJECT_UEVENT);
        if fd < 0 {
            return None;
        }
        let owned = OwnedFd::from_raw_fd(fd);
        let addr = SockaddrNl { nl_family: AF_NETLINK as u16, nl_pad: 0, nl_pid: 0, nl_groups: 1 };
        if bind(fd, &addr, std::mem::size_of::<SockaddrNl>() as u32) < 0 {
            return None;
        }
        Some(owned)
    }
}

/// Blocks until a power_supply uevent arrives, or a second passes.
fn wait_for_power_event(fd: &OwnedFd) {
    use std::os::fd::AsRawFd;
    let mut buf = [0u8; 8192];
    loop {
        let mut pfd = PollFd { fd: fd.as_raw_fd(), events: POLLIN, revents: 0 };
        // SAFETY: one valid pollfd for the duration of the call
        let ready = unsafe { poll(&mut pfd, 1, 1_000) };
        if ready <= 0 {
            return; // timeout (the periodic re-check) or an interrupted call
        }
        // SAFETY: buf outlives the call and its length is passed
        let n = unsafe { recv(fd.as_raw_fd(), buf.as_mut_ptr(), buf.len(), 0) };
        if n <= 0 {
            return;
        }
        let message = &buf[..n as usize];
        // "change@/devices/.../power_supply/ADP1\0...SUBSYSTEM=power_supply\0..."
        const WANT: &[u8] = b"SUBSYSTEM=power_supply\0";
        if message.windows(WANT.len()).any(|w| w == WANT) {
            EVENTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    }
}

/// Any mains supply (ADP1, AC, ACAD...) online.
fn on_ac() -> Option<bool> {
    let mut seen = false;
    for entry in std::fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let path = entry.path();
        if std::fs::read_to_string(path.join("type")).is_ok_and(|t| t.trim() == "Mains") {
            seen = true;
            if std::fs::read_to_string(path.join("online")).is_ok_and(|o| o.trim() == "1") {
                return Some(true);
            }
        }
    }
    seen.then_some(false)
}

/// The corner card at `y`, while it should show, and how present it is (0..1).
pub(super) fn card(screen_width: f32, y: f32) -> Option<(Rectangle, f32)> {
    let (on_ac, changed, low) = {
        let p = Power::read();
        (p.on_ac, p.changed, p.low)
    };
    let recent_change = changed.filter(|t| t.elapsed().as_secs_f32() < SHOW_FOR.as_secs_f32() + FADE);
    let (on_ac, changed) = match (on_ac, recent_change) {
        (Some(on_ac), Some(changed)) => (on_ac, changed),
        _ => return low.and_then(|(percent, at)| low_card(screen_width, y, percent, at)),
    };
    let age = changed.elapsed().as_secs_f32();
    let total = SHOW_FOR.as_secs_f32();
    if age >= total + FADE {
        return None;
    }
    amane::request_frame();
    let presence = if age < 0.12 {
        age / 0.12
    } else if age > total {
        1.0 - (age - total) / FADE
    } else {
        1.0
    }
    .clamp(0.0, 1.0);

    let percent = {
        let b = amane::Battery::read();
        if !b.present() {
            return None;
        }
        b.percent()
    };
    let p = crate::theme::palette();
    let level = if on_ac { p.green } else if percent <= 15 { p.red } else { p.foreground };
    let title = if on_ac { "Charging" } else { "On Battery" };
    let card = Rectangle::new()
        .width(SIZE.0)
        .height(SIZE.1)
        .translate(screen_width - 10.0 - SIZE.0, y - 6.0 * (1.0 - presence))
        .radius(crate::theme::RADIUS)
        .fill(crate::theme::glass())
        .border(1.0, crate::theme::hairline())
        .opacity(presence)
        .padding(Padding { left: 16.0, right: 16.0, top: 0.0, bottom: 0.0 })
        .align_child(Start, Center)
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(30.0)
                    .height(16.0)
                    .align_child(Center, Center)
                    .child(symbols::battery(percent, on_ac, p.foreground, level)),
                Text::new(title).size(13.5).weight(Weight::SemiBold).color(p.foreground),
                Text::new(format!("{percent}%")).size(13.0).color(p.secondary)
            ])
            .gap(10.0)
            .align(Center),
        );
    Some((card, presence))
}

/// `amane ipc call power [preview|status]`: preview shows the card for the
/// current state (no chime), status prints it.
pub fn command(args: &[String]) -> String {
    match args.first().map(String::as_str) {
        Some("preview") if args.get(1).is_some_and(|a| a == "low") => {
            let percent = amane::Battery::read().percent();
            Power::write().low = Some((percent, Instant::now()));
            "ok".into()
        }
        Some("preview") => {
            let mut p = Power::write();
            if p.on_ac.is_none() {
                p.on_ac = on_ac();
            }
            p.changed = Some(Instant::now());
            "ok".into()
        }
        Some(other) if other != "status" => {
            "usage: power status | preview [low]".into()
        }
        _ => {
            let state = match on_ac() {
                Some(true) => "ac",
                Some(false) => "battery",
                None => "no mains supply",
            };
            let events = EVENTS.load(std::sync::atomic::Ordering::Relaxed);
            format!("{state} (power events seen: {events})")
        }
    }
}

/// "Low Battery · 20%" (amber) / "· 10%" (red), for LOW_FOR.
fn low_card(screen_width: f32, y: f32, percent: u8, at: Instant) -> Option<(Rectangle, f32)> {
    let age = at.elapsed().as_secs_f32();
    let total = LOW_FOR.as_secs_f32();
    if age >= total + FADE {
        return None;
    }
    amane::request_frame();
    let presence = if age < 0.12 { age / 0.12 } else if age > total { 1.0 - (age - total) / FADE } else { 1.0 }
        .clamp(0.0, 1.0);
    let p = crate::theme::palette();
    let level = if percent <= 10 { p.red } else { p.yellow };
    let card = Rectangle::new()
        .width(SIZE.0 + 40.0)
        .height(SIZE.1)
        .translate(screen_width - 10.0 - SIZE.0 - 40.0, y - 6.0 * (1.0 - presence))
        .radius(crate::theme::RADIUS)
        .fill(crate::theme::glass())
        .border(1.0, Color::rgba(level.red(), level.green(), level.blue(), 110))
        .opacity(presence)
        .padding(Padding { left: 16.0, right: 16.0, top: 0.0, bottom: 0.0 })
        .align_child(Start, Center)
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(30.0)
                    .height(16.0)
                    .align_child(Center, Center)
                    .child(symbols::battery(percent, false, p.foreground, level)),
                Text::new("Low Battery").size(13.5).weight(Weight::SemiBold).color(p.foreground),
                Text::new(format!("{percent}% remaining")).size(13.0).color(level)
            ])
            .gap(10.0)
            .align(Center),
        );
    Some((card, presence))
}

/// macOS-style battery detail from the battery's own counters: "2:46 left ·
/// 9.8 W", "1:05 until full", "Fully charged". None without a battery.
pub(super) fn battery_detail() -> Option<String> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?.flatten().map(|e| e.path()).find(|p| {
        std::fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Battery")
    })?;
    let read = |name: &str| -> Option<f64> {
        std::fs::read_to_string(dir.join(name)).ok()?.trim().parse::<f64>().ok()
    };
    let status = std::fs::read_to_string(dir.join("status")).unwrap_or_default();
    // energy (uWh / uW) when the firmware reports it, else charge (uAh / uA)
    let (now, full, rate) = match (read("energy_now"), read("energy_full"), read("power_now")) {
        (Some(n), Some(f), Some(r)) => (n, f, r),
        _ => (read("charge_now")?, read("charge_full")?, read("current_now")?),
    };
    let watts = match (read("power_now"), read("current_now"), read("voltage_now")) {
        (Some(p), _, _) => p / 1e6,
        (None, Some(c), Some(v)) => c * v / 1e12,
        _ => 0.0,
    };
    let hm = |hours: f64| {
        let minutes = (hours * 60.0).round() as i64;
        format!("{}:{:02}", minutes / 60, minutes % 60)
    };
    Some(match status.trim() {
        "Full" => "Fully charged".to_string(),
        "Charging" if rate > 0.0 => format!("{} until full · {:.1} W", hm((full - now).max(0.0) / rate), watts),
        "Discharging" if rate > 0.0 => format!("{} left · {:.1} W", hm(now / rate), watts),
        "Not charging" => "Plugged in, not charging".to_string(),
        _ => "Calculating…".to_string(),
    })
}
