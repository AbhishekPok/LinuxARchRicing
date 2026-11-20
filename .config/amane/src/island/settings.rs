//! In-island vertical settings, with independent accordion sections.
use super::*;
use amane::{Bluetooth, Network, ScrollArea, TextInput, Widget};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::{fs, path::PathBuf, process::Command};
#[derive(Clone, Copy, PartialEq)]
enum Section {
    History,
    Network,
    Bluetooth,
    Customize,
    Home,
}
struct Settings {
    open: bool,
    sections: [bool; 5],
    sidebar_labels: bool,
    hovered: Option<usize>,
    // keyboard focus on the Control Center home (Tab / arrows, Enter / Space)
    kbd: Option<usize>,
    page_started: Instant,
    sidebar_width: amane::Animation,
    motion: Spring,
    target_height: f32,
    /// Manual override; `None` follows the theme's CORNER_RADIUS.
    radius: Option<f32>,
    ssid: Option<String>,
    password: String,
    status: String,
    /// A Wi-Fi join in flight, watched until it lands or fails.
    joining: Option<Join>,
    theme_busy: bool,
    themes: Vec<Theme>,
    /// Theme currently opened in the editor.
    editing_theme: Option<String>,
    /// One entry per KEY=VALUE line of the edited theme, in file order.
    editor: Vec<EditorField>,
    /// Media files from the wallpapers directory, sorted by folder.
    wallpapers: Vec<WallItem>,
    /// Position of the wallpaper preview carousel.
    wallpaper_index: usize,
    /// Sidebar category filter; "all" shows every folder.
    wallpaper_filter: String,
}
/// A media file from the wallpapers directory.
#[derive(Clone)]
struct WallItem {
    path: String,
    name: String,
    /// Subfolder inside ~/Pictures/Wallpapers ("walls" for loose files).
    category: String,
    /// Video files are shown as thumbnails and painted with mpvpaper.
    video: bool,
}
/// An editable KEY=VALUE line of the current theme-editor session. The input id
/// is leaked once per session so `TextInput::set_text` can seed it and the
/// identity stays stable while the rows rebuild every frame.
#[derive(Clone)]
struct EditorField {
    key: String,
    value: String,
    id: &'static str,
}
struct Join {
    ssid: String,
    /// No profile existed before, so a failed attempt's profile is removed
    /// again and the next try asks for the password instead of reusing it.
    fresh: bool,
    started: Instant,
}
#[derive(Clone)]
struct Theme {
    name: String,
    colors: Vec<Color>,
}
impl Service for Settings {
    fn new() -> Self {
        let mut this = Self {
            open: false,
            sections: [false, false, false, false, true],
            sidebar_labels: false,
            hovered: None,
            kbd: None,
            page_started: Instant::now(),
            sidebar_width: amane::Animation::new(SIDEBAR_ICONS).duration(Duration::from_millis(300)),
            motion: Spring::slow(0.0, super::FACE_PACE),
            target_height: 0.0,
            radius: fs::read_to_string(state_dir().join("border-radius"))
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .map(|v| v.clamp(0.0, 96.0)),
            ssid: None,
            password: String::new(),
            status: String::new(),
            joining: None,
            theme_busy: false,
            themes: themes(),
            editing_theme: None,
            editor: vec![],
            wallpapers: wallpapers(),
            wallpaper_index: 0,
            wallpaper_filter: "all".into(),
        };
        // Point the carousel at the wallpaper that is currently active.
        this.wallpaper_index = active_wallpaper_index(&this.wallpapers);
        this
    }
    fn interval() -> Duration {
        Duration::from_millis(35)
    }
    fn update(&mut self) -> bool {
        let target = if self.open {
            content_height(self).min(620.0)
        } else {
            0.0
        };
        self.motion.to(target);
        let changed = self.target_height != target;
        self.target_height = target;
        changed | self.watch_join()
    }
}

impl Settings {
    /// NetworkManager answers asynchronously, so a join is followed here until
    /// the device reports the network, or settles on anything else.
    fn watch_join(&mut self) -> bool {
        let Some(join) = &self.joining else {
            return false;
        };
        let (joined, connecting) = {
            let net = Network::read();
            (net.ssid() == join.ssid, net.connecting())
        };
        let elapsed = join.started.elapsed();
        if joined && !connecting {
            self.status = format!("Connected to {}.", join.ssid);
            self.joining = None;
            return true;
        }
        // a few seconds' grace before "not connecting" can mean it gave up
        if (elapsed < Duration::from_secs(6) || connecting) && elapsed < Duration::from_secs(45) {
            return false;
        }
        let join = self.joining.take().unwrap();
        if join.fresh {
            let ssid = join.ssid.clone();
            std::thread::spawn(move || {
                let _ = std::process::Command::new("nmcli")
                    .args(["connection", "delete", "id", &ssid])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            });
            self.status = format!("Couldn't join {}. Check the password and try again.", join.ssid);
            self.ssid = Some(join.ssid);
        } else {
            self.status = format!(
                "Couldn't join {}. It may be out of range, or its saved password changed.",
                join.ssid
            );
        }
        true
    }
}

fn join_network(ssid: String, password: Option<String>, fresh: bool) {
    {
        let mut s = Settings::write();
        s.status = format!("Connecting to {ssid}…");
        s.joining = Some(Join {
            ssid: ssid.clone(),
            fresh,
            started: Instant::now(),
        });
    }
    Network::connect(&ssid, password.as_deref());
}
pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/state")
        })
        .join("amane")
}
pub(super) fn radius() -> f32 {
    let manual = Settings::read().radius; // release this lock before taking the theme's
    manual.unwrap_or_else(crate::theme::corner_radius)
}
pub(super) fn open() -> bool {
    Settings::read().open
}
pub(super) fn height() -> f32 {
    Settings::read().motion.precise_value().max(0.0)
}
pub(super) fn keyboard() -> bool {
    let s = Settings::read();
    s.open && s.ssid.is_some() && s.sections[1]
}
pub(super) fn close() {
    let mut s = Settings::write();
    s.open = false;
    s.password.clear();
    s.ssid = None;
    s.motion.to(0.0);
    drop(s);
    TextInput::set_text("amane-wifi-password", "");
}

/// Opens or closes the Control Center on `monitor` (None: where it already
/// is, else the focused screen). Already open on another screen, it moves
/// there instead of closing, so a click always lands where you clicked.
pub(super) fn toggle_on(monitor: Option<String>) {
    let elsewhere = {
        let island = Island::read();
        monitor.is_some() && island.expanded && island.owner != monitor
    };
    if elsewhere && open() {
        let mut i = Island::write();
        i.open_on(monitor);
        i.leave_deadline = None;
        i.sync();
        return;
    }
    popups::dismiss();
    let mut s = Settings::write();
    s.open = !s.open;
    if s.open {
        // the Control Center always opens on its home page
        s.kbd = None;
        s.sections = [false; 5];
        s.sections[index(Section::Home)] = true;
        s.page_started = Instant::now();
        let target = content_height(&s).min(620.0);
        s.motion.to(target);
    } else {
        s.motion.to(0.0);
    }
    drop(s);
    let mut i = Island::write();
    i.open_on(monitor);
    i.leave_deadline = None;
    i.sync();
}
// sidebar widths: icons only, and with labels (kept narrow enough that the
// 480 px Control Center still leaves a readable page)
const SIDEBAR_ICONS: f32 = 46.0;
const SIDEBAR_LABELS: f32 = 140.0;
const SIDEBAR_ROW: f32 = 38.0;
const SIDEBAR_GAP: f32 = 4.0;
// space between the sidebar and the page
const PAGE_GUTTER: f32 = 12.0;
// the settings frame's own padding: left / right (FRAME_X is both) and top /
// bottom; kept tight so the pages get the width
const FRAME_LEFT: f32 = 10.0;
const FRAME_X: f32 = 22.0;
const FRAME_Y: f32 = 12.0;
// heading block (title + subtitle) and the gap under it
const HEADING_H: f32 = 44.0;
const HEADING_GAP: f32 = 12.0;
fn index(section: Section) -> usize {
    match section {
        Section::History => 0,
        Section::Network => 1,
        Section::Bluetooth => 2,
        Section::Customize => 3,
        Section::Home => 4,
    }
}
fn section_toggle(section: Section) {
    let mut s = Settings::write();
    s.sections = [false; 5];
    s.sections[index(section)] = true;
    s.page_started = Instant::now();
    let target = content_height(&s);
    s.motion.to(target);
}
// Pages that scroll (history, networks, appearance) get a fixed height; the
// Control Center home sizes to what it shows, like macOS, so it has no empty
// space when nothing is playing. Plain arithmetic: this runs every 35 ms.
fn content_height(s: &Settings) -> f32 {
    if !s.sections[index(Section::Home)] {
        return 560.0;
    }
    // card heights as home_page builds them: connectivity (3 rows), Focus +
    // Keep Awake, Battery, the DevOps tiles, and Now Playing when it shows
    let mut cards = vec![156.0, 64.0, 64.0, 64.0];
    if now_playing_visible() {
        cards.push(142.0);
    }
    let body: f32 = cards.iter().sum::<f32>() + 12.0 * (cards.len() as f32 - 1.0);
    (FRAME_Y * 2.0 + HEADING_H + HEADING_GAP + body + 8.0).clamp(300.0, 620.0)
}

/// Something is playing, or is paused partway through a track.
fn now_playing_visible() -> bool {
    let media = amane::Media::read();
    media.active().is_some_and(|p| {
        !p.title().is_empty() && (p.playing() || p.position() >= Duration::from_secs(1))
    })
}
fn content_width(s: &Settings) -> f32 {
    PANEL.0 - FRAME_X - s.sidebar_width.value() - PAGE_GUTTER
}
pub fn command(args: &[String]) -> String {
    command_on(None, args)
}

