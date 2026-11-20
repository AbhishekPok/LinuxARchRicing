//! Lock screen with a macOS feel: the wallpaper stays sharp under soft top and
//! bottom shading, a big date and clock sit up top, and the account sits at the
//! bottom (avatar, name, a small frosted password pill). A now-playing pill and
//! a status cluster (layout, Wi-Fi, battery, notifications) stay out of the way.
//! Drawn by Amane's own ext-session-lock surface on every monitor and unlocked
//! through PAM, so it has mouse input and nothing forks while it is up.
//!
//! `amane ipc call lock` starts it; `~/.config/hypr/scripts/lock.sh` calls that
//! and falls back to hyprlock when the shell is not running.
//!
//! Locking fades the backdrop up out of black and lifts the content in a beat
//! later. Unlocking cannot animate this surface at all — the compositor takes
//! it away the moment pam accepts the password — so the same picture is carried
//! over by a veil window on the overlay layer, which fades out instead; see
//! `veil` and `lift`.
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use amane::{
    Battery, Button, Center, Color, Column, Full, Gradient, Horizontal, Keyboard, Layer,
    LayerWindow, Lock, Media, Monitor, Network, Pointer, Rectangle, Row, Service, Stack, Text,
    TextInput, Vertical, Weight, Widget, Zone, children,
};

use crate::island::motion::Easing;

const SANS: &str = "JetBrainsMono Nerd Font";
const ICONS: &str = "Material Symbols Rounded";
const FIELD: &str = "lock-password";
const WHITE: Color = Color::rgb(0xff, 0xff, 0xff);

// ─── state that only exists while locked ───

/// What the screen shows, refreshed once a second and only while locked.
struct Info {
    clock: (u32, u32),
    date: String,
    weather: Option<(String, String)>,
    notes: usize,
    offset: i64,
    ticks: u32,
}

impl Service for Info {
    fn new() -> Self {
        let mut info = Self {
            clock: (0, 0),
            date: String::new(),
            weather: None,
            notes: 0,
            offset: utc_offset(),
            ticks: 0,
        };
        info.refresh(true);
        info
    }

    fn interval() -> Duration {
        Duration::from_secs(1)
    }

    fn update(&mut self) -> bool {
        if !Lock::active() {
            return false;
        }
        self.refresh(false)
    }
}

impl Info {
    fn refresh(&mut self, all: bool) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        // the offset only moves with DST; once an hour is plenty
        if self.ticks % 3600 == 0 {
            self.offset = utc_offset();
        }
        let local = now_secs() + self.offset;
        let clock = (((local / 3600) % 24) as u32, ((local / 60) % 60) as u32);
        let mut changed = clock != self.clock;
        if changed || all {
            self.clock = clock;
            self.date = date_line(local);
        }
        if all || self.ticks % 5 == 0 {
            let notes = notification_count();
            changed |= notes != self.notes;
            self.notes = notes;
        }
        if all || self.ticks % 60 == 0 {
            let weather = weather();
            changed |= weather != self.weather;
            self.weather = weather;
        }
        changed
    }
}

