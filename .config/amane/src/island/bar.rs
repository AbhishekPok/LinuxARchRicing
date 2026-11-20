//! The macOS-style menu bar ("Graphite Glass", 12 Nov 2026).
//!
//!   left    logo (utility panel), focused app, this screen's workspace dots
//!   centre  kept EMPTY: the island hangs there as a notch, so nothing in the
//!           bar may ever be drawn inside `NOTCH_SLOT`
//!   right   ops chips (aws / kube / vpn / ssh), cpu+mem, network, battery,
//!           the Control Center button and the clock
//!
//! Each side gets exactly `(screen - NOTCH_SLOT) / 2` minus margins. Items are
//! measured and the least important ones are dropped until the side fits, so
//! a narrow screen loses cpu/mem before anything slides under the notch.
//!
//! Modes (`amane ipc call bar autohide|always|off|toggle|reveal|status`,
//! kept in ~/.local/state/amane/bar-mode):
//!   autohide  default. Hidden and reserving nothing; touching the top edge
//!             slides it down over the windows, leaving hides it again.
//!             The middle of the edge belongs to the notch's own sensor.
//!   always    always shown, reserves its 32px like a normal bar.
//!   off       no bar at all (quick rollback; see rollback-macbar.sh for
//!             the full one).
use super::*;
use amane::{Battery, Layer, Network, Size, Stack, Widget, Workspaces, Zone};

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;

pub(crate) const HEIGHT: f32 = 32.0;
/// Width reserved in the middle of the bar for the notch. The widest resting
/// notch is the OSD (310); the rest is breathing room on both sides.
pub(crate) const NOTCH_SLOT: f32 = 360.0;
const EDGE: f32 = 10.0;
const GAP: f32 = 8.0;

const MONO: &str = "JetBrainsMono Nerd Font";
const ICONS: &str = "Material Symbols Rounded";

// Material Symbols
const LAN: &str = "\u{eb2f}";

pub fn view_per_monitor(monitor: &amane::Monitor) -> LayerWindow {
    view(&monitor.name)
}

fn view(monitor: &str) -> LayerWindow {
    let (width, _) = amane::window_size();
    let (mode, hidden) = {
        let state = BarState::read();
        (state.mode, state.slide.value().clamp(0.0, 1.0))
    };
    let offset = -(HEIGHT + 2.0) * hidden;
    let side = ((width - NOTCH_SLOT) / 2.0 - EDGE).max(0.0);
    let p = crate::theme::palette();

    let glass = Rectangle::new()
        .width(width)
        .height(HEIGHT)
        .fill(crate::theme::bar_fill());
    let hairline = Rectangle::new()
        .width(width)
        .height(1.0)
        .translate(0.0, HEIGHT - 1.0)
        .fill(crate::theme::hairline());

    let left = Rectangle::new()
        .width(side)
        .height(HEIGHT)
        .translate(EDGE, 0.0)
        .align_child(Start, Center)
        .child(Row::new(fit(left_items(monitor, &p), side)).gap(GAP + 4.0).align(Center));
    let right = Rectangle::new()
        .width(side)
        .height(HEIGHT)
        .translate(width - EDGE - side, 0.0)
        .align_child(End, Center)
        .child(Row::new(fit(right_items(monitor, &p), side)).gap(GAP).align(Center));

    // Fully hidden: only a 2px strip on the top edge takes the pointer, so
    // the windows underneath keep every click. Off: nothing at all.
    let input = if mode == Mode::Off {
        vec![]
    } else if hidden > 0.98 {
        vec![amane::InputArea { x: 0, y: 0, width: width as i32, height: EDGE_SENSOR }]
    } else {
        vec![amane::InputArea { x: 0, y: 0, width: width as i32, height: HEIGHT as i32 }]
    };
    let content = amane::Stack::new(children![glass, hairline, left, right])
        .width(width)
        .height(HEIGHT);
    let root = Rectangle::new()
        .width(width)
        .height(HEIGHT)
        .fill(Color::TRANSPARENT)
        .on_hover(pointer)
        .child(Rectangle::new().width(width).height(HEIGHT).translate(0.0, offset).child(content));

    LayerWindow::new()
        .width(Full)
        .height(HEIGHT)
        .anchor_horizontal(Horizontal::Middle)
        .anchor_vertical(Vertical::Top)
        .layer(Layer::Top)
        .space(if mode == Mode::Always { Zone::Reserve } else { Zone::Respect })
        .keyboard(amane::Keyboard::None)
        .input_region(input)
        .namespace("amane-bar")
        .child(root)
}

