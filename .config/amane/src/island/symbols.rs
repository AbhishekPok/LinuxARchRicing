//! SF-Symbols-style menu bar icons, drawn as vectors (Apple's SF Symbols are
//! licensed for Apple platforms only, so they are redrawn here in the same
//! spirit: thin even strokes, rounded caps, outline shapes). Every icon sits
//! in a 16px-tall box so the bar keeps one rhythm.
use super::*;
use amane::{Arc, Cap, Canvas, Circle, Line, Path, Shape, Stack, Widget};

const H: f32 = 16.0;
const STROKE: f32 = 1.5;

fn canvas(width: f32, shapes: Vec<Box<dyn Shape>>) -> Canvas {
    Canvas::new().width(width).height(H).shapes(shapes)
}

fn stroked<S: Shape + 'static>(shape: S, color: Color) -> Box<dyn Shape> {
    Box::new(shape.stroke(STROKE, color).cap(Cap::Round))
}

/// wifi: three arcs and a dot; arcs above the signal strength are dimmed.
pub(super) fn wifi(connected: bool, strength: u8, color: Color) -> Canvas {
    let dim = Color::rgba(color.red(), color.green(), color.blue(), 80);
    let (cx, cy) = (9.0, 14.0);
    let bars = if !connected {
        0
    } else if strength >= 67 {
        3
    } else if strength >= 34 {
        2
    } else {
        1
    };
    let mut shapes: Vec<Box<dyn Shape>> = vec![Box::new(
        Circle::new().center(cx, cy - 0.5).radius(1.6).fill(if connected { color } else { dim }),
    )];
    for (i, r) in [4.5_f32, 8.0, 11.5].into_iter().enumerate() {
        let on = i < bars;
        shapes.push(stroked(
            Arc::new().center(cx, cy).radius(r).start(-45.0).sweep(90.0),
            if on { color } else { dim },
        ));
    }
    canvas(18.0, shapes)
}

/// battery: outlined body with the charge level inside and a nub, like macOS.
pub(super) fn battery(percent: u8, charging: bool, color: Color, level: Color) -> Stack {
    let (w, h) = (23.0, 11.5);
    let outline = Color::rgba(color.red(), color.green(), color.blue(), 140);
    let inner = (w - 4.0) * f32::from(percent.min(100)) / 100.0;
    let mut layers: Vec<Box<dyn Widget>> = vec![
        Box::new(
            Rectangle::new()
                .width(w)
                .height(h)
                .translate(0.0, (H - h) / 2.0)
                .radius(3.5)
                .border(1.0, outline)
                .fill(Color::TRANSPARENT),
        ),
        Box::new(
            Rectangle::new()
                .width(inner.max(1.5))
                .height(h - 4.0)
                .translate(2.0, (H - h) / 2.0 + 2.0)
                .radius(1.8)
                .fill(level),
        ),
        Box::new(
            Rectangle::new()
                .width(1.6)
                .height(4.0)
                .translate(w + 0.8, (H - 4.0) / 2.0)
                .radius(1.0)
                .fill(outline),
        ),
    ];
    if charging {
        let bolt = Path::new()
            .move_to(13.0, 2.5)
            .line_to(8.0, 8.8)
            .line_to(11.2, 8.8)
            .line_to(10.0, 13.5)
            .line_to(15.0, 7.2)
            .line_to(11.8, 7.2)
            .close();
        layers.push(Box::new(canvas(w, vec![Box::new(bolt.fill(color).stroke(1.0, Color::BLACK))])));
    }
    Stack::new(layers).width(w + 3.0).height(H)
}