/// `command`, opening on `monitor` (a bar button's own screen).
pub(super) fn command_on(monitor: Option<&str>, args: &[String]) -> String {
    let monitor = monitor.map(str::to_string);
    match args.first().map(String::as_str) {
        Some("hide") => close(),
        Some("home" | "history" | "network" | "bluetooth" | "customization") => {
            if !open() {
                toggle_on(monitor);
            } else if monitor.is_some() && Island::read().owner != monitor {
                toggle_on(monitor);
            }
            let section = match args[0].as_str() {
                "home" => Section::Home,
                "history" => Section::History,
                "network" => Section::Network,
                "bluetooth" => Section::Bluetooth,
                _ => Section::Customize,
            };
            let mut s = Settings::write();
            s.sections = [false; 5];
            s.sections[index(section)] = true;
            s.page_started = Instant::now();
            let target = content_height(&s).min(620.0);
            s.motion.to(target);
        }
        None => toggle_on(monitor),
        Some(other) => {
            return format!("usage: settings [home|history|network|bluetooth|customization|hide], not {other}")
        }
    }
    "ok".into()
}
fn settings_foreground() -> Color {
    crate::theme::readable(crate::theme::palette().foreground, setting_surface(), 4.5)
}
fn settings_secondary() -> Color {
    crate::theme::readable(crate::theme::palette().secondary, setting_surface(), 4.5)
}
fn label(text: impl Into<String>) -> Text {
    Text::new(text).size(14.0).color(settings_foreground())
}
fn button(text: impl Into<String>, width: f32, action: impl Fn() + 'static) -> Rectangle {
    let text = text.into();
    let key = format!("settings:{text}");
    let ck = key.clone();
    let t = label(&text).weight(Weight::SemiBold);
    let tw = match Widget::width(&t) {
        amane::Size::Fixed(w) => w.min(width - 20.0),
        _ => width - 20.0,
    };
    Rectangle::new()
        .width(width)
        .height(36.0)
        .scale(crate::feedback::scale(&key))
        .radius(12.0)
        .fill(crate::theme::card_raised())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .child(
            Rectangle::new()
                .width(tw)
                .height(22.0)
                .align_child(Center, Center)
                .child(t.elide()),
        )
}
fn contents(s: &Settings) -> Column {
    let panel = content_width(s) + 48.0 - 18.0;
    // Drop the history lock before the shared card renderer reads live IDs.
    let history_entries = history::History::read().entries.clone();
    let net = Network::read();
    let bt = Bluetooth::read();
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    if s.sections[0] {
        // Windows notification centre with macOS stacks: a count and "Clear
        // all" on top, then one group per app (newest group first), each with
        // its own clear button. Only cards near the viewport are built (up to
        // 200 entries); the rest are same-height spacers.
        let card_width = panel - 48.0;
        let offset = settings_scroll::offset("settings-history-page");
        let (near_top, near_bottom) = (offset - 500.0, offset + 1000.0);
        let total = history_entries.len();
        let mut groups: Vec<(String, Vec<&history::Entry>)> = vec![];
        for entry in history_entries.iter().rev() {
            match groups.iter_mut().find(|(app, _)| *app == entry.app) {
                Some((_, list)) => list.push(entry),
                None => groups.push((entry.app.clone(), vec![entry])),
            }
        }
        rows.push(Box::new(section_head(
            &match total {
                0 => "No notifications".to_string(),
                1 => "1 notification".to_string(),
                n => format!("{n} notifications"),
            },
            (total > 0).then(|| pill("Clear all", false, Box::new(history::clear))),
            card_width,
        )));
        let mut y = 40.0;
        let mut column: Vec<Box<dyn Widget>> = vec![];
        for (app, entries) in &groups {
            let keys: Vec<u64> = entries.iter().map(|e| e.key).collect();
            let name = if app.is_empty() { "Other" } else { app.as_str() };
            let clear = Rectangle::new()
                .width(26.0)
                .height(26.0)
                .radius(Full)
                .fill(crate::theme::card_raised())
                .cursor(Pointer)
                .align_child(Center, Center)
                .on_click(move |b| {
                    if b == Button::Left {
                        for key in &keys {
                            history::remove(*key);
                        }
                    }
                })
                .child(super::symbols::sized(super::symbols::xmark(settings_secondary()), 10.0));
            column.push(Box::new(section_head(
                &format!("{name}  ·  {}", entries.len()),
                Some(clear),
                card_width,
            )));
            y += 36.0;
            for entry in entries {
                let height = notices::history_card_height(entry, card_width);
                if y + height >= near_top && y <= near_bottom {
                    column.push(Box::new(notices::history_card(entry, card_width)));
                } else {
                    column.push(Box::new(Rectangle::new().width(card_width).height(height)));
                }
                y += height + 8.0;
            }
        }
        if groups.is_empty() {
            column.push(Box::new(
                label("You're all caught up.").size(13.0).color(settings_secondary()),
            ));
        }
        rows.push(Box::new(Column::new(column).width(card_width).gap(8.0)));
    }
    if s.sections[4] {
        rows.extend(home_page(&net, &bt, panel - 48.0, s.kbd));
    }
    if s.sections[1] {
        rows.extend(network_page(s, &net, panel - 48.0));
    }
    if s.sections[2] {
        rows.extend(bluetooth_page(&bt, panel - 48.0));
    }
    if s.sections[3] {
        rows.push(Box::new(
            label("Background").size(17.0).weight(Weight::SemiBold),
        ));
        rows.push(Box::new(amoled_switch(panel - 48.0, false)));
        rows.push(Box::new(amoled_switch(panel - 48.0, true)));

        rows.push(Box::new(section_label(
            "Wallpaper",
            &format!(
                "{} file{} in ~/Pictures/Wallpapers",
                s.wallpapers.len(),
                if s.wallpapers.len() == 1 { "" } else { "s" }
            ),
            panel - 48.0,
        )));
        rows.push(Box::new(wallpaper_section(s, panel - 48.0)));

        rows.push(Box::new(
            label("Notch")
                .size(17.0)
                .weight(Weight::SemiBold),
        ));
        rows.push(Box::new(visibility_switch(false, panel - 48.0)));
        rows.push(Box::new(visibility_switch(true, panel - 48.0)));
        let note = label("The notch shows the time, Live Activities and alerts.")
            .size(12.0)
            .color(settings_secondary())
            .wrap()
            .max_lines(2);
        let nh = note.height_in(panel - 48.0);
        rows.push(Box::new(
            Rectangle::new().width(panel - 48.0).height(nh).child(note),
        ));

        rows.push(Box::new(
            label("Edge activation").size(17.0).weight(Weight::SemiBold),
        ));
        rows.push(Box::new(
            label("Reveal each island by touching its screen edge.")
                .size(13.0)
                .color(settings_secondary()),
        ));
        for name in crate::edge_activation::NAMES {
            rows.push(Box::new(edge_switch(name, panel - 48.0)));
        }
        rows.push(Box::new(
            label("IPC activation always stays available.")
                .size(12.0)
                .color(crate::theme::accent()),
        ));

        let radius = s.radius.unwrap_or_else(crate::theme::corner_radius);
        let preview = Rectangle::new()
            .width(106.0)
            .height(70.0)
            .radius((radius * 0.6).min(32.0))
            .border(4.0, crate::theme::palette().blue)
            .fill(crate::theme::palette().background)
            .align_child(Center, Center)
            .child(
                label(format!("{} px", radius as u32))
                    .size(20.0)
                    .weight(Weight::SemiBold),
            );
        let controls = Column::new(children![
            label("Screen curvature")
                .size(16.0)
                .weight(Weight::SemiBold),
            label("Rounds the corners of every screen")
                .size(12.0)
                .color(settings_secondary()),
            Row::new(children![
                button("−", 40.0, || change_radius(-4.0)),
                button("+", 40.0, || change_radius(4.0)),
                button("Reset", 68.0, reset_radius)
            ])
            .gap(8.0)
        ])
        .gap(10.0);
        rows.push(Box::new(
            Rectangle::new()
                .width(panel - 48.0)
                .height(100.0)
                .padding(12.0)
                .radius(16.0)
                .fill(setting_surface())
                .child(
                    Row::new(children![preview, controls])
                        .gap(18.0)
                        .align(Center),
                ),
        ));
        rows.push(Box::new(
            label("Theme library").size(17.0).weight(Weight::SemiBold),
        ));
        let current = crate::theme::current_name();
        rows.push(Box::new(
            label(if s.theme_busy {
                s.status.clone()
            } else {
                format!("Active theme · {current}")
            })
            .size(13.0)
            .color(crate::theme::accent()),
        ));
        rows.push(Box::new(
            label("Click a theme to apply it, right-click to edit it.")
                .size(12.0)
                .color(settings_secondary()),
        ));
        rows.extend(theme_editor_rows(s, panel - 48.0));
        // one theme per row: name (checked when current) and its swatches,
        // macOS-list style, so nothing is clipped at any panel width
        let row_w = panel - 48.0;
        let swatch_room = (row_w - 24.0 - 150.0).max(40.0);
        let max_swatches = ((swatch_room + 6.0) / 22.0).floor().max(1.0) as usize;
        let mut cards = vec![];
        for theme in &s.themes {
            let name = theme.name.clone();
            let key = format!("theme:{name}");
            let ck = key.clone();
            let edit_name = name.clone();
            let is_current = theme.name == current;
            let swatches: Vec<Box<dyn Widget>> = theme
                .colors
                .iter()
                .take(max_swatches)
                .map(|c| {
                    Box::new(
                        Rectangle::new()
                            .width(16.0)
                            .height(16.0)
                            .radius(Full)
                            .border(1.0, crate::theme::hairline())
                            .fill(*c),
                    ) as Box<dyn Widget>
                })
                .collect();
            let check: Box<dyn Widget> = if is_current {
                Box::new(
                    Text::new("\u{e5ca}")
                        .font("Material Symbols Rounded").weight(amane::Weight::Light)
                        .size(18.0)
                        .tight()
                        .color(crate::theme::accent()),
                )
            } else {
                Box::new(Rectangle::new().width(18.0).height(18.0))
            };
            cards.push(Box::new(
                Rectangle::new()
                    .width(row_w)
                    .height(46.0)
                    .scale(crate::feedback::scale(&key))
                    .padding(amane::Padding { left: 12.0, right: 12.0, top: 0.0, bottom: 0.0 })
                    .radius(12.0)
                    .fill(if is_current {
                        crate::theme::card_raised()
                    } else {
                        setting_surface()
                    })
                    .cursor(Pointer)
                    .on_click(move |b| {
                        if b == Button::Left {
                            crate::feedback::click(&ck);
                            apply_theme(name.clone())
                        } else if b == Button::Right {
                            crate::feedback::click(&ck);
                            edit_theme(edit_name.clone())
                        }
                    })
                    .align_child(Start, Center)
                    .child(
                        Row::new(vec![
                            check,
                            Box::new(
                                Rectangle::new()
                                    .width(row_w - 24.0 - 18.0 - 10.0 - swatch_room)
                                    .height(20.0)
                                    .align_child(Start, Center)
                                    .child(label(&theme.name).weight(Weight::SemiBold).elide()),
                            ),
                            Box::new(
                                Rectangle::new()
                                    .width(swatch_room)
                                    .height(20.0)
                                    .align_child(End, Center)
                                    .child(Row::new(swatches).gap(6.0).align(Center)),
                            ),
                        ])
                        .gap(10.0)
                        .align(Center),
                    ),
            ) as Box<dyn Widget>);
        }
        rows.push(Box::new(Column::new(cards).gap(6.0)));
    }
    if !s.status.is_empty() {
        let t = label(&s.status)
            .size(13.0)
            .color(settings_secondary())
            .wrap()
            .max_lines(3);
        let h = t.height_in(panel - 48.0);
        rows.push(Box::new(
            Rectangle::new().width(panel - 48.0).height(h).child(t),
        ));
    }
    Column::new(rows).width(panel - 48.0).gap(12.0)
}
fn connect_selected() {
    let (ssid, password) = {
        let mut s = Settings::write();
        let values = (s.ssid.take(), std::mem::take(&mut s.password));
        values
    };
    TextInput::set_text("amane-wifi-password", "");
    if let Some(ssid) = ssid {
        let saved = Network::read()
            .access_points()
            .iter()
            .any(|ap| ap.ssid() == ssid && ap.saved());
        let password = (!password.is_empty()).then_some(password);
        join_network(ssid, password, !saved);
    }
}

fn cancel_join_prompt() {
    {
        let mut s = Settings::write();
        s.ssid = None;
        s.password.clear();
    }
    TextInput::set_text("amane-wifi-password", "");
}
fn change_radius(delta: f32) {
    let value = radius() + delta;
    set_radius(value);
}
fn set_radius(value: f32) {
    let value = if value.is_finite() {
        value.clamp(0.0, 96.0)
    } else {
        return;
    };
    Settings::write().radius = Some(value);
    persist_radius();
}
/// Drop the manual override so the radius follows the active theme again.
fn reset_radius() {
    Settings::write().radius = None;
    persist_radius();
}
static RADIUS_FILE: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// Saves whatever the radius override is *when this runs*, one writer at a time, so
/// overlapping saves (e.g. `+` then Reset) always converge on the latest state.
fn persist_radius() {
    std::thread::spawn(|| {
        let _guard = RADIUS_FILE.lock().unwrap_or_else(|e| e.into_inner());
        let current = Settings::read().radius;
        let dir = state_dir();
        let path = dir.join("border-radius");
        match current {
            Some(value) => {
                let _ = fs::create_dir_all(&dir);
                let tmp = dir.join("border-radius.tmp");
                if fs::write(&tmp, format!("{value}\n")).is_ok() {
                    let _ = fs::rename(tmp, path);
                }
            }
            None => {
                let _ = fs::remove_file(path);
            }
        }
    });
}
fn themes() -> Vec<Theme> {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
        .join("theme-manager/themes");
    let mut themes = vec![];
    if let Ok(list) = fs::read_dir(dir) {
        for entry in list.flatten() {
            if !entry
                .file_type()
                .is_ok_and(|t| t.is_file() || t.is_symlink())
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let colors = ["BASE00", "BLUE", "GREEN", "PURPLE", "YELLOW"]
                .iter()
                .filter_map(|key| {
                    text.lines()
                        .filter_map(|l| l.split_once('='))
                        .find(|(k, _)| k.trim() == *key)
                        .and_then(|(_, v)| parse_hex(v.trim()))
                })
                .collect();
            themes.push(Theme { name, colors });
        }
    }
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}
fn parse_hex(s: &str) -> Option<Color> {
    let rgb = u32::from_str_radix(s.strip_prefix('#')?.get(..6)?, 16).ok()?;
    Some(Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
}
fn setting_surface() -> Color {
    crate::theme::card()
}
fn switch_track(on: bool, accent: Color) -> Rectangle {
    Rectangle::new()
        .width(40.0)
        .height(24.0)
        .radius(Full)
        .fill(if on {
            accent
        } else {
            crate::theme::card_raised()
        })
        .child(
            Rectangle::new()
                .width(18.0)
                .height(18.0)
                .radius(Full)
                .translate(if on { 19.0 } else { 3.0 }, 3.0)
                .fill(if on {
                    crate::theme::background()
                } else {
                    accent
                }),
        )
}
fn toggle_row(
    glyph: &str,
    title: &str,
    detail: &str,
    width: f32,
    on: bool,
    accent: Color,
    action: impl Fn() + 'static,
) -> Rectangle {
    let accent = crate::theme::readable(accent, setting_surface(), 3.0);
    let tw = (width - 112.0).max(40.0);
    let subtitle = label(detail)
        .size(12.0)
        .color(settings_secondary())
        .wrap()
        .max_lines(2);
    let sh = subtitle.height_in(tw);
    Rectangle::new()
        .width(width)
        .height((sh + 34.0).max(66.0))
        .padding(10.0)
        .radius(14.0)
        .fill(setting_surface())
        .cursor(Pointer)
        .scale(crate::feedback::scale(&format!("switch:{title}")))
        .on_click({
            let key = format!("switch:{title}");
            move |b| {
                if b == Button::Left {
                    crate::feedback::click(&key);
                    action()
                }
            }
        })
        .align_child(Start, Center)
        .child(
            Row::new(children![
                Text::new(glyph)
                    .font("Material Symbols Rounded").weight(amane::Weight::Light)
                    .size(24.0)
                    .tight()
                    .color(accent),
                Column::new(children![
                    label(title).size(14.0).weight(Weight::SemiBold),
                    Rectangle::new().width(tw).height(sh).child(subtitle)
                ])
                .width(tw)
                .gap(4.0),
                switch_track(on, accent)
            ])
            .gap(12.0)
            .align(Center),
        )
}
fn amoled_switch(width: f32, global: bool) -> Rectangle {
    let on = if global {
        crate::theme::global_amoled()
    } else {
        crate::theme::amoled()
    };
    toggle_row(
        "\u{e3a7}",
        if global {
            "System AMOLED"
        } else {
            "Amane AMOLED"
        },
        if global {
            "Theme manager · other applications"
        } else {
            "This shell only · independent of system themes"
        },
        width,
        on,
        crate::theme::accent(),
        move || {
            if global {
                set_amoled(!on)
            } else {
                match crate::theme::set_amoled(!on) {
                    Ok(()) => {}
                    Err(e) => Settings::write().status = format!("Cannot save Amane AMOLED: {e}"),
                }
            }
        },
    )
}
fn set_amoled(enabled: bool) {
    {
        let mut s = Settings::write();
        if s.theme_busy {
            return;
        }
        s.theme_busy = true;
        s.status = "Updating AMOLED mode…".into();
    }
    std::thread::spawn(move || {
        let output = Command::new("theme-set")
            .args(["--amoled", if enabled { "on" } else { "off" }])
            .output();
        let result = match output {
            Ok(o) if o.status.success() => format!(
                "AMOLED {}. Theme manager will reload the palette.",
                if enabled { "enabled" } else { "disabled" }
            ),
            Ok(o) => format!(
                "AMOLED update failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ),
            Err(e) => format!("Cannot launch theme-set: {e}"),
        };
        let mut s = Settings::write();
        s.theme_busy = false;
        s.status = result;
    });
}
fn apply_theme(name: String) {
    {
        let mut state = Settings::write();
        if state.theme_busy {
            return;
        }
        state.theme_busy = true;
        state.status = format!("Applying {name}…");
    }
    std::thread::spawn(move || {
        let output = Command::new("theme-set").arg(&name).output();
        let result = match output {
            Ok(o) if o.status.success() => {
                let mut confirmed = false;
                for _ in 0..50 {
                    if crate::theme::current_name() == name {
                        confirmed = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                if confirmed {
                    format!("Theme {name} is active.")
                } else {
                    format!(
                        "Theme {name} was requested, but theme-daemon has not applied it. Check the theme-daemon user service."
                    )
                }
            }
            Ok(o) => format!(
                "theme-set failed: {}",
                String::from_utf8_lossy(&o.stderr)
                    .chars()
                    .take(240)
                    .collect::<String>()
            ),
            Err(e) => format!("Cannot launch theme-set: {e}"),
        };
        let mut state = Settings::write();
        state.theme_busy = false;
        state.status = result;
    });
}

fn theme_file(name: &str) -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    config.join("theme-manager/themes").join(name)
}

/// Every location the theme file should be written to: the theme-set-visible
/// path (`~/.config/theme-manager/themes`) plus the live file behind the
/// symlink when they differ. Duplicates single-target setups.
fn theme_write_targets(name: &str) -> Vec<PathBuf> {
    let direct = theme_file(name);
    let mut targets = vec![direct.clone()];
    if let Ok(canonical) = fs::canonicalize(&direct) {
        if canonical != direct {
            targets.push(canonical);
        }
    }
    targets
}

// ---------------------------------------------------------------- theme editor

fn group_name(key: &str) -> Option<&'static str> {
    match key {
        "VARIANT" | "MODE" | "OPACITY" => Some("General"),
        "BASE00" => Some("Base 16"),
        "WHITE" => Some("Base 30"),
        "CORNER_RADIUS" => Some("Shaping"),
        _ => None,
    }
}

fn edit_theme(name: String) {
    let text = fs::read_to_string(theme_file(&name)).unwrap_or_default();
    let mut fields = vec![];
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap();
        let key = key.trim().to_string();
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        let id: &'static str = Box::leak(
            format!("theme-edit:{name}::{key}").into_boxed_str(),
        );
        fields.push(EditorField {
            key,
            id,
            value: value.trim().to_string(),
        });
    }
    let mut s = Settings::write();
    for f in &s.editor {
        TextInput::set_text(f.id, "");
    }
    for f in &fields {
        TextInput::set_text(f.id, &f.value);
    }
    s.editor = fields;
    s.editing_theme = Some(name);
}