// ---------------------------------------------------------- visibility

const EDGE_SENSOR: i32 = 2;
const LINGER: Duration = Duration::from_millis(700);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Autohide,
    Always,
    Off,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Autohide => "autohide",
            Mode::Always => "always",
            Mode::Off => "off",
        }
    }
    fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "autohide" | "auto" => Some(Mode::Autohide),
            "always" | "show" | "on" => Some(Mode::Always),
            "off" | "none" => Some(Mode::Off),
            _ => None,
        }
    }
}

struct BarState {
    mode: Mode,
    // the pointer is on the bar (or its edge strip)
    inside: bool,
    // shown until then after the pointer left, or after an IPC reveal
    until: Option<Instant>,
    // 0 shown .. 1 hidden
    slide: motion::Curve,
    hidden: bool,
}

impl Service for BarState {
    fn new() -> Self {
        let mode = std::fs::read_to_string(mode_path())
            .ok()
            .and_then(|t| Mode::parse(&t))
            .unwrap_or(Mode::Autohide);
        let hidden = mode != Mode::Always;
        Self {
            mode,
            inside: false,
            until: None,
            slide: motion::Curve::new(if hidden { 1.0 } else { 0.0 }, motion::Easing::Standard),
            hidden,
        }
    }
    fn interval() -> Duration {
        Duration::from_millis(50)
    }
    fn update(&mut self) -> bool {
        let lingering = self.until.is_some_and(|t| Instant::now() < t);
        let hidden = match self.mode {
            Mode::Always => false,
            Mode::Off => true,
            Mode::Autohide => !self.inside && !lingering && !exposure::panel_open(),
        };
        if hidden == self.hidden {
            return !self.slide.settled();
        }
        self.hidden = hidden;
        self.slide.to_over(
            if hidden { 1.0 } else { 0.0 },
            Duration::from_millis(if hidden { 180 } else { 220 }),
        );
        true
    }
}

fn mode_path() -> std::path::PathBuf {
    settings::state_dir().join("bar-mode")
}

fn pointer(inside: bool) {
    hover(inside);
    // bar and notch come down together, like one menu bar
    exposure::pointer(inside);
}

/// The pointer entered or left the bar (or the notch sensor above it).
pub(super) fn hover(inside: bool) {
    let mut s = BarState::write();
    s.inside = inside;
    if !inside {
        s.until = Some(Instant::now() + LINGER);
    }
}

fn set_mode(mode: Mode) {
    let path = mode_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, format!("{}\n", mode.name()));
    BarState::write().mode = mode;
}

/// `amane ipc call bar <autohide|always|off|toggle|reveal|status>`
pub fn command(args: &[String]) -> String {
    match args.first().map(String::as_str) {
        None | Some("status") => {}
        Some("toggle") => {
            let next = if BarState::read().mode == Mode::Always {
                Mode::Autohide
            } else {
                Mode::Always
            };
            set_mode(next);
        }
        Some("reveal") => {
            BarState::write().until = Some(Instant::now() + Duration::from_millis(3000));
            exposure::reveal();
        }
        Some(other) => match Mode::parse(other) {
            Some(mode) => set_mode(mode),
            None => return "usage: bar autohide|always|off|toggle|reveal|status".into(),
        },
    }
    let s = BarState::read();
    format!("mode={} hidden={}", s.mode.name(), s.hidden)
}

