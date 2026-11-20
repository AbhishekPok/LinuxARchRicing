//! Analytic, underdamped springs: frame-rate independent, with continuous
//! position and velocity when a transition is interrupted.
use std::time::{Duration, Instant};

// About 4% overshoot, then a gentle return. Shared by every island mode.
const FREQUENCY: f32 = 21.0;
const DAMPING: f32 = 0.72;
const SETTLE: Duration = Duration::from_millis(620);

#[derive(Clone)]
pub(crate) struct Spring {
    from: f32,
    velocity: f32,
    target: f32,
    started: Instant,
    time_scale: f32,
}

impl Spring {
    pub(crate) fn new(value: f32) -> Self {
        Self {
            from: value,
            velocity: 0.0,
            target: value,
            started: Instant::now(),
            time_scale: 1.0,
        }
    }

    pub(crate) fn slow(value: f32, time_scale: f32) -> Self {
        Self {
            time_scale,
            ..Self::new(value)
        }
    }

    pub(crate) fn to(&mut self, target: f32) {
        if self.target == target {
            return;
        }
        let now = Instant::now();
        let (position, velocity) = self.sample(now.duration_since(self.started));
        self.from = position;
        self.velocity = velocity;
        self.target = target;
        self.started = now;
    }

    pub(crate) fn value(&self) -> f32 {
        // Only layer geometry is pixel-snapped. Tiny pill width/height and
        // tint animations need an unclamped, subpixel spring sample.
        (self.precise_value().max(2.0) / 2.0).round() * 2.0
    }

    pub(crate) fn precise_value(&self) -> f32 {
        let elapsed = self.started.elapsed();
        if elapsed < SETTLE.mul_f32(self.time_scale)
            && (self.from != self.target || self.velocity != 0.0)
        {
            amane::request_frame();
        }
        self.sample(elapsed).0
    }

    fn sample(&self, elapsed: Duration) -> (f32, f32) {
        let elapsed = elapsed.div_f32(self.time_scale);
        if elapsed.is_zero() {
            return (self.from, self.velocity);
        }
        if elapsed >= SETTLE {
            return (self.target, 0.0);
        }
        let t = elapsed.as_secs_f32();
        let decay = DAMPING * FREQUENCY;
        let frequency = FREQUENCY * (1.0 - DAMPING * DAMPING).sqrt();
        let displacement = self.from - self.target;
        let sine_weight = (self.velocity + decay * displacement) / frequency;
        let (sin, cos) = (frequency * t).sin_cos();
        let wave = displacement * cos + sine_weight * sin;
        let envelope = (-decay * t).exp();
        let position = self.target + envelope * wave;
        let velocity =
            envelope * (frequency * (-displacement * sin + sine_weight * cos) - decay * wave);
        (position, velocity)
    }
}

/// Caelestia's Material 3 motion tokens (caelestia-dots/shell,
/// plugin/src/Caelestia/Config/tokens.hpp). Each curve is a chain of cubic
/// bezier segments from (0, 0): c1x, c1y, c2x, c2y, endX, endY. The spatial
/// ones overshoot (y > 1) for that soft "expressive" settle.
// The full token set is kept so new animations can pick from it.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Easing {
    Standard,
    Emphasized,
    FastSpatial,
    DefaultSpatial,
    SlowSpatial,
    FastEffects,
    DefaultEffects,
    SlowEffects,
}