/// Control Center: two switches, knobs on opposite sides.
pub(super) fn control_center(color: Color) -> Stack {
    let pill = |y: f32, knob_right: bool| -> Vec<Box<dyn Widget>> {
        vec![
            Box::new(
                Rectangle::new()
                    .width(16.0)
                    .height(6.5)
                    .translate(0.0, y)
                    .radius(Full)
                    .border(1.3, color)
                    .fill(Color::TRANSPARENT),
            ),
            Box::new(
                Rectangle::new()
                    .width(4.0)
                    .height(4.0)
                    .translate(if knob_right { 10.6 } else { 1.4 }, y + 1.25)
                    .radius(Full)
                    .fill(color),
            ),
        ]
    };
    let mut layers = pill(1.0, true);
    layers.extend(pill(8.5, false));
    Stack::new(layers).width(16.0).height(H)
}

/// cpu: a chip with two pins on each side.
pub(super) fn cpu(color: Color) -> Stack {
    let mut layers: Vec<Box<dyn Widget>> = vec![Box::new(
        Rectangle::new()
            .width(10.0)
            .height(10.0)
            .translate(3.0, 3.0)
            .radius(2.2)
            .border(1.4, color)
            .fill(Color::TRANSPARENT),
    )];
    for o in [5.5_f32, 9.0] {
        for (x, y, w, h) in [(o, 0.5, 1.4, 2.5), (o, 13.0, 1.4, 2.5), (0.5, o, 2.5, 1.4), (13.0, o, 2.5, 1.4)] {
            layers.push(Box::new(
                Rectangle::new().width(w).height(h).translate(x, y).radius(0.7).fill(color),
            ));
        }
    }
    layers.push(Box::new(
        Rectangle::new().width(4.0).height(4.0).translate(6.0, 6.0).radius(1.0).fill(color),
    ));
    Stack::new(layers).width(16.0).height(H)
}

/// memory: a long module with a row of contacts under it.
pub(super) fn memory(color: Color) -> Stack {
    let mut layers: Vec<Box<dyn Widget>> = vec![Box::new(
        Rectangle::new()
            .width(17.0)
            .height(8.0)
            .translate(0.5, 3.0)
            .radius(1.8)
            .border(1.4, color)
            .fill(Color::TRANSPARENT),
    )];
    for i in 0..4 {
        layers.push(Box::new(
            Rectangle::new()
                .width(1.4)
                .height(2.8)
                .translate(3.0 + i as f32 * 3.8, 11.5)
                .radius(0.7)
                .fill(color),
        ));
    }
    Stack::new(layers).width(18.0).height(H)
}

/// cloud (aws): an outlined cloud.
pub(super) fn cloud(color: Color) -> Canvas {
    // One smooth closed outline (SF "cloud"): flat base, small left lobe,
    // big top lobe, small right lobe. The old three-arc version was joined by
    // straight segments, which drew kinks and stray edges.
    let path = Path::new()
        .move_to(4.8, 13.2)
        .cubic_to(2.8, 13.2, 1.3, 11.8, 1.3, 10.0)
        .cubic_to(1.3, 8.4, 2.5, 7.1, 4.1, 6.8)
        .cubic_to(4.7, 4.5, 6.7, 2.9, 9.1, 2.9)
        .cubic_to(11.8, 2.9, 13.9, 4.9, 14.2, 7.5)
        .cubic_to(15.8, 7.9, 16.8, 9.2, 16.8, 10.5)
        .cubic_to(16.8, 12.0, 15.6, 13.2, 14.1, 13.2)
        .close();
    canvas(18.0, vec![Box::new(path.stroke(STROKE, color).cap(Cap::Round))])
}

/// kubernetes context: a helm wheel.
pub(super) fn helm(color: Color) -> Canvas {
    let (cx, cy) = (8.0, 8.0);
    let mut shapes: Vec<Box<dyn Shape>> = vec![
        stroked(Circle::new().center(cx, cy).radius(5.0), color),
        Box::new(Circle::new().center(cx, cy).radius(1.6).fill(color)),
    ];
    for k in 0..7 {
        let a = (k as f32) * std::f32::consts::TAU / 7.0;
        let (s, c) = a.sin_cos();
        shapes.push(stroked(
            Line::new().from(cx + s * 2.6, cy - c * 2.6).to(cx + s * 7.2, cy - c * 7.2),
            color,
        ));
    }
    canvas(16.0, shapes)
}

