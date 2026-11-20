//! Live Activities in the notch: long commands wrapped with `track`
//! (~/.local/bin/track) report start and finish over IPC, and the notch shows
//! the newest one instead of the time:
//!
//!   running   ring + "terraform apply" + elapsed (redraws once a second)
//!   ok        check + label + duration, for 5 s
//!   failed    cross + label + duration, until the notch is clicked
//!
//!   amane ipc call activity start <label...>      -> prints the job id
//!   amane ipc call activity done <id> ok|fail
//!   amane ipc call activity clear | status
use super::*;
use amane::Shape;
use std::sync::atomic::{AtomicU64, Ordering};

/// Notch width while an activity shows (fits inside the bar's NOTCH_SLOT).
pub(super) const WIDTH: f32 = 300.0;
const OK_FOR: Duration = Duration::from_secs(5);
const FAIL_FOR: Duration = Duration::from_secs(600);

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq)]
enum State {
    Running,
    Ok,
    Failed,
}

#[derive(Clone)]
struct Job {
    id: u64,
    label: String,
    started: Instant,
    ended: Option<Instant>,
    state: State,
}

impl Job {
    fn elapsed(&self) -> Duration {
        self.ended.unwrap_or_else(Instant::now).duration_since(self.started)
    }
    fn expired(&self) -> bool {
        match (self.state, self.ended) {
            (State::Ok, Some(t)) => t.elapsed() >= OK_FOR,
            (State::Failed, Some(t)) => t.elapsed() >= FAIL_FOR,
            _ => false,
        }
    }
}

struct Activity {
    jobs: Vec<Job>,
}

impl Service for Activity {
    fn new() -> Self {
        Self { jobs: vec![] }
    }
    // the elapsed time only changes once a second, so that is the redraw rate
    fn interval() -> Duration {
        Duration::from_secs(1)
    }
    fn update(&mut self) -> bool {
        let before = self.jobs.len();
        self.jobs.retain(|j| !j.expired());
        before != self.jobs.len() || self.jobs.iter().any(|j| j.state == State::Running)
    }
}

/// Something to show in the notch.
pub(super) fn shown() -> bool {
    !Activity::read().jobs.is_empty()
}

/// A click on the notch dismisses finished activities.
pub(super) fn dismiss_finished() {
    Activity::write().jobs.retain(|j| j.state == State::Running);
}

fn duration(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

/// The notch content for the newest activity.
pub(super) fn view(height: f32) -> Row {
    let job = Activity::read().jobs.last().cloned();
    let Some(job) = job else {
        return Row::new(vec![]);
    };
    let p = crate::theme::palette();
    let status: Box<dyn amane::Widget> = match job.state {
        State::Running => {
            // a quarter ring that steps round once a second: alive, but cheap
            let step = (job.elapsed().as_secs() % 4) as f32 * 90.0;
            let ring: Vec<Box<dyn amane::Shape>> = vec![
                Box::new(amane::Arc::new().center(8.0, 8.0).radius(6.0).stroke(2.0, crate::theme::control_fill(true))),
                Box::new(
                    amane::Arc::new()
                        .center(8.0, 8.0)
                        .radius(6.0)
                        .start(step)
                        .sweep(110.0)
                        .stroke(2.0, crate::theme::accent())
                        .cap(amane::Cap::Round),
                ),
            ];
            Box::new(symbols::sized(amane::Canvas::new().width(16.0).height(16.0).shapes(ring), 16.0))
        }
        State::Ok => Box::new(symbols::sized(symbols::checkmark(p.green), 15.0)),
        State::Failed => Box::new(symbols::sized(symbols::xmark(p.red), 15.0)),
    };
    let time = Text::new(duration(job.elapsed()))
        .font("JetBrainsMono Nerd Font")
        .size(11.5)
        .color(p.secondary);
    let time_w = match amane::Widget::width(&time) {
        amane::Size::Fixed(w) => w,
        _ => 40.0,
    };
    let label_w = (WIDTH - 36.0 - 16.0 - 10.0 - 10.0 - time_w).max(40.0);
    Row::new(vec![
        status,
        Box::new(
            Rectangle::new()
                .width(label_w)
                .height(height)
                .align_child(Start, Center)
                .child(
                    Text::new(&job.label)
                        .size(12.5)
                        .weight(Weight::SemiBold)
                        .color(p.foreground)
                        .elide(),
                ),
        ),
        Box::new(time),
    ])
    .gap(10.0)
    .align(Center)
}

/// `amane ipc call activity <start LABEL... | done ID ok|fail | clear | status>`
pub fn command(args: &[String]) -> String {
    match args.first().map(String::as_str) {
        Some("start") => {
            let label: String = args[1..].join(" ").chars().take(60).collect();
            let label = if label.trim().is_empty() { "task".to_string() } else { label };
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let mut a = Activity::write();
            a.jobs.push(Job { id, label, started: Instant::now(), ended: None, state: State::Running });
            // keep the list short: drop the oldest finished ones
            while a.jobs.len() > 8 {
                if let Some(i) = a.jobs.iter().position(|j| j.state != State::Running) {
                    a.jobs.remove(i);
                } else {
                    break;
                }
            }
            id.to_string()
        }
        Some("done") => {
            let id = args.get(1).and_then(|v| v.parse::<u64>().ok());
            let ok = args.get(2).is_some_and(|v| v == "ok" || v == "0");
            let mut a = Activity::write();
            match a.jobs.iter_mut().find(|j| Some(j.id) == id) {
                Some(job) => {
                    job.state = if ok { State::Ok } else { State::Failed };
                    job.ended = Some(Instant::now());
                    // the finished one moves to the front of the notch
                    let i = a.jobs.iter().position(|j| Some(j.id) == id).unwrap();
                    let job = a.jobs.remove(i);
                    a.jobs.push(job);
                    "ok".into()
                }
                None => "unknown id".into(),
            }
        }
        Some("clear") => {
            Activity::write().jobs.clear();
            "ok".into()
        }
        Some("status") | None => {
            let a = Activity::read();
            if a.jobs.is_empty() {
                return "none".into();
            }
            a.jobs
                .iter()
                .map(|j| {
                    let state = match j.state {
                        State::Running => "running",
                        State::Ok => "ok",
                        State::Failed => "failed",
                    };
                    format!("{} {} {} {}", j.id, state, duration(j.elapsed()), j.label)
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Some(other) => format!("usage: activity start LABEL | done ID ok|fail | clear | status, not {other}"),
    }
}