/// Lock start (entry motion), wrong-password shake, armed power button, and
/// the typed password (so the → button can submit it).
struct Moments {
    failed_seen: bool,
    failed_at: Option<Instant>,
    armed: Option<(&'static str, Instant)>,
    locked_at: Instant,
    typed: String,
}

static MOMENTS: Mutex<Option<Moments>> = Mutex::new(None);

fn moments<R>(f: impl FnOnce(&mut Moments) -> R) -> R {
    let mut guard = MOMENTS.lock().unwrap_or_else(|e| e.into_inner());
    let m = guard.get_or_insert_with(|| Moments {
        failed_seen: false,
        failed_at: None,
        armed: None,
        // every real lock sets this again, see command
        locked_at: Instant::now(),
        typed: String::new(),
    });
    f(m)
}

fn submit(password: String) {
    if password.is_empty() {
        return;
    }
    TextInput::set_text(FIELD, "");
    moments(|m| m.typed.clear());
    Lock::unlock(&password);
}

// ─── ipc ───

/// `lock` locks now; `lock status` reports whether the session is locked.
pub fn command(args: &[String]) -> String {
    match args.first().map(String::as_str) {
        Some("status") => String::from(if Lock::active() { "locked" } else { "unlocked" }),
        None | Some("now") => {
            if !Lock::active() {
                TextInput::set_text(FIELD, "");
                moments(|m| {
                    m.failed_seen = false;
                    m.failed_at = None;
                    m.armed = None;
                    m.typed.clear();
                    m.locked_at = Instant::now();
                });
                Info::write().refresh(true);
                // the crash supervisor (amane supervise) reads this: a shell
                // that dies while locked must leave a lock behind, not a
                // "lock screen app died" session
                let _ = std::fs::write(locked_marker(), "");
                Lock::start();
            }
            String::from("ok")
        }
        Some(other) => format!("usage: lock [now|status], not {other}"),
    }
}

// ─── view ───

pub fn view(monitor: &Monitor) -> LayerWindow {
    let (w, h) = (monitor.width as f32, monitor.height as f32);
    let p = crate::theme::palette();
    let lock = Lock::read();
    let info = Info::read();

    let (since_lock, shake, typed, armed) = moments(|m| {
        if lock.failed() && !m.failed_seen {
            m.failed_at = Some(Instant::now());
        }
        m.failed_seen = lock.failed();
        // macOS shakes the field side to side, three quick swings that settle
        let shake = m.failed_at.map_or(0.0, |at| {
            let t = at.elapsed().as_secs_f32();
            if t > 0.55 {
                return 0.0;
            }
            amane::request_frame();
            (t * 42.0).sin() * 16.0 * (1.0 - t / 0.55)
        });
        if m.armed.is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(3)) {
            m.armed = None;
        }
        (
            m.locked_at.elapsed().as_secs_f32(),
            shake,
            !m.typed.is_empty(),
            m.armed.map(|(k, _)| k),
        )
    });
    let (back, entry) = entry_motion(since_lock);

    // sizes follow the screen height, so both monitors look the same
    let u = h / 1080.0;
    // the picture comes up out of a slight zoom, then holds still
    let mut layers = backdrop(w, h, 1.0 + 0.05 * (1.0 - back), back);

    // ── top right: the status cluster ──
    layers.push(Box::new(
        Rectangle::new()
            .width(w - 48.0 * u)
            .height(30.0 * u)
            .translate(0.0, 18.0 * u)
            .opacity(entry)
            .align_child(amane::End, Center)
            .child(status_row(&info, u)),
    ));

    // ── top: date, clock, now playing ──
    let rise = 24.0 * u * (1.0 - entry);
    let mut top = headline(&info, u);
    if let Some((temp, cond)) = &info.weather {
        top.push(Box::new(
            Row::new(children![
                icon(weather_icon(cond), 20.0 * u, alpha(WHITE, 0xcc)),
                Text::new(format!("{}  {cond}", temp.trim_start_matches('+')))
                    .font(SANS)
                    .size(17.0 * u)
                    .weight(Weight::Medium)
                    .color(alpha(WHITE, 0xcc)),
            ])
            .gap(8.0 * u)
            .align(Center),
        ));
    }
    if let Some(pill) = now_playing(u) {
        top.push(Box::new(pill));
    }
    layers.push(Box::new(
        Rectangle::new()
            .width(w)
            .height(h * 0.5)
            .translate(0.0, h * 0.075 - rise)
            .opacity(entry)
            .align_child(Center, amane::Start)
            .child(Column::new(top).gap(10.0 * u).align(Center)),
    ));

