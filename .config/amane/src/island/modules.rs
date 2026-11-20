//! The two lists below are the layout configuration. Reorder/remove entries to
//! choose mode contents; anchors and shared-element motion follow automatically.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Module {
    Controls,
    Time,
    Date,
}
use Module::*;
// Collapsed (the notch) is the clock alone.
pub(super) const COLLAPSED: &[Module] = &[Time];
// The Control Center row holds controls only. Workspaces, ops, system and
// battery live in the menu bar, the time in the notch, and settings are the
// panel itself, so none of them repeats here. The date is the calendar's
// handle (nothing else shows the date). Media lives in the Control Center's
// Now Playing card, which shows only while something plays.
pub(super) const EXPANDED: &[Module] = &[Controls, Date];
impl Module {
    pub(super) const fn width(self) -> f32 {
        match self {
            Controls => CONTROL_WIDTH * 2.0 + CONTROL_GAP,
            Time => 44.0,
            Date => 52.0,
        }
    }
}
const fn gap(previous: Module, next: Module, expanded: bool) -> f32 {
    if matches!(previous, Time) && matches!(next, Date) {
        0.0
    } else if expanded {
        ROW_GAP
    } else {
        12.0
    }
}
pub(super) const fn total(list: &[Module], expanded: bool) -> f32 {
    let mut width = ROW_PADDING * 2.0;
    let mut i = 0;
    while i < list.len() {
        if i > 0 {
            width += gap(list[i - 1], list[i], expanded);
        }
        width += list[i].width();
        i += 1;
    }
    width
}
pub(super) fn position(module: Module, expanded: bool) -> Option<f32> {
    let list = if expanded { EXPANDED } else { COLLAPSED };
    // a row narrower than the panel's minimum width sits centred in it
    let mut x = ROW_PADDING
        + if expanded { (PANEL.0 - total(EXPANDED, true)) / 2.0 } else { 0.0 };
    for (i, m) in list.iter().enumerate() {
        if i > 0 {
            x += gap(list[i - 1], *m, expanded);
        }
        if *m == module {
            return Some(x);
        }
        x += m.width();
    }
    None
}
fn widget(module: Module, clock: &Clock, monitor: &str, mine: bool) -> Rectangle {
    let here = monitor.to_string();
    match module {
        Controls => {
            // copied out: control_icon reads Audio itself
            let muted = Audio::read().muted();
            row_cell(
                module.width(),
                Center,
                Row::new(children![
                    control_icon(Osd::Volume, muted, false),
                    control_icon(Osd::Brightness, muted, false)
                ])
                .gap(CONTROL_GAP)
                .align(Center),
            )
        }
        Time => row_cell(module.width(), Center, clock_text(clock))
        .on_click(move |button| clicked(button, &here))
        .on_hover(move |inside| {
            let expanded = { Island::read().expanded };
            if mine && expanded {
                popups::hover(popups::Popup::Calendar, inside);
            }
        }),
        Date => {
            let mut parts = clock.date.split_whitespace();
            let weekday: String = parts.next().unwrap_or("").chars().take(3).collect();
            let day = parts.next().unwrap_or("").trim_end_matches(',');
            row_cell(
                module.width(),
                Center,
                Text::new(format!("{weekday} {day}"))
                    .size(14.0)
                    .weight(Weight::SemiBold)
                    .color(crate::theme::palette().foreground),
            )
            .on_hover(move |inside| {
                let expanded = { Island::read().expanded };
                if mine && expanded {
                    popups::hover(popups::Popup::Calendar, inside);
                }
            })
        }
    }
}
/// The minute rolls over like caelestia's window title: the old text lifts
/// and fades while the new one rises into place.
fn clock_text(clock: &Clock) -> amane::Stack {
    let t = clock.swap.value().clamp(0.0, 1.0);
    let line = |text: &str, opacity: f32, dy: f32| {
        Box::new(
            Rectangle::new()
                .width(Time.width())
                .height(36.0)
                .align_child(Center, Center)
                .translate(0.0, dy)
                .opacity(opacity)
                .child(
                    Text::new(text)
                        .size(14.0)
                        .weight(Weight::SemiBold)
                        .color(crate::theme::palette().foreground),
                ),
        ) as Box<dyn amane::Widget>
    };
    amane::Stack::new(vec![
        line(&clock.previous, 1.0 - t, -6.0 * t),
        line(&clock.time, t, 6.0 * (1.0 - t)),
    ])
    .width(Time.width())
    .height(36.0)
}

pub(super) fn view(
    expansion: f32,
    monitor: &str,
    mine: bool,
    clock: &Clock,
    width: f32,
    height: f32,
) -> amane::Stack {
    let p = expansion;
    let mut list = EXPANDED.to_vec();
    for m in COLLAPSED {
        if !list.contains(m) {
            list.push(*m);
        }
    }
    let mut widgets: Vec<Box<dyn amane::Widget>> = vec![];
    for (i, module) in list.into_iter().enumerate() {
        let from = position(module, false);
        let to = position(module, true);
        let source = from.unwrap_or_else(|| position(Time, false).unwrap_or(ROW_PADDING));
        let target = to.unwrap_or_else(|| position(Time, true).unwrap_or(ROW_PADDING));
        let center_from = source + module.width() / 2.0 - IDLE.0 / 2.0;
        let center_to = target + module.width() / 2.0 - PANEL.0 / 2.0;
        let x = width / 2.0 + center_from + (center_to - center_from) * p - module.width() / 2.0;
        let fade = match (from, to) {
            (Some(_), Some(_)) => 1.0,
            (None, Some(_)) => ((p - i as f32 * 0.025) / 0.8).clamp(0.0, 1.0),
            (Some(_), None) => (1.0 - p).clamp(0.0, 1.0),
            _ => 0.0,
        };
        // Always retain module slots/descendant target indices across frames.
        // A zero transform removes invisible modules from hit testing without
        // changing target identity in Amane's index-based pointer dispatcher.
        let visible = fade > 0.01;
        widgets.push(Box::new(
            widget(module, clock, monitor, mine)
                .translate(
                    x,
                    (height - 36.0) / 2.0
                        + if from.is_none() {
                            10.0 * (1.0 - fade)
                        } else {
                            0.0
                        },
                )
                .opacity(fade)
                .scale(if visible { 1.0 } else { 0.0 }),
        ));
    }
    amane::Stack::new(widgets).width(width).height(height)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn module_registry_budgets_both_modes_and_centres_the_row() {
        assert_eq!(total(COLLAPSED, false), IDLE.0);
        // the Control Center is never narrower than its row
        assert!(PANEL.0 >= total(EXPANDED, true));
        // a row narrower than the panel sits centred in it
        let inset = (PANEL.0 - total(EXPANDED, true)) / 2.0;
        assert_eq!(position(Controls, true).unwrap(), ROW_PADDING + inset);
        assert_eq!(
            position(Controls, true).unwrap() + Controls.width() + ROW_GAP,
            position(Date, true).unwrap()
        );
        // nothing that the bar or the notch already shows is in the row
        assert_eq!(EXPANDED, &[Controls, Date]);
    }
}