/// ssh / ssm sessions: a terminal window with a prompt.
pub(super) fn terminal(color: Color) -> Stack {
    Stack::new(vec![
        Box::new(
            Rectangle::new()
                .width(16.0)
                .height(12.0)
                .translate(0.5, 2.0)
                .radius(2.5)
                .border(1.4, color)
                .fill(Color::TRANSPARENT),
        ) as Box<dyn Widget>,
        Box::new(canvas(
            17.0,
            vec![
                stroked(Path::new().move_to(4.0, 6.0).line_to(6.5, 8.0).line_to(4.0, 10.0), color),
                stroked(Line::new().from(8.5, 10.5).to(12.0, 10.5), color),
            ],
        )),
    ])
    .width(17.0)
    .height(H)
}

/// vpn: a key.
pub(super) fn key(color: Color) -> Canvas {
    canvas(
        18.0,
        vec![
            stroked(Circle::new().center(5.0, 8.0).radius(3.2), color),
            stroked(Line::new().from(8.2, 8.0).to(16.0, 8.0), color),
            stroked(Line::new().from(13.0, 8.0).to(13.0, 11.0), color),
            stroked(Line::new().from(15.5, 8.0).to(15.5, 10.5), color),
        ],
    )
}

/// the logo: a ring with a dot, the shell's own mark.
pub(super) fn mark(color: Color) -> Canvas {
    canvas(
        16.0,
        vec![
            stroked(Circle::new().center(8.0, 8.0).radius(6.2), color),
            Box::new(Circle::new().center(8.0, 8.0).radius(2.6).fill(color)),
        ],
    )
}

// ---------------------------------------------------------------- panels,
// toolbar, HUD and Spotlight. All drawn in a 16x16 box; `sized` scales one
// to any size, so every surface shares the same strokes.

/// Any symbol at `size` px (drawn at 16, scaled about its centre).
pub(crate) fn sized(symbol: impl Widget + 'static, size: f32) -> Rectangle {
    Rectangle::new()
        .width(size)
        .height(size)
        .align_child(Center, Center)
        .child(Rectangle::new().width(16.0).height(16.0).scale(size / 16.0).child(symbol))
}

fn square(shapes: Vec<Box<dyn Shape>>) -> Canvas {
    Canvas::new().width(16.0).height(16.0).shapes(shapes)
}

/// speaker.wave.N / speaker.slash
pub(crate) fn speaker(volume: u8, muted: bool, color: Color) -> Canvas {
    let body = Path::new()
        .move_to(1.8, 6.0)
        .line_to(4.6, 6.0)
        .line_to(8.2, 2.8)
        .line_to(8.2, 13.2)
        .line_to(4.6, 10.0)
        .line_to(1.8, 10.0)
        .close();
    let mut shapes: Vec<Box<dyn Shape>> = vec![Box::new(body.fill(color))];
    if muted || volume == 0 {
        shapes.push(stroked(Line::new().from(10.5, 6.0).to(14.5, 10.0), color));
        shapes.push(stroked(Line::new().from(14.5, 6.0).to(10.5, 10.0), color));
    } else {
        let waves = if volume >= 66 { 3 } else if volume >= 33 { 2 } else { 1 };
        for (i, r) in [3.0_f32, 5.2, 7.4].into_iter().enumerate().take(waves) {
            let _ = i;
            shapes.push(stroked(Arc::new().center(8.2, 8.0).radius(r).start(50.0).sweep(80.0), color));
        }
    }
    square(shapes)
}

/// sun.max
pub(crate) fn sun(color: Color) -> Canvas {
    let mut shapes: Vec<Box<dyn Shape>> = vec![stroked(Circle::new().center(8.0, 8.0).radius(3.0), color)];
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0;
        let (s, c) = a.sin_cos();
        shapes.push(stroked(
            Line::new().from(8.0 + s * 5.2, 8.0 - c * 5.2).to(8.0 + s * 6.8, 8.0 - c * 6.8),
            color,
        ));
    }
    square(shapes)
}