    // ── bottom: account, password, hint ──
    let initial = std::env::var("USER")
        .ok()
        .and_then(|u| u.chars().next())
        .map_or(String::from("?"), |c| c.to_uppercase().to_string());
    let avatar_size = 84.0 * u;
    let avatar = Rectangle::new()
        .width(avatar_size)
        .height(avatar_size)
        .radius(avatar_size / 2.0)
        .border(2.0, alpha(WHITE, 0x40));
    let avatar = match face() {
        Some(path) => avatar.fill(amane::Image::cover(path)),
        None => avatar
            .fill(Gradient::linear(
                160.0,
                [(0.0, blend(p.blue, WHITE, 0.15)), (1.0, blend(p.cyan, Color::BLACK, 0.2))],
            ))
            .align_child(Center, Center)
            .child(
                Text::new(initial)
                    .font(SANS)
                    .size(38.0 * u)
                    .weight(Weight::SemiBold)
                    .color(WHITE),
            ),
    };

    let (hint, hint_color) = if lock.checking() {
        ("Checking…", alpha(WHITE, 0xb3))
    } else if lock.failed() {
        ("Wrong password", blend(p.red, WHITE, 0.35))
    } else {
        // macOS only writes under the field when there is something to say
        ("", alpha(WHITE, 0x00))
    };

    let field_w = 260.0 * u;
    let field_h = 38.0 * u;
    let go = Rectangle::new()
        .width(26.0 * u)
        .height(26.0 * u)
        .radius(13.0 * u)
        .fill(alpha(WHITE, if typed { 0x40 } else { 0x00 }))
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(|b| {
            if b == Button::Left {
                let typed = moments(|m| m.typed.clone());
                submit(typed);
            }
        })
        .child(icon(
            if lock.checking() { "\u{e863}" } else { "\u{e5c8}" },
            18.0 * u,
            alpha(WHITE, if typed || lock.checking() { 0xee } else { 0x00 }),
        ));
    let field = Rectangle::new()
        .width(field_w)
        .height(field_h)
        .translate(shake, 0.0)
        .radius(field_h / 2.0)
        .blur(18.0)
        .fill(alpha(WHITE, 0x2e))
        .border(1.0, alpha(WHITE, if lock.failed() { 0x66 } else { 0x26 }))
        .padding(amane::Padding {
            top: 0.0,
            right: 6.0 * u,
            bottom: 0.0,
            left: 16.0 * u,
        })
        .align_child(amane::Start, Center)
        .child(
            Row::new(children![
                Rectangle::new().width(field_w - 16.0 * u - 6.0 * u - 26.0 * u - 4.0).height(field_h).align_child(amane::Start, Center).child(
                    TextInput::new(FIELD)
                        .password()
                        .focused()
                        .size(15.0 * u)
                        .color(WHITE)
                        .placeholder("Enter Password")
                        .on_change(|text| moments(|m| m.typed = text))
                        .on_submit(submit),
                ),
                go,
            ])
            .align(Center),
        );

    let account = Column::new(children![
        avatar,
        Text::new(display_name())
            .font(SANS)
            .size(19.0 * u)
            .weight(Weight::SemiBold)
            .color(WHITE),
        field,
        Text::new(hint).font(SANS).size(13.0 * u).color(hint_color),
    ])
    .gap(12.0 * u)
    .align(Center);
    layers.push(Box::new(
        Rectangle::new()
            .width(w)
            .height(330.0 * u)
            .translate(0.0, h - 330.0 * u - 128.0 * u + rise)
            .opacity(entry)
            .align_child(Center, amane::End)
            .child(account),
    ));

    // ── very bottom: power buttons, quiet until hovered or armed ──
    layers.push(Box::new(
        Rectangle::new()
            .width(w)
            .height(96.0 * u)
            .translate(0.0, h - 104.0 * u)
            .opacity(entry)
            .align_child(Center, Center)
            .child(power_row(armed, u, &p)),
    ));

    LayerWindow::new()
        .width(1.0)
        .height(1.0)
        .child(amane::Stack::new(layers).width(w).height(h))
}

// ─── motion ───

/// How long the lock entry runs, backdrop and content together.
const ENTRY: f32 = 0.62;

/// How long the veil takes to lift once the session is unlocked.
const LIFT: f32 = 0.42;