fn close_editor() {
    let mut s = Settings::write();
    for f in &s.editor {
        TextInput::set_text(f.id, "");
    }
    s.editor.clear();
    s.editing_theme = None;
}

/// Rewrites the theme file with the editor's values. Preserves comments and
/// layout by replacing the right-hand side of each known KEY= line; keys that
/// the original file lacked are appended. If the theme directory is not
/// writable the draft is written beside it instead, and the status says where.
fn save_theme_edits(apply: bool) {
    let (name, fields) = {
        let mut s = Settings::write();
        if s.theme_busy {
            return;
        }
        let Some(name) = s.editing_theme.clone() else {
            return;
        };
        s.theme_busy = true;
        s.status = format!("Saving {name}…");
        (name, s.editor.clone())
    };
    std::thread::spawn(move || {
        let file = theme_file(&name);
        let original = fs::read_to_string(&file).unwrap_or_default();
        let pairs: Vec<(String, String)> = fields
            .iter()
            .map(|f| (f.key.clone(), f.value.clone()))
            .collect();
        let out = merge_theme_values(&original, &pairs);
        // Write both candidates at once: the theme-set-visible path and the
        // live file behind the symlink, so an edit never depends on the link.
        let targets = theme_write_targets(&name);
        let mut saved: Vec<String> = vec![];
        let mut errors: Vec<String> = vec![];
        for target in targets {
            match fs::write(&target, &out) {
                Ok(()) => saved.push(target.display().to_string().chars().take(60).collect()),
                Err(e) => errors.push(format!("{} ({e})", target.display())),
            }
        }
        let result = if saved.is_empty() {
            // The Nix store copy cannot be written directly; point the user
            // at the live-folder link and the relink button instead.
            let tmp = state_dir().join("theme-edit.tmp");
            let staged = fs::create_dir_all(tmp.parent().unwrap())
                .and_then(|_| fs::write(&tmp, &out))
                .is_ok();
            if staged {
                format!(
                    "Theme files are read-only ({}); the draft went to {}.",
                    errors.join("; "),
                    tmp.display()
                )
            } else {
                format!("Save failed: {}", errors.join("; "))
            }
        } else {
            let mut s = Settings::write();
            s.themes = themes();
            if errors.is_empty() {
                format!("Saved to {} location(s).", saved.len())
            } else {
                format!(
                    "Saved to {} location(s); failed: {}",
                    saved.len(),
                    errors.join("; ")
                )
            }
        };
        let mut s = Settings::write();
        s.theme_busy = false;
        s.status = result;
        drop(s);
        if apply {
            apply_theme(name);
        }
    });
}

/// Rebuilds a theme file from the original text with edited values: the
/// KEY=VALUE line layout, values and comments survive; unknown keys append.
fn merge_theme_values(original: &str, fields: &[(String, String)]) -> String {
    let mut used = vec![false; fields.len()];
    let mut out = String::new();
    for line in original.lines() {
        if line.trim_start().starts_with('#') || !line.contains('=') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let Some((prefix, value)) = line.split_once('=') else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let _ = value;
        let key = prefix.trim();
        let found = fields
            .iter()
            .zip(&used)
            .position(|((key2, _), taken)| !*taken && key2 == key);
        if let Some(i) = found {
            used[i] = true;
            out.push_str(prefix);
            out.push('=');
            out.push_str(fields[i].1.trim());
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for ((key, value), taken) in fields.iter().zip(&used) {
        if !*taken && !key.is_empty() {
            out.push_str(&format!("{key}={}\n", value.trim()));
        }
    }
    out
}


fn theme_editor_rows(s: &Settings, width: f32) -> Vec<Box<dyn Widget>> {
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    if let Some(editing) = &s.editing_theme {
        rows.push(Box::new(
            label(format!("Editing · {editing}"))
                .size(17.0)
                .weight(Weight::SemiBold),
        ));
        rows.push(Box::new(
            Row::new(children![
                button("Cancel", 84.0, close_editor),
                button("Save", 84.0, || save_theme_edits(false)),
                button("Save · Apply", 130.0, || save_theme_edits(true)),
            ])
            .gap(8.0),
        ));
        let hint = label(
            "Saving writes the theme file directly to ~/.config/theme-manager/themes.",
        )
        .size(12.0)
        .color(settings_secondary())
        .wrap()
        .max_lines(2);
        let hint_height = hint.height_in(width);
        rows.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(hint_height)
                .child(hint),
        ));
    }
    for (i, field) in s.editor.iter().enumerate() {
        if let Some(group) = group_name(&field.key) {
            rows.push(Box::new(label(group).size(15.0).weight(Weight::SemiBold)));
        }
        rows.push(Box::new(editor_row(s, i, width)));
    }
    rows
}

fn editor_row(s: &Settings, index: usize, width: f32) -> Rectangle {
    let field = &s.editor[index];
    let color = parse_hex(&field.value);
    let id = field.id;
    Rectangle::new()
        .width(width)
        .height(40.0)
        .padding(6.0)
        .radius(10.0)
        .fill(setting_surface())
        .align_child(Start, Center)
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(28.0)
                    .height(20.0)
                    .radius(5.0)
                    .fill(color.unwrap_or(crate::theme::palette().red))
                    .border(if color.is_some() { 0.0 } else { 1.0 }, crate::theme::palette().red),
                label(&field.key).size(12.0),
                Rectangle::new()
                    .width(150.0)
                    .height(26.0)
                    .child(
                        TextInput::new(id)
                            .size(13.0)
                            .color(settings_foreground())
                            .on_change(move |text| {
                                let mut s = Settings::write();
                                if let Some(f) = s.editor.get_mut(index) {
                                    f.value = text;
                                }
                            }),
                    ),
            ])
            .gap(10.0),
        )
}

// ------------------------------------------------------------------ wallpapers

/// Recursively (two levels) lists media files under ~/Pictures/Wallpapers, skipping
/// theme-manager's own generated files. Category is the subfolder name.
fn wallpapers() -> Vec<WallItem> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let root = home.join("Pictures/Wallpapers");
    let mut found = vec![];
    let mut stack = vec![(root.clone(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(list) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in list.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if name.starts_with("current") || name.starts_with("readme") || name.starts_with("README")
            {
                continue;
            }
            if path.is_dir() {
                if depth < 2 {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            let Some(video) = wall_kind(&name) else {
                continue;
            };
            let category = dir
                .file_name()
                .filter(|d| *d != root.file_name().unwrap_or_default())
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_else(|| "walls".into());
            found.push(WallItem {
                path: path.to_string_lossy().into_owned(),
                name,
                category,
                video,
            });
        }
    }
    found.sort_by(|a, b| a.category.cmp(&b.category).then(a.path.cmp(&b.path)));
    found.truncate(60);
    found
}

/// (is a video file?) based on the extension; `None` means unsupported.
fn wall_kind(name: &str) -> Option<bool> {
    let ext = name.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "png" => false,
        "webp" | "gif" | "bmp" | "tga" | "tiff" | "avif" => false,
        "mp4" | "m4v" | "webm" | "mkv" | "mov" | "avi" | "mpg" | "mpeg" => true,
        _ => return None,
    })
}

/// Files the shell decodes natively — everything else needs a generated thumb.
fn wall_native(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png")
}

/// Cached ffmpeg/imagemagick thumbnails: source path → displayable png.
static WALL_THUMBS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static WALL_THUMB_QUEUED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// The image the shell can show for `item`: natives pass through, webp/gif/
/// videos use a generated (cached) thumbnail; queued generations start here.
fn wall_display(item: &WallItem) -> String {
    if !item.video && wall_native(&item.name) {
        return item.path.clone();
    }
    let path = item.path.clone();
    if let Some(thumb) = WALL_THUMBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|m| m.get(&path).cloned())
    {
        return thumb;
    }
    let queued = WALL_THUMB_QUEUED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock();
    let Ok(pending_guard) = queued else {
        return String::new();
    };
    if pending_guard.contains(&path) {
        return String::new();
    }
    drop(pending_guard);
    let mut pending = WALL_THUMB_QUEUED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap();
    if pending.insert(path.clone()) {
        let item = item.clone();
        std::thread::spawn(move || {
            let thumb = generate_wall_thumb(&item);
            let mut map = WALL_THUMBS
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .unwrap();
            if let Some(thumb) = thumb {
                map.insert(item.path.clone(), thumb);
            }
            WALL_THUMB_QUEUED
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
                .unwrap()
                .remove(&item.path);
        });
    }
    String::new()
}

/// Generates (or finds cached) a png snapshot for non-native media.
fn generate_wall_thumb(item: &WallItem) -> Option<String> {
    let dst = wall_thumb_path(&item.path)?;
    if !dst.exists() {
        let ok = if item.video {
            match ffmpeg_bin() {
                Some(ffmpeg) => Command::new(ffmpeg)
                    .args(["-y", "-v", "error", "-ss", "1", "-i", &item.path, "-frames:v", "1"])
                    .arg(&dst)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false),
                None => false,
            }
        } else {
            match magick_bin() {
                Some(magick) => Command::new(magick)
                    .arg(format!("{}[0]", item.path))
                    .arg(&dst)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false),
                None => match ffmpeg_bin() {
                    Some(ffmpeg) => Command::new(ffmpeg)
                        .args(["-y", "-v", "error", "-i", &item.path, "-frames:v", "1"])
                        .arg(&dst)
                        .stdin(std::process::Stdio::null())
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false),
                    None => false,
                },
            }
        };
        if !ok {
            return None;
        }
    }
    Some(dst.to_string_lossy().into_owned())
}

/// Stable cache path for a source file's thumbnail.
fn wall_thumb_path(src: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let dir = state_dir().join("wall-thumbs");
    let _ = fs::create_dir_all(&dir);
    let name = std::path::Path::new(src)
        .file_stem()?
        .to_string_lossy()
        .into_owned();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hash);
    Some(dir.join(format!("{}-{:08x}.png", name, hash.finish())))
}

static FFMPEG_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();
static MAGICK_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();

/// ffmpeg from PATH, else a store checkout found by glob.
fn ffmpeg_bin() -> Option<PathBuf> {
    FFMPEG_BIN
        .get_or_init(|| resolve_bin("ffmpeg").or_else(|| store_bin("ffmpeg", "bin/ffmpeg")))
        .clone()
}

/// imagemagick from PATH, else a store checkout found by glob.
fn magick_bin() -> Option<PathBuf> {
    MAGICK_BIN
        .get_or_init(|| resolve_bin("magick").or_else(|| store_bin("imagemagick", "bin/magick")))
        .clone()
}

/// A plain PATH lookup that also tolerates a missing binary.
fn resolve_bin(name: &str) -> Option<PathBuf> {
    let probed = Command::new(name).arg("-version").output();
    if probed.map(|o| o.status.success()).unwrap_or(false) {
        return Some(PathBuf::from(name));
    }
    None
}

fn store_bin(prefix: &str, tail: &str) -> Option<PathBuf> {
    let store = PathBuf::from("/nix/store");
    let mut candidates: Vec<PathBuf> = fs::read_dir(&store)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with(prefix))
                .unwrap_or(false)
        })
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .rev()
        .map(|p| p.join(tail))
        .find(|bin| bin.exists())
}