impl Easing {
    fn points(self) -> &'static [f32] {
        match self {
            Easing::Standard => &[0.2, 0.0, 0.0, 1.0, 1.0, 1.0],
            Easing::Emphasized => &[
                0.05,
                0.0,
                2.0 / 15.0,
                0.06,
                1.0 / 6.0,
                0.4,
                5.0 / 24.0,
                0.82,
                0.25,
                1.0,
                1.0,
                1.0,
            ],
            Easing::FastSpatial => &[0.42, 1.67, 0.21, 0.9, 1.0, 1.0],
            Easing::DefaultSpatial => &[0.38, 1.21, 0.22, 1.0, 1.0, 1.0],
            Easing::SlowSpatial => &[0.39, 1.29, 0.35, 0.98, 1.0, 1.0],
            Easing::FastEffects => &[0.31, 0.94, 0.34, 1.0, 1.0, 1.0],
            Easing::DefaultEffects => &[0.34, 0.8, 0.34, 1.0, 1.0, 1.0],
            Easing::SlowEffects => &[0.34, 0.88, 0.34, 1.0, 1.0, 1.0],
        }
    }

    pub(crate) const fn duration(self) -> Duration {
        Duration::from_millis(match self {
            Easing::Standard | Easing::Emphasized => 400,
            Easing::FastSpatial => 350,
            Easing::DefaultSpatial => 500,
            Easing::SlowSpatial => 650,
            Easing::FastEffects => 150,
            Easing::DefaultEffects => 200,
            Easing::SlowEffects => 300,
        })
    }

    /// The curve's value at `progress`, clamped: for motion staged by hand
    /// rather than driven by a `Curve`, like the lock screen's entry.
    pub(crate) fn at(self, progress: f32) -> f32 {
        self.apply(progress.clamp(0.0, 1.0))
    }

    /// Progress `x` in 0..=1 to eased value, like QEasingCurve::BezierSpline.
    fn apply(self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let mut start = (0.0, 0.0);
        for seg in self.points().chunks_exact(6) {
            let (c1, c2, end) = ((seg[0], seg[1]), (seg[2], seg[3]), (seg[4], seg[5]));
            if x <= end.0 {
                let at = |t: f32, a: f32, b: f32, c: f32, d: f32| {
                    let u = 1.0 - t;
                    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                };
                // x(t) is monotonic within a segment, so bisection is exact enough.
                let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
                for _ in 0..24 {
                    let mid = (lo + hi) / 2.0;
                    if at(mid, start.0, c1.0, c2.0, end.0) < x {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                return at((lo + hi) / 2.0, start.1, c1.1, c2.1, end.1);
            }
            start = end;
        }
        1.0
    }
}

/// A fixed-duration tween on one of Caelestia's curves: the QML
/// `Behavior { Anim {} }` equivalent. Retargeting mid-flight restarts from
/// the current value, exactly as a NumberAnimation does.
pub(crate) struct Curve {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
    easing: Easing,
}

impl Curve {
    pub(crate) fn new(value: f32, easing: Easing) -> Self {
        Self {
            from: value,
            target: value,
            started: Instant::now(),
            duration: easing.duration(),
            easing,
        }
    }

    pub(crate) fn to(&mut self, target: f32) {
        self.to_over(target, self.easing.duration());
    }

    /// Same curve, different length: the workspace indicator's trailing edge.
    pub(crate) fn to_over(&mut self, target: f32, duration: Duration) {
        if self.target == target {
            return;
        }
        self.from = self.value();
        self.target = target;
        self.duration = duration;
        self.started = Instant::now();
    }


    pub(crate) fn settled(&self) -> bool {
        self.started.elapsed() >= self.duration
    }

    pub(crate) fn value(&self) -> f32 {
        let elapsed = self.started.elapsed();
        if elapsed >= self.duration || self.from == self.target {
            return self.target;
        }
        amane::request_frame();
        let x = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        self.from + (self.target - self.from) * self.easing.apply(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_start_and_end_exactly() {
        for e in [
            Easing::Standard,
            Easing::Emphasized,
            Easing::FastSpatial,
            Easing::DefaultSpatial,
            Easing::SlowSpatial,
            Easing::DefaultEffects,
        ] {
            assert_eq!(e.apply(0.0), 0.0);
            assert_eq!(e.apply(1.0), 1.0);
            assert!((e.apply(0.999) - 1.0).abs() < 0.02, "{e:?}");
        }
    }

    #[test]
    fn spatial_curves_overshoot_effects_do_not() {
        let peak = |e: Easing| (1..100).map(|i| e.apply(i as f32 / 100.0)).fold(0.0, f32::max);
        assert!(peak(Easing::DefaultSpatial) > 1.0);
        assert!(peak(Easing::FastSpatial) > 1.0);
        assert!(peak(Easing::DefaultEffects) <= 1.0);
    }

    fn transition(from: f32, target: f32) -> Spring {
        Spring {
            target,
            ..Spring::new(from)
        }
    }

    #[test]
    fn slow_hide_spring_preserves_shape_but_extends_duration() {
        let normal = transition(0.0, 1.0);
        let slow = Spring {
            time_scale: 2.2,
            ..transition(0.0, 1.0)
        };
        let normal_pos = normal.sample(Duration::from_millis(200)).0;
        let slow_pos = slow.sample(Duration::from_millis(440)).0;
        assert!((normal_pos - slow_pos).abs() < 0.0001);
        assert_ne!(slow.sample(Duration::from_millis(620)).0, 1.0);
        assert_eq!(slow.sample(Duration::from_millis(1365)).0, 1.0);
    }
    #[test]
    fn normalized_spring_is_not_clamped_or_pixel_snapped() {
        assert_eq!(Spring::new(0.0).precise_value(), 0.0);
        assert_eq!(Spring::new(1.0).precise_value(), 1.0);
        assert_eq!(Spring::new(0.25).precise_value(), 0.25);
    }

    #[test]
    fn no_residual_motion_after_short_settle() {
        let spring = transition(104.0, 672.0);
        for ms in [620, 700, 800, 1600] {
            assert_eq!(spring.sample(Duration::from_millis(ms)), (672.0, 0.0));
        }
    }

    #[test]
    fn starts_at_rest_and_finishes_exactly() {
        let spring = transition(96.0, 400.0);
        assert_eq!(spring.sample(Duration::ZERO), (96.0, 0.0));
        assert_eq!(spring.sample(SETTLE), (400.0, 0.0));
    }

    #[test]
    fn expansion_and_shrink_both_overshoot() {
        let expansion = transition(96.0, 400.0);
        let shrink = transition(400.0, 96.0);
        let bounce = Duration::from_millis(225);
        let expanded = expansion.sample(bounce).0;
        let shrunk = shrink.sample(bounce).0;
        assert!(expanded > 400.0 && expanded < 416.0);
        assert!(shrunk < 96.0 && shrunk > 80.0);
    }

    #[test]
    fn retarget_preserves_position_and_velocity() {
        let mut spring = transition(96.0, 400.0);
        spring.started = Instant::now() - Duration::from_millis(80);
        spring.to(96.0);
        let initial = spring.sample(Duration::ZERO);
        assert!((initial.0 - spring.from).abs() < 0.001);
        assert!((initial.1 - spring.velocity).abs() < 0.001);
        assert!(initial.0 > 96.0 && initial.0 < 400.0);
        assert!(initial.1 > 0.0);
        assert_eq!(spring.sample(SETTLE), (96.0, 0.0));
    }
}