/// Lock entry, staged: how far the backdrop is up, and how far the content is
/// in behind it. The screen is handed over whole by the compositor, so the two
/// are separated here to keep it from cutting in all at once.
fn entry_motion(seconds: f32) -> (f32, f32) {
    if seconds < ENTRY {
        amane::request_frame();
    }
    let stage = |delay: f32, span: f32, easing: Easing| easing.at((seconds - delay) / span);
    (
        stage(0.0, 0.26, Easing::DefaultEffects),
        stage(0.14, 0.48, Easing::Emphasized),
    )
}

/// The wallpaper and the shading that keeps text readable over it, shared with
/// the veil so the handover after unlocking shows the same picture. `zoom`
/// scales it around the centre and `fade` takes the pair down to black.
fn backdrop(w: f32, h: f32, zoom: f32, fade: f32) -> Vec<Box<dyn Widget>> {
    // the wallpaper stays sharp; only the shading is drawn over it
    let wall = Rectangle::new().width(w).height(h).scale(zoom).opacity(fade);
    children![
        match wallpaper() {
            Some(path) => wall.fill(amane::Image::cover(path)),
            None => wall.fill(crate::theme::palette().background),
        },
        Rectangle::new().width(w).height(h).opacity(fade).fill(Gradient::linear(
            180.0,
            [
                (0.0, Color::rgba(0, 0, 0, 0x8c)),
                (0.32, Color::rgba(0, 0, 0, 0x26)),
                (0.62, Color::rgba(0, 0, 0, 0x1a)),
                (1.0, Color::rgba(0, 0, 0, 0xa6)),
            ],
        )),
    ]
}

/// The date and the big clock, also shared with the veil, so the time does not
/// move when the lock screen hands over to it.
fn headline(info: &Info, u: f32) -> Vec<Box<dyn Widget>> {
    children![
        Text::new(&info.date)
            .font(SANS)
            .size(26.0 * u)
            .weight(Weight::SemiBold)
            .color(alpha(WHITE, 0xe6)),
        Text::new(format!("{:02}:{:02}", info.clock.0, info.clock.1))
            .font(SANS)
            .size(150.0 * u)
            .weight(Weight::Bold)
            .tight()
            .color(alpha(WHITE, 0xeb)),
    ]
}

// ─── the veil that lifts after unlocking ───

/// The compositor takes the lock surface away the moment pam accepts the
/// password, so nothing on it can be animated out. The veil is a copy of the
/// backdrop and clock on the overlay layer, which a lock surface draws above:
/// it is held there, unseen, for as long as the session is locked, so it is
/// already on screen when the lock surface goes, and then fades out.
#[derive(Clone, Copy)]
enum Veil {
    /// Unlocked and settled: the window stays mapped and draws nothing.
    Resting,

    /// Behind the lock screen, waiting for it to go away.
    Held,

    Lifting(Instant),
}

static VEIL: Mutex<Veil> = Mutex::new(Veil::Resting);

/// How much of the veil to draw: all of it while locked, then the lift, then
/// nothing. A lift that was never held through a lock does not start, so an
/// unlock the veil did not see leaves the desktop uncovered rather than
/// flashing it over.
fn lift(locked: bool) -> f32 {
    let mut veil = VEIL.lock().unwrap_or_else(|e| e.into_inner());
    if locked {
        *veil = Veil::Held;

        return 1.0;
    }
    if matches!(*veil, Veil::Held) {
        *veil = Veil::Lifting(Instant::now());
    }
    let Veil::Lifting(at) = *veil else {
        return 0.0;
    };
    let progress = at.elapsed().as_secs_f32() / LIFT;
    if progress >= 1.0 {
        return 0.0;
    }
    amane::request_frame();
    1.0 - Easing::DefaultEffects.at(progress)
}