/// The raw WALLPAPER value recorded by the theme daemon ("none" until a wall
/// has been applied or a theme declares one).
fn active_wallpaper() -> String {
    let path = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/state/theme/active.json");
    let text = fs::read_to_string(path).unwrap_or_default();
    text.split_once("\"WALLPAPER\"")
        .and_then(|(_, tail)| tail.split_once(':'))
        .map(|(_, v)| {
            v.trim_start()
                .trim_start_matches('"')
                .split('"')
                .next()
                .unwrap_or("")
        })
        .unwrap_or("")
        .to_string()
}

/// Carousel start position; the slide that matches the applied wallpaper.
fn active_wallpaper_index(walls: &[WallItem]) -> usize {
    let active = active_wallpaper();
    if active.is_empty() || active == "none" {
        return 0;
    }
    walls
        .iter()
        .position(|w| w.path == active)
        .or_else(|| {
            let stem = std::path::Path::new(&active)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned());
            walls.iter().position(|w| {
                std::path::Path::new(&w.path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .as_deref()
                    == stem.as_deref()
            })
        })
        .unwrap_or(0)
}

/// Sets the current theme's WALLPAPER variable (or clears it), re-applies the
/// theme, and repaints directly: awww for stills, mpvpaper for videos. Dynamic
/// palettes are regenerated by theme-set from the new image.
fn set_wallpaper(image: Option<String>, video: bool) {
    std::thread::spawn(move || {
        let theme = crate::theme::current_name();
        if theme.is_empty() {
            let mut s = Settings::write();
            s.status = "No active theme — apply one first.".into();
            return;
        }
        {
            let mut s = Settings::write();
            if s.theme_busy {
                return;
            }
            s.theme_busy = true;
            s.status = format!("Switching wallpaper for {theme}…");
        }
        let value = image.as_deref().unwrap_or("none");
        let file = theme_file(&theme);
        let original = fs::read_to_string(&file).unwrap_or_default();
        let mut out = String::new();
        let mut replaced = false;
        for line in original.lines() {
            let key = line.split('=').next().map(str::trim).unwrap_or("");
            if key == "WALLPAPER" {
                out.push_str(&format!("WALLPAPER={value}\n"));
                replaced = true;
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !replaced {
            out.push_str(&format!("WALLPAPER={value}\n"));
        }
        // Dual write: the theme-set-visible copy and the live file, both
        // updated with the new WALLPAPER so theme-set always sees it.
        let mut wrote_any = false;
        let mut failures: Vec<String> = vec![];
        for target in theme_write_targets(&theme) {
            match fs::write(&target, &out) {
                Ok(()) => wrote_any = true,
                Err(e) => failures.push(format!("{} ({e})", target.display())),
            }
        }
        if !wrote_any {
            let mut s = Settings::write();
            s.status = format!(
                "Could not write {theme}: {}; theme files are only writable when themesDir links a live folder.",
                failures.join("; ")
            );
            s.theme_busy = false;
            return;
        }
        {
            let mut s = Settings::write();
            s.themes = themes();
            if failures.is_empty() {
                s.status = format!("Wallpaper saved — running theme-set for {theme}…");
            } else {
                s.status = format!(
                    "Wallpaper saved ({} failed) — running theme-set for {theme}…",
                    failures.join("; ")
                );
            }
        }
        // Synchronous apply: theme-set regenerates every config, then awww
        // paints the image — the pinned theme-manager paints its wallpaper via
        // Quickshell, which is absent on this machine, so the repaint is ours.
        let mut notes: Vec<String> = vec![];
        match Command::new("theme-set").arg(&theme).output() {
            Ok(o) if o.status.success() => {
                note_theme_applied(&theme, &mut notes);
            }
            Ok(o) => notes.push(format!(
                "theme-set failed: {}",
                String::from_utf8_lossy(&o.stderr)
                    .chars()
                    .take(120)
                    .collect::<String>()
            )),
            Err(e) => notes.push(format!("Cannot launch theme-set: {e}")),
        }
        notes.push(paint_wallpaper(image.as_deref(), video));
        let mut s = Settings::write();
        s.status = notes.join(" ");
        s.theme_busy = false;
    });
}

/// theme-daemon applies its config generation async after the state write; a
/// short wait with confirmation keeps the status honest about "applied".
fn note_theme_applied(theme: &str, notes: &mut Vec<String>) {
    for _ in 0..30 {
        if crate::theme::current_name() == theme {
            notes.push(format!("theme-set applied {theme}."));
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    notes.push(format!(
        "theme-set requested {theme}, but theme-daemon has not confirmed it."
    ));
}

/// Paints (or clears) the wallpaper.
///
/// Upstream drives awww (stills) and mpvpaper (videos); neither is installed
/// here and this machine paints with hyprlax instead. Rather than duplicate
/// that, hand the file to the script that already owns the whole sequence --
/// hyprlax layer swap, the rotator's state file,
/// and the lock-bg symlink hyprlock needs -- so the island and SUPER+W stay in
/// agreement about what the current wallpaper is.
fn paint_wallpaper(image: Option<&str>, video: bool) -> String {
    let Some(file) = image else {
        return "Wallpaper cleared.".into();
    };
    if video {
        return "Video wallpapers need mpvpaper; hyprlax paints stills only.".into();
    }
    let Some(home) = std::env::var_os("HOME") else {
        return "HOME is not set.".into();
    };
    let script = PathBuf::from(home).join(".config/hypr/scripts/wallpaper-picker.sh");
    match Command::new(&script).arg("--set").arg(file).output() {
        Ok(o) if o.status.success() => "Painted with hyprlax.".into(),
        Ok(o) => {
            let message = String::from_utf8_lossy(&o.stderr);
            let message = message.trim();
            if message.is_empty() {
                "wallpaper-picker.sh --set failed.".into()
            } else {
                format!("wallpaper-picker.sh: {}", message.chars().take(160).collect::<String>())
            }
        }
        Err(_) => format!("{} is not executable.", script.display()),
    }
}


fn wallpaper_root() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Pictures/Wallpapers")
}

/// Opens the wallpapers folder in the desktop file manager.
fn open_wallpaper_folder() {
    let root = wallpaper_root();
    let _ = fs::create_dir_all(&root);
    let _ = Command::new("xdg-open")
        .arg(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Rescans the wallpaper folders, keeping the current selection when possible.
fn refresh_wallpapers() {
    let mut s = Settings::write();
    let previous = s.wallpapers.get(s.wallpaper_index).map(|w| w.path.clone());
    s.wallpapers = wallpapers();
    s.wallpaper_index = previous
        .and_then(|path| s.wallpapers.iter().position(|w| w.path == path))
        .unwrap_or_else(|| active_wallpaper_index(&s.wallpapers));
    let count = s.wallpapers.len();
    s.wallpaper_index = s.wallpaper_index.min(count.saturating_sub(1));
    let indices = wall_indices(&s);
    if !indices.contains(&s.wallpaper_index) {
        s.wallpaper_index = indices.first().copied().unwrap_or(0);
    }
    s.status = format!(
        "Wallpaper library refreshed · {count} file{}.",
        if count == 1 { "" } else { "s" }
    );
}

/// Indices into s.wallpapers that match the sidebar folder filter.
fn wall_indices(s: &Settings) -> Vec<usize> {
    s.wallpapers
        .iter()
        .enumerate()
        .filter(|(_, w)| s.wallpaper_filter == "all" || s.wallpaper_filter == w.category)
        .map(|(i, _)| i)
        .collect()
}

/// Jumps the featured preview to a gallery row.
fn select_wallpaper(index: usize) {
    let mut s = Settings::write();
    if index < s.wallpapers.len() {
        s.wallpaper_index = index;
    }
}

/// Switches the sidebar folder filter (clicking the active filter goes back to
/// "All") and keeps the carousel on a visible slide.
fn filter_wallpaper(category: String) {
    let mut s = Settings::write();
    s.wallpaper_filter = if s.wallpaper_filter == category {
        "all".into()
    } else {
        category
    };
    let indices = wall_indices(&s);
    if !indices.is_empty() && !indices.contains(&s.wallpaper_index) {
        s.wallpaper_index = indices[0];
    }
}

/// Trashes the selected wallpaper from ~/Pictures/Wallpapers (recoverable via the
/// desktop Trash) and refreshes the list.
fn remove_wallpaper(index: usize) {
    let item = {
        let mut s = Settings::write();
        let item = s.wallpapers.get(index).cloned();
        if let Some(item) = &item {
            s.status = format!("Removing {}…", item.name);
        }
        item
    };
    let Some(item) = item else { return };
    std::thread::spawn(move || {
        let was_active = active_wallpaper() == item.path;
        let out = Command::new("gio")
            .args(["trash", "--", &item.path])
            .output();
        let status = match out {
            Ok(o) if o.status.success() => {
                let hint = if was_active {
                    " It was active — apply another one to repaint."
                } else {
                    ""
                };
                format!(
                    "Trashed {} (recoverable in the desktop Trash).{}",
                    item.name, hint
                )
            }
            Ok(o) => format!(
                "Could not trash {}: {}",
                item.name,
                String::from_utf8_lossy(&o.stderr)
                    .chars()
                    .take(120)
                    .collect::<String>()
            ),
            Err(e) => format!("Could not trash {}: {e}", item.name),
        };
        let mut s = Settings::write();
        s.wallpapers = wallpapers();
        let count = s.wallpapers.len();
        s.wallpaper_index = s.wallpaper_index.min(count.saturating_sub(1));
        let indices = wall_indices(&s);
        if !indices.contains(&s.wallpaper_index) {
            s.wallpaper_index = indices.first().copied().unwrap_or(0);
        }
        s.status = status;
    });
}

/// The height of the featured preview. The gallery beside it matches it.
const WALL_HERO_H: f32 = 220.0;

/// Text drawn over the featured image, always light regardless of the theme.
fn wallpaper_hero_text(text: impl Into<String>, size: f32) -> Text {
    Text::new(text).size(size).color(Color::WHITE)
}

/// A small accent link used for secondary wallpaper actions.
fn wallpaper_link(text: &str, color: Color, action: impl Fn() + 'static) -> Rectangle {
    let key = format!("wall-link:{text}");
    let ck = key.clone();
    let content = Text::new(text)
        .size(11.0)
        .weight(Weight::SemiBold)
        .color(color);
    let width = match Widget::width(&content) {
        amane::Size::Fixed(width) => width + 10.0,
        _ => 80.0,
    };
    Rectangle::new()
        .width(width)
        .height(22.0)
        .radius(8.0)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&ck))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .align_child(Center, Center)
        .child(content)
}

/// A filled primary action, full width beneath the featured preview.
fn wallpaper_primary(text: &str, width: f32, action: impl Fn() + 'static) -> Rectangle {
    let key = format!("wall-action:{text}");
    let ck = key.clone();
    let fill = blend_color(crate::theme::accent(), crate::theme::background(), 0.14);
    let color = crate::theme::readable(crate::theme::background(), fill, 4.5);
    let content = Text::new(text)
        .size(12.0)
        .weight(Weight::SemiBold)
        .color(color)
        .elide();
    Rectangle::new()
        .width(width)
        .height(38.0)
        .radius(12.0)
        .fill(fill)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&ck))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .align_child(Center, Center)
        .child(
            Rectangle::new()
                .width(width - 20.0)
                .height(20.0)
                .align_child(Center, Center)
                .child(content),
        )
}

/// A compact status pill drawn over a wallpaper image.
fn wallpaper_badge(text: &str, width: f32, fill: Color) -> Rectangle {
    let color = crate::theme::readable(crate::theme::background(), fill, 4.5);
    Rectangle::new()
        .width(width)
        .height(22.0)
        .radius(Full)
        .fill(fill)
        .align_child(Center, Center)
        .child(
            Text::new(text)
                .size(9.5)
                .weight(Weight::SemiBold)
                .color(color),
        )
}

/// A soft theme-coloured fill for media that has no decoded preview yet.
fn wallpaper_placeholder(width: f32, height: f32, text: &str, radius: f32) -> Rectangle {
    let palette = crate::theme::palette();
    let _ = palette;
    let top = crate::theme::card_raised();
    let bottom = crate::theme::card();
    Rectangle::new()
        .width(width)
        .height(height)
        .radius(radius)
        .clip()
        .fill(amane::Gradient::linear(
            135.0,
            [(0.0, top), (1.0, bottom)],
        ))
        .align_child(Center, Center)
        .child(
            Text::new(text)
                .size(11.0)
                .color(crate::theme::readable(Color::WHITE, top, 3.0)),
        )
}

fn wallpaper_chip_label(label: &str, count: usize) -> String {
    format!("{label}  {count}")
}

fn wallpaper_chip_width(text: &str) -> f32 {
    let content = Text::new(text).size(11.0);
    let width = match Widget::width(&content) {
        amane::Size::Fixed(width) => width,
        _ => text.chars().count() as f32 * 6.8,
    };
    (width + 22.0).clamp(56.0, 176.0)
}

/// A folder/category filter chip. `filter` is the value stored in settings,
/// which is distinct from the human-readable label ("all" vs "All").
fn wallpaper_chip(
    label: &str,
    filter: String,
    count: usize,
    active_filter: bool,
) -> Rectangle {
    let key = format!("wall-filter:{filter}");
    let ck = key.clone();
    let text = wallpaper_chip_label(label, count);
    let width = wallpaper_chip_width(&text);
    let accent = crate::theme::accent();
    let fill = if active_filter {
        blend_color(accent, crate::theme::background(), 0.12)
    } else {
        setting_surface()
    };
    let color = if active_filter {
        crate::theme::readable(crate::theme::background(), fill, 4.5)
    } else {
        settings_foreground()
    };
    let content = Text::new(text)
        .size(11.0)
        .weight(if active_filter {
            Weight::SemiBold
        } else {
            Weight::Regular
        })
        .color(color)
        .elide();
    Rectangle::new()
        .width(width)
        .height(28.0)
        .radius(14.0)
        .fill(fill)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&ck))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                filter_wallpaper(filter.clone());
            }
        })
        .align_child(Center, Center)
        .child(
            Rectangle::new()
                .width(width - 14.0)
                .height(17.0)
                .align_child(Center, Center)
                .child(content),
        )
}