type Item = (u8, Box<dyn Widget>);

fn measure(widget: &dyn Widget) -> f32 {
    match widget.width() {
        Size::Fixed(w) => w,
        Size::Parent => 0.0,
    }
}

/// Drops the lowest-priority items until the rest fit in `room`.
fn fit(mut items: Vec<Item>, room: f32) -> Vec<Box<dyn Widget>> {
    loop {
        let total: f32 = items.iter().map(|(_, w)| measure(w.as_ref())).sum::<f32>()
            + GAP * items.len().saturating_sub(1) as f32;
        if total <= room || items.is_empty() {
            break;
        }
        let lowest = items
            .iter()
            .enumerate()
            .min_by_key(|(_, (priority, _))| *priority)
            .map(|(i, _)| i)
            .unwrap();
        items.remove(lowest);
    }
    items.into_iter().map(|(_, w)| w).collect()
}

fn icon(glyph: &str, size: f32, color: Color) -> Text {
    Text::new(glyph).font(ICONS).weight(Weight::Light).size(size).tight().color(color)
}

/// A rounded hit target sized to its content.
fn hit(content: impl Widget + 'static, pad: f32, fill: Color) -> Rectangle {
    let w = measure(&content) + pad * 2.0;
    Rectangle::new()
        .width(w)
        .height(22.0)
        .radius(6.0)
        .fill(fill)
        .align_child(Center, Center)
        .child(content)
}

// ---------------------------------------------------------------- left

fn left_items(monitor: &str, p: &crate::theme::Palette) -> Vec<Item> {
    let here = monitor.to_string();
    let logo = hit(symbols::mark(p.foreground), 4.0, Color::TRANSPARENT)
        .cursor(Pointer)
        .on_click(move |button| {
            if matches!(button, Button::Left) {
                crate::auxiliary::command_on(&here, &["utility".to_string()]);
            }
        });
    let app = Text::new(ActiveWindow::read().label.clone())
        .size(13.0)
        .weight(Weight::Bold)
        .color(p.foreground);
    vec![
        (100, Box::new(logo)),
        (90, Box::new(app)),
        (80, Box::new(workspaces(monitor, p))),
    ]
}

fn workspaces(monitor: &str, p: &crate::theme::Palette) -> Row {
    let mut list: Vec<amane::Workspace> = Workspaces::read()
        .list()
        .iter()
        .filter(|w| w.output().is_none_or(|o| o == monitor) && w.id() > 0)
        .cloned()
        .collect();
    list.sort_by_key(|w| w.id());
    let dots: Vec<Box<dyn Widget>> = list
        .into_iter()
        .map(|w| {
            let id = w.id();
            let (width, color) = if w.active() {
                (22.0, p.blue)
            } else if w.urgent() {
                (8.0, p.red)
            } else if w.windows() > 0 {
                (8.0, Color::rgba(245, 245, 247, 150))
            } else {
                (8.0, Color::rgba(245, 245, 247, 60))
            };
            Box::new(
                Rectangle::new()
                    .width(width + 6.0)
                    .height(22.0)
                    .align_child(Center, Center)
                    .cursor(Pointer)
                    .on_click(move |button| {
                        if matches!(button, Button::Left) {
                            Workspaces::focus(id);
                        }
                    })
                    .child(Rectangle::new().width(width).height(8.0).radius(Full).fill(color)),
            ) as Box<dyn Widget>
        })
        .collect();
    Row::new(dots).gap(0.0).align(Center)
}

// --------------------------------------------------------------- right

/// The chip's SF-style symbol for an ops kind.
fn ops_symbol(kind: &str, color: Color) -> Box<dyn Widget> {
    match kind {
        "aws" => Box::new(symbols::cloud(color)),
        "kube" => Box::new(symbols::helm(color)),
        "vpn" => Box::new(symbols::key(color)),
        _ => Box::new(symbols::terminal(color)),
    }
}