/// One per monitor, passive and click-through, drawn only while the session is
/// locked and for the moment after. See `Veil`.
pub fn veil(monitor: &Monitor) -> LayerWindow {
    // Lock::active() is an untracked atomic, so the service is read as well:
    // that read is what has this window drawn again on the wake that unlocks.
    let locked = {
        let _subscribe = Lock::read();
        Lock::active()
    };
    // unlocked (or freshly started): no lock for the supervisor to restore
    if !locked {
        let marker = locked_marker();
        if marker.exists() {
            let _ = std::fs::remove_file(marker);
        }
    }
    let (w, h) = (monitor.width as f32, monitor.height as f32);
    let u = h / 1080.0;
    let left = lift(locked);
    // Nothing to show. The window stays mapped around a surface of no size,
    // since unmapping it is what makes a compositor close it for good.
    if left <= 0.0 {
        return window(Stack::new(Vec::new()).width(1.0).height(1.0));
    }
    // the picture drifts up and away as it goes, the reverse of the entry
    let mut layers = backdrop(w, h, 1.0 + 0.04 * (1.0 - left), left);
    let info = Info::read();
    layers.push(Box::new(
        Rectangle::new()
            .width(w)
            .height(h * 0.5)
            .translate(0.0, h * 0.075 - 20.0 * u * (1.0 - left))
            .opacity(left)
            .align_child(Center, amane::Start)
            .child(Column::new(headline(&info, u)).gap(10.0 * u).align(Center)),
    ));
    drop(info);
    window(Stack::new(layers).width(w).height(h))
}

fn window(child: Stack) -> LayerWindow {
    LayerWindow::new()
        .width(Full)
        .height(Full)
        // above every other layer, and over anything a bar reserved; a lock
        // surface still draws above this, which is what keeps it unseen
        .layer(Layer::Overlay)
        .space(Zone::Ignore)
        .keyboard(Keyboard::None)
        .click_through()
        .anchor_horizontal(Horizontal::Middle)
        .anchor_vertical(Vertical::Top)
        .namespace("amane-veil")
        .child(child)
}

// ─── pieces ───

/// Keyboard layout, Wi-Fi, battery and the unread count, like the macOS menu bar.
fn status_row(info: &Info, u: f32) -> Row {
    let dim = alpha(WHITE, 0xd9);
    let text = |s: String| Text::new(s).font(SANS).size(14.0 * u).weight(Weight::Medium).color(dim);
    let mut items: Vec<Box<dyn amane::Widget>> = vec![Box::new(text(String::from("US")))];
    let net = Network::read();
    let wifi = if !net.connected() {
        "\u{e1da}"
    } else if net.ssid().is_empty() {
        "\u{eb2f}"
    } else {
        match net.strength() {
            0..=33 => "\u{ebe4}",
            34..=66 => "\u{ebd6}",
            _ => "\u{e63e}",
        }
    };
    drop(net);
    items.push(Box::new(icon(wifi, 19.0 * u, dim)));
    let battery = Battery::read();
    if battery.present() {
        let pct = battery.percent();
        items.push(Box::new(text(format!("{pct}%"))));
        items.push(Box::new(icon(
            if battery.charging() {
                "\u{e1a3}"
            } else if pct < 20 {
                "\u{e19c}"
            } else {
                "\u{e1a4}"
            },
            19.0 * u,
            dim,
        )));
    }
    if info.notes > 0 {
        items.push(Box::new(
            Row::new(children![icon("\u{e7f4}", 18.0 * u, dim), text(info.notes.to_string())])
                .gap(3.0 * u)
                .align(Center),
        ));
    }
    Row::new(items).gap(14.0 * u).align(Center)
}

