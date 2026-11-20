//! Lower-edge islands, keyboard first: the launcher (bottom centre) and the
//! utility panel (bottom right: screenshots + power). Work runs in owned
//! worker subprocesses.
#[path = "auxiliary/modes.rs"]
mod modes;
use amane::{
    Apps, Button, Center, Column, Full, Horizontal, InputArea, Key, Keyboard, LayerWindow, Pointer,
    Rectangle, Row, Service, Start, Text, Vertical, Widget, children,
};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    time::Duration,
};
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// screenshots and power, one horizontal row
    Utility,
    Search,
}
/// The list a keyword mode is showing; `entries` is empty while it loads.
struct Menu {
    keyword: &'static str,
    entries: Vec<modes::Entry>,
    loading: bool,
}
/// One launcher result row.
#[derive(Clone)]
enum Hit {
    App(amane::DesktopApp),
    Mode(&'static modes::Mode),
    Action(&'static modes::Action),
    Item(&'static modes::Mode, modes::Entry),
    Web(String),
}
struct Auxiliary {
    kind: Kind,
    open: bool,
    motion: crate::island::motion::Spring,
    body_height: amane::Animation,
    target_height: f32,
    deadline: Option<std::time::Instant>,
    results: amane::Animation,
    query: String,
    caret: usize,
    caret_x: f32,
    blink_on: bool,
    edited: std::time::Instant,
    closing_search: Option<std::time::Instant>,
    close_wait: Duration,
    deferred_action: Option<(String, Vec<String>)>,
    status: String,
    confirm: Option<String>,
    /// a power action the launcher asked for, armed when the panel opens
    armed: Option<String>,
    file: Option<String>,
    busy: bool,
    selected: usize,
    pending: Option<Kind>,
    switching_since: Option<std::time::Instant>,
    menu: Option<Menu>,
    /// a picked row asked to reopen the launcher on this query (a submenu)
    reopen: Option<String>,
    /// True from the moment a region/window capture or recording starts until
    /// the pick is made or cancelled. The screen border carries the cue.
    selecting: bool,
    /// Name of the monitor whose edge sensor opened this. The sensors live on
    /// every monitor, but the modal is drawn only on the one you triggered, so
    /// a second screen does not mirror the search box back at you.
    origin: Option<String>,
    /// Border tint (0 = normal black frame, 1 = full capture colour).
    tint: amane::Animation,
    tint_color: amane::Color,
    /// The running action worker, so a cancel can reach it.
    worker_pid: Option<u32>,
    /// Pixel size of the captured image, for the preview overlay.
    file_size: Option<(f32, f32)>,
}
impl Service for Auxiliary {
    fn new() -> Self {
        Self {
            kind: Kind::Utility,
            open: false,
            origin: None,
            motion: crate::island::motion::Spring::slow(0.0, 0.6),
            body_height: amane::Animation::new(76.0).duration(Duration::from_millis(340)),
            target_height: 76.0,
            deadline: None,
            results: amane::Animation::new(0.0)
                .duration(Duration::from_millis(420))
                .easing(amane::Easing::InOut),
            query: String::new(),
            caret: 0,
            caret_x: 0.0,
            blink_on: false,
            edited: std::time::Instant::now(),
            closing_search: None,
            close_wait: Duration::from_millis(460),
            deferred_action: None,
            status: String::new(),
            confirm: None,
            armed: None,
            file: None,
            busy: false,
            selected: 0,
            pending: None,
            switching_since: None,
            menu: None,
            reopen: None,
            selecting: false,
            tint: amane::Animation::new(0.0).duration(Duration::from_millis(220)),
            tint_color: amane::Color::BLACK,
            worker_pid: None,
            file_size: None,
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(30)
    }
    fn update(&mut self) -> bool {
        let mut changed = false;
        // the caret blinks on/off (macOS-style); redraw only when it flips
        // instead of requesting a frame every refresh while search is open
        let blink = self.open && self.kind == Kind::Search && caret_on(self.edited);
        if blink != self.blink_on {
            self.blink_on = blink;
            changed = true;
        }
        if let Some(start) = self.closing_search {
            if start.elapsed() >= self.close_wait {
                self.motion.to(0.0);
            }
            if start.elapsed() >= self.close_wait + Duration::from_millis(960) {
                self.closing_search = None;
                self.open = false;
                // the next Spotlight starts empty, whether this one launched
                // something, was closed with Esc or was clicked away
                set_query(self, String::new());
                if let Some((action, args)) = self.deferred_action.take() {
                    std::thread::spawn(move || worker(&action, args));
                }
                changed = true;
            }
        }
        if self.switching_since.is_some_and(|t| {
            t.elapsed()
                >= if self.kind == Kind::Search {
                    self.close_wait + Duration::from_millis(960)
                } else {
                    Duration::from_millis(940)
                }
        }) {
            if let Some(kind) = self.pending.take() {
                self.kind = kind;
                self.open = true;
                self.motion.to(1.0);
                self.confirm = self.armed.take();
                self.selected = safe_default(&self.confirm);
                self.deadline = None;
            }
            self.switching_since = None;
            changed = true;
        }
        if self.file.is_some() {
            // A capture waiting for Save/Annotate is only dismissed explicitly.
            self.deadline = None;
        }
        if self
            .deadline
            .is_some_and(|d| std::time::Instant::now() >= d)
        {
            begin_close(self);
            self.deadline = None;
            return true;
        }
        self.results.to(
            if !self.open || self.query.is_empty() || self.closing_search.is_some() {
                0.0
            } else {
                1.0
            },
        );
        // Nothing on screen (closed and settled): there is no panel to size.
        // Building it here every 30 ms measured ~17 labels for nothing.
        if !self.open && self.closing_search.is_none() && self.motion.precise_value() < 0.001 {
            return changed;
        }
        let content = if self.kind == Kind::Search {
            search_results(self, SPOT_W)
        } else {
            utility_panel(self)
        };
        let target = match Widget::height(&content) {
            amane::Size::Fixed(h) => h + if self.kind == Kind::Search { SPOT_PAD * 2.0 } else { 32.0 },
            _ => 180.0,
        };
        changed |= target != self.target_height;
        self.target_height = target;
        if !(self.kind == Kind::Search && (self.query.is_empty() || self.closing_search.is_some()))
        {
            self.body_height.to(target);
        }
        changed
    }
}
/// The monitor holding the focused workspace, which is where an IPC-triggered
/// island belongs. Compositor-agnostic: it comes from the workspace service,
/// not from hyprctl.
pub(crate) fn focused_monitor() -> Option<String> {
    let workspaces = amane::Workspaces::read();
    let list = workspaces.list();
    list.iter()
        .find(|w| w.focused())
        .or_else(|| list.iter().find(|w| w.active()))
        .and_then(|w| w.output())
        .map(str::to_string)
}

pub fn command(args: &[String]) -> String {
    command_at(focused_monitor(), args)
}

/// The same, opened on `monitor` instead of the focused one: a click on a
/// bar button opens its panel on the screen the button is on.
pub fn command_on(monitor: &str, args: &[String]) -> String {
    command_at(Some(monitor.to_string()), args)
}

fn command_at(focused: Option<String>, args: &[String]) -> String {
    let mut state = Auxiliary::write();
    let requested = args.first().map(String::as_str);
    // An IPC open has no edge behind it, so aim it at the screen in use;
    // otherwise every monitor would draw the same modal.
    let utility = matches!(
        requested,
        Some("utility" | "screenshot" | "power" | "session" | "sessions")
    );
    if utility || requested == Some("search") {
        state.origin = focused;
    }
    // Any island command while a pick is pending (hide, or the same/another
    // island) cancels the pick instead of stacking a second flow on top of it.
    if state.selecting && (utility || requested == Some("hide")) {
        cancel_selection(&state);
        return "ok".into();
    }
    let kind = match requested {
        _ if utility => Kind::Utility,
        Some("search") => Kind::Search,
        Some("hide") => {
            state.pending = None;
            state.switching_since = None;
            begin_close(&mut state);
            return "ok".into();
        }
        Some("status") => {
            return format!(
                "open={} kind={:?} screen={} selecting={} message={}",
                state.open,
                state.kind,
                state.origin.as_deref().unwrap_or("-"),
                state.selecting,
                if state.status.is_empty() { "-" } else { &state.status },
            );
        }
        _ => return "usage: auxiliary utility|search [query]|hide|status".into(),
    };
    // `search clip` opens straight into a keyword mode; the trailing space is
    // what enters it, and IPC arguments cannot carry one.
    let query = args.get(1..).unwrap_or_default().join(" ");
    let query = if modes::find(&query).is_some() { query + " " } else { query };
    if kind == Kind::Search && state.open && state.kind == kind && !query.is_empty() && state.query != query {
        set_query(&mut state, query);
        return "ok".into();
    }
    if (state.kind == kind && state.open) || state.pending == Some(kind) {
        state.pending = None;
        state.switching_since = None;
        begin_close(&mut state);
        return "ok".into();
    }
    request_kind(&mut state, kind);
    state.selected = 0;
    if kind == Kind::Search {
        set_query(&mut state, query);
    }
    "ok".into()
}
/// Replaces the query and starts loading its mode's list if it entered one.
fn set_query(s: &mut Auxiliary, query: String) {
    s.query = query;
    s.caret = s.query.len();
    s.selected = 0;
    update_caret(s);
    sync_menu(s);
}
/// Keeps `menu` in step with the query: entering a mode loads its list on a
/// thread (scripts can take a moment), leaving it drops the list so the next
/// visit is fresh (the clipboard and windows change all the time).
fn sync_menu(s: &mut Auxiliary) {
    let Some((mode, _)) = modes::active(&s.query) else {
        s.menu = None;
        return;
    };
    if s.menu.as_ref().is_some_and(|m| m.keyword == mode.keyword) {
        return;
    }
    s.menu = Some(Menu {
        keyword: mode.keyword,
        entries: vec![],
        loading: true,
    });
    std::thread::spawn(move || {
        let entries = modes::load(mode);
        let mut s = Auxiliary::write();
        if let Some(menu) = s.menu.as_mut().filter(|m| m.keyword == mode.keyword) {
            menu.entries = entries;
            menu.loading = false;
        }
    });
}
fn begin_close(s: &mut Auxiliary) {
    s.deadline = None;
    if s.kind == Kind::Search && (s.open || s.motion.precise_value() > 0.01) {
        if s.closing_search.is_none() {
            s.close_wait = if s.results.value() > 0.001 {
                Duration::from_millis(460)
            } else {
                Duration::ZERO
            };
            s.closing_search = Some(std::time::Instant::now());
            if s.close_wait.is_zero() {
                s.motion.to(0.0);
            }
        }
        s.results.to(0.0);
    } else {
        s.open = false;
        s.motion.to(0.0);
        // Nothing from a finished flow may survive the island closing.
        if !s.busy {
            discard_capture(s);
            s.status.clear();
            s.confirm = None;
        }
    }
}
/// Cancelling anywhere throws the unsaved capture away.
fn discard_capture(s: &mut Auxiliary) {
    if let Some(file) = s.file.take() {
        let path = std::path::Path::new(&file);
        if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("amane-shot-"))
        {
            let _ = std::fs::remove_file(path);
        }
    }
    s.file_size = None;
}
fn png_size(path: &str) -> Option<(f32, f32)> {
    use std::io::Read;
    let mut bytes = [0u8; 24];
    std::fs::File::open(path).ok()?.read_exact(&mut bytes).ok()?;
    if &bytes[1..4] != b"PNG" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width as f32, height as f32))
}
fn cancel_selection(s: &Auxiliary) {
    if let Some(pid) = s.worker_pid {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}
/// Colour the screen border takes while a screenshot pick is pending.
fn capture_color() -> amane::Color {
    crate::theme::palette().blue
}
fn hex(color: amane::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red(), color.green(), color.blue())
}
/// (colour, strength 0..1) for the main island's border frame, if tinted.
pub fn tint() -> Option<(amane::Color, f32)> {
    let s = Auxiliary::read();
    let amount = s.tint.value();
    (amount > 0.002).then_some((s.tint_color, amount.clamp(0.0, 1.0)))
}
fn update_caret(s: &mut Auxiliary) {
    let prefix = &s.query[..s.caret];
    let width = match Widget::width(&text(prefix).size(SPOT_TEXT)) {
        amane::Size::Fixed(w) => w,
        _ => 0.0,
    };
    s.caret_x = width;
    s.edited = std::time::Instant::now();
}
fn request_kind(s: &mut Auxiliary, kind: Kind) {
    s.deadline = None;
    if s.kind != kind && (s.open || s.motion.precise_value() > 0.01) {
        s.pending = Some(kind);
        if s.switching_since.is_none() {
            s.switching_since = Some(std::time::Instant::now());
        }
        begin_close(s);
    } else {
        s.pending = None;
        s.switching_since = None;
        s.closing_search = None;
        s.deferred_action = None;
        if s.kind != kind {
            s.confirm = None;
            s.selected = 0;
        }
        if let Some(action) = s.armed.take() {
            s.confirm = Some(action);
            s.selected = safe_default(&s.confirm);
        }
        s.kind = kind;
        s.open = true;
        s.motion.to(1.0);
    }
}
fn hide() {
    let mut s = Auxiliary::write();
    s.pending = None;
    s.switching_since = None;
    begin_close(&mut s);
}

fn text(t: impl Into<String>) -> Text {
    Text::new(t)
        .size(15.0)
        .color(crate::theme::palette().foreground)
}
/// One utility-panel slot: its label (which also picks its symbol, see
/// `slot_symbol`) and the key that fires it.
struct Slot {
    label: &'static str,
    key: char,
}
const fn slot(label: &'static str, key: char) -> Slot {
    Slot { label, key }
}
const SHOTS: [Slot; 3] = [
    slot("Window", 'w'),
    slot("Region", 'r'),
    slot("Screen", 'f'),
];
const POWER: [Slot; 5] = [
    slot("Lock", 'l'),
    slot("Sleep", 'z'),
    slot("Log out", 'o'),
    slot("Restart", 'b'),
    slot("Shut down", 'p'),
];
const CAPTURED: [Slot; 2] = [
    slot("Save", 's'),
    slot("Annotate", 'a'),
];
const CONFIRM: [Slot; 2] = [
    slot("Confirm", 'y'),
    slot("Cancel", 'n'),
];
/// What the panel shows right now, in order. The gap after the screenshot
/// group is drawn by the view, not stored here.
fn slots(s: &Auxiliary) -> Vec<&'static Slot> {
    if s.file.is_some() {
        CAPTURED.iter().collect()
    } else if s.confirm.is_some() {
        CONFIRM.iter().collect()
    } else {
        SHOTS.iter().chain(POWER.iter()).collect()
    }
}
/// Fires slot `index` of the current panel.
fn activate(index: usize) {
    let mut s = Auxiliary::write();
    let Some(slot) = slots(&s).get(index).copied() else {
        return;
    };
    match slot.label {
        "Window" | "Region" | "Screen" => {
            let kind = match slot.label {
                "Window" => "window",
                "Region" => "region",
                _ => "full",
            };
            drop(s);
            worker("screenshot", vec![kind.into()]);
        }
        "Save" | "Annotate" => {
            let Some(file) = s.file.clone() else { return };
            drop(s);
            worker(if slot.label == "Save" { "save" } else { "edit" }, vec![file]);
        }
        "Confirm" => {
            let Some(action) = s.confirm.take() else { return };
            drop(s);
            worker("session", vec![action]);
        }
        "Cancel" => {
            s.confirm = None;
            s.selected = 0;
        }
        // lock and sleep are harmless to fire; the rest ask first
        "Lock" => {
            drop(s);
            worker("session", vec!["lock".into()]);
        }
        "Sleep" => {
            drop(s);
            worker("session", vec!["sleep".into()]);
        }
        label => {
            s.confirm = Some(
                match label {
                    "Log out" => "logout",
                    "Restart" => "restart",
                    _ => "shutdown",
                }
                .into(),
            );
            s.selected = safe_default(&s.confirm);
        }
    }
}
/// A confirm opens on Cancel: a stray second Enter never powers off, `y` does.
fn safe_default(confirm: &Option<String>) -> usize {
    usize::from(confirm.is_some())
}
/// Only what can lose work is coloured; everything else is plain white,
/// like the macOS screenshot toolbar.
fn destructive(label: &str) -> bool {
    matches!(label, "Shut down" | "Restart" | "Log out" | "Confirm")
}
fn button(slot: &'static Slot, index: usize, selected: bool, confirming: bool) -> Rectangle {
    let key = format!("aux:{}", slot.label);
    let click_key = key.clone();
    let palette = crate::theme::palette();
    let color = if confirming && destructive(slot.label) {
        palette.red
    } else {
        palette.foreground
    };
    Rectangle::new()
        .width(BUTTON)
        .height(BUTTON_H)
        .scale(crate::feedback::scale(&key))
        .radius(10.0)
        .fill(if selected {
            crate::theme::control_fill(true)
        } else {
            amane::Color::TRANSPARENT
        })
        .cursor(Pointer)
        .align_child(Center, Center)
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&click_key);
                activate(index)
            }
        })
        // the pointer and the arrow keys move the same highlight
        .on_hover(move |inside| {
            if inside {
                let mut s = Auxiliary::write();
                s.selected = index;
                s.deadline = None;
            }
        })
        .child(
            Column::new(children![
                slot_symbol(slot.label, color),
                Text::new(slot.label).size(11.0).color(if selected {
                    palette.foreground
                } else {
                    palette.secondary
                })
            ])
            .gap(6.0)
            .align(Center),
        )
}
/// The toolbar's SF-style symbol for a slot, like the macOS screenshot bar.
fn slot_symbol(label: &str, color: amane::Color) -> Rectangle {
    use crate::island::symbols as sf;
    let size = 24.0;
    match label {
        "Window" => sf::sized(sf::window(color), size),
        "Region" => sf::sized(sf::selection(color), size),
        "Screen" => sf::sized(sf::display(color), size),
        "Lock" => sf::sized(sf::lock(color), size),
        "Sleep" => sf::sized(sf::moon(color), size),
        "Log out" => sf::sized(sf::logout(color), size),
        "Restart" => sf::sized(sf::restart(color), size),
        "Save" => sf::sized(sf::save(color), size),
        "Annotate" => sf::sized(sf::pencil(color), size),
        "Confirm" => sf::sized(sf::checkmark(color), size),
        "Cancel" => sf::sized(sf::xmark(color), size),
        _ => sf::sized(sf::power(color), size),
    }
}
const BUTTON: f32 = 68.0;
const BUTTON_H: f32 = 60.0;
const BUTTON_GAP: f32 = 2.0;
/// The divider between the screenshot group and the power group.
const GROUP_GAP: f32 = 17.0;
/// The toolbar: a caption naming the highlighted slot and its key (or the
/// status / confirm question), over one row of labelled buttons.
fn utility_panel(s: &Auxiliary) -> Column {
    let list = slots(s);
    let selected = s.selected.min(list.len().saturating_sub(1));
    let confirming = s.confirm.is_some();
    let mut row: Vec<Box<dyn Widget>> = vec![];
    for (i, slot) in list.iter().enumerate() {
        if i == SHOTS.len() && list.len() > SHOTS.len() && s.file.is_none() && !confirming {
            row.push(Box::new(
                Rectangle::new()
                    .width(GROUP_GAP - BUTTON_GAP * 2.0)
                    .height(BUTTON_H)
                    .align_child(Center, Center)
                    .child(
                        Rectangle::new()
                            .width(1.0)
                            .height(36.0)
                            .fill(crate::theme::hairline()),
                    ),
            ));
        }
        row.push(Box::new(button(slot, i, i == selected, confirming)));
    }
    let palette = crate::theme::palette();
    let caption = if !s.status.is_empty() {
        text(&s.status).size(12.0).color(palette.red)
    } else if let Some(action) = &s.confirm {
        let pretty = match action.as_str() {
            "logout" => "Log out now?",
            "restart" => "Restart now?",
            _ => "Shut down now?",
        };
        text(format!("{pretty}   y confirm · n cancel")).size(12.0).color(palette.foreground)
    } else {
        let slot = list[selected];
        text(format!("{}  ·  {}", slot.label, slot.key))
            .size(12.0)
            .color(palette.secondary)
    };
    Column::new(children![
        Rectangle::new()
            .width(panel_width(s) - 24.0)
            .height(18.0)
            .align_child(Center, Center)
            .child(caption),
        Row::new(row).gap(BUTTON_GAP).align(Center)
    ])
    .gap(4.0)
    .align(Center)
}
fn panel_width(s: &Auxiliary) -> f32 {
    let n = slots(s).len() as f32;
    let groups = if s.file.is_none() && s.confirm.is_none() { GROUP_GAP - BUTTON_GAP } else { 0.0 };
    (n * BUTTON + (n - 1.0) * BUTTON_GAP + groups + 20.0).max(260.0)
}
fn report(_action: &str, line: &str) {
    let (kind, message) = line.split_once('\t').unwrap_or(("ERROR", line));
    let mut s = Auxiliary::write();
    match kind {
        // The pick is made: the border goes back to normal before the capture.
        "SELECTED" => {
            s.selecting = false;
            s.tint.to(0.0);
        }
        // Cancelled at any point: leave the whole flow, show nothing.
        "CANCEL" => finish(&mut s),
        "FILE" => {
            s.file = Some(message.into());
            s.file_size = png_size(message);
            s.busy = false;
            s.selecting = false;
            s.tint.to(0.0);
            request_kind(&mut s, Kind::Utility);
            s.selected = 0;
        }
        // a launcher row opened a submenu; applied once the worker exits
        "OPEN" => s.reopen = Some(message.into()),
        "ERROR" => {
            finish(&mut s);
            s.status = message.into();
            s.kind = Kind::Utility;
            s.open = true;
            s.motion.to(1.0)
        }
        // DONE: the action is over, nothing stays on screen.
        _ => finish(&mut s),
    }
}
/// Ends every phase: no tint, no pending pick, no leftover file or status.
fn finish(s: &mut Auxiliary) {
    s.busy = false;
    s.selecting = false;
    s.tint.to(0.0);
    s.worker_pid = None;
    s.file = None;
    s.file_size = None;
    s.status.clear();
    s.confirm = None;
    s.pending = None;
    s.switching_since = None;
}
fn worker(action: &str, mut args: Vec<String>) {
    let action = action.to_owned();
    {
        let mut s = Auxiliary::write();
        if s.busy {
            s.status = "Finish the current capture first.".into();
            return;
        }
        // From the launcher: let the pill leave first, then run.
        if s.kind == Kind::Search && s.closing_search.is_none() && s.open {
            s.deferred_action = Some((action.clone(), args));
            begin_close(&mut s);
            return;
        }
        s.busy = true;
        s.pending = None;
        s.switching_since = None;
        s.status = "Working…".into();
        s.open = false;
        s.motion.to(0.0);
        if action == "screenshot" {
            let color = capture_color();
            if matches!(args.first().map(String::as_str), Some("region" | "window")) {
                s.selecting = true;
                s.tint_color = color;
                s.tint.to(1.0);
            }
            args.push(hex(color));
        }
    }
    std::thread::spawn(move || {
        let result = Command::new("python3")
            .args(["-u", "-c", include_str!("auxiliary/actions.py"), &action])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        match result {
            Ok(mut child) => {
                Auxiliary::write().worker_pid = Some(child.id());
                if let Some(output) = child.stdout.take() {
                    for line in BufReader::new(output).lines().map_while(Result::ok) {
                        report(&action, &line);
                    }
                }
                let result = child.wait();
                let mut s = Auxiliary::write();
                s.busy = false;
                s.selecting = false;
                s.tint.to(0.0);
                s.worker_pid = None;
                let silent_failure = s.status == "Working…";
                if silent_failure {
                    s.status.clear();
                }
                if silent_failure && result.is_ok_and(|v| !v.success()) {
                    s.status = "Action failed. Check the tool / permissions.".into();
                    s.kind = Kind::Utility;
                    s.open = true;
                    s.motion.to(1.0);
                }
                if let Some(query) = s.reopen.take() {
                    let query = if modes::find(&query).is_some() { query + " " } else { query };
                    request_kind(&mut s, Kind::Search);
                    set_query(&mut s, query);
                }
            }
            Err(e) => report(&action, &format!("ERROR\t{e}")),
        }
    });
}
fn apps(query: &str) -> Vec<amane::DesktopApp> {
    let needle = query.trim().to_lowercase();
    let apps = Apps::read();
    let mut list = apps
        .list()
        .iter()
        .filter(|a| {
            a.name().to_lowercase().contains(&needle)
                || a.description()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
        })
        .cloned()
        .collect::<Vec<_>>();
    list.sort_by_key(|a| {
        (
            !a.name().to_lowercase().starts_with(&needle),
            a.name().to_lowercase(),
        )
    });
    list.truncate(8);
    list
}
/// Everything the query matches, best first.
///   s <text>        web search (engines: see websearch.sh)
///   <keyword> <..>  that mode's rows
///   anything else   a mode whose keyword is exactly the query, then apps,
///                   then other modes, then built-in actions
fn hits(s: &Auxiliary) -> Vec<Hit> {
    if let Some(search) = s.query.strip_prefix("s ") {
        return vec![Hit::Web(search.to_string())];
    }
    if let Some((mode, filter)) = modes::active(&s.query) {
        return match &s.menu {
            Some(menu) if menu.keyword == mode.keyword => modes::filter(&menu.entries, filter)
                .into_iter()
                .map(|e| Hit::Item(mode, e.clone()))
                .collect(),
            _ => vec![],
        };
    }
    if s.query.trim().is_empty() {
        return vec![];
    }
    let suggested = modes::suggest(&s.query);
    let exact = |m: &modes::Mode| m.keyword == s.query.trim();
    let mut out: Vec<Hit> = suggested.iter().filter(|m| exact(m)).map(|m| Hit::Mode(m)).collect();
    out.extend(apps(&s.query).into_iter().map(Hit::App));
    out.extend(suggested.iter().filter(|m| !exact(m)).map(|m| Hit::Mode(m)));
    out.extend(modes::actions(&s.query).into_iter().map(Hit::Action));
    out
}
/// Rows shown at once; the window follows the selection through long lists.
const VISIBLE: usize = 8;
fn window(selected: usize, len: usize) -> std::ops::Range<usize> {
    let start = selected.saturating_sub(VISIBLE - 1).min(len.saturating_sub(VISIBLE));
    start..(start + VISIBLE).min(len)
}
fn enter_mode(s: &mut Auxiliary, mode: &modes::Mode) {
    set_query(s, format!("{} ", mode.keyword));
}
fn run(hit: Hit) {
    match hit {
        Hit::App(app) => worker("launch", vec![app.exec().to_owned()]),
        Hit::Web(query) => worker("browser", vec![query]),
        Hit::Mode(mode) => enter_mode(&mut Auxiliary::write(), mode),
        Hit::Item(mode, entry) => worker("pick", modes::argv(mode, "--menu-pick", &entry.value)),
        Hit::Action(action) => match action.act {
            modes::Act::Shot(kind) => worker("screenshot", vec![kind.into()]),
            modes::Act::Session(name) => worker("session", vec![name.into()]),
            modes::Act::Guarded(name) => {
                let mut s = Auxiliary::write();
                s.armed = Some(name.into());
                request_kind(&mut s, Kind::Utility);
            }
            modes::Act::Script(name, args) => {
                let mut argv = vec![modes::script(name).to_string_lossy().into_owned()];
                argv.extend(args.iter().map(|a| a.to_string()));
                worker("pick", argv)
            }
        },
    }
}
fn word_range(text: &str, caret: usize, forward: bool) -> std::ops::Range<usize> {
    if forward {
        let mut end = caret;
        let mut in_word = false;
        for (i, c) in text[caret..].char_indices() {
            if !c.is_whitespace() {
                in_word = true;
            } else if in_word {
                break;
            }
            end = caret + i + c.len_utf8();
        }
        caret..end
    } else {
        let mut start = caret;
        let mut in_word = false;
        for (i, c) in text[..caret].char_indices().rev() {
            if !c.is_whitespace() {
                in_word = true;
            } else if in_word {
                break;
            }
            start = i;
        }
        start..caret
    }
}
fn delete_word(s: &mut Auxiliary, forward: bool) {
    let range = word_range(&s.query, s.caret, forward);
    s.caret = range.start;
    s.query.replace_range(range, "");
    s.selected = 0;
    update_caret(s);
}
fn key(key: Key) {
    let mut s = Auxiliary::write();
    if key == Key::Escape {
        // Esc backs out one step: a pending confirm, then a mode, then closes
        if s.kind == Kind::Utility && s.confirm.is_some() {
            s.confirm = None;
            s.selected = 0;
            return;
        }
        if s.kind == Kind::Search && s.menu.is_some() {
            set_query(&mut s, String::new());
            return;
        }
        s.pending = None;
        s.switching_since = None;
        begin_close(&mut s);
        return;
    }
    if s.closing_search.is_some() {
        return;
    }
    s.deadline = None;
    if s.kind == Kind::Utility {
        let count = slots(&s).len();
        match key {
            Key::Left | Key::Up => s.selected = s.selected.saturating_sub(1),
            Key::Right | Key::Down | Key::Tab => s.selected = (s.selected + 1).min(count - 1),
            Key::Enter | Key::Space => {
                let i = s.selected;
                drop(s);
                activate(i);
            }
            Key::Character(c) => {
                if let Some(i) = slots(&s).iter().position(|slot| slot.key == c.to_ascii_lowercase()) {
                    drop(s);
                    activate(i);
                }
            }
            _ => {}
        }
        return;
    }
    let count = hits(&s).len();
    let edited = |s: &mut Auxiliary| {
        s.selected = 0;
        update_caret(s);
        sync_menu(s);
    };
    match key {
        Key::Character('\u{a}') | Key::Character('\u{e}') | Key::Down => {
            s.selected = (s.selected + 1).min(count.saturating_sub(1))
        }
        Key::Character('\u{b}') | Key::Character('\u{10}') | Key::Up => {
            s.selected = s.selected.saturating_sub(1)
        }
        Key::Character('\u{17}') => {
            delete_word(&mut s, false);
            sync_menu(&mut s);
        }
        Key::Character('\u{4}') => {
            delete_word(&mut s, true);
            sync_menu(&mut s);
        }
        // Ctrl+X: drop the highlighted row, in modes that allow it
        Key::Character('\u{18}') => {
            if let Some(Hit::Item(mode, entry)) = hits(&s).into_iter().nth(s.selected) {
                if mode.delete {
                    s.menu = None;
                    drop(s);
                    std::thread::spawn(move || {
                        modes::delete(mode, &entry.value);
                        sync_menu(&mut Auxiliary::write());
                    });
                }
            }
        }
        Key::Character(c) if !c.is_control() => {
            let pos = s.caret;
            s.query.insert(pos, c);
            s.caret += c.len_utf8();
            edited(&mut s);
        }
        Key::Space => {
            let pos = s.caret;
            s.query.insert(pos, ' ');
            s.caret += 1;
            edited(&mut s);
        }
        // Tab completes a keyword: the highlighted mode, or the only one the
        // query is a prefix of
        Key::Tab => {
            let selected = hits(&s).into_iter().nth(s.selected);
            let mode = match selected {
                Some(Hit::Mode(mode)) => Some(mode),
                _ => {
                    let only = modes::suggest(&s.query);
                    (only.len() == 1).then(|| only[0])
                }
            };
            if let Some(mode) = mode {
                enter_mode(&mut s, mode);
            }
        }
        Key::Backspace => {
            if let Some((pos, _)) = s.query[..s.caret].char_indices().next_back() {
                let end = s.caret;
                s.query.replace_range(pos..end, "");
                s.caret = pos;
                edited(&mut s);
            }
        }
        Key::Left => {
            s.caret = s.query[..s.caret]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            update_caret(&mut s);
        }
        Key::Right => {
            if let Some(c) = s.query[s.caret..].chars().next() {
                s.caret += c.len_utf8();
                update_caret(&mut s);
            }
        }
        Key::Home => {
            s.caret = 0;
            update_caret(&mut s);
        }
        Key::End => {
            s.caret = s.query.len();
            update_caret(&mut s);
        }
        Key::Enter => {
            if let Some(hit) = hits(&s).into_iter().nth(s.selected) {
                drop(s);
                run(hit);
            }
        }
        _ => {}
    }
}

