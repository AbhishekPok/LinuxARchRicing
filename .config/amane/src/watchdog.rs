//! Restarts the shell if its main thread stops answering. IPC is served on the
//! main thread, so an unanswered call means the windows and every keybind are
//! dead too, while this thread and the services keep running.
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EVERY: Duration = Duration::from_secs(10);
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
// In a row, so one slow answer around a suspend or a heavy frame is forgiven.
const STRIKES: u32 = 3;
const PR_SET_PTRACER: i32 = 0x59616d61;

unsafe extern "C" {
    fn prctl(option: i32, ...) -> i32;
}

pub fn start() {
    thread::spawn(|| {
        let mut strikes = 0;
        loop {
            thread::sleep(EVERY);
            strikes = if answers() { 0 } else { strikes + 1 };
            if strikes >= STRIKES {
                recover();
                return;
            }
        }
    });
}

fn answers() -> bool {
    let Ok(mut stream) = UnixStream::connect(amane::ipc_socket()) else {
        // Nothing is listening yet, which is startup and not a freeze.
        return true;
    };
    let mut reply = [0; 1];
    stream.set_read_timeout(Some(ANSWER_WITHIN)).is_ok()
        && stream.write_all(b"auxiliary\nstatus\n").is_ok()
        && stream.shutdown(Shutdown::Write).is_ok()
        && stream.read(&mut reply).is_ok_and(|n| n > 0)
}

fn recover() {
    eprintln!("amane: main thread stopped answering, restarting");
    record_stacks();
    let amane = home().join(".cargo/bin/amane");
    let mut restart = Command::new(&amane);
    restart.arg("restart");
    // A killed lock client leaves the session locked with nothing to unlock it,
    // so hand it to hyprlock; see the lock screen notes.
    if amane::Lock::active() {
        restart = Command::new("sh");
        restart.arg("-c").arg(format!(
            "{} restart; hyprctl dispatch 'hl.dsp.exec_cmd(\"hyprlock\")'",
            amane.display()
        ));
    }
    let _ = restart
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Leaves the evidence a restart destroys. Yama only lets a process trace its
/// descendants, so this process names the debugger as allowed first.
fn record_stacks() {
    let folder = home().join(".cache/amane");
    let _ = fs::create_dir_all(&folder);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |t| t.as_secs());
    let Ok(file) = File::create(folder.join(format!("freeze-{stamp}.log"))) else {
        return;
    };
    let Ok(mut child) = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "sleep 0.2; exec timeout 30 gdb -p {} -batch -q -ex 'set debuginfod enabled off' -ex 'thread apply all bt'",
            std::process::id()
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    unsafe { prctl(PR_SET_PTRACER, child.id() as std::ffi::c_ulong) };
    let _ = child.wait();
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}
