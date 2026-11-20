//! Stable full-output island surface. Only explicit island/popup areas take input.
use super::*;
pub(super) const THICKNESS: f32 = 4.0;
/// The menu bar replaced the always-on screen frame; it is kept only as the
/// "waiting for a capture pick" signal.
pub(super) fn frame_if_tinted(width: f32, height: f32) -> amane::Stack {
    if crate::auxiliary::tint().is_none() {
        return amane::Stack::new(vec![]).width(0.0).height(0.0);
    }
    frame(width, height)
}
pub(super) fn frame(width: f32, height: f32) -> amane::Stack {
    let path = std::path::Path::new(file!())
        .parent()
        .unwrap()
        .join("frame.wgsl");
    // While a region/window pick is pending the frame takes the capture colour
    // (and thickens a little) so it is obvious the screen is waiting for input.
    let mut values = crate::theme::shader_values(vec![[THICKNESS, settings::radius(), 0.0, 0.0]]);
    if let Some((color, amount)) = crate::auxiliary::tint() {
        values[0][0] = THICKNESS + 3.0 * amount;
        let mix = |from: f32, to: u8| from + (f32::from(to) / 255.0 - from) * amount;
        values[1][0] = mix(values[1][0], color.red());
        values[1][1] = mix(values[1][1], color.green());
        values[1][2] = mix(values[1][2], color.blue());
    }
    amane::Stack::new(children![
        Rectangle::new()
            .width(width)
            .height(height)
            .shader(path)
            .shader_values(values)
    ])
    .width(width)
    .height(height)
}

/// Rounded screen corners like a Mac display: four black quarter-circle
/// cut-outs of `radius` (Appearance > Screen curvature). Plain vector shapes,
/// so they cost nothing per frame and never take the pointer.
pub(super) fn corners(width: f32, height: f32, radius: f32) -> amane::Stack {
    use amane::{Canvas, Path, Shape};
    if radius < 1.0 {
        return amane::Stack::new(vec![]).width(0.0).height(0.0);
    }
    let r = radius;
    // the shape of the top-left corner; the others are the same piece
    // flipped into place
    let piece = |flip_x: bool, flip_y: bool| {
        let fx = |x: f32| if flip_x { r - x } else { x };
        let fy = |y: f32| if flip_y { r - y } else { y };
        // a quarter circle approximated with one cubic (k = 0.5523)
        let k = 0.5523 * r;
        let path = Path::new()
            .move_to(fx(0.0), fy(0.0))
            .line_to(fx(r), fy(0.0))
            .cubic_to(fx(r - k), fy(0.0), fx(0.0), fy(r - k), fx(0.0), fy(r))
            .close();
        Canvas::new().width(r).height(r).shapes(vec![Box::new(path.fill(Color::BLACK))])
    };
    let place = |canvas: Canvas, x: f32, y: f32| -> Box<dyn amane::Widget> {
        Box::new(Rectangle::new().width(r).height(r).translate(x, y).child(canvas))
    };
    amane::Stack::new(vec![
        place(piece(false, false), 0.0, 0.0),
        place(piece(true, false), width - r, 0.0),
        place(piece(false, true), 0.0, height - r),
        place(piece(true, true), width - r, height - r),
    ])
    .width(width)
    .height(height)
}