/// The featured preview with an image scrim, filename, badges and apply action.
fn wallpaper_hero(s: &Settings, width: f32, active: &str) -> Column {
    let index = s
        .wallpaper_index
        .min(s.wallpapers.len().saturating_sub(1));
    let item = s.wallpapers.get(index).cloned();
    let selected_active = item
        .as_ref()
        .is_some_and(|w| w.path == active);
    let display = item
        .as_ref()
        .map(wall_display)
        .unwrap_or_default();

    let mut layers: Vec<Box<dyn Widget>> = vec![];
    if display.is_empty() {
        let text = match &item {
            Some(w) if w.video => "Preparing video preview…",
            Some(_) => "Preparing preview…",
            None => "No wallpapers in this folder",
        };
        layers.push(Box::new(wallpaper_placeholder(
            width,
            WALL_HERO_H,
            text,
            16.0,
        )));
    } else {
        layers.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(WALL_HERO_H)
                .radius(16.0)
                .clip()
                .fill(
                    amane::Image::cover(display)
                        .thumbnail(width as u32, WALL_HERO_H as u32),
                ),
        ));
    }

    let mut badges = 12.0;
    if selected_active {
        layers.push(Box::new(
            wallpaper_badge("ACTIVE", 62.0, crate::theme::accent())
                .translate(badges, 12.0),
        ));
        badges += 68.0;
    }
    if let Some(w) = &item {
        if w.video {
            layers.push(Box::new(
                wallpaper_badge("VIDEO", 58.0, Color::rgba(0, 0, 0, 170))
                    .translate(badges, 12.0),
            ));
        }
    }

    if let Some(w) = &item {
        // A dark gradient keeps the caption readable over bright wallpapers.
        layers.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(96.0)
                .translate(0.0, WALL_HERO_H - 96.0)
                .fill(amane::Gradient::linear(
                    0.0,
                    [
                        (0.0, Color::rgba(0, 0, 0, 205)),
                        (1.0, Color::rgba(0, 0, 0, 0)),
                    ],
                )),
        ));
        let detail = format!(
            "{}  ·  {}",
            w.category,
            if w.video { "Video" } else { "Image" }
        );
        let caption = Column::new(children![
            Rectangle::new()
                .width((width - 28.0).max(20.0))
                .height(18.0)
                .child(wallpaper_hero_text(w.name.clone(), 14.0).weight(Weight::SemiBold).elide()),
            Rectangle::new()
                .width((width - 28.0).max(20.0))
                .height(14.0)
                .child(
                    wallpaper_hero_text(detail, 10.5)
                        .color(Color::rgba(255, 255, 255, 190))
                        .elide(),
                ),
        ])
        .gap(2.0)
        .width((width - 28.0).max(20.0));
        layers.push(Box::new(
            Rectangle::new()
                .width((width - 28.0).max(20.0))
                .height(38.0)
                .translate(14.0, WALL_HERO_H - 50.0)
                .child(caption),
        ));
    }

    if selected_active {
        layers.push(Box::new(
            Rectangle::new()
                .width(width)
                .height(WALL_HERO_H)
                .radius(16.0)
                .border(2.0, crate::theme::accent()),
        ));
    }

    let key = format!(
        "wall-hero:{}",
        item.as_ref().map(|w| w.path.as_str()).unwrap_or("empty")
    );
    let ck = key.clone();
    let apply = item.clone();
    let mut preview = Rectangle::new()
        .width(width)
        .height(WALL_HERO_H)
        .radius(16.0)
        .clip()
        .fill(setting_surface())
        .scale(crate::feedback::scale(&key));
    if let Some(item) = apply {
        preview = preview.cursor(Pointer).on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                set_wallpaper(Some(item.path.clone()), item.video);
            }
        });
    }
    let preview = preview.child(
        amane::Stack::new(layers)
            .width(width)
            .height(WALL_HERO_H),
    );

    let mut actions: Vec<Box<dyn Widget>> = vec![];
    match &item {
        Some(w) => {
            let apply = w.clone();
            let label = if selected_active {
                "Reapply wallpaper"
            } else if w.video {
                "Play video wallpaper"
            } else {
                "Apply wallpaper"
            };
            actions.push(Box::new(wallpaper_primary(label, width, move || {
                set_wallpaper(Some(apply.path.clone()), apply.video);
            })));
            let remove_index = index;
            actions.push(Box::new(
                Row::new(children![
                    wallpaper_link("Stop wallpaper", settings_secondary(), || {
                        set_wallpaper(None, false)
                    }),
                    wallpaper_link(
                        "Remove file",
                        crate::theme::readable(
                            crate::theme::palette().red,
                            setting_surface(),
                            4.5
                        ),
                        move || remove_wallpaper(remove_index),
                    ),
                ])
                .gap(14.0)
                .justify(amane::SpaceBetween)
                .width(width),
            ));
        }
        None => {
            actions.push(Box::new(wallpaper_primary(
                "Open wallpapers folder",
                width,
                open_wallpaper_folder,
            )));
            actions.push(Box::new(
                Row::new(children![
                    wallpaper_link(
                        "Rescan library",
                        crate::theme::accent(),
                        refresh_wallpapers
                    ),
                    wallpaper_link("Stop wallpaper", settings_secondary(), || {
                        set_wallpaper(None, false)
                    }),
                ])
                .gap(14.0)
                .justify(amane::SpaceBetween)
                .width(width),
            ));
        }
    }

    let mut rows: Vec<Box<dyn Widget>> = vec![Box::new(preview)];
    rows.extend(actions);
    Column::new(rows).width(width).gap(9.0)
}

/// A single row in the gallery: thumbnail, name, folder/type and active dot.
fn wallpaper_gallery_row(
    s: &Settings,
    index: usize,
    width: f32,
    active: &str,
) -> Rectangle {
    let Some(item) = s.wallpapers.get(index).cloned() else {
        return Rectangle::new().width(width).height(0.0);
    };
    let display = wall_display(&item);
    let selected = s.wallpaper_index == index;
    let is_active = item.path == active;
    let key = format!("wall-row:{}", item.path);
    let ck = key.clone();
    let thumb_w = 66.0;
    let thumb_h = 44.0;
    let fill = if selected {
        crate::theme::card_raised()
    } else {
        setting_surface()
    };
    let thumb = if display.is_empty() {
        wallpaper_placeholder(thumb_w, thumb_h, "…", 8.0)
    } else {
        Rectangle::new()
            .width(thumb_w)
            .height(thumb_h)
            .radius(8.0)
            .clip()
            .fill(amane::Image::cover(display).thumbnail(thumb_w as u32, thumb_h as u32))
    };
    let info_w = (width - 14.0 - thumb_w - 10.0 - if is_active { 18.0 } else { 0.0 })
        .max(36.0);
    let detail = format!(
        "{}  ·  {}",
        item.category,
        if item.video { "Video" } else { "Image" }
    );
    let info = Column::new(children![
        Rectangle::new()
            .width(info_w)
            .height(16.0)
            .child(
                label(item.name.clone())
                    .size(12.0)
                    .weight(if selected {
                        Weight::SemiBold
                    } else {
                        Weight::Regular
                    })
                    .elide(),
            ),
        Rectangle::new()
            .width(info_w)
            .height(13.0)
            .child(
                label(detail)
                    .size(10.0)
                    .color(settings_secondary())
                    .elide(),
            ),
    ])
    .gap(2.0)
    .width(info_w);
    let mut contents: Vec<Box<dyn Widget>> = vec![
        Box::new(thumb) as Box<dyn Widget>,
        Box::new(info) as Box<dyn Widget>,
    ];
    if is_active {
        contents.push(Box::new(
            Rectangle::new()
                .width(8.0)
                .height(8.0)
                .radius(Full)
                .fill(crate::theme::accent()),
        ));
    }

    let mut row = Rectangle::new()
        .width(width)
        .height(58.0)
        .padding(7.0)
        .radius(12.0)
        .fill(fill)
        .cursor(Pointer)
        .scale(crate::feedback::scale(&key));
    if selected {
        row = row.border(
            1.0,
            blend_color(
                crate::theme::accent(),
                crate::theme::background(),
                0.35,
            ),
        );
    }
    row.on_click(move |b| {
        if b == Button::Left {
            crate::feedback::click(&ck);
            select_wallpaper(index);
        }
    })
    .child(Row::new(contents).gap(10.0).align(Center))
}

/// The scannable wallpaper list beside the featured preview.
fn wallpaper_gallery(s: &Settings, width: f32, active: &str) -> Column {
    let indices = wall_indices(s);
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    for index in &indices {
        rows.push(Box::new(wallpaper_gallery_row(
            s, *index, width, active,
        )));
    }
    let list: Box<dyn Widget> = if rows.is_empty() {
        Box::new(
            Rectangle::new()
                .width(width)
                .height(110.0)
                .radius(14.0)
                .fill(setting_surface())
                .align_child(Center, Center)
                .child(
                    Column::new(children![
                        label("No media files here").size(12.0).weight(Weight::SemiBold),
                        label("Add images or videos to ~/Pictures/Wallpapers")
                            .size(10.0)
                            .color(settings_secondary()),
                        wallpaper_link(
                            "Open folder",
                            crate::theme::accent(),
                            open_wallpaper_folder
                        ),
                    ])
                    .gap(5.0)
                    .align(Center),
                ),
        )
    } else {
        Box::new(
            ScrollArea::new(
                "wall-gallery-scroll",
                Column::new(rows).width(width - 8.0).gap(6.0),
            )
            .width(width)
            .height(WALL_HERO_H + 39.0),
        )
    };
    let heading = Row::new(children![
        Column::new(children![
            label("Gallery").size(13.0).weight(Weight::SemiBold),
            label(format!("{} shown", indices.len()))
                .size(10.0)
                .color(settings_secondary()),
        ])
        .gap(1.0),
        Row::new(children![
            wallpaper_link(
                "Rescan",
                crate::theme::accent(),
                refresh_wallpapers
            ),
            wallpaper_link("Open", settings_secondary(), open_wallpaper_folder),
        ])
        .gap(8.0),
    ])
    .justify(amane::SpaceBetween)
    .align(Center)
    .width(width);
    Column::new(vec![Box::new(heading) as Box<dyn Widget>, list])
        .gap(8.0)
        .width(width)
}

/// Filters, featured preview and gallery. This replaces the old carousel so the
/// selected wallpaper stays visible while browsing the whole folder.
fn wallpaper_section(s: &Settings, width: f32) -> Column {
    let side_w = (width * 0.38).clamp(188.0, 244.0);
    // the row is hero, a 1px divider and the gallery with two 14px gaps
    let main_w = (width - side_w - 1.0 - 14.0 * 2.0).max(180.0);

    let mut categories: Vec<String> = s.wallpapers.iter().map(|w| w.category.clone()).collect();
    categories.sort();
    categories.dedup();
    let mut chips: Vec<(Box<dyn Widget>, f32)> = vec![];
    let all_text = wallpaper_chip_label("All", s.wallpapers.len());
    chips.push((
        Box::new(wallpaper_chip(
            "All",
            "all".into(),
            s.wallpapers.len(),
            s.wallpaper_filter == "all",
        )),
        wallpaper_chip_width(&all_text),
    ));
    for cat in &categories {
        let count = s
            .wallpapers
            .iter()
            .filter(|w| &w.category == cat)
            .count();
        if count == 0 {
            continue;
        }
        let text = wallpaper_chip_label(cat, count);
        chips.push((
            Box::new(wallpaper_chip(
                cat,
                cat.clone(),
                count,
                s.wallpaper_filter == *cat,
            )),
            wallpaper_chip_width(&text),
        ));
    }
    let mut chip_rows: Vec<Box<dyn Widget>> = vec![];
    let mut row: Vec<Box<dyn Widget>> = vec![];
    let mut row_width = 0.0;
    for (chip, chip_width) in chips {
        if !row.is_empty() && row_width + 6.0 + chip_width > width {
            chip_rows.push(Box::new(Row::new(row).gap(6.0)));
            row = vec![];
            row_width = 0.0;
        }
        if !row.is_empty() {
            row_width += 6.0;
        }
        row.push(chip);
        row_width += chip_width;
    }
    if !row.is_empty() {
        chip_rows.push(Box::new(Row::new(row).gap(6.0)));
    }

    let head = Column::new(chip_rows).gap(6.0);
    let active = active_wallpaper();
    let parts: Vec<Box<dyn Widget>> = children![
        wallpaper_hero(s, main_w, &active),
        Rectangle::new().width(1.0).height(WALL_HERO_H + 62.0).fill(setting_surface()),
        wallpaper_gallery(s, side_w, &active)
    ];

    Column::new(children![
        head,
        Row::new(parts).gap(14.0).align(Start)
    ])
    .gap(12.0)
}