fn chip(kind: &str, text: &str, fg: Color, fill: Color, border: Option<Color>) -> Rectangle {
    let row = Row::new(vec![
        ops_symbol(kind, fg),
        Box::new(Text::new(text).font(MONO).size(11.5).color(fg)),
    ])
    .gap(5.0)
    .align(Center);
    let chip = hit(row, 8.0, fill);
    match border {
        Some(c) => chip.border(1.0, c),
        None => chip,
    }
}

fn env_chip(kind: &str, class: &str, text: &str, p: &crate::theme::Palette) -> Rectangle {
    match class {
        "prod" => chip(
            kind,
            text,
            p.red,
            Color::rgba(p.red.red(), p.red.green(), p.red.blue(), 46),
            Some(Color::rgba(p.red.red(), p.red.green(), p.red.blue(), 100)),
        ),
        "staging" | "stage" | "preprod" => chip(
            kind,
            text,
            p.yellow,
            Color::rgba(p.yellow.red(), p.yellow.green(), p.yellow.blue(), 40),
            Some(Color::rgba(p.yellow.red(), p.yellow.green(), p.yellow.blue(), 82)),
        ),
        _ => chip(kind, text, p.foreground, crate::theme::control_fill(false), None),
    }
}

/// An SF-style symbol with its value beside it, like a macOS status item.
fn stat(symbol: impl Widget + 'static, value: String, color: Color) -> Row {
    Row::new(vec![
        Box::new(symbol) as Box<dyn Widget>,
        Box::new(Text::new(value).size(12.5).weight(Weight::Medium).color(color)),
    ])
    .gap(4.0)
    .align(Center)
}

