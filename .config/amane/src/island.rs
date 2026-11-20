// A dynamic island: one pill at the top of the screen that grows to show what
// just happened, then shrinks back to the clock.
//
//   idle          the clock
//   osd           a volume or brightness bar
//   notification  title and text, click to open it (or dismiss it)
//
// Events only set deadlines. `wanted_mode` turns the deadlines into what to
// show: the osd first (you just pressed a key), then a notification, then idle.
// An explicitly opened panel stays open while controls are used.
// All of it runs in `Island::update`, the view only reads and draws.

use std::time::{Duration, Instant, SystemTime};

use amane::{
    Align, Audio, Button, Center, Color, Column, End, Full, Horizontal, LayerWindow, Media,
    Notifications, Padding, Pointer, Rectangle, Row, Service, Start, Text, Vertical, Weight,
    children,
};

// Keep the layer surface snug to the animated pill, not a transparent canvas.
const GAP_TOP: f32 = 0.0;

// width and height of the pill in each mode
const IDLE: (f32, f32) = (modules::total(modules::COLLAPSED, false), bar::HEIGHT);
const ROW_PADDING: f32 = 24.0;
const ROW_GAP: f32 = 20.0;
const CONTROL_GAP: f32 = 4.0;
const CONTROL_WIDTH: f32 = 32.0;
// The Control Center is at least this wide so the settings pages stay readable.
const PANEL_MIN: f32 = 480.0;
const PANEL: (f32, f32) = (max_width(modules::total(modules::EXPANDED, true), PANEL_MIN), 46.0);
const fn max_width(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

pub(crate) mod activity;
pub(crate) mod awake;
pub(crate) mod bar;
pub(crate) mod power;
pub(crate) mod symbols;
mod border;
mod exposure;
// mod fullscreen;
pub(crate) mod fullscreen;
mod history;
mod modules;
pub(crate) mod motion;
mod network_status;
mod notice_hints;
pub(crate) mod notices;
mod ops;
mod system;
pub mod popups;
pub mod settings;
mod settings_scroll;
mod visibility;
use motion::Spring;
const OSD_FOR: Duration = Duration::from_millis(2500);
const FACE_PACE: f32 = 0.55;


/// How one screen's pill is moving. The monitor that owns the open panel
/// draws `Island::face`; every other screen draws `Island::rest`, which never
/// leaves Idle/Osd, so opening the island on one screen leaves the rest alone.
#[derive(Clone)]
struct Face {
    mode: Mode,
    previous_mode: Mode,
    transition: Instant,
    width: Spring,
    height: Spring,
    expansion: Spring,
}

impl Face {
    fn new() -> Self {
        let (width, height) = IDLE;
        Self {
            mode: Mode::Idle,
            previous_mode: Mode::Idle,
            transition: Instant::now() - Duration::from_secs(1),
            // ~340 ms instead of 620: snappier, Control-Center-like, and
            // about half the (expensive) panel frames per open/close
            width: Spring::slow(width, FACE_PACE),
            height: Spring::slow(height, FACE_PACE),
            expansion: Spring::slow(0.0, FACE_PACE),
        }
    }

    // moves the pill to `mode`, true when that changed; `idle_width` is the
    // notch's width at rest (wider while a Live Activity shows)
    fn sync(&mut self, mode: Mode, idle_width: f32) -> bool {
        let changed = mode != self.mode;
        if changed {
            self.previous_mode = self.mode;
            self.transition = Instant::now();
        }
        self.mode = mode;

        let (width, height) = match mode {
            Mode::Idle => (idle_width, IDLE.1),
            Mode::Panel => PANEL,
        };

        self.expansion
            .to(if mode == Mode::Panel { 1.0 } else { 0.0 });
        self.width.to(width);
        self.height.to(height);

        changed
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Idle,
    Panel,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Osd {
    Volume,
    Brightness,
}

impl Osd {
}

pub struct Island {
    // the owner's pill, and the one every other screen shows
    face: Face,
    rest: Face,
    // the monitor the panel was opened on; only it expands
    owner: Option<String>,
    expanded: bool,
    leave_deadline: Option<Instant>,

    osd: Osd,
    percent: u8,
    muted: bool,
    osd_until: Instant,

    // the newest notification already shown, so each one pops up once

    // `None` until the first look, so starting the shell doesn't show the volume
    last_audio: Option<(u8, bool)>,
    last_brightness: Option<u8>,
    // audio settles for a moment after start; changes before this are not
    // the user's and must not pop the HUD
    quiet_until: Instant,
}

impl Service for Island {
    fn new() -> Self {
        Self {
            face: Face::new(),
            rest: Face::new(),
            owner: None,
            expanded: false,
            leave_deadline: None,

            osd: Osd::Volume,
            percent: 0,
            muted: false,
            osd_until: Instant::now(),

            last_audio: None,
            last_brightness: None,
            quiet_until: Instant::now() + Duration::from_secs(3),
        }
    }

    // a poll that finds nothing costs nothing, so this can be quick
    fn interval() -> Duration {
        Duration::from_millis(50)
    }

    fn update(&mut self) -> bool {
        let mut changed = self.check_audio();

        // Notifications render independently below either island mode.

        let brightness = crate::backlight::percent();
        changed |= brightness != self.last_brightness;
        self.last_brightness = brightness;

        if self.expanded
            && !settings::open()
            && self
                .leave_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.expanded = false;
            self.leave_deadline = None;
            popups::dismiss();
            changed = true;
        }
        changed |= self.sync();

        changed
    }
}

impl Island {
    // brightness comes in through IPC, so backlight.rs calls this. The notch
    // only ever shows the time now, so this no longer reveals it; the value
    // still reaches the open panel's popups.
    pub fn show_osd(&mut self, osd: Osd, percent: u8, muted: bool) {
        self.osd = osd;
        self.percent = percent;
        self.muted = muted;
        self.osd_until = Instant::now() + OSD_FOR;

        if self.expanded {
            popups::osd(osd);
        }
        self.sync();
    }

    fn check_audio(&mut self) -> bool {
        let (volume, muted) = {
            let audio = Audio::read();

            (audio.volume(), audio.muted())
        };

        let last = self.last_audio.replace((volume, muted));

        if last.is_none_or(|last| last == (volume, muted)) || Instant::now() < self.quiet_until {
            return false;
        }

        self.show_osd(Osd::Volume, volume, muted);

        true
    }

    fn wanted_mode(&self) -> Mode {
        if self.expanded {
            Mode::Panel
        } else {
            self.rest_mode()
        }
    }

    // screens that don't own the panel only ever show the osd or the clock
    fn rest_mode(&self) -> Mode {
        // the notch shows the time and nothing else
        Mode::Idle
    }

    // moves the pills to what should be showing now, true when that changed
    fn sync(&mut self) -> bool {
        let (open, rest) = (self.wanted_mode(), self.rest_mode());
        let idle = if activity::shown() { activity::WIDTH } else { IDLE.0 };
        let changed = self.face.sync(open, idle) | self.rest.sync(rest, idle);
        exposure::collapsed(hide_eligible(
            self.face.mode,
            self.face.previous_mode,
            self.face.transition.elapsed(),
        ));

        changed
    }

    /// Expand on `monitor` (None: keep the current owner, else the focused
    /// screen). A new owner starts from its resting pill, so it grows from
    /// what that screen was showing and the old owner simply goes back to rest.
    fn open_on(&mut self, monitor: Option<String>) {
        let monitor = monitor
            .or_else(|| self.owner.clone())
            .or_else(crate::auxiliary::focused_monitor);
        if self.owner != monitor {
            self.face = self.rest.clone();
            self.owner = monitor;
        }
        self.expanded = true;
    }

    fn owns(&self, monitor: &str) -> bool {
        self.owner.as_deref() == Some(monitor)
    }
}

fn hide_eligible(mode: Mode, previous: Mode, elapsed: Duration) -> bool {
    mode != Mode::Panel && (previous != Mode::Panel || elapsed >= Duration::from_millis(620))
}

struct Clock {
    time: String,
    date: String,
    // seconds east of UTC, re-read hourly (the only fork the clock makes)
    offset: i64,
    offset_at: Instant,
    // the minute being replaced, faded out as `swap` runs 0 -> 1
    previous: String,
    swap: motion::Curve,
}

impl Service for Clock {
    fn new() -> Self {
        let offset = crate::lock::utc_offset();
        let local = crate::lock::now_secs() + offset;
        Self {
            time: hhmm(local),
            date: crate::lock::date_line(local),
            offset,
            offset_at: Instant::now(),
            previous: String::new(),
            swap: motion::Curve::new(1.0, motion::Easing::DefaultEffects),
        }
    }

    fn update(&mut self) -> bool {
        // computed in-process: this used to start `sh` + `date` twice a second
        if self.offset_at.elapsed() >= Duration::from_secs(3600) {
            self.offset = crate::lock::utc_offset();
            self.offset_at = Instant::now();
        }
        let local = crate::lock::now_secs() + self.offset;
        let time = hhmm(local);

        let date = crate::lock::date_line(local);
        let changed = time != self.time || date != self.date;
        self.date = date;

        if time != self.time {
            self.previous = std::mem::replace(&mut self.time, time);
            self.swap = motion::Curve::new(0.0, motion::Easing::SlowEffects);
            self.swap.to(1.0);
        }

        changed
    }
}

fn hhmm(local: i64) -> String {
    let minutes = local.rem_euclid(86_400) / 60;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn content_motion(seconds: f32) -> (bool, f32, f32) {
    if seconds < 0.08 {
        let remaining = (1.0 - seconds / 0.08).clamp(0.0, 1.0);
        (true, remaining * remaining, 1.0)
    } else {
        let progress = ((seconds - 0.08) / 0.26).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - progress).powi(3);
        (false, eased, 1.0)
    }
}

fn blend_color(a: Color, b: Color, t: f32) -> Color {
    let mix = |x: u8, y: u8| {
        (f32::from(x) + (f32::from(y) - f32::from(x)) * t.clamp(0.0, 1.0)).round() as u8
    };
    Color::rgb(
        mix(a.red(), b.red()),
        mix(a.green(), b.green()),
        mix(a.blue(), b.blue()),
    )
}

fn icon_gradient(color: Color) -> amane::Gradient {
    amane::Gradient::linear(
        120.0,
        [
            (
                0.0,
                blend_color(color, crate::theme::palette().foreground, 0.20),
            ),
            (0.48, color),
            (1.0, blend_color(color, Color::BLACK, 0.25)),
        ],
    )
}


/// One island per monitor. Amane opens a window for each output at startup,
/// adds one when a monitor is plugged in and drops it when it is unplugged, so
/// nothing here has to watch for hotplug.
///
/// The monitor is not read yet: the island centres itself from
/// `amane::window_size()`, which already reports the size of the window being
/// drawn, so each screen positions its own copy correctly. The argument is
/// kept because per-screen behaviour (its own workspace pills, for instance)
/// would start here.
pub fn view_per_monitor(monitor: &amane::Monitor) -> LayerWindow {
    view(&monitor.name)
}

pub fn view(monitor: &str) -> LayerWindow {
    // A fullscreen app is up: keep the layer surface mapped but invisible.
    // Unmapping (attach NULL buffer) makes mango close the surface, which
    // removes the last window and shuts the whole shell down.
    if fullscreen::blocked() {
        return LayerWindow::new()
            .width(Full)
            .height(Full)
            .keyboard(amane::Keyboard::None)
            .input_region(vec![])
            .anchor_horizontal(Horizontal::Middle)
            .anchor_vertical(Vertical::Top)
            .space(amane::Zone::Ignore)
        .namespace("island")
            .child(
                Rectangle::new()
                    .width(1.0)
                    .height(1.0)
                    .fill(Color::TRANSPARENT),
            );
    }
    let island = Island::read();
    let clock = Clock::read();
    // Only the screen the panel was opened on expands, takes settings, and
    // catches clicks outside; the others keep drawing their resting pill.
    let mine = island.owns(monitor);
    let face = if mine { &island.face } else { &island.rest };
    let settings_open = mine && settings::open();

    let expansion = face.expansion.precise_value().clamp(0.0, 1.0);
    let width = face.width.value().max(48.0);
    let width = if settings_open {
        width.max(PANEL.0)
    } else {
        width
    };
    let height = face.height.value();

    let elapsed = face.transition.elapsed().as_secs_f32();
    let (outgoing, opacity, scale) = content_motion(elapsed);
    if elapsed < 0.34 {
        amane::request_frame();
    }
    let mode = if outgoing {
        face.previous_mode
    } else {
        face.mode
    };
    let shared = matches!(face.mode, Mode::Idle | Mode::Panel);
    let content: Box<dyn amane::Widget> = if face.mode == Mode::Idle && activity::shown() {
        Box::new(
            Rectangle::new()
                .width(width - 36.0)
                .height(height)
                .align_child(Start, Center)
                .child(activity::view(height)),
        )
    } else if shared {
        Box::new(modules::view(expansion, monitor, mine, &clock, width, height))
    } else {
        let content_width = match mode {
            Mode::Idle => IDLE.0,
            Mode::Panel => PANEL.0,
        } - if mode == Mode::Panel {
            ROW_PADDING * 2.0
        } else {
            36.0
        };
        let content = Rectangle::new()
            .width(content_width)
            .height(height)
            .align_child(Center, Center)
            .opacity(opacity)
            .scale(scale);
        let content = match mode {
            Mode::Idle => content.cursor(Pointer).child(
                Text::new(&clock.time)
                    .size(16.0)
                    .weight(Weight::SemiBold)
                    .color(crate::theme::palette().foreground),
            ),
            Mode::Panel => content.child(modules::view(
                expansion,
                monitor,
                mine,
                &clock,
                content_width,
                height,
            )),
        };
        let blur = 0.0;
        let content = amane::Stack::new(children![
            content,
            Rectangle::new()
                .width(content_width)
                .height(height)
                .blur(blur)
                .fill(Color::TRANSPARENT),
        ])
        .width(content_width)
        .height(height);
        Box::new(content)
    };
    let row_height = height;
    let extra = if face.mode == Mode::Panel {
        settings::height()
    } else {
        0.0
    };
    let row_content = amane::Stack::new(vec![content])
        .width(width)
        .height(row_height);
    let mut layers: Vec<Box<dyn amane::Widget>> = vec![Box::new(row_content)];
    if mine {
        layers.push(Box::new(
            settings::view().translate((width - PANEL.0) / 2.0, row_height),
        ));
    }
    let content: Box<dyn amane::Widget> = Box::new(
        amane::Stack::new(layers)
            .width(width)
            .height(row_height + extra),
    );
    let height = height + extra;
    let pill = pill(width, height, monitor, mine)
        .padding(Padding {
            left: if shared {
                0.0
            } else if mode == Mode::Panel {
                ROW_PADDING
            } else {
                18.0
            },
            right: if shared {
                0.0
            } else if mode == Mode::Panel {
                ROW_PADDING
            } else {
                18.0
            },
            top: 0.0,
            bottom: 0.0,
        })
        .child(amane::Stack::new(vec![content]).width(width).height(height));

    let scene = popups::scene(width, height, row_height, face.mode == Mode::Panel);
    let hide = exposure::amount();
    // Control Center placement: the owner's panel slides from the notch to
    // the top right, under the menu bar, as it expands.
    let cc = if mine { expansion } else { 0.0 };
    let shift = GAP_TOP - hide * (height + GAP_TOP + 8.0) + cc * (bar::HEIGHT + 6.0);
    // A stable output-sized surface keeps compositor-local pointer coordinates
    // fixed while the island grows or popups alter the scene envelope.
    let (screen_width, screen_height) = amane::window_size();
    let screen_width = screen_width.max(PANEL.0 + 72.0);
    let screen_height = screen_height.max(600.0);
    let canvas_width = scene.width;
    let centred = (screen_width - canvas_width) / 2.0;
    let right = screen_width - 10.0 - (canvas_width + width) / 2.0;
    let dx = (centred + cc * (right - centred)).round();
    let mut input = scene.input;
    for area in &mut input {
        area.x += dx.round() as i32;
        area.y += (shift - scene.top).round() as i32;
    }
    // the empty middle of the menu bar is the hover sensor for the notch
    let sensor_width = bar::NOTCH_SLOT;
    let sensor_x = (screen_width - sensor_width) / 2.0;
    if crate::edge_activation::enabled("main") && exposure::tucked() {
        input.push(amane::InputArea {
            x: sensor_x as i32,
            y: 0,
            width: sensor_width as i32,
            height: bar::HEIGHT as i32,
        });
    }
    let pill = pill.translate(((canvas_width - width) / 2.0).round(), scene.top);
    let pill = if island.expanded && mine { pill } else { pill.cursor(amane::Default) };
    let main_modal = mine && (island.expanded || settings_open);
    if main_modal {
        input.push(amane::InputArea {
            x: 0,
            y: 0,
            width: screen_width as i32,
            height: screen_height as i32,
        });
    }
    let moving = Rectangle::new()
        .width(canvas_width)
        .height(scene.height)
        .on_click(|_| {})
        .translate(dx, shift - scene.top)
        .child(
            amane::Stack::new(vec![scene.background, Box::new(pill), scene.content])
                .width(canvas_width)
                .height(scene.height),
        );
    let sensor = Rectangle::new()
        .width(sensor_width)
        .height(bar::HEIGHT)
        .translate(sensor_x, 0.0)
        .fill(Color::TRANSPARENT)
        .on_hover(|inside| {
            if crate::edge_activation::enabled("main") {
                exposure::pointer(inside);
                bar::hover(inside);
            }
        });
    let outside = Rectangle::new()
        .width(if main_modal { screen_width } else { 0.0 })
        .height(if main_modal { screen_height } else { 0.0 })
        .fill(Color::TRANSPARENT)
        .on_click(|_| dismiss_outside());
    // macOS HUD + notification banners, top right under the bar, on the
    // screen that has focus
    let focused = crate::auxiliary::focused_monitor().is_none_or(|f| f == monitor);
    let mut corner: Vec<Box<dyn amane::Widget>> = vec![];
    let mut first_banner = bar::HEIGHT + 8.0;
    // While this screen's panel is open it owns the top-right corner: the
    // volume HUD would repeat its slider, and banners wait in History.
    let panel_here = mine && island.expanded;
    if focused && !panel_here {
        if let Some(card) = hud(&island, screen_width) {
            first_banner += (HUD.1 + 8.0) * card.1;
            corner.push(Box::new(card.0));
        }
        // charger plugged in / pulled out, stacked under the volume HUD
        if let Some((card, presence)) = power::card(screen_width, first_banner) {
            first_banner += 60.0 * presence;
            corner.push(Box::new(card));
        }
        let (cards, areas) = notices::banners(screen_width, first_banner);
        corner.extend(cards);
        input.extend(areas);
    }
    let layer = amane::Stack::new(children![
        outside,
        // the screen frame only shows while a capture pick tints it
        border::frame_if_tinted(screen_width, screen_height),
        // Screen curvature: rounded corners on every screen
        border::corners(screen_width, screen_height, settings::radius()),
        moving,
        sensor,
        amane::Stack::new(corner).width(screen_width).height(screen_height),
    ])
    .width(screen_width)
    .height(screen_height);
    LayerWindow::new()
        .width(Full)
        .height(Full)
        // While this screen's Control Center is open it takes the keyboard,
        // so Esc closes it (and its calendar / media popups) like the other
        // amane panels; a click outside still closes it too.
        .keyboard(if main_modal || (mine && settings::keyboard()) {
            amane::Keyboard::Exclusive
        } else {
            amane::Keyboard::None
        })
        .on_key(|key| {
            if key == amane::Key::Escape {
                dismiss_outside()
            } else {
                // Tab / arrows / Enter / Space on the Control Center home
                settings::key(key);
            }
        })
        .input_region(input)
        .anchor_horizontal(Horizontal::Middle)
        .anchor_vertical(Vertical::Top)
        .space(amane::Zone::Ignore)
        .namespace("island")
        .child(layer)
}

fn pill(width: f32, height: f32, monitor: &str, mine: bool) -> Rectangle {
    let here = monitor.to_string();
    Rectangle::new()
        .width(width)
        .height(height)
        .radius((height / 2.0).min(34.0))
        .fill(Color::TRANSPARENT)
        .padding(Padding {
            top: 0.0,
            right: 18.0,
            bottom: 0.0,
            left: 18.0,
        })
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| clicked(button, &here))
        .on_hover(move |inside| {
            // hovering another screen's pill must not hold the open panel
            if mine {
                pointer_presence(inside)
            } else {
                exposure::pointer(inside)
            }
        })
}

const HUD: (f32, f32) = (300.0, 52.0);

/// The macOS volume / brightness HUD: a small glass card in the top-right
/// corner for `OSD_FOR`, fading in fast and out slowly. Returns the card and
/// how present it is (0..1), so banners below can make room smoothly.
fn hud(island: &Island, screen_width: f32) -> Option<(Rectangle, f32)> {
    let now = Instant::now();
    let shown = island.osd_until.checked_sub(OSD_FOR)?;
    if now >= island.osd_until + Duration::from_millis(260) || now < shown {
        return None;
    }
    amane::request_frame();
    let since = now.duration_since(shown).as_secs_f32();
    let left = island.osd_until.saturating_duration_since(now).as_secs_f32();
    let presence = if now >= island.osd_until {
        let gone = now.duration_since(island.osd_until).as_secs_f32();
        1.0 - (gone / 0.26).clamp(0.0, 1.0)
    } else {
        (since / 0.12).clamp(0.0, 1.0).min(1.0) * if left > 0.0 { 1.0 } else { 0.0 }
    };
    let p = crate::theme::palette();
    let fraction = if island.muted { 0.0 } else { f32::from(island.percent.min(100)) / 100.0 };
    let track_w = HUD.0 - 32.0 - 24.0 - 12.0 - 40.0 - 12.0;
    let track = Rectangle::new()
        .width(track_w)
        .height(6.0)
        .radius(Full)
        .fill(crate::theme::control_fill(true))
        .child(
            Rectangle::new()
                .width((track_w * fraction).max(6.0))
                .height(6.0)
                .radius(Full)
                .fill(p.foreground),
        );
    let value = if island.muted { String::from("Muted") } else { format!("{}%", island.percent) };
    let card = Rectangle::new()
        .width(HUD.0)
        .height(HUD.1)
        .translate(screen_width - 10.0 - HUD.0, bar::HEIGHT + 8.0 - 6.0 * (1.0 - presence))
        .radius(crate::theme::RADIUS)
        .fill(crate::theme::glass())
        .border(1.0, crate::theme::hairline())
        .opacity(presence)
        .padding(Padding { left: 16.0, right: 16.0, top: 0.0, bottom: 0.0 })
        .align_child(Start, Center)
        .child(
            Row::new(children![
                control_symbol(island.osd, island.percent, island.muted, 22.0, p.foreground),
                track,
                Rectangle::new()
                    .width(40.0)
                    .height(20.0)
                    .align_child(End, Center)
                    .child(Text::new(value).size(12.5).weight(Weight::Medium).color(p.secondary)),
            ])
            .gap(12.0)
            .align(Center),
        );
    Some((card, presence))
}


fn cell(width: f32, align: impl Into<Align>, content: impl amane::Widget + 'static) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(20.0)
        .align_child(align, Center)
        .child(content)
}


// Child popup hover handlers call this too, so moving into a popup is not a leave.
fn pointer_presence(inside: bool) {
    exposure::pointer(inside);
    let mut state = Island::write();
    state.leave_deadline = if inside || !state.expanded {
        None
    } else {
        Some(Instant::now() + Duration::from_secs(3))
    };
}

fn dismiss_outside() {
    popups::dismiss();
    settings::close();
    let mut state = Island::write();
    state.expanded = false;
    state.leave_deadline = None;
    state.osd_until = Instant::now();
    state.sync();
    exposure::hide();
}
// A click on another screen's pill moves the panel there instead of closing it.
fn clicked(button: Button, monitor: &str) {
    // the notch is the clock only; everything else opens from the bar's
    // settings icon. An open panel still closes from its own row.
    if !matches!(button, Button::Left) {
        return;
    }
    if !Island::read().expanded {
        // a finished Live Activity (a failure stays) goes away on click
        activity::dismiss_finished();
        return;
    }
    popups::dismiss();
    settings::close();
    let mut island = Island::write();
    if island.expanded && island.owns(monitor) {
        island.expanded = false;
        island.leave_deadline = None;
    } else {
        island.open_on(Some(monitor.to_string()));
        island.leave_deadline = Some(Instant::now() + Duration::from_secs(3));
    }
    island.osd_until = Instant::now();
    island.sync();
}

// IPC and panel buttons use the same controls. Audio changes are asynchronous;
// check_audio displays the value actually reported by the sound server.
pub fn volume(arguments: &[String]) -> String {
    const USAGE: &str = "usage: volume up|down|mute|show|status|<0-200>";
    let Some(argument) = arguments.first() else {
        return String::from(USAGE);
    };
    let (current, muted) = {
        let audio = Audio::read();
        (audio.volume(), audio.muted())
    };
    match argument.as_str() {
        "show" => Island::write().show_osd(Osd::Volume, current, muted),
        "status" => return format!("{current}{}", if muted { " muted" } else { "" }),
        "mute" => Audio::toggle_mute(),
        value => {
            let next = match value {
                "up" => i32::from(current) + 5,
                "down" => i32::from(current) - 5,
                number => match number.parse::<i32>() {
                    Ok(value) => value,
                    Err(_) => return String::from(USAGE),
                },
            }
            // up to 200 %, the same ceiling as the volume keys (wpctl -l 2),
            // so a step from 124 % goes to 129, not down to 100
            .clamp(0, 200) as u8;
            Audio::set_volume(next);
            // Show even at the limit, where no audio change event is emitted.
            Island::write().show_osd(Osd::Volume, current, muted);
        }
    }
    String::from("ok")
}

pub fn media(arguments: &[String]) -> String {
    match arguments.first().map(String::as_str) {
        Some("play-pause") => Media::play_pause(),
        Some("next") => Media::next(),
        Some("previous") => Media::previous(),
        _ => return String::from("usage: media play-pause|next|previous"),
    }
    String::from("ok")
}

pub fn toggle(arguments: &[String]) -> String {
    if arguments.first().is_some_and(|a| a == "status") {
        let island = Island::read();
        return format!(
            "expanded={} owner={} focused={} hidden={} settings={}",
            island.expanded,
            island.owner.as_deref().unwrap_or("-"),
            crate::auxiliary::focused_monitor().as_deref().unwrap_or("-"),
            exposure::tucked(),
            settings::open(),
        );
    }
    if arguments
        .first()
        .is_some_and(|a| a == "reveal" || a == "show")
    {
        exposure::reveal();
        return "ok".into();
    }
    let focused = crate::auxiliary::focused_monitor();
    let open_here = {
        let island = Island::read();
        island.expanded && island.owner == focused
    };
    // `island` toggles; `open` / `close` are for scripts that need a known end
    // state. A tucked (auto-hidden) island opens in one call: revealing it
    // without opening used to swallow the first SUPER+A.
    let open = match arguments.first().map(String::as_str) {
        Some("open") => true,
        Some("close") => false,
        None | Some("toggle") => !open_here,
        Some(other) => return format!("usage: island [toggle|open|close|reveal|status], not {other}"),
    };
    // The expanded island IS the Control Center now: open it whole (home
    // page) on the focused screen; a bare row with no panel under it was
    // the old island and has nothing to use.
    if open {
        if !settings::open() || !open_here {
            settings::command_on(focused.as_deref(), &["home".to_string()]);
        }
    } else {
        dismiss_outside();
    }
    "ok".into()
}

fn adjust(control: Osd, direction: &str) {
    let arguments = [direction.to_string()];
    match control {
        Osd::Volume => {
            volume(&arguments);
        }
        Osd::Brightness => {
            let reply = crate::backlight::brightness(&arguments);
            if crate::backlight::percent().is_none() {
                eprintln!("{reply}");
            }
        }
    }
}

// A single slim status row. Each section has a fixed budget so metadata
// changes never push the audio/brightness controls off the island.
fn reveal_slider(control: Osd) {
    popups::show(match control {
        Osd::Volume => popups::Popup::Volume,
        Osd::Brightness => popups::Popup::Brightness,
    });
}

// macOS keeps controls monochrome; the selected one gets the control fill
fn control_color(_control: Osd) -> Color {
    crate::theme::palette().foreground
}


/// The speaker (with 0-3 waves, or slashed) or the sun, as SF-style
/// vectors: shared by the Control Center buttons, their sliders and the HUD.
fn control_symbol(control: Osd, volume: u8, muted: bool, size: f32, color: Color) -> Rectangle {
    match control {
        Osd::Volume => symbols::sized(symbols::speaker(volume, muted, color), size),
        Osd::Brightness => symbols::sized(symbols::sun(color), size),
    }
}

fn control_icon(control: Osd, muted: bool, selected: bool) -> Rectangle {
    let volume = Audio::read().volume();
    let key = if control == Osd::Volume {
        "control:volume"
    } else {
        "control:brightness"
    };
    Rectangle::new()
        .width(CONTROL_WIDTH)
        .height(36.0)
        .scale(crate::feedback::scale(key))
        .radius(Full)
        .fill(crate::theme::control_fill(selected))
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_hover(move |entered| {
            popups::hover(
                match control {
                    Osd::Volume => popups::Popup::Volume,
                    Osd::Brightness => popups::Popup::Brightness,
                },
                entered,
            )
        })
        .on_click(move |button| match button {
            Button::Left => {
                crate::feedback::click(key);
                reveal_slider(control)
            }
            Button::Right if control == Osd::Volume => Audio::toggle_mute(),
            _ => {}
        })
        .on_scroll(move |scroll| {
            if scroll.y != 0.0 {
                reveal_slider(control);
                adjust(control, if scroll.y < 0.0 { "up" } else { "down" });
            }
        })
        .child(control_symbol(control, volume, muted, 20.0, control_color(control)))
}

fn row_cell(
    width: f32,
    align: impl Into<Align>,
    content: impl amane::Widget + 'static,
) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(36.0)
        .align_child(align, Center)
        .child(content)
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_never_hides_and_collapse_must_finish_first() {
        assert!(!hide_eligible(
            Mode::Panel,
            Mode::Idle,
            Duration::from_secs(10)
        ));
        assert!(!hide_eligible(
            Mode::Idle,
            Mode::Panel,
            Duration::from_millis(619)
        ));
        assert!(hide_eligible(
            Mode::Idle,
            Mode::Panel,
            Duration::from_millis(620)
        ));
    }

    #[test]
    fn content_transition_fades_out_then_returns_to_crisp_rest() {
        assert_eq!(content_motion(0.0), (true, 1.0, 1.0));
        assert!(content_motion(0.07).1 < 0.05);
        assert_eq!(content_motion(0.08).1, 0.0);
        assert!(content_motion(0.26).1 > 0.5);
        assert_eq!(content_motion(0.5), (false, 1.0, 1.0));
    }

    #[test]
    fn starts_collapsed_without_an_osd() {
        let island = Island::new();
        assert!(island.wanted_mode() == Mode::Idle);
        assert_eq!(island.face.width.value(), IDLE.0);
        assert_eq!(island.face.height.value(), IDLE.1);
    }

    #[test]
    fn opened_panel_remains_open_during_osd_changes() {
        let mut island = Island::new();
        island.expanded = true;
        island.show_osd(Osd::Volume, 75, false);
        assert!(island.face.mode == Mode::Panel);
        assert_eq!(island.percent, 75);
        island.expanded = false;
        island.sync();
        assert!(island.face.mode == Mode::Idle);
    }

}