/// magnifyingglass
pub(crate) fn magnifier(color: Color) -> Canvas {
    square(vec![
        stroked(Circle::new().center(6.8, 6.8).radius(4.6), color),
        stroked(Line::new().from(10.2, 10.2).to(14.2, 14.2), color),
    ])
}

/// lock
pub(crate) fn lock(color: Color) -> Stack {
    Stack::new(vec![
        Box::new(square(vec![stroked(
            Path::new().move_to(5.0, 7.5).line_to(5.0, 5.2).arc(8.0, 5.2, 3.0, 270.0, 180.0).line_to(11.0, 7.5),
            color,
        )])) as Box<dyn Widget>,
        Box::new(
            Rectangle::new().width(10.0).height(7.0).translate(3.0, 7.5).radius(1.8).border(1.4, color),
        ),
    ])
    .width(16.0)
    .height(16.0)
}

/// moon (sleep): a crescent opening to the upper right
pub(crate) fn moon(color: Color) -> Canvas {
    let crescent = Path::new()
        .arc(8.0, 8.0, 6.0, 100.0, 270.0)
        .arc(13.64, 4.05, 5.0, 293.1, -116.2)
        .close();
    square(vec![Box::new(crescent.stroke(1.4, color).cap(Cap::Round))])
}

/// cup.and.saucer (Keep Awake)
pub(crate) fn cup(color: Color) -> Canvas {
    let body = Path::new()
        .move_to(2.4, 6.0)
        .line_to(11.0, 6.0)
        .line_to(10.2, 12.0)
        .line_to(3.2, 12.0)
        .close();
    square(vec![
        Box::new(body.stroke(STROKE, color).cap(Cap::Round)),
        stroked(Arc::new().center(11.0, 8.6).radius(2.0).start(0.0).sweep(180.0), color),
        stroked(Line::new().from(1.4, 14.4).to(12.6, 14.4), color),
        stroked(Line::new().from(5.2, 1.8).to(5.2, 3.6), color),
        stroked(Line::new().from(8.2, 1.8).to(8.2, 3.6), color),
    ])
}

/// power
pub(crate) fn power(color: Color) -> Canvas {
    square(vec![
        stroked(Arc::new().center(8.0, 8.8).radius(5.4).start(35.0).sweep(290.0), color),
        stroked(Line::new().from(8.0, 1.8).to(8.0, 7.6), color),
    ])
}

/// arrow.clockwise (restart)
pub(crate) fn restart(color: Color) -> Canvas {
    square(vec![
        stroked(Arc::new().center(8.0, 8.5).radius(5.4).start(20.0).sweep(300.0), color),
        stroked(Path::new().move_to(9.2, 0.8).line_to(10.2, 3.6).line_to(7.4, 4.6), color),
    ])
}

/// rectangle.portrait.and.arrow.right (log out)
pub(crate) fn logout(color: Color) -> Canvas {
    square(vec![
        stroked(
            Path::new().move_to(8.5, 2.5).line_to(3.0, 2.5).line_to(3.0, 13.5).line_to(8.5, 13.5),
            color,
        ),
        stroked(Line::new().from(6.5, 8.0).to(14.0, 8.0), color),
        stroked(Path::new().move_to(11.2, 5.2).line_to(14.0, 8.0).line_to(11.2, 10.8), color),
    ])
}

/// macwindow
pub(crate) fn window(color: Color) -> Stack {
    Stack::new(vec![
        Box::new(Rectangle::new().width(14.0).height(11.0).translate(1.0, 2.5).radius(2.0).border(1.4, color))
            as Box<dyn Widget>,
        Box::new(Rectangle::new().width(14.0).height(1.4).translate(1.0, 5.5).fill(color)),
    ])
    .width(16.0)
    .height(16.0)
}