/// A small frosted pill under the clock while something plays, with controls.
fn now_playing(u: f32) -> Option<Rectangle> {
    let media = Media::read();
    if media.active().is_none() || media.title().is_empty() {
        return None;
    }
    let art = Rectangle::new().width(36.0 * u).height(36.0 * u).radius(8.0 * u);
    let art = match media.art_url().strip_prefix("file://") {
        Some(path) if std::path::Path::new(path).is_file() => art.fill(amane::Image::cover(path)),
        _ => art
            .fill(alpha(WHITE, 0x26))
            .align_child(Center, Center)
            .child(icon("\u{e405}", 20.0 * u, WHITE)),
    };
    let button = |glyph: &'static str, action: fn()| {
        let key = format!("lock-media:{glyph}");
        let ck = key.clone();
        Rectangle::new()
            .width(32.0 * u)
            .height(32.0 * u)
            .radius(16.0 * u)
            .scale(crate::feedback::scale(&key))
            .align_child(Center, Center)
            .cursor(Pointer)
            .on_click(move |b| {
                if b == Button::Left {
                    crate::feedback::click(&ck);
                    action();
                }
            })
            .child(icon(glyph, 22.0 * u, WHITE))
    };
    let title = format!("{}  —  {}", media.title(), media.artist());
    let playing = media.playing();
    Some(
        Rectangle::new()
            .width(420.0 * u)
            .height(52.0 * u)
            .radius(26.0 * u)
            .blur(18.0)
            .fill(alpha(WHITE, 0x24))
            .border(1.0, alpha(WHITE, 0x1f))
            .padding(8.0 * u)
            .align_child(amane::Start, Center)
            .child(
                Row::new(children![
                    art,
                    Rectangle::new()
                        .width(420.0 * u - 16.0 * u - 36.0 * u - 3.0 * 32.0 * u - 24.0 * u)
                        .height(20.0 * u)
                        .child(Text::new(title).font(SANS).size(14.0 * u).weight(Weight::Medium).color(WHITE).elide()),
                    button("\u{e045}", Media::previous),
                    button(if playing { "\u{e034}" } else { "\u{e037}" }, Media::play_pause),
                    button("\u{e044}", Media::next),
                ])
                .gap(6.0 * u)
                .align(Center),
            ),
    )
}

/// Sleep / Restart / Shut Down, round and labelled like the macOS login window.
/// One click arms a button, a second within three seconds runs it.
fn power_row(armed: Option<&'static str>, u: f32, p: &crate::theme::Palette) -> Row {
    let button = |kind: &'static str, glyph: &'static str, label: &'static str, args: &'static [&'static str]| {
        let hot = armed == Some(kind);
        let key = format!("lock-power:{kind}");
        let ck = key.clone();
        let size = 46.0 * u;
        Column::new(children![
            Rectangle::new()
                .width(size)
                .height(size)
                .radius(size / 2.0)
                .scale(crate::feedback::scale(&key))
                .blur(14.0)
                .fill(if hot { alpha(p.red, 0x66) } else { alpha(WHITE, 0x1f) })
                .border(1.0, if hot { blend(p.red, WHITE, 0.3) } else { alpha(WHITE, 0x1a) })
                .align_child(Center, Center)
                .cursor(Pointer)
                .on_click(move |b| {
                    if b != Button::Left {
                        return;
                    }
                    crate::feedback::click(&ck);
                    let go = moments(|m| {
                        if m.armed.is_some_and(|(k, _)| k == kind) {
                            m.armed = None;
                            true
                        } else {
                            m.armed = Some((kind, Instant::now()));
                            false
                        }
                    });
                    if go {
                        let _ = std::process::Command::new("systemctl").args(args).spawn();
                    }
                })
                .child(icon(glyph, 22.0 * u, WHITE)),
            Text::new(if hot { "Click again" } else { label })
                .font(SANS)
                .size(12.0 * u)
                .weight(Weight::Medium)
                .color(if hot { blend(p.red, WHITE, 0.4) } else { alpha(WHITE, 0xb3) }),
        ])
        .gap(8.0 * u)
        .align(Center)
    };
    Row::new(children![
        button("sleep", "\u{ef44}", "Sleep", &["suspend"]),
        button("restart", "\u{f053}", "Restart", &["reboot"]),
        button("shutdown", "\u{e8ac}", "Shut Down", &["poweroff"]),
    ])
    .gap(44.0 * u)
}

