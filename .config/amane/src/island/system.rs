//! CPU load, package temperature and memory use for the menu bar: waybar's old `group/system`
//! and `group/storage` (cpu, temperature, memory). Read straight from /proc and
//! the coretemp hwmon, no fork, every 3s. Same thresholds as the waybar config:
//! cpu 75/90 %, memory 75/85 %, temperature critical at 80 C, where a
//! thermometer takes the cpu cell (waybar's format-critical).
use super::*;



const POLL: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct System {
    cpu: u8,
    memory: u8,
    temperature: u32,
}

impl Service for System {
    fn new() -> Self {
        Self::default()
    }
    fn listen() {
        let sensor = coretemp();
        let mut last = cpu_times();
        loop {
            std::thread::sleep(POLL);
            let now = cpu_times();
            let cpu = match (last, now) {
                (Some((busy0, total0)), Some((busy1, total1))) if total1 > total0 => {
                    (100 * (busy1 - busy0) / (total1 - total0)).min(100) as u8
                }
                _ => 0,
            };
            last = now;
            let next = System {
                cpu,
                memory: memory_percent().unwrap_or(0),
                temperature: sensor
                    .as_ref()
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .map_or(0, |milli| milli / 1000),
            };
            if *Self::read() != next {
                *Self::write() = next;
            }
        }
    }
}

/// (busy, total) jiffies from the aggregate line of /proc/stat.
fn cpu_times() -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let fields: Vec<u64> = stat
        .lines()
        .next()?
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    let total: u64 = fields.iter().sum();
    // idle + iowait
    let idle = fields.get(3)? + fields.get(4).copied().unwrap_or(0);
    Some((total - idle, total))
}

fn memory_percent() -> Option<u8> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let field = |name: &str| {
        info.lines()
            .find(|l| l.starts_with(name))?
            .split_whitespace()
            .nth(1)?
            .parse::<u64>()
            .ok()
    };
    let total = field("MemTotal:")?;
    let available = field("MemAvailable:")?;
    Some((100 * (total - available) / total.max(1)) as u8)
}

/// temp1_input of the coretemp hwmon, the same sensor waybar read; the hwmonN
/// number changes between boots, so it is looked up once at start.
fn coretemp() -> Option<std::path::PathBuf> {
    std::fs::read_dir("/sys/devices/platform/coretemp.0/hwmon")
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path().join("temp1_input"))
        .find(|p| p.exists())
}




/// (cpu %, memory %, package temperature C), for the menu bar.
pub(super) fn snapshot() -> (u8, u8, u32) {
    let s = *System::read();
    (s.cpu, s.memory, s.temperature)
}