/// a dashed selection rectangle
pub(crate) fn selection(color: Color) -> Canvas {
    let mut shapes: Vec<Box<dyn Shape>> = vec![];
    for x in [1.5_f32, 6.2, 10.9] {
        shapes.push(stroked(Line::new().from(x, 2.5).to(x + 2.6, 2.5), color));
        shapes.push(stroked(Line::new().from(x, 13.5).to(x + 2.6, 13.5), color));
    }
    for y in [2.5_f32, 6.6, 10.7] {
        shapes.push(stroked(Line::new().from(1.5, y).to(1.5, y + 2.4), color));
        shapes.push(stroked(Line::new().from(14.5, y).to(14.5, y + 2.4), color));
    }
    square(shapes)
}

/// display
pub(crate) fn display(color: Color) -> Stack {
    Stack::new(vec![
        Box::new(Rectangle::new().width(14.0).height(9.5).translate(1.0, 2.0).radius(1.8).border(1.4, color))
            as Box<dyn Widget>,
        Box::new(square(vec![
            stroked(Line::new().from(8.0, 11.5).to(8.0, 13.8), color),
            stroked(Line::new().from(5.0, 14.0).to(11.0, 14.0), color),
        ])),
    ])
    .width(16.0)
    .height(16.0)
}

/// square.and.arrow.down (save)
pub(crate) fn save(color: Color) -> Canvas {
    square(vec![
        stroked(Path::new().move_to(5.0, 6.5).line_to(2.5, 6.5).line_to(2.5, 14.0).line_to(13.5, 14.0).line_to(13.5, 6.5).line_to(11.0, 6.5), color),
        stroked(Line::new().from(8.0, 1.5).to(8.0, 10.0), color),
        stroked(Path::new().move_to(5.3, 7.5).line_to(8.0, 10.2).line_to(10.7, 7.5), color),
    ])
}

/// pencil.tip (annotate)
pub(crate) fn pencil(color: Color) -> Canvas {
    square(vec![stroked(
        Path::new().move_to(2.5, 13.5).line_to(3.2, 10.4).line_to(11.2, 2.4).line_to(13.6, 4.8).line_to(5.6, 12.8).close(),
        color,
    )])
}

/// checkmark
pub(crate) fn checkmark(color: Color) -> Canvas {
    square(vec![stroked(Path::new().move_to(2.5, 8.5).line_to(6.2, 12.2).line_to(13.5, 3.8), color)])
}

/// xmark
pub(crate) fn xmark(color: Color) -> Canvas {
    square(vec![
        stroked(Line::new().from(3.5, 3.5).to(12.5, 12.5), color),
        stroked(Line::new().from(12.5, 3.5).to(3.5, 12.5), color),
    ])
}

/// chevron.left / chevron.right
pub(crate) fn chevron(right: bool, color: Color) -> Canvas {
    let (a, b) = if right { (5.5, 10.5) } else { (10.5, 5.5) };
    square(vec![stroked(Path::new().move_to(a, 2.5).line_to(b, 8.0).line_to(a, 13.5), color)])
}

/// backward.fill / forward.fill
pub(crate) fn skip(forward: bool, color: Color) -> Canvas {
    let tri = |x0: f32| {
        let (a, b) = if forward { (x0, x0 + 6.5) } else { (x0 + 6.5, x0) };
        Box::new(Path::new().move_to(a, 3.5).line_to(b, 8.0).line_to(a, 12.5).close().fill(color)) as Box<dyn Shape>
    };
    square(vec![tri(1.5), tri(8.0)])
}

/// bell (notifications)
pub(crate) fn bell(color: Color) -> Canvas {
    square(vec![
        stroked(
            Path::new()
                .move_to(3.0, 11.5)
                .line_to(4.0, 10.0)
                .line_to(4.0, 6.8)
                .arc(8.0, 6.8, 4.0, 270.0, 180.0)
                .line_to(12.0, 10.0)
                .line_to(13.0, 11.5)
                .close(),
            color,
        ),
        stroked(Arc::new().center(8.0, 12.6).radius(1.6).start(90.0).sweep(180.0), color),
    ])
}