fn section_label(title: &str, detail: &str, width: f32) -> Row {
    Row::new(children![
        label(title).size(16.0).weight(Weight::SemiBold),
        Rectangle::new()
            .width((width - 150.0).max(20.0))
            .height(24.0)
            .align_child(End, Center)
            .child(label(detail).size(12.0).color(settings_secondary()))
    ])
    .gap(10.0)
    .align(Center)
}
fn info_row(
    glyph: &str,
    title: &str,
    subtitle: &str,
    trailing: &str,
    width: f32,
    accent: Color,
) -> Rectangle {
    let accent = crate::theme::readable(accent, setting_surface(), 3.0);
    let subtitle = label(subtitle)
        .size(12.0)
        .color(settings_secondary())
        .wrap()
        .max_lines(3)
        .elide();
    let sh = subtitle.height_in((width - 160.0).max(90.0));
    Rectangle::new()
        .width(width)
        .height((sh + 38.0).max(68.0))
        .padding(10.0)
        .radius(14.0)
        .fill(setting_surface())
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(38.0)
                    .height(40.0)
                    .align_child(Center, Center)
                    .child(
                        Text::new(glyph)
                            .font("Material Symbols Rounded").weight(amane::Weight::Light)
                            .size(29.0)
                            .tight()
                            .color(accent)
                    ),
                Column::new(children![
                    label(title).size(15.0).weight(Weight::SemiBold).elide(),
                    Rectangle::new()
                        .width((width - 160.0).max(90.0))
                        .height(sh)
                        .child(subtitle)
                ])
                .width((width - 160.0).max(90.0))
                .gap(5.0),
                Rectangle::new()
                    .width(80.0)
                    .height(30.0)
                    .align_child(End, Center)
                    .child(label(trailing).size(12.0).color(accent).elide())
            ])
            .gap(10.0)
            .align(Center),
        )
}
fn network_page(s: &Settings, net: &Network, width: f32) -> Vec<Box<dyn Widget>> {
    use super::symbols as sf;
    let p = crate::theme::palette();
    let fg = settings_foreground();
    let status = network_status::snapshot();
    let inner = width - 24.0;
    let mut rows: Vec<Box<dyn Widget>> = vec![];

    if !status.managed {
        rows.push(Box::new(info_row("\u{e1da}","Wi-Fi controls unavailable","No Wi-Fi manager is running. Wired networking and your networkd configuration are unchanged.","",width,p.yellow)));
    } else {
        // Windows quick-settings header: one big switch for the radio
        let on = net.wifi_enabled();
        let detail = if !on {
            "Off".to_string()
        } else if net.connecting() {
            "Connecting…".to_string()
        } else if !net.ssid().is_empty() {
            format!("Connected to {}", net.ssid())
        } else {
            "On · not connected".to_string()
        };
        rows.push(Box::new(master_row(
            |c| sf::sized(sf::wifi_full(c), 18.0),
            "Wi-Fi",
            &detail,
            on,
            width,
            Box::new(move || Network::set_wifi(!on)),
        )));

        if on {
            if !net.ssid().is_empty() {
                rows.push(Box::new(section_head("Connected", None, width)));
                let strength = net.strength();
                rows.push(Box::new(list_card(
                    vec![list_row(
                        sf::sized(sf::wifi(true, strength, crate::theme::accent()), 22.0),
                        net.ssid(),
                        &format!("Connected · signal {strength}%"),
                        Some(pill("Disconnect", false, Box::new(Network::disconnect))),
                        inner,
                    )],
                    width,
                )));
            }

            rows.push(Box::new(section_head(
                "Networks",
                Some(pill("Refresh", false, Box::new(Network::scan))),
                width,
            )));
            let joining = s.joining.as_ref().map(|j| j.ssid.clone());
            let mut list: Vec<Rectangle> = vec![];
            for ap in net.access_points().iter().filter(|ap| !ap.active()) {
                let name = ap.ssid().to_owned();
                if name.is_empty() {
                    continue;
                }
                let (secured, saved, strength) = (ap.secured(), ap.saved(), ap.strength());
                let detail = if joining.as_deref() == Some(name.as_str()) {
                    "Connecting…"
                } else if saved {
                    "Saved"
                } else if secured {
                    "Secured"
                } else {
                    "Open"
                };
                let right = secured.then(|| sf::sized(sf::lock(settings_secondary()), 14.0));
                let key = format!("ap:{name}");
                let ck = key.clone();
                let pick = name.clone();
                list.push(
                    list_row(sf::sized(sf::wifi(true, strength, fg), 22.0), &name, detail, right, inner)
                        .cursor(Pointer)
                        .scale(crate::feedback::scale(&key))
                        .on_click(move |b| {
                            if b != Button::Left {
                                return;
                            }
                            crate::feedback::click(&ck);
                            if saved || !secured {
                                cancel_join_prompt();
                                join_network(pick.clone(), None, !saved);
                            } else {
                                // Windows-style: the row opens in place for the password
                                TextInput::set_text("amane-wifi-password", "");
                                let mut state = Settings::write();
                                state.ssid = Some(pick.clone());
                                state.password.clear();
                                state.status = String::new();
                            }
                        }),
                );
                if s.ssid.as_deref() == Some(name.as_str()) {
                    list.push(password_block(inner, p.foreground));
                }
            }
            if list.is_empty() {
                list.push(list_row(
                    sf::sized(sf::magnifier(settings_secondary()), 18.0),
                    "No networks found",
                    "Press Refresh to look again",
                    None,
                    inner,
                ));
            }
            rows.push(Box::new(list_card(list, width)));
        }
    }

    // the adapters underneath, compact (Windows "Advanced network settings")
    if !status.links.is_empty() {
        rows.push(Box::new(section_head("Hardware", None, width)));
        let mut list: Vec<Rectangle> = vec![];
        for link in &status.links {
            let (name, rest) = link.split_once(" · ").unwrap_or((link, ""));
            let mut fields = rest.split_whitespace();
            let up = fields.next() == Some("up");
            let address = fields.next().unwrap_or("").to_string();
            let wireless = name.starts_with("wl");
            let icon = if wireless {
                sf::sized(sf::wifi_full(fg), 18.0)
            } else {
                Rectangle::new().width(18.0).height(18.0).align_child(Center, Center).child(
                    Text::new("\u{eb2f}")
                        .font("Material Symbols Rounded")
                        .weight(amane::Weight::Light)
                        .size(18.0)
                        .tight()
                        .color(fg),
                )
            };
            let state = label(if up { "Connected" } else { "Not connected" })
                .size(12.0)
                .color(if up { crate::theme::accent() } else { settings_secondary() });
            let state_w = match Widget::width(&state) {
                amane::Size::Fixed(w) => w,
                _ => 80.0,
            };
            list.push(list_row(
                icon,
                if wireless { "Wi-Fi adapter" } else { "Ethernet" },
                &if address.is_empty() { name.to_string() } else { format!("{name} · {address}") },
                Some(Rectangle::new().width(state_w).height(20.0).align_child(End, Center).child(state)),
                inner,
            ));
        }
        rows.push(Box::new(list_card(list, width)));
    }
    rows
}

/// The password field and buttons a secured network's row opens into.
fn password_block(width: f32, text: Color) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(92.0)
        .padding(amane::Padding { left: 40.0, right: 0.0, top: 4.0, bottom: 8.0 })
        .child(
            Column::new(children![
                Rectangle::new()
                    .width(width - 40.0)
                    .height(38.0)
                    .padding(9.0)
                    .radius(10.0)
                    .fill(crate::theme::card_raised())
                    .border(1.0, crate::theme::hairline())
                    .child(
                        TextInput::new("amane-wifi-password")
                            .password()
                            .size(14.0)
                            .color(text)
                            .placeholder("Network security key")
                            .on_change(|v| Settings::write().password = v)
                            .on_submit(|_| connect_selected()),
                    ),
                Row::new(children![
                    pill("Connect", true, Box::new(connect_selected)),
                    pill("Cancel", false, Box::new(cancel_join_prompt))
                ])
                .gap(8.0)
            ])
            .gap(8.0),
        )
}

fn bluetooth_page(bt: &Bluetooth, width: f32) -> Vec<Box<dyn Widget>> {
    use super::symbols as sf;
    let p = crate::theme::palette();
    let fg = settings_foreground();
    let inner = width - 24.0;
    let mut rows: Vec<Box<dyn Widget>> = vec![];
    if !bt.available() {
        rows.push(Box::new(info_row(
            "\u{e1da}",
            "No adapter detected",
            "Bluetooth requires a supported adapter and the BlueZ service.",
            "",
            width,
            p.yellow,
        )));
        return rows;
    }
    let on = bt.powered();
    let connected = bt.devices().iter().filter(|d| d.connected()).count();
    let detail = if !on {
        "Off".to_string()
    } else if connected > 0 {
        format!("On · {connected} connected")
    } else {
        "On · discoverable as this computer".to_string()
    };
    rows.push(Box::new(master_row(
        |c| sf::sized(sf::bluetooth(c), 18.0),
        "Bluetooth",
        &detail,
        on,
        width,
        Box::new(move || Bluetooth::set_powered(!on)),
    )));
    if bt.power_failed() {
        rows.push(Box::new(info_row(
            "\u{e1da}",
            "Couldn't turn Bluetooth on",
            "The adapter stayed off. A hardware switch may be blocking it.",
            "",
            width,
            p.yellow,
        )));
    }
    if !on {
        return rows;
    }
    let scanning = bt.scanning();
    for paired_group in [true, false] {
        let devices = bt.devices().iter().filter(|d| d.paired() == paired_group).collect::<Vec<_>>();
        let right = (!paired_group).then(|| {
            pill(
                if scanning { "Stop" } else { "Search" },
                false,
                Box::new(move || if scanning { Bluetooth::stop_scan() } else { Bluetooth::start_scan() }),
            )
        });
        if devices.is_empty() && paired_group {
            continue;
        }
        rows.push(Box::new(section_head(
            if paired_group { "My devices" } else if scanning { "Other devices · searching…" } else { "Other devices" },
            right,
            width,
        )));
        let mut list: Vec<Rectangle> = vec![];
        for d in devices {
            let path = d.path().to_owned();
            let (connected, paired) = (d.connected(), d.paired());
            let glyph = if d.icon().contains("head") || d.icon().contains("audio") {
                "\u{e310}"
            } else if d.icon().contains("phone") {
                "\u{e324}"
            } else if d.icon().contains("keyboard") {
                "\u{e312}"
            } else if d.icon().contains("mouse") {
                "\u{e323}"
            } else {
                "\u{e1a7}"
            };
            let icon = Rectangle::new().width(28.0).height(28.0).radius(Full)
                .fill(if connected { crate::theme::accent() } else { crate::theme::card_raised() })
                .align_child(Center, Center)
                .child(
                    Text::new(glyph)
                        .font("Material Symbols Rounded")
                        .weight(amane::Weight::Light)
                        .size(17.0)
                        .tight()
                        .color(if connected { crate::theme::readable(Color::rgb(255, 255, 255), crate::theme::accent(), 3.0) } else { fg }),
                );
            let state = match (connected, paired, d.battery()) {
                (true, _, Some(b)) => format!("Connected · battery {b}%"),
                (true, _, None) => "Connected".to_string(),
                (false, true, _) => "Not connected".to_string(),
                (false, false, _) => d.address().to_string(),
            };
            let (text, primary): (&'static str, bool) = if connected {
                ("Disconnect", false)
            } else if paired {
                ("Connect", true)
            } else {
                ("Pair", true)
            };
            let action: Box<dyn Fn()> = Box::new(move || {
                if connected {
                    Bluetooth::disconnect(&path)
                } else if paired {
                    Bluetooth::connect(&path)
                } else {
                    Bluetooth::pair(&path);
                    Settings::write().status =
                        "Pairing requested. PIN/passkey devices may need a pairing agent.".into();
                }
            });
            list.push(list_row(icon, d.name(), &state, Some(pill(text, primary, action)), inner));
        }
        if list.is_empty() {
            list.push(list_row(
                sf::sized(sf::magnifier(settings_secondary()), 18.0),
                if scanning { "Looking for devices…" } else { "No other devices" },
                "Make your device discoverable, then press Search",
                None,
                inner,
            ));
        }
        rows.push(Box::new(list_card(list, width)));
    }
    rows
}

// ------------------------------------------------ shared list building blocks

/// A Windows-quick-settings header row: symbol, title, state, big switch.
/// The whole row toggles.
fn master_row(
    symbol: impl Fn(Color) -> Rectangle,
    title: &str,
    detail: &str,
    on: bool,
    width: f32,
    action: Box<dyn Fn()>,
) -> Rectangle {
    let key = format!("master:{title}");
    let ck = key.clone();
    let accent = crate::theme::accent();
    let fg = if on { crate::theme::readable(Color::rgb(255, 255, 255), accent, 3.0) } else { settings_foreground() };
    let text_w = width - 28.0 - 36.0 - 12.0 - 40.0 - 12.0;
    Rectangle::new()
        .width(width)
        .height(64.0)
        .padding(amane::Padding { left: 14.0, right: 14.0, top: 0.0, bottom: 0.0 })
        .radius(14.0)
        .fill(setting_surface())
        .cursor(Pointer)
        .scale(crate::feedback::scale(&key))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .align_child(Start, Center)
        .child(
            Row::new(children![
                Rectangle::new()
                    .width(36.0)
                    .height(36.0)
                    .radius(Full)
                    .fill(if on { accent } else { crate::theme::card_raised() })
                    .align_child(Center, Center)
                    .child(symbol(fg)),
                Rectangle::new().width(text_w).height(40.0).align_child(Start, Center).child(
                    Column::new(children![
                        label(title).size(15.0).weight(Weight::SemiBold),
                        label(detail).size(12.5).color(settings_secondary()).elide()
                    ])
                    .gap(2.0)
                ),
                switch_track(on, accent)
            ])
            .gap(12.0)
            .align(Center),
        )
}

/// A macOS grouped list: rows on one card, hairlines between them.
fn list_card(rows: Vec<Rectangle>, width: f32) -> Rectangle {
    let inner = width - 24.0;
    let mut items: Vec<Box<dyn Widget>> = vec![];
    let count = rows.len();
    for (i, row) in rows.into_iter().enumerate() {
        items.push(Box::new(row));
        if i + 1 < count {
            items.push(Box::new(
                Rectangle::new()
                    .width(inner - 40.0)
                    .height(1.0)
                    .translate(40.0, 0.0)
                    .fill(crate::theme::hairline()),
            ));
        }
    }
    let column = Column::new(items).width(inner);
    let h = match Widget::height(&column) {
        amane::Size::Fixed(h) => h,
        _ => 0.0,
    };
    Rectangle::new()
        .width(width)
        .height(h + 8.0)
        .padding(amane::Padding { left: 12.0, right: 12.0, top: 4.0, bottom: 4.0 })
        .radius(14.0)
        .fill(setting_surface())
        .child(column)
}