fn right_items(monitor: &str, p: &crate::theme::Palette) -> Vec<Item> {
    let mut items: Vec<Item> = vec![];
    let mut kube_shown = false;

    for (kind, class, text) in ops::chips() {
        let priority = match kind {
            "aws" => 60,
            "vpn" => 40,
            _ => 45,
        };
        let text = if text.is_empty() { kind.to_string() } else { text };
        // SSO session about to run out / already out: say so on the chip
        let (text, class) = match (kind, sso_left(&text)) {
            ("aws", Some(left)) if left <= 0 => (format!("{text} · expired"), "prod".to_string()),
            ("aws", Some(left)) if left < 15 * 60 => {
                (format!("{text} · {}m", (left + 59) / 60), "staging".to_string())
            }
            _ => (text, class),
        };
        items.push((priority, Box::new(env_chip(kind, &class, &text, p))));
        if kind == "aws" {
            if let Some(kube) = Kube::read().label.clone() {
                let class = if kube.contains("prod") && !kube.contains("preprod") {
                    "prod"
                } else {
                    "dev"
                };
                items.push((55, Box::new(env_chip("kube", class, &kube, p))));
                kube_shown = true;
            }
        }
    }
    // kube without an aws profile still shows
    if !kube_shown {
        if let Some(kube) = Kube::read().label.clone() {
            items.push((55, Box::new(env_chip("kube", "dev", &kube, p))));
        }
    }

    let (cpu, mem, temp) = system::snapshot();
    let level = |v: u8, warn: u8, crit: u8| {
        if v >= crit {
            p.red
        } else if v >= warn {
            p.yellow
        } else {
            p.foreground
        }
    };
    // a hot package swaps the cpu chip's value for the temperature
    let (cpu_text, cpu_color) = if temp >= 80 {
        (format!("{temp}°"), p.red)
    } else {
        (format!("{cpu}%"), level(cpu, 75, 90))
    };
    let stats = Row::new(vec![
        Box::new(stat(symbols::cpu(p.foreground), cpu_text, cpu_color)) as Box<dyn Widget>,
        Box::new(stat(symbols::memory(p.foreground), format!("{mem}%"), level(mem, 75, 85))),
    ])
    .gap(10.0)
    .align(Center);
    items.push((20, Box::new(Rectangle::new().width(measure(&stats) + 6.0).height(22.0).align_child(Center, Center).child(stats))));

    // each button opens its panel on this bar's own screen
    let here_net = monitor.to_string();
    let here_cc = monitor.to_string();
    let net = Network::read();
    let (link, connected, strength) = (net.link(), net.connected(), net.strength());
    drop(net);
    // Keep Awake: a cup while auto-lock is held off; click turns it off
    if super::awake::on() {
        items.push((
            65,
            Box::new(
                hit(symbols::cup(p.foreground), 4.0, crate::theme::control_fill(false))
                    .cursor(Pointer)
                    .on_click(|b| {
                        if matches!(b, Button::Left) {
                            super::awake::set(false);
                        }
                    }),
            ),
        ));
    }
    let color = if connected { p.foreground } else { p.secondary };
    let network: Box<dyn Widget> = if matches!(link, amane::Link::Wired) {
        Box::new(icon(LAN, 17.0, color))
    } else {
        Box::new(symbols::wifi(connected, strength, color))
    };
    items.push((
        70,
        Box::new(
            Rectangle::new()
                .width(measure(network.as_ref()) + 8.0)
                .height(22.0)
                .radius(6.0)
                .align_child(Center, Center)
                .cursor(Pointer)
                .on_click(move |b| {
                    if matches!(b, Button::Left) {
                        settings::command_on(Some(&here_net), &["network".to_string()]);
                    }
                })
                .child(Stack::new(vec![network]).width(18.0).height(16.0)),
        ),
    ));

    let (present, percent, charging) = {
        let b = Battery::read();
        (b.present(), b.percent(), b.charging())
    };
    if present {
        let level = if percent <= 15 && !charging { p.red } else { p.foreground };
        items.push((
            75,
            Box::new(
                Row::new(vec![
                    Box::new(Text::new(format!("{percent}%")).size(12.5).weight(Weight::Medium).color(p.foreground))
                        as Box<dyn Widget>,
                    Box::new(symbols::battery(percent, charging, p.foreground, level)),
                ])
                .gap(5.0)
                .align(Center),
            ),
        ));
    }

    items.push((
        95,
        Box::new(
            hit(symbols::control_center(p.foreground), 6.0, crate::theme::control_fill(false))
                .cursor(Pointer)
                .on_click(move |b| {
                    if matches!(b, Button::Left) {
                        settings::command_on(Some(&here_cc), &[]);
                    }
                }),
        ),
    ));

    items
}

// ------------------------------------------------------- active window

/// The focused window's class, prettied for the bar ("org.gnome.Nautilus"
/// -> "Nautilus", "google-chrome" -> "Google Chrome"). Follows Hyprland's
/// `activewindow>>` events; no polling.
struct ActiveWindow {
    label: String,
}