fn pointer(inside: bool) {
    let mut s = Auxiliary::write();
    s.deadline = if inside {
        None
    } else {
        Some(std::time::Instant::now() + Duration::from_millis(3000))
    };
}
fn edge(kind: Kind, inside: bool, origin: &str) {
    if !crate::edge_activation::enabled(edge_name(kind)) {
        return;
    }
    let mut s = Auxiliary::write();
    if s.busy {
        return;
    }
    if inside {
        s.origin = Some(origin.to_string());
    }
    if !inside {
        if s.kind == kind && s.open {
            s.deadline = Some(std::time::Instant::now() + Duration::from_secs(3));
        }
        return;
    }
    if kind == Kind::Search && s.kind != kind {
        set_query(&mut s, String::new());
    }
    request_kind(&mut s, kind);
}
fn row_icon(hit: &Hit) -> Rectangle {
    let icon = || Rectangle::new().width(32.0).height(32.0).radius(10.0);
    let glyph = |g: &str| {
        icon()
            .align_child(Center, Center)
            .child(
                Text::new(g)
                    .font("Material Symbols Rounded").weight(amane::Weight::Light)
                    .size(24.0)
                    .tight()
                    .color(crate::theme::palette().blue),
            )
    };
    match hit {
        Hit::App(app) => match app.icon_path() {
            Some(path) => icon().fill(amane::Image::contain(path)),
            None => icon().fill(crate::theme::palette().blue),
        },
        Hit::Item(mode, entry) => match &entry.image {
            Some(path) => icon().fill(amane::Image::cover(path.clone())),
            None => glyph(mode.glyph),
        },
        Hit::Mode(mode) => glyph(mode.glyph),
        Hit::Action(action) => glyph(action.glyph),
        Hit::Web(_) => glyph("\u{e894}"),
    }
}
fn row_label(hit: &Hit) -> String {
    match hit {
        Hit::App(app) => app.name().to_string(),
        Hit::Item(_, entry) => entry.label.clone(),
        Hit::Mode(mode) => format!("{}   ·  {}  (Tab)", mode.title, mode.keyword),
        Hit::Action(action) => action.label.to_string(),
        Hit::Web(query) if query.trim().is_empty() => {
            "Search the web: s <text> · s gh / aw / aur / yt / tf <text> · or a URL".into()
        }
        Hit::Web(query) => format!("Search the web for “{}”", query.trim()),
    }
}
/// What kind of thing a row is, shown dimmed on its right like Spotlight.
fn row_kind(hit: &Hit) -> String {
    match hit {
        Hit::App(_) => "Application".into(),
        Hit::Item(mode, _) => mode.title.to_string(),
        Hit::Mode(mode) => format!("{} ⇥", mode.keyword),
        Hit::Action(_) => "Action".into(),
        Hit::Web(_) => "Web".into(),
    }
}
/// Spotlight-style list: best match on top, a tinted highlight on the
/// selection, a quiet footer with the keys. Its measured height drives the
/// card's height animation (`body_height`).
fn search_results(s: &Auxiliary, width: f32) -> Column {
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    let palette = crate::theme::palette();
    let inner = width - SPOT_PAD * 2.0;
    let all = hits(s);
    let mode = modes::active(&s.query).map(|(m, _)| m);
    if all.is_empty() && !s.query.is_empty() {
        let message = match (mode, &s.menu) {
            (Some(_), Some(menu)) if menu.loading => "Loading…",
            (Some(_), _) => "Nothing matches",
            _ => "No results",
        };
        rows.push(Box::new(
            Rectangle::new()
                .width(inner)
                .height(44.0)
                .padding(amane::Padding { left: 12.0, right: 12.0, top: 0.0, bottom: 0.0 })
                .align_child(Start, Center)
                .child(text(message).size(14.0).color(palette.secondary)),
        ));
    }
    let shown = window(s.selected, all.len());
    for i in shown.clone() {
        let hit = all[i].clone();
        let key = format!("hit:{i}:{}", row_label(&hit));
        let ck = key.clone();
        let clicked = hit.clone();
        let selected = i == s.selected;
        let accent = palette.blue;
        let kind = text(row_kind(&hit)).size(12.0).color(if selected {
            palette.foreground
        } else {
            palette.secondary
        });
        let kind_w = match Widget::width(&kind) {
            amane::Size::Fixed(w) => w,
            _ => 80.0,
        };
        rows.push(Box::new(
            Rectangle::new()
                .width(inner)
                .height(44.0)
                .radius(10.0)
                .padding(amane::Padding { left: 10.0, right: 12.0, top: 0.0, bottom: 0.0 })
                .fill(if selected {
                    amane::Color::rgba(accent.red(), accent.green(), accent.blue(), 150)
                } else {
                    amane::Color::TRANSPARENT
                })
                .scale(crate::feedback::scale(&key))
                .cursor(Pointer)
                .on_click(move |b| {
                    if b == Button::Left {
                        crate::feedback::click(&ck);
                        run(clicked.clone());
                    }
                })
                .align_child(Start, Center)
                .child(
                    Row::new(children![
                        row_icon(&hit),
                        Rectangle::new()
                            .width((inner - 22.0 - 28.0 - 12.0 - kind_w - 12.0).max(40.0))
                            .height(28.0)
                            .align_child(Start, Center)
                            .child(text(row_label(&hit)).size(14.0).elide()),
                        kind
                    ])
                    .gap(12.0)
                    .align(Center),
                ),
        ));
    }
    let hint = match mode {
        Some(mode) => format!(
            "{}  ·  {} of {}{}",
            mode.title,
            all.len().min(shown.end),
            all.len(),
            if mode.delete { "  ·  ctrl x delete" } else { "" }
        ),
        None => String::from("↵ open   ⇥ complete   esc close"),
    };
    rows.push(Box::new(
        Rectangle::new()
            .width(inner)
            .height(26.0)
            .padding(amane::Padding { left: 10.0, right: 10.0, top: 0.0, bottom: 0.0 })
            .align_child(Start, Center)
            .child(text(hint).size(11.5).color(palette.secondary)),
    ));
    Column::new(rows).width(inner).gap(2.0)
}
const SPOT_W: f32 = 680.0;
const SPOT_TEXT: f32 = 21.0;
const SPOT_PAD: f32 = 8.0;
const SPOT_INPUT: f32 = 56.0;
/// Spotlight glass: the theme background, mostly opaque, under the layer's
/// compositor blur (hyprland.lua `amane-auxiliary-blur`).
/// The caret is solid for 0.5 s after an edit, then blinks every 530 ms.
fn caret_on(edited: std::time::Instant) -> bool {
    let ms = edited.elapsed().as_millis();
    ms < 500 || (ms / 530) % 2 == 0
}
fn area(x: f32, y: f32, w: f32, h: f32) -> InputArea {
    InputArea {
        x: x.round() as i32,
        y: y.round() as i32,
        width: w.round() as i32,
        height: h.round() as i32,
    }
}
fn slide_content_alpha(p: f32) -> f32 {
    ((p - 0.30) / 0.45).clamp(0.0, 1.0)
}
fn edge_name(kind: Kind) -> &'static str {
    match kind {
        // the stored preference keeps its old name
        Kind::Utility => "screenshot",
        Kind::Search => "search",
    }
}
/// Hover strips that open each panel, both on the bottom edge: nothing sits
/// on a side edge, where the cursor crosses between monitors.
fn sensors(sw: f32, sh: f32) -> [(Kind, f32, f32, f32, f32); 2] {
    [
        (Kind::Search, sw * 0.30, sh - 4.0, sw * 0.40, 4.0),
        (Kind::Utility, sw * 0.78, sh - 4.0, sw * 0.22, 4.0),
    ]
}
fn modal_active(s: &Auxiliary, p: f32) -> bool {
    !s.busy
        && (s.open || p > 0.001 || s.pending.is_some() || s.closing_search.is_some())
}