fn icon(glyph: &str, size: f32, color: Color) -> Text {
    Text::new(glyph).font(ICONS).weight(amane::Weight::Light).size(size).tight().color(color)
}

fn blend(a: Color, b: Color, t: f32) -> Color {
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color::rgb(mix(a.red(), b.red()), mix(a.green(), b.green()), mix(a.blue(), b.blue()))
}

fn alpha(c: Color, a: u8) -> Color {
    Color::rgba(c.red(), c.green(), c.blue(), a)
}

// ─── data ───

fn home() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn wallpaper() -> Option<std::path::PathBuf> {
    std::fs::canonicalize(home().join(".config/hypr/lock-bg"))
        .ok()
        .filter(|p| p.is_file())
}

/// `~/.face`, the usual account picture; the initial is drawn without one.
fn face() -> Option<std::path::PathBuf> {
    let path = home().join(".face");
    path.is_file().then_some(path)
}

/// The account's full name from passwd (GECOS), else the login name.
fn display_name() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    std::fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .find(|l| l.split(':').next() == Some(user.as_str()))
        .and_then(|l| l.split(':').nth(4))
        .and_then(|g| g.split(',').next())
        .filter(|g| !g.trim().is_empty())
        .map_or(user.clone(), str::to_string)
}

pub(crate) fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Seconds east of UTC, from `date +%z` (one fork per hour while locked).
pub(crate) fn utc_offset() -> i64 {
    let z = amane::output("date +%z");
    let z = z.trim();
    let sign = if z.starts_with('-') { -1 } else { 1 };
    let digits = z.trim_start_matches(['+', '-']);
    let (h, m) = (
        digits.get(0..2).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
        digits.get(2..4).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
    );
    sign * (h * 3600 + m * 60)
}

/// "Tuesday 27 October", as macOS writes it, from local seconds since the epoch.
pub(crate) fn date_line(local: i64) -> String {
    let days = local.div_euclid(86_400);
    // 1970-01-01 was a Thursday
    let weekday = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"]
        [days.rem_euclid(7) as usize];
    // Howard Hinnant's civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let name = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ][(month - 1) as usize];
    format!("{weekday} {day} {name}")
}

/// The cache lock.sh refreshes: "temp|condition|place|humidity|wind".
fn weather() -> Option<(String, String)> {
    let text = std::fs::read_to_string(home().join(".cache/hyprlock/weather")).ok()?;
    let mut parts = text.trim().split('|').map(str::trim);
    let temp = parts.next()?.to_string();
    let cond = parts.next().unwrap_or("").to_string();
    (!temp.is_empty()).then_some((temp, cond))
}

fn weather_icon(condition: &str) -> &'static str {
    let c = condition.to_lowercase();
    if c.contains("thunder") {
        "\u{ebdb}"
    } else if c.contains("snow") || c.contains("sleet") {
        "\u{e2cd}"
    } else if c.contains("rain") || c.contains("drizzle") || c.contains("shower") {
        "\u{f176}"
    } else if ["fog", "mist", "haze", "smok", "dust"].iter().any(|k| c.contains(k)) {
        "\u{e818}"
    } else if c.contains("partly") {
        "\u{f172}"
    } else if c.contains("cloud") || c.contains("overcast") {
        "\u{e2bd}"
    } else {
        "\u{e81a}"
    }
}

/// Notifications from the last day in amane's history (key, time, ...), so the
/// badge means "recent", not everything ever kept.
fn notification_count() -> usize {
    let since = now_secs() - 86_400;
    std::fs::read_to_string(home().join(".local/state/amane/notifications.v1"))
        .map_or(0, |t| {
            t.lines()
                .filter_map(|l| l.split('\t').nth(1)?.parse::<i64>().ok())
                .filter(|&time| time >= since)
                .count()
        })
}

/// `~/.cache/amane/locked` exists while the session is locked by this shell.
pub(crate) fn locked_marker() -> std::path::PathBuf {
    home().join(".cache/amane/locked")
}
