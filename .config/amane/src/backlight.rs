// Laptop panel brightness, through the kernel backlight class.
//
// Replaces the upstream ddc.rs, which drove an external monitor over DDC/CI
// with ddcutil. This machine has no i2c display and no ddcutil, so that path
// could never read a value and the whole brightness UI sat disabled.
//
// Reads are free: /sys/class/backlight/<device>/brightness is world-readable,
// so the current level costs one small file read and no process. Only a change
// shells out, to brightnessctl, because the sysfs file is root-owned.
//
//   amane ipc call brightness up
//   amane ipc call brightness down
//   amane ipc call brightness 40

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::thread;
use std::time::Duration;

use amane::Service;

use crate::island::{Island, Osd};

const STEP: i32 = 5;

// the worker naps longer while idle; a key press unparks it so nothing lags
static WORKER: OnceLock<thread::Thread> = OnceLock::new();

// Never let the screen go fully black: the same floor the old waybar binding
// used (brightnessctl -n2).
const FLOOR: i32 = 2;

const UNKNOWN: i32 = -1;

static WANTED: AtomicI32 = AtomicI32::new(UNKNOWN);

static DEVICE: LazyLock<Option<PathBuf>> = LazyLock::new(find_device);

/// Prefer the integrated panel, else whatever the kernel exposes first.
fn find_device() -> Option<PathBuf> {
    let root = Path::new("/sys/class/backlight");
    let mut names: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    names.sort();
    names
        .iter()
        .find(|p| p.file_name().is_some_and(|n| n == "intel_backlight"))
        .or_else(|| names.first())
        .cloned()
}

fn number(path: &Path) -> Option<i32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Current level as a percentage of maximum, straight from sysfs.
fn read() -> Option<i32> {
    let device = DEVICE.as_ref()?;
    let maximum = number(&device.join("max_brightness"))?;
    if maximum <= 0 {
        return None;
    }
    let current = number(&device.join("brightness"))?;
    Some((current * 100 / maximum).clamp(0, 100))
}

pub fn percent() -> Option<u8> {
    let value = WANTED.load(Ordering::Relaxed);
    (value != UNKNOWN).then_some(value.clamp(0, 100) as u8)
}

pub fn start() {
    let handle = thread::spawn(worker);
    let _ = WORKER.set(handle.thread().clone());
}

const BRIGHTNESS_USAGE: &str = "usage: brightness up|down|show|status|<0-100>";

pub fn brightness(arguments: &[String]) -> String {
    let Some(argument) = arguments.first() else {
        return String::from(BRIGHTNESS_USAGE);
    };

    let current = WANTED.load(Ordering::Relaxed);

    if current == UNKNOWN {
        return String::from("no backlight device under /sys/class/backlight");
    }

    if argument == "status" {
        return current.to_string();
    }

    if argument == "show" {
        Island::write().show_osd(Osd::Brightness, current as u8, false);
        return current.to_string();
    }

    let next = match argument.as_str() {
        "up" => current + STEP,
        "down" => current - STEP,
        number => match number.parse::<i32>() {
            Ok(number) => number,
            Err(_) => return String::from(BRIGHTNESS_USAGE),
        },
    }
    .clamp(FLOOR, 100);

    WANTED.store(next, Ordering::Relaxed);
    if let Some(worker) = WORKER.get() {
        worker.unpark();
    }
    Island::write().show_osd(Osd::Brightness, next as u8, false);
    next.to_string()
}

fn worker() {
    let Some(mut applied) = read() else {
        eprintln!("backlight: no readable device under /sys/class/backlight");
        return;
    };
    WANTED.store(applied, Ordering::Relaxed);

    // 120 ms while keys are being pressed, then 500 ms while nothing changes
    let mut last_change = std::time::Instant::now();
    loop {
        let recent = last_change.elapsed() < Duration::from_secs(3);
        thread::park_timeout(Duration::from_millis(if recent { 120 } else { 500 }));

        let wanted = WANTED.load(Ordering::Relaxed);
        if wanted != applied {
            // Keys pressed quickly collapse into one call, as before.
            amane::output(&format!("brightnessctl -q set {wanted}%"));
            applied = wanted;
            last_change = std::time::Instant::now();
            continue;
        }

        // Pick up changes made elsewhere: XF86 keys, hypridle dimming, or
        // anything else writing the same device.
        if let Some(actual) = read() {
            if actual != applied {
                applied = actual;
                WANTED.store(actual, Ordering::Relaxed);
                last_change = std::time::Instant::now();
            }
        }
    }
}