/// the Bluetooth rune
pub(crate) fn bluetooth(color: Color) -> Canvas {
    square(vec![stroked(
        Path::new()
            .move_to(4.5, 4.8)
            .line_to(11.0, 10.8)
            .line_to(8.0, 13.8)
            .line_to(8.0, 2.2)
            .line_to(11.0, 5.2)
            .line_to(4.5, 11.2),
        color,
    )])
}

/// paintpalette (appearance)
pub(crate) fn palette(color: Color) -> Canvas {
    square(vec![
        stroked(
            Path::new()
                .arc(8.0, 8.0, 6.2, 150.0, 300.0)
                .quad_to(9.5, 10.5, 10.5, 12.0)
                .quad_to(11.0, 14.2, 8.0, 14.2),
            color,
        ),
        Box::new(Circle::new().center(5.2, 7.0).radius(1.1).fill(color)),
        Box::new(Circle::new().center(8.0, 4.8).radius(1.1).fill(color)),
        Box::new(Circle::new().center(10.9, 6.4).radius(1.1).fill(color)),
    ])
}

/// sidebar.left
pub(crate) fn sidebar(color: Color) -> Stack {
    Stack::new(vec![
        Box::new(Rectangle::new().width(14.0).height(11.0).translate(1.0, 2.5).radius(2.2).border(1.4, color))
            as Box<dyn Widget>,
        Box::new(Rectangle::new().width(1.4).height(11.0).translate(5.6, 2.5).fill(color)),
    ])
    .width(16.0)
    .height(16.0)
}

/// wifi at a fixed full strength, for menus (the bar uses `wifi`)
pub(crate) fn wifi_full(color: Color) -> Canvas {
    let (cx, cy) = (8.0, 13.0);
    let mut shapes: Vec<Box<dyn Shape>> =
        vec![Box::new(Circle::new().center(cx, cy - 0.5).radius(1.5).fill(color))];
    for r in [4.0_f32, 7.2, 10.4] {
        shapes.push(stroked(Arc::new().center(cx, cy).radius(r).start(-45.0).sweep(90.0), color));
    }
    square(shapes)
}

/// chevron.left.forwardslash.chevron.right (code / GitHub)
pub(crate) fn code(color: Color) -> Canvas {
    square(vec![
        stroked(Path::new().move_to(5.0, 4.0).line_to(1.5, 8.0).line_to(5.0, 12.0), color),
        stroked(Path::new().move_to(11.0, 4.0).line_to(14.5, 8.0).line_to(11.0, 12.0), color),
        stroked(Line::new().from(9.4, 2.8).to(6.6, 13.2), color),
    ])
}

/// play.fill
pub(crate) fn play(color: Color) -> Canvas {
    square(vec![Box::new(
        Path::new().move_to(4.5, 2.5).line_to(13.5, 8.0).line_to(4.5, 13.5).close().fill(color),
    )])
}

/// pause.fill
pub(crate) fn pause(color: Color) -> Stack {
    Stack::new(vec![
        Box::new(Rectangle::new().width(3.6).height(11.0).translate(3.6, 2.5).radius(1.2).fill(color))
            as Box<dyn Widget>,
        Box::new(Rectangle::new().width(3.6).height(11.0).translate(8.8, 2.5).radius(1.2).fill(color)),
    ])
    .width(16.0)
    .height(16.0)
}

/// music.note
pub(crate) fn music_note(color: Color) -> Canvas {
    square(vec![
        Box::new(Circle::new().center(5.0, 12.2).radius(2.4).fill(color)),
        Box::new(Circle::new().center(12.0, 10.6).radius(2.4).fill(color)),
        stroked(Line::new().from(7.2, 12.0).to(7.2, 3.6), color),
        stroked(Line::new().from(14.2, 10.4).to(14.2, 2.0), color),
        stroked(Line::new().from(7.2, 3.6).to(14.2, 2.0), color),
    ])
}