/// One list row: icon, title over detail, and an optional control on the right.
fn list_row(icon: Rectangle, title: &str, detail: &str, right: Option<Rectangle>, width: f32) -> Rectangle {
    let right_w = right.as_ref().map_or(0.0, |r| match Widget::width(r) {
        amane::Size::Fixed(w) => w + 8.0,
        _ => 0.0,
    });
    let text_w = (width - 28.0 - 12.0 - right_w).max(60.0);
    let mut cells: Vec<Box<dyn Widget>> = vec![
        Box::new(Rectangle::new().width(28.0).height(28.0).align_child(Center, Center).child(icon)),
        Box::new(Rectangle::new().width(text_w).height(40.0).align_child(Start, Center).child(
            Column::new(children![
                label(title).size(14.0).weight(Weight::Medium).elide(),
                label(detail).size(12.0).color(settings_secondary()).elide()
            ])
            .gap(2.0),
        )),
    ];
    if let Some(r) = right {
        cells.push(Box::new(r));
    }
    Rectangle::new()
        .width(width)
        .height(52.0)
        .align_child(Start, Center)
        .child(Row::new(cells).gap(12.0).align(Center))
}

/// A small button: accent-filled when primary (Windows), neutral otherwise.
fn pill(text: &str, primary: bool, action: Box<dyn Fn()>) -> Rectangle {
    let key = format!("pill:{text}");
    let ck = key.clone();
    let accent = crate::theme::accent();
    let fill = if primary { accent } else { crate::theme::card_raised() };
    let colour = if primary { crate::theme::readable(Color::rgb(255, 255, 255), accent, 3.0) } else { settings_foreground() };
    let t = label(text).size(12.5).weight(Weight::SemiBold).color(colour);
    let w = match Widget::width(&t) {
        amane::Size::Fixed(w) => w,
        _ => 60.0,
    } + 24.0;
    Rectangle::new()
        .width(w)
        .height(28.0)
        .radius(8.0)
        .fill(fill)
        .border(1.0, crate::theme::hairline())
        .cursor(Pointer)
        .scale(crate::feedback::scale(&key))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                action()
            }
        })
        .align_child(Center, Center)
        .child(t)
}

/// A section title with an optional control on its right (Refresh, Search).
fn section_head(title: &str, right: Option<Rectangle>, width: f32) -> Rectangle {
    let t = label(title).size(13.0).weight(Weight::SemiBold).color(settings_secondary());
    let mut layers: Vec<Box<dyn Widget>> = vec![Box::new(
        Rectangle::new().width(width).height(30.0).padding(amane::Padding { left: 4.0, right: 0.0, top: 0.0, bottom: 0.0 }).align_child(Start, End).child(t),
    )];
    if let Some(r) = right {
        layers.push(Box::new(Rectangle::new().width(width).height(30.0).align_child(End, End).child(r)));
    }
    Rectangle::new().width(width).height(30.0).child(amane::Stack::new(layers).width(width).height(30.0))
}

fn visibility_switch(fullscreen: bool, width: f32) -> Rectangle {
    let p = crate::theme::palette();
    let (on, glyph, title, detail, accent) = if fullscreen {
        (
            visibility::over_fullscreen(),
            "\u{e5d0}",
            "Show over fullscreen apps",
            "Keeps the notch and alerts above fullscreen video",
            crate::theme::accent(),
        )
    } else {
        (
            visibility::auto_hide(),
            "\u{e8f4}",
            "Hide when idle",
            "Off keeps the clock notch always showing",
            p.blue,
        )
    };
    toggle_row(glyph, title, detail, width, on, accent, move || {
        if fullscreen {
            visibility::toggle_fullscreen()
        } else {
            visibility::toggle_auto_hide()
        }
    })
}

fn edge_switch(name: &'static str, width: f32) -> Rectangle {
    let on = crate::edge_activation::enabled(name);
    let (glyph, title, detail, accent) = match name {
        "main" => ("\u{e5ce}", "Menu bar & notch", "Top edge", crate::theme::accent()),
        "screenshot" => ("\u{e3b0}", "Screenshots & power", "Bottom-right edge", crate::theme::accent()),
        _ => ("\u{e8b6}", "Search", "Bottom edge", crate::theme::accent()),
    };
    toggle_row(glyph, title, detail, width, on, accent, move || {
        crate::edge_activation::toggle(name)
    })
}

fn sidebar(s: &Settings) -> Column {
    let w = s.sidebar_width.value();
    let expanded = s.sidebar_labels;
    let toggle = Rectangle::new()
        .width(w)
        .height(SIDEBAR_ROW)
        .radius(10.0)
        .fill(Color::TRANSPARENT)
        .cursor(Pointer)
        .align_child(Center, Center)
        .on_click(|b| {
            if b == Button::Left {
                crate::feedback::click("sidebar-toggle");
                let mut st = Settings::write();
                st.sidebar_labels = !st.sidebar_labels;
                let target = if st.sidebar_labels { SIDEBAR_LABELS } else { SIDEBAR_ICONS };
                st.sidebar_width.to(target);
            }
        })
        .scale(crate::feedback::scale("sidebar-toggle"))
        .child(super::symbols::sized(super::symbols::sidebar(settings_foreground()), 18.0));
    let mut items = children![toggle];
    for (section, name) in [
        (Section::Home, "Control Center"),
        (Section::History, "History"),
        (Section::Network, "Network"),
        (Section::Bluetooth, "Bluetooth"),
        (Section::Customize, "Appearance"),
    ] {
        let selected = s.sections[index(section)];
        let accent = if selected {
            crate::theme::accent()
        } else {
            crate::theme::palette().foreground
        };
        let key = format!("nav:{name}");
        let ck = key.clone();
        let tint = if selected {
            accent
        } else {
            blend_color(accent, settings_foreground(), 0.2)
        };
        let symbol = match section {
            Section::Home => super::symbols::sized(super::symbols::control_center(tint), 18.0),
            Section::History => super::symbols::sized(super::symbols::bell(tint), 18.0),
            Section::Network => super::symbols::sized(super::symbols::wifi_full(tint), 18.0),
            Section::Bluetooth => super::symbols::sized(super::symbols::bluetooth(tint), 18.0),
            Section::Customize => super::symbols::sized(super::symbols::palette(tint), 18.0),
        };
        let mut row = children![symbol];
        if expanded && w > 130.0 {
            row.push(Box::new(label(name).size(14.0).weight(Weight::SemiBold)));
        }
        items.push(Box::new(
            Rectangle::new()
                .width(w)
                .height(SIDEBAR_ROW)
                .radius(10.0)
                .fill(if selected {
                    crate::theme::card_raised()
                } else {
                    Color::TRANSPARENT
                })
                .scale(crate::feedback::scale(&key))
                .cursor(Pointer)
                .on_hover(move |inside| {
                    let mut st = Settings::write();
                    st.hovered = if inside { Some(index(section)) } else { None };
                })
                .align_child(
                    if expanded && w > 130.0 {
                        Align::Start
                    } else {
                        Align::Center
                    },
                    Center,
                )
                .padding(amane::Padding { left: 10.0, right: 10.0, top: 0.0, bottom: 0.0 })
                .on_click(move |b| {
                    if b == Button::Left {
                        crate::feedback::click(&ck);
                        section_toggle(section)
                    }
                })
                .child(Row::new(row).gap(10.0).align(Center)),
        ));
    }
    Column::new(items).width(w).gap(SIDEBAR_GAP)
}
pub(super) fn view() -> Rectangle {
    let s = Settings::read();
    let h = s.motion.precise_value().max(0.0);
    // A closed settings page used to build, measure and draw its entire hidden
    // tree on every island frame (including all theme cards).
    if h <= 0.5 && !s.open {
        return Rectangle::new().width(PANEL.0).height(0.0);
    }
    let cw = content_width(&s);
    let selected = s.sections.iter().position(|v| *v).unwrap_or(0);
    let (title, subtitle) = match selected {
        4 => ("Control Center", "Quick toggles & DevOps"),
        0 => ("Notifications", "History & recent activity"),
        1 => ("Network", "Connections & nearby networks"),
        2 => ("Bluetooth", "Devices & discovery"),
        _ => ("Appearance", "Personalize your shell"),
    };
    let heading = Column::new(children![
        label(title).size(20.0).weight(Weight::Bold),
        label(subtitle).size(12.5).color(settings_secondary())
    ])
    .gap(4.0)
    .width(cw);
    let page = Column::new(children![
        heading,
        settings_scroll::view(
            match selected {
                0 => "settings-history-page",
                1 => "settings-network-page",
                2 => "settings-bt-page",
                _ => "settings-custom-page",
            },
            contents(&s),
            cw,
            (h - FRAME_Y * 2.0 - HEADING_H - HEADING_GAP).max(0.0)
        )
    ])
    .gap(HEADING_GAP)
    .width(cw);
    let elapsed = s.page_started.elapsed().as_secs_f32();
    let page_opacity = (elapsed / 0.22).clamp(0.0, 1.0);
    if page_opacity < 1.0 {
        amane::request_frame();
    }
    let page = Rectangle::new()
        .width(cw)
        .height((h - FRAME_Y * 2.0).max(0.0))
        .opacity(page_opacity)
        .translate(8.0 * (1.0 - page_opacity), 0.0)
        .child(page);
    let mut layout = children![
        Row::new(children![sidebar(&s), page])
            .gap(PAGE_GUTTER)
            .align(Start)
    ];
    if !s.sidebar_labels {
        if let Some(i) = s.hovered {
            // `hovered` is a section index; the sidebar lists Control Center
            // first, so name and row come from one table (a 4-entry name list
            // here panicked on the Control Center icon)
            let (name, row) = match i {
                4 => ("Control Center", 0),
                0 => ("History", 1),
                1 => ("Network", 2),
                2 => ("Bluetooth", 3),
                _ => ("Appearance", 4),
            };
            layout.push(Box::new(
                Rectangle::new()
                    .width(140.0)
                    .height(38.0)
                    .radius(12.0)
                    .fill(crate::theme::glass())
                    .border(1.0, crate::theme::hairline())
                    .translate(
                        s.sidebar_width.value() + 8.0,
                        // level with its row: rows start below the toggle
                        (SIDEBAR_ROW + SIDEBAR_GAP) * (row as f32 + 1.0),
                    )
                    .align_child(Center, Center)
                    .child(label(name).weight(Weight::SemiBold)),
            ));
        }
    }
    Rectangle::new()
        .width(PANEL.0)
        .height(h)
        .padding(Padding {
            top: FRAME_Y,
            left: FRAME_LEFT,
            right: FRAME_X - FRAME_LEFT,
            bottom: FRAME_Y,
        })
        // No opacity here: any opacity != 1 makes the renderer draw the whole
        // panel on a window-sized offscreen canvas every frame of the open
        // animation. The clip alone reveals the content as the height grows.
        .clip()
        .on_hover(pointer_presence)
        .on_click(|_| {})
        .child(
            amane::Stack::new(layout)
                .width(PANEL.0 - FRAME_X)
                .height((h - FRAME_Y * 2.0).max(0.0)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidebar_budgets_fit_single_page() {
        // icons only: a comfortable page; with labels: still readable
        assert!(PANEL.0 - FRAME_X - SIDEBAR_ICONS - PAGE_GUTTER >= 340.0);
        assert!(PANEL.0 - FRAME_X - SIDEBAR_LABELS - PAGE_GUTTER >= 260.0);
    }
    #[test]
    fn theme_hex_parser_accepts_current_theme_format() {
        assert_eq!(parse_hex("#81A1C1"), Some(Color::rgb(129, 161, 193)));
        assert_eq!(parse_hex("no"), None);
    }
    #[test]
    fn theme_merge_preserves_layout_and_updates_values() {
        let original = "# aquarium theme\nVARIANT=dark\n\n# base_16\nBASE00 = '#20202A'\nWHITE         =#C6D0E9\nCORNER_RADIUS=20\n";
        let out = merge_theme_values(
            original,
            &[
                ("BASE00".into(), "#131C19".into()),
                ("WHITE".into(), " #444F4C ".into()),
            ],
        );
        assert_eq!(
            out,
            "# aquarium theme\nVARIANT=dark\n\n# base_16\nBASE00 =#131C19\nWHITE         =#444F4C\nCORNER_RADIUS=20\n"
        );
    }
    #[test]
    fn theme_merge_appends_new_keys() {
        let out = merge_theme_values("VARIANT=dark\n", &[("MODE".into(), "dynamic".into())]);
        assert_eq!(out, "VARIANT=dark\nMODE=dynamic\n");
    }
    #[test]
    fn theme_merge_skips_comment_lines() {
        let out = merge_theme_values(
            "# x=y\nBASE00=#20202A",
            &[("BASE00".into(), "#000000".into())],
        );
        assert_eq!(out, "# x=y\nBASE00=#000000\n");
    }
}

// ------------------------------------------------------------ Control Center home

/// One macOS-style tile row: a round symbol button (accent when on) that
/// toggles, then a title and detail; clicking the text opens `page`.
fn tile_row(
    symbol: impl Fn(Color) -> Rectangle,
    title: &str,
    detail: &str,
    on: bool,
    width: f32,
    toggle: Option<Box<dyn Fn()>>,
    page: Option<Section>,
    focused: bool,
) -> Rectangle {
    let key = format!("tile:{title}");
    let ck = key.clone();
    let fg = if on { crate::theme::readable(Color::rgb(255, 255, 255), crate::theme::accent(), 3.0) } else { settings_foreground() };
    let mut knob = Rectangle::new()
        .width(32.0)
        .height(32.0)
        .radius(Full)
        .fill(if on { crate::theme::accent() } else { crate::theme::card_raised() })
        .align_child(Center, Center)
        .child(symbol(fg));
    // keyboard focus ring
    if focused {
        knob = knob.border(2.0, settings_foreground());
    }
    let toggle: Option<std::rc::Rc<dyn Fn()>> = toggle.map(std::rc::Rc::from);
    let tk = format!("{key}:toggle");
    if let Some(toggle) = toggle.clone() {
        let tk = tk.clone();
        knob = knob.cursor(Pointer).scale(crate::feedback::scale(&tk)).on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&tk);
                toggle()
            }
        });
    }
    let text = Column::new(children![
        label(title).size(13.5).weight(Weight::SemiBold),
        label(detail).size(12.0).color(settings_secondary()).elide()
    ])
    .gap(2.0);
    let mut body = Rectangle::new()
        .width(width - 32.0 - 10.0)
        .height(36.0)
        .align_child(Start, Center)
        .child(text);
    if let Some(section) = page {
        body = body.cursor(Pointer).scale(crate::feedback::scale(&ck)).on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                section_toggle(section)
            }
        });
    }
    let row = Rectangle::new()
        .width(width)
        .height(40.0)
        .align_child(Start, Center)
        .child(Row::new(children![knob, body]).gap(10.0).align(Center));
    // a tile with no page of its own (Focus, Keep Awake) is one big switch,
    // like macOS: the text and empty space toggle it too, not just the knob
    match (page, toggle) {
        (None, Some(toggle)) => row.cursor(Pointer).on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&tk);
                toggle()
            }
        }),
        _ => row,
    }
}