impl Service for ActiveWindow {
    fn new() -> Self {
        Self { label: pretty(&initial_class()) }
    }
    fn listen() {
        loop {
            if let Some(dir) = hypr_dir() {
                if let Ok(stream) = UnixStream::connect(dir.join(".socket2.sock")) {
                    for line in BufReader::new(stream).lines().map_while(Result::ok) {
                        if let Some(rest) = line.strip_prefix("activewindow>>") {
                            let class = rest.split(',').next().unwrap_or("");
                            let label = pretty(class);
                            if Self::read().label != label {
                                Self::write().label = label;
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

fn hypr_dir() -> Option<std::path::PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    Some(std::path::PathBuf::from(runtime).join("hypr").join(sig))
}

fn initial_class() -> String {
    let Some(dir) = hypr_dir() else {
        return String::new();
    };
    let Ok(mut stream) = UnixStream::connect(dir.join(".socket.sock")) else {
        return String::new();
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    if stream.write_all(b"j/activewindow").is_err() {
        return String::new();
    }
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    reply
        .split("\"class\": \"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("")
        .to_string()
}

fn pretty(class: &str) -> String {
    let class = class.rsplit('.').next().unwrap_or(class);
    if class.is_empty() {
        return "Desktop".into();
    }
    class
        .split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------- kube

/// `context/namespace` from the kubeconfig, re-read only when its mtime
/// changes (checked every 3 s, no fork). None when there is no kubeconfig.
struct Kube {
    label: Option<String>,
}

impl Service for Kube {
    fn new() -> Self {
        Self { label: None }
    }
    fn listen() {
        let mut seen = None;
        loop {
            let path = kubeconfig();
            let stamp = path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .and_then(|m| m.modified().ok());
            if stamp != seen {
                seen = stamp;
                let label = path
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|text| parse_kube(&text));
                if Self::read().label != label {
                    Self::write().label = label;
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

fn kubeconfig() -> Option<std::path::PathBuf> {
    if let Some(list) = std::env::var_os("KUBECONFIG") {
        if let Some(first) = std::env::split_paths(&list).next() {
            return Some(first);
        }
    }
    Some(std::path::PathBuf::from(std::env::var_os("HOME")?).join(".kube/config"))
}

/// Minimal kubeconfig reader: `current-context`, then that context's
/// namespace. EKS contexts are ARNs, so only the part after the last `/` is
/// kept ("arn:aws:eks:...:cluster/prod" -> "prod").
fn parse_kube(text: &str) -> Option<String> {
    let current = text
        .lines()
        .find_map(|l| l.strip_prefix("current-context:"))?
        .trim()
        .trim_matches('"')
        .to_string();
    if current.is_empty() {
        return None;
    }
    // walk the contexts list; each entry is `- context: {..., namespace}` + `name:`
    let mut namespace: Option<String> = None;
    let mut block_ns: Option<String> = None;
    let mut in_contexts = false;
    for line in text.lines() {
        if !line.starts_with(' ') && !line.starts_with('-') {
            in_contexts = line.starts_with("contexts:");
            continue;
        }
        if !in_contexts {
            continue;
        }
        let t = line.trim_start_matches(['-', ' ']);
        if line.trim_start().starts_with("- ") {
            block_ns = None;
        }
        if let Some(ns) = t.strip_prefix("namespace:") {
            block_ns = Some(ns.trim().trim_matches('"').to_string());
        }
        if let Some(name) = t.strip_prefix("name:") {
            if name.trim().trim_matches('"') == current {
                namespace = block_ns.clone();
            }
        }
    }
    let short = current.rsplit('/').next().unwrap_or(&current).to_string();
    Some(match namespace {
        Some(ns) if !ns.is_empty() => format!("{short}/{ns}"),
        _ => short,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kube_context_and_namespace() {
        let cfg = "apiVersion: v1\ncontexts:\n- context:\n    cluster: a\n    namespace: payments\n    user: u\n  name: arn:aws:eks:ap-south-1:1:cluster/eks-prod\n- context:\n    cluster: b\n  name: kind\ncurrent-context: arn:aws:eks:ap-south-1:1:cluster/eks-prod\n";
        assert_eq!(parse_kube(cfg).as_deref(), Some("eks-prod/payments"));
        let cfg2 = cfg.replace("current-context: arn:aws:eks:ap-south-1:1:cluster/eks-prod", "current-context: kind");
        assert_eq!(parse_kube(&cfg2).as_deref(), Some("kind"));
    }

    #[test]
    fn pretty_class() {
        assert_eq!(pretty("org.gnome.Nautilus"), "Nautilus");
        assert_eq!(pretty("google-chrome"), "Google Chrome");
        assert_eq!(pretty("kitty"), "Kitty");
        assert_eq!(pretty(""), "Desktop");
    }
}

// ---------------------------------------------------------- AWS SSO expiry

/// When each AWS profile's SSO session runs out (unix seconds). Built from
/// ~/.aws/config (profile -> sso-session -> start URL) and the CLI's SSO cache
/// (start URL -> expiresAt); only those two fields of the cache are read,
/// never the tokens. Re-read every 30 s, no AWS command is run.
struct AwsSso {
    expiry: std::collections::HashMap<String, i64>,
}

impl Service for AwsSso {
    fn new() -> Self {
        Self { expiry: read_sso() }
    }
    fn listen() {
        loop {
            std::thread::sleep(Duration::from_secs(30));
            let next = read_sso();
            if Self::read().expiry != next {
                Self::write().expiry = next;
            }
        }
    }
}

/// Seconds left on `profile`'s SSO session, if it is an SSO profile with a
/// cached login.
fn sso_left(profile: &str) -> Option<i64> {
    let at = *AwsSso::read().expiry.get(profile)?;
    Some(at - crate::lock::now_secs())
}

fn read_sso() -> std::collections::HashMap<String, i64> {
    let mut out = std::collections::HashMap::new();
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return out;
    };
    let Ok(config) = std::fs::read_to_string(home.join(".aws/config")) else {
        return out;
    };
    // profile -> session name or start URL; session -> start URL
    let mut profile_ref: Vec<(String, String)> = vec![];
    let mut session_url = std::collections::HashMap::new();
    let mut section = String::new();
    for line in config.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim().to_string());
        if let Some(profile) = section.strip_prefix("profile ") {
            if key == "sso_session" || key == "sso_start_url" {
                profile_ref.push((profile.trim().to_string(), value));
            }
        } else if let Some(session) = section.strip_prefix("sso-session ") {
            if key == "sso_start_url" {
                session_url.insert(session.trim().to_string(), value);
            }
        }
    }
    // start URL -> latest expiry among cached logins
    let mut url_expiry: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    if let Ok(dir) = std::fs::read_dir(home.join(".aws/sso/cache")) {
        for entry in dir.flatten() {
            let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
            let (Some(url), Some(at)) = (json_str(&text, "startUrl"), json_str(&text, "expiresAt")) else {
                continue;
            };
            let Some(at) = parse_utc(&at) else { continue };
            let slot = url_expiry.entry(url).or_insert(at);
            *slot = (*slot).max(at);
        }
    }
    for (profile, reference) in profile_ref {
        let url = session_url.get(&reference).cloned().unwrap_or(reference);
        if let Some(at) = url_expiry.get(&url) {
            out.insert(profile, *at);
        }
    }
    out
}

/// The string value of `"key": "value"` in a small flat JSON object.
fn json_str(text: &str, key: &str) -> Option<String> {
    let start = text.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = text[start..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// "2026-11-12T13:45:00Z" (or "...UTC") as unix seconds.
fn parse_utc(text: &str) -> Option<i64> {
    let t = text.get(..19)?;
    let num = |a: usize, b: usize| t.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, s) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    // days from civil (Howard Hinnant)
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

#[cfg(test)]
mod sso_tests {
    use super::*;

    #[test]
    fn utc_timestamps_parse() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("2026-11-12T07:28:25Z"), Some(1_794_468_505));
        assert_eq!(parse_utc("2026-11-12T07:28:25UTC"), Some(1_794_468_505));
        assert_eq!(parse_utc("nope"), None);
    }

    #[test]
    fn flat_json_fields() {
        let text = r#"{"startUrl": "https://example.awsapps.com/start", "expiresAt":"2026-11-12T07:28:25Z", "accessToken": "x"}"#;
        assert_eq!(json_str(text, "startUrl").as_deref(), Some("https://example.awsapps.com/start"));
        assert_eq!(json_str(text, "expiresAt").as_deref(), Some("2026-11-12T07:28:25Z"));
        assert_eq!(json_str(text, "missing"), None);
    }
}