fn dormant(s: &Auxiliary, p: f32) -> bool {
    !s.open
        && !s.busy
        && !s.selecting
        && p <= 0.001
        && s.pending.is_none()
        && s.closing_search.is_none()
}
/// One auxiliary layer per monitor, so the edge sensors exist on every screen
/// -- including one plugged in later. Only the screen whose edge fired draws
/// the modal; see `Auxiliary::origin`.
pub fn view_per_monitor(monitor: &amane::Monitor) -> LayerWindow {
    view(&monitor.name)
}

pub fn view(monitor: &str) -> LayerWindow {
    let s = Auxiliary::read();
    // No origin yet (IPC, or nothing opened) -> whichever screen asks may draw.
    let mine = s.origin.as_deref().is_none_or(|o| o == monitor);
    let p = s.motion.precise_value().clamp(0.0, 1.0);
    // Keep the surface mapped but invisible when dormant during fullscreen;
    // unmapping makes mango close the surface and shut the shell down.
    if crate::island::fullscreen::blocked() && dormant(&s, p) {
        return LayerWindow::new()
            .width(Full)
            .height(Full)
            .input_region(vec![])
            .anchor_horizontal(Horizontal::Middle)
            .anchor_vertical(Vertical::Top)
            .space(amane::Zone::Ignore)
            .namespace("amane-auxiliary")
            .child(amane::Stack::new(vec![]).width(1.0).height(1.0));
    }
    let (sw, sh) = amane::window_size();
    let sw = sw.max(800.0);
    let sh = sh.max(600.0);
    let sensors = sensors(sw, sh);
    let mut input = vec![];
    let mut widgets: Vec<Box<dyn Widget>> = vec![];
    let modal = modal_active(&s, p) && mine;
    if modal {
        input.push(area(0.0, 0.0, sw, sh));
        // No dimming: the shield is invisible and only catches click-away.
        widgets.push(Box::new(
            Rectangle::new()
                .width(sw)
                .height(sh)
                .on_click(|_| hide())
                .on_scroll(|_| {}),
        ));
    }
    for (kind, x, y, w, h) in sensors {
        if !s.busy && crate::edge_activation::enabled(edge_name(kind)) {
            input.push(area(x, y, w, h));
            widgets.push(Box::new(
                Rectangle::new()
                    .width(w)
                    .height(h)
                    .translate(x, y)
                    .on_hover({
                        let origin = monitor.to_string();
                        move |inside| edge(kind, inside, &origin)
                    })
                    .cursor(Pointer),
            ));
        }
    }
    if (s.open || p > 0.001) && mine {
        if s.kind == Kind::Search {
            // Spotlight: a glass card in the upper third, results grow below.
            let width = SPOT_W;
            let x = ((sw - width) / 2.0).round();
            let y = (sh * 0.22).round() - 10.0 * (1.0 - p);
            let rp = s.results.value().clamp(0.0, 1.0);
            let rh = s.body_height.value().max(0.0);
            let body = (rh + 1.0) * rp;
            let card_h = SPOT_INPUT + body;
            let alpha = slide_content_alpha(p);
            widgets.push(Box::new(
                Rectangle::new()
                    .width(width)
                    .height(card_h)
                    .translate(x, y)
                    .radius(crate::theme::RADIUS)
                    .fill(crate::theme::glass())
                    .border(1.0, crate::theme::hairline())
                    .opacity(p)
                    .on_hover(pointer)
                    .on_click(|_| {}),
            ));
            if rp > 0.001 {
                widgets.push(Box::new(
                    Rectangle::new()
                        .width(width - 32.0)
                        .height(1.0)
                        .translate(x + 16.0, y + SPOT_INPUT)
                        .fill(crate::theme::hairline())
                        .opacity(rp * p),
                ));
                widgets.push(Box::new(
                    Rectangle::new()
                        .width(width)
                        .height(body.max(0.0))
                        .translate(x, y + SPOT_INPUT + 1.0)
                        .clip()
                        .child(
                            Rectangle::new()
                                .width(width)
                                .height(rh)
                                .padding(SPOT_PAD)
                                .opacity(slide_content_alpha(rp) * alpha)
                                .on_hover(pointer)
                                .child(search_results(&s, width)),
                        ),
                ));
            }
            // Edits reset the blink so the insertion point stays visible.
            let blink = if caret_on(s.edited) { 1.0 } else { 0.0 };
            let field = width - 20.0 - 26.0 - 12.0 - 20.0;
            let caret = s.caret_x;
            let offset = (caret - (field - 12.0)).max(0.0);
            let palette = crate::theme::palette();
            let mut text_widgets: Vec<Box<dyn Widget>> = vec![];
            if s.query.is_empty() {
                text_widgets.push(Box::new(
                    text("Spotlight Search").size(SPOT_TEXT).color(palette.secondary),
                ));
            } else {
                text_widgets.push(Box::new(
                    Rectangle::new()
                        .width(2400.0)
                        .height(28.0)
                        .translate(-offset, 0.0)
                        .child(text(&s.query).size(SPOT_TEXT)),
                ));
            }
            text_widgets.push(Box::new(
                Rectangle::new()
                    .width(2.0)
                    .height(24.0)
                    .radius(1.0)
                    .fill(palette.blue)
                    .translate(caret - offset, 2.0)
                    .opacity(blink),
            ));
            widgets.push(Box::new(
                Rectangle::new()
                    .width(width)
                    .height(SPOT_INPUT)
                    .padding(amane::Padding { left: 20.0, right: 20.0, top: 0.0, bottom: 0.0 })
                    .translate(x, y)
                    .opacity(alpha)
                    .on_hover(pointer)
                    .on_click(|_| {})
                    .align_child(Start, Center)
                    .child(
                        Row::new(children![
                            crate::island::symbols::sized(
                                crate::island::symbols::magnifier(palette.secondary),
                                22.0,
                            ),
                            Rectangle::new()
                                .width(field)
                                .height(28.0)
                                .clip()
                                .child(amane::Stack::new(text_widgets).width(field).height(28.0)),
                        ])
                        .gap(12.0)
                        .align(Center),
                    ),
            ));
            if alpha > 0.05 {
                input.push(area(x, y, width, card_h));
            }
        } else {
            // Utility row, bottom right. Same motion as the search pill: a
            // full-size body rises out of a solid bottom half-plane.
            let panel = utility_panel(&s);
            let width = panel_width(&s);
            let h = match Widget::height(&panel) {
                amane::Size::Fixed(h) => h + 20.0,
                _ => 90.0,
            };
            let x = ((sw - width) / 2.0).round();
            let y = sh - h - 28.0;
            let body_y = y + 14.0 * (1.0 - p);
            let alpha = slide_content_alpha(p);
            // The capture itself, shown over the screen while Save/Annotate is up.
            if s.open && !s.busy {
                if let (Some(file), Some((iw, ih))) = (&s.file, s.file_size) {
                    let fit = (sw * 0.55 / iw).min(sh * 0.55 / ih).min(1.0);
                    let (pw, ph) = ((iw * fit).round(), (ih * fit).round());
                    widgets.push(Box::new(
                        Rectangle::new()
                            .width(pw)
                            .height(ph)
                            .translate(((sw - pw) / 2.0).round(), ((sh - ph) / 2.0).round())
                            .radius(16.0)
                            .clip()
                            .fill(amane::Image::contain(std::path::PathBuf::from(file)))
                            .border(3.0, crate::theme::palette().blue)
                            .on_click(|_| {}),
                    ));
                }
            }
            widgets.push(Box::new(
                Rectangle::new()
                    .width(width)
                    .height(h)
                    .padding(10.0)
                    .translate(x, body_y)
                    .radius(crate::theme::RADIUS)
                    .fill(crate::theme::glass())
                    .border(1.0, crate::theme::hairline())
                    .opacity(p)
                    .on_hover(pointer)
                    .on_click(|_| {})
                    .align_child(Center, Center)
                    .child(panel),
            ));
            if alpha > 0.05 {
                input.push(area(x, body_y, width, h));
            }
        }
    }
    LayerWindow::new()
        .width(Full)
        .height(Full)
        .anchor_horizontal(Horizontal::Middle)
        .anchor_vertical(Vertical::Top)
        .space(amane::Zone::Ignore)
            .namespace("amane-auxiliary")
        .input_region(input)
        .keyboard(if modal && s.open {
            Keyboard::Exclusive
        } else {
            Keyboard::None
        })
        .on_key(key)
        .child(amane::Stack::new(widgets).width(sw).height(sh))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_closes_results_before_pill() {
        let mut s = Auxiliary::new();
        s.kind = Kind::Search;
        s.open = true;
        s.motion = crate::island::motion::Spring::new(1.0);
        s.query = "test".into();
        s.results = amane::Animation::new(1.0);
        s.caret = 4;
        begin_close(&mut s);
        assert!(s.open);
        assert_eq!(s.motion.precise_value(), 1.0);
        assert!(s.closing_search.is_some());
        s.closing_search = Some(std::time::Instant::now() - Duration::from_millis(1500));
        s.update();
        assert!(!s.open);
    }
    #[test]
    fn caret_uses_utf8_boundaries() {
        let mut s = Auxiliary::new();
        s.query = "日a".into();
        s.caret = 3;
        update_caret(&mut s);
        assert_eq!(&s.query[..s.caret], "日");
    }
    #[test]
    fn rounding_waits_for_content_to_disappear() {
        assert_eq!(slide_content_alpha(0.30), 0.0);
    }
    #[test]
    fn sensors_sit_on_the_bottom_edge_only() {
        for (_, x, y, w, h) in sensors(1920.0, 1080.0) {
            assert_eq!(y + h, 1080.0);
            assert!(x > 0.0 && x + w <= 1920.0);
        }
    }
    #[test]
    fn leaving_edge_schedules_hide_without_resetting_query() {
        let mut s = Auxiliary::new();
        s.query = "keep".into();
        s.open = true;
        s.deadline = Some(std::time::Instant::now() - Duration::from_secs(1));
        assert!(s.update());
        assert!(!s.open);
        assert_eq!(s.query, "keep");
    }
    #[test]
    fn changing_menu_preserves_old_until_exit() {
        let mut s = Auxiliary::new();
        s.open = true;
        s.motion = crate::island::motion::Spring::new(1.0);
        request_kind(&mut s, Kind::Search);
        assert!(s.kind == Kind::Utility);
        assert!(s.pending == Some(Kind::Search));
        assert!(!s.open);
        s.switching_since = Some(std::time::Instant::now() - Duration::from_secs(1));
        s.update();
        assert!(s.kind == Kind::Search);
        assert!(s.open);
        assert!(s.pending.is_none());
    }
    #[test]
    fn utility_keys_are_unique() {
        let keys: Vec<char> = SHOTS.iter().chain(POWER.iter()).map(|s| s.key).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(keys.len(), sorted.len());
    }
    #[test]
    fn capture_work_disables_modal_shield() {
        let mut s = Auxiliary::new();
        s.open = true;
        assert!(modal_active(&s, 1.0));
        s.busy = true;
        assert!(!modal_active(&s, 1.0));
        s.busy = false;
        s.open = false;
        assert!(!modal_active(&s, 0.0));
    }
    #[test]
    fn cancel_or_finish_leaves_nothing_behind() {
        let mut s = Auxiliary::new();
        s.selecting = true;
        s.busy = true;
        s.file = Some("/tmp/x.png".into());
        s.status = "Working…".into();
        s.tint.to(1.0);
        finish(&mut s);
        assert!(!s.selecting && !s.busy);
        assert!(s.file.is_none() && s.status.is_empty());
        let mut s = Auxiliary::new();
        s.kind = Kind::Utility;
        s.open = true;
        s.file = Some("/tmp/x.png".into());
        s.status = "Copied".into();
        begin_close(&mut s);
        assert!(!s.open && s.file.is_none() && s.status.is_empty());
    }
    #[test]
    fn cancelling_deletes_the_unsaved_capture() {
        let path = std::env::temp_dir().join("amane-shot-test.png");
        std::fs::write(&path, b"x").unwrap();
        let mut s = Auxiliary::new();
        s.open = true;
        s.file = Some(path.to_string_lossy().into_owned());
        begin_close(&mut s);
        assert!(!path.exists() && s.file.is_none());
        // Anything not named like our scratch captures is never touched.
        let other = std::env::temp_dir().join("amane-keep.png");
        std::fs::write(&other, b"x").unwrap();
        s.file = Some(other.to_string_lossy().into_owned());
        discard_capture(&mut s);
        assert!(other.exists());
        let _ = std::fs::remove_file(other);
    }
    #[test]
    fn word_deletion_bounds() {
        assert_eq!(word_range("one two", 7, false), 4..7);
        assert_eq!(word_range("one two", 0, true), 0..3);
        assert_eq!(word_range("日 本", 3, false), 0..3);
    }
    #[test]
    fn empty_search_closes_without_merge_wait() {
        let mut s = Auxiliary::new();
        s.kind = Kind::Search;
        s.open = true;
        begin_close(&mut s);
        assert!(s.close_wait.is_zero());
    }
    #[test]
    fn query_prefix_is_explicit() {
        assert!("s query".starts_with("s "));
        assert!(!"spotify".starts_with("s "));
    }
}