/// A group of rows on one neutral card, like a Control Center module.
fn tile_card(width: f32, rows: Vec<Box<dyn Widget>>) -> Rectangle {
    let inner = Column::new(rows).width(width - 24.0).gap(6.0);
    let h = match Widget::height(&inner) {
        amane::Size::Fixed(h) => h,
        _ => 0.0,
    };
    Rectangle::new()
        .width(width)
        .height(h + 24.0)
        .padding(12.0)
        .radius(14.0)
        .fill(setting_surface())
        .child(inner)
}

/// The Control Center's home page. Status the menu bar already shows (AWS
/// profile, kube context, cpu, battery) is not repeated: connectivity and
/// focus are toggles, the ops card is actions that open launcher modes.
fn home_page(net: &Network, bt: &Bluetooth, width: f32, focus: Option<usize>) -> Vec<Box<dyn Widget>> {
    let at = |i: usize| focus == Some(i);
    use super::symbols as sf;
    let inner = width - 24.0;
    let wifi_on = net.wifi_enabled();
    let wifi_detail = if !wifi_on {
        "Off".to_string()
    } else if net.connected() && !net.ssid().is_empty() {
        net.ssid().to_string()
    } else {
        "Not connected".to_string()
    };
    let bt_on = bt.powered();
    let vpn = super::ops::chips().into_iter().find(|(kind, _, _)| *kind == "vpn");
    let quiet = notices::dnd();

    let connectivity = tile_card(
        width,
        vec![
            Box::new(tile_row(
                |c| sf::sized(sf::wifi_full(c), 18.0),
                "Wi-Fi",
                &wifi_detail,
                wifi_on,
                inner,
                Some(Box::new(move || Network::set_wifi(!wifi_on))),
                Some(Section::Network),
                at(FOCUS_WIFI),
            )),
            Box::new(tile_row(
                |c| sf::sized(sf::bluetooth(c), 18.0),
                "Bluetooth",
                if bt_on { "On" } else { "Off" },
                bt_on,
                inner,
                Some(Box::new(move || Bluetooth::set_powered(!bt_on))),
                Some(Section::Bluetooth),
                at(FOCUS_BT),
            )),
            Box::new(tile_row(
                |c| sf::sized(sf::key(c), 18.0),
                "VPN",
                &vpn.as_ref().map_or("Not connected".to_string(), |(_, _, text)| {
                    if text.is_empty() { "Connected".to_string() } else { text.clone() }
                }),
                vpn.is_some(),
                inner,
                None,
                None,
                false,
            )),
        ],
    );
    // Focus and Keep Awake side by side, Battery full width under them
    let half = ((width - 8.0) / 2.0).floor();
    let battery = super::power::battery_detail();
    let (bat_percent, bat_charging) = {
        let b = amane::Battery::read();
        (b.percent(), b.charging() || b.full())
    };
    let awake = super::awake::on();
    let focus_row = Row::new(children![
        tile_card(
            half,
            vec![Box::new(tile_row(
                |c| sf::sized(sf::moon(c), 18.0),
                "Focus",
                if quiet { "Do Not Disturb" } else { "Off" },
                quiet,
                half - 24.0,
                Some(Box::new(move || notices::set_dnd(!quiet))),
                None,
                at(FOCUS_DND),
            ))],
        ),
        tile_card(
            half,
            vec![Box::new(tile_row(
                |c| sf::sized(sf::cup(c), 18.0),
                "Keep Awake",
                if awake { "No auto-lock" } else { "Off" },
                awake,
                half - 24.0,
                Some(Box::new(move || super::awake::set(!awake))),
                None,
                at(FOCUS_AWAKE),
            ))],
        )
    ])
    .gap(8.0);
    let battery_card = tile_card(
        width,
        vec![Box::new(tile_row(
            move |c| {
                Rectangle::new().width(26.0).height(16.0).align_child(Center, Center)
                    .child(sf::battery(bat_percent, bat_charging, c, c))
            },
            &format!("Battery {bat_percent}%"),
            battery.as_deref().unwrap_or("No battery"),
            bat_charging,
            inner,
            None,
            None,
            false,
        ))],
    );
    let fg = settings_foreground();
    // DevOps: four compact square modules in one row, like Control Center's
    // small tiles; each opens its launcher mode
    let gap = 8.0;
    let tile_w = ((width - gap * 3.0) / 4.0).floor();
    let ops = Row::new(children![
        ops_tile(sf::sized(sf::cloud(fg), 20.0), "AWS", tile_w, "aws", at(FOCUS_OPS)),
        ops_tile(sf::sized(sf::helm(fg), 20.0), "Kube", tile_w, "kube", at(FOCUS_OPS + 1)),
        ops_tile(sf::sized(sf::terminal(fg), 20.0), "SSM", tile_w, "ssm", at(FOCUS_OPS + 2)),
        ops_tile(sf::sized(sf::code(fg), 20.0), "GitHub", tile_w, "gh", at(FOCUS_OPS + 3))
    ])
    .gap(gap)
    .align(Center);
    let mut page: Vec<Box<dyn Widget>> = vec![Box::new(connectivity), Box::new(focus_row), Box::new(battery_card), Box::new(ops)];
    if let Some(card) = now_playing(width, focus) {
        page.push(Box::new(card));
    }
    page
}

/// A small square module: symbol over a short label; opens a launcher mode.
fn ops_tile(symbol: Rectangle, name: &str, width: f32, mode: &'static str, focused: bool) -> Rectangle {
    let key = format!("ops:{mode}");
    let ck = key.clone();
    Rectangle::new()
        .width(width)
        .height(64.0)
        // keyboard focus ring (no border when not focused)
        .border(if focused { 2.0 } else { 0.0 }, settings_foreground())
        .radius(14.0)
        .fill(setting_surface())
        .cursor(Pointer)
        .scale(crate::feedback::scale(&key))
        .on_click(move |b| {
            if b == Button::Left {
                crate::feedback::click(&ck);
                open_mode(mode);
            }
        })
        .align_child(Center, Center)
        .child(
            Column::new(children![
                symbol,
                label(name).size(12.0).weight(Weight::Medium)
            ])
            .gap(6.0)
            .align(Center),
        )
}

/// Now Playing, only while something plays or is paused partway through a
/// track (a player that is merely open, or stopped at 0:00, shows nothing).
fn now_playing(width: f32, focus: Option<usize>) -> Option<Rectangle> {
    use super::symbols as sf;
    if !now_playing_visible() {
        return None;
    }
    let media = amane::Media::read();
    let player = media.active()?;
    let (title, artist, art, playing) = (
        player.title().to_string(),
        player.artist().to_string(),
        player.art_url().to_string(),
        player.playing(),
    );
    let (position, length) = (player.position(), player.length());
    drop(media);

    let fg = settings_foreground();
    let inner = width - 24.0;
    let art_path = art.strip_prefix("file://").map(std::path::PathBuf::from).filter(|p| p.is_file());
    let artwork = Rectangle::new().width(52.0).height(52.0).radius(10.0).clip();
    let artwork = match art_path {
        Some(path) => artwork.fill(amane::Image::cover(path)),
        None => artwork
            .fill(crate::theme::accent())
            .align_child(Center, Center)
            .child(sf::sized(sf::music_note(Color::rgb(255, 255, 255)), 24.0)),
    };
    let text_w = inner - 52.0 - 12.0;
    let header = Row::new(children![
        artwork,
        Column::new(children![
            Rectangle::new()
                .width(text_w)
                .height(20.0)
                .align_child(Start, Center)
                .child(label(&title).size(14.0).weight(Weight::SemiBold).elide()),
            Rectangle::new()
                .width(text_w)
                .height(18.0)
                .align_child(Start, Center)
                .child(label(if artist.is_empty() { "Now Playing" } else { artist.as_str() }).size(12.5).color(settings_secondary()).elide())
        ])
        .gap(2.0)
    ])
    .gap(12.0)
    .align(Center);

    let fraction = if length.is_zero() { 0.0 } else { (position.as_secs_f32() / length.as_secs_f32()).clamp(0.0, 1.0) };
    let clock = |d: Duration| format!("{}:{:02}", d.as_secs() / 60, d.as_secs() % 60);
    let timeline = Column::new(children![
        Rectangle::new()
            .width(inner)
            .height(4.0)
            .radius(Full)
            .fill(crate::theme::control_fill(true))
            .child(Rectangle::new().width((inner * fraction).max(4.0)).height(4.0).radius(Full).fill(fg)),
        Row::new(children![
            Rectangle::new().width(inner / 2.0).height(14.0).align_child(Start, Center)
                .child(label(clock(position)).size(11.0).color(settings_secondary())),
            Rectangle::new().width(inner / 2.0).height(14.0).align_child(End, Center)
                .child(label(if length.is_zero() { String::new() } else { clock(length) }).size(11.0).color(settings_secondary()))
        ])
    ])
    .gap(4.0);

    let control = |symbol: Rectangle, key: &'static str, action: fn(), index: usize| {
        Rectangle::new()
            .width(40.0)
            .height(32.0)
            .border(if focus == Some(index) { 2.0 } else { 0.0 }, settings_foreground())
            .radius(8.0)
            .cursor(Pointer)
            .scale(crate::feedback::scale(key))
            .align_child(Center, Center)
            .on_click(move |b| {
                if b == Button::Left {
                    crate::feedback::click(key);
                    action()
                }
            })
            .child(symbol)
    };
    let transport = Rectangle::new().width(inner).height(32.0).align_child(Center, Center).child(
        Row::new(children![
            control(sf::sized(sf::skip(false, fg), 20.0), "np:prev", amane::Media::previous, FOCUS_MEDIA),
            if playing {
                control(sf::sized(sf::pause(fg), 22.0), "np:toggle", amane::Media::play_pause, FOCUS_MEDIA + 1)
            } else {
                control(sf::sized(sf::play(fg), 22.0), "np:toggle", amane::Media::play_pause, FOCUS_MEDIA + 1)
            },
            control(sf::sized(sf::skip(true, fg), 20.0), "np:next", amane::Media::next, FOCUS_MEDIA + 2)
        ])
        .gap(18.0)
        .align(Center),
    );
    Some(tile_card(width, vec![Box::new(header), Box::new(timeline), Box::new(transport)]))
}

// ------------------------------------------- Control Center keyboard focus

const FOCUS_WIFI: usize = 0;
const FOCUS_BT: usize = 1;
const FOCUS_DND: usize = 2;
const FOCUS_AWAKE: usize = 3;
const FOCUS_OPS: usize = 4; // AWS, Kube, SSM, GitHub
const FOCUS_MEDIA: usize = 8; // previous, play/pause, next (when shown)
const OPS_MODES: [&str; 4] = ["aws", "kube", "ssm", "gh"];

fn focus_count() -> usize {
    if now_playing_visible() { FOCUS_MEDIA + 3 } else { FOCUS_MEDIA }
}

/// Opens a launcher mode on the Control Center's own screen and closes it.
fn open_mode(mode: &'static str) {
    let owner = Island::read().owner.clone();
    super::dismiss_outside();
    let args = ["search".to_string(), mode.to_string()];
    match owner {
        Some(m) => crate::auxiliary::command_on(&m, &args),
        None => crate::auxiliary::command(&args),
    };
}

/// Keys on the open Control Center home: Tab / arrows move the focus ring,
/// Enter / Space use the focused control. True when the key was used.
pub(super) fn key(key: amane::Key) -> bool {
    use amane::Key;
    let (open, home, current) = {
        let s = Settings::read();
        (s.open, s.sections[index(Section::Home)], s.kbd)
    };
    if !open || !home {
        return false;
    }
    let count = focus_count();
    let next = |step: isize| -> usize {
        match current {
            None => if step > 0 { 0 } else { count - 1 },
            Some(i) => ((i as isize + step).rem_euclid(count as isize)) as usize,
        }
    };
    match key {
        Key::Tab | Key::Right | Key::Down => Settings::write().kbd = Some(next(1)),
        Key::Left | Key::Up => Settings::write().kbd = Some(next(-1)),
        Key::Enter | Key::Space => {
            let Some(i) = current else { return true };
            match i {
                FOCUS_WIFI => {
                    let on = Network::read().wifi_enabled();
                    Network::set_wifi(!on)
                }
                FOCUS_BT => {
                    let on = Bluetooth::read().powered();
                    Bluetooth::set_powered(!on)
                }
                FOCUS_DND => notices::set_dnd(!notices::dnd()),
                FOCUS_AWAKE => super::awake::set(!super::awake::on()),
                i if (FOCUS_OPS..FOCUS_OPS + 4).contains(&i) => open_mode(OPS_MODES[i - FOCUS_OPS]),
                i if i == FOCUS_MEDIA => amane::Media::previous(),
                i if i == FOCUS_MEDIA + 1 => amane::Media::play_pause(),
                i if i == FOCUS_MEDIA + 2 => amane::Media::next(),
                _ => {}
            }
        }
        _ => return false,
    }
    true
}
