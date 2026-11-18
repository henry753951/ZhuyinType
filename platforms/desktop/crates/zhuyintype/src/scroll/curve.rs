//! Chromium's scroll animation curve, ported so a wheel notch, a key and a scroll-to move the way they do in Chrome and
//! Edge on the same computer.  Pure: positions and times in, positions out.
//!
//! Source: Chromium `cc/animation/scroll_offset_animation_curve.cc` and `scroll_offset_animation_curve_factory.cc`
//! (main, 2026-11), `cc/base/features.cc` for the programmatic curve.  The constants keep Chromium's names and units
//! (durations in 60 Hz frames).  Everything Chromium does is kept except `delayed_by` (the time between the input event
//! and the frame that starts the animation), which GPUI does not report; it is zero here.
//!
//! | Input | Chromium `ScrollType` | Duration | Easing |
//! |---|---|---|---|
//! | Wheel notch | `kMouseWheel` → `kInverseDelta` | 12 frames (200 ms) up to 120 px, falling linearly to 6 frames (100 ms) at 480 px | `cubic-bezier(0.42, 0, 0.58, 1)` |
//! | Page Up/Down, Home/End | `kKeyboard` → `kConstant` | 9 frames (150 ms) | `cubic-bezier(0.42, 0, 0.58, 1)` |
//! | Scroll-to (a link, keyboard focus) | `kProgrammatic` → `kDeltaBased` | √distance frames, at most 1.5 s | `cubic-bezier(0.4, 0, 0, 1)` |
//!
//! A new notch while one is running moves the target (`UpdateTarget`): the curve restarts from where the content is,
//! with its first control point scaled so the velocity carries over, and lasts the shorter of the new segment's
//! duration and 2.5× the time the remaining distance takes at the current velocity.

/// Chromium's `kConstantDuration` (frames).
const CONSTANT_DURATION: f64 = 9.0;
/// Chromium's `kDurationDivisor`: frames per second.
const DURATION_DIVISOR: f64 = 60.0;
/// Chromium's `kInverseDelta*`: wheel durations fall from 12 to 6 frames between 120 and 480 px.
const INVERSE_DELTA_RAMP_START_PX: f64 = 120.0;
const INVERSE_DELTA_RAMP_END_PX: f64 = 480.0;
const INVERSE_DELTA_MIN_DURATION: f64 = 6.0;
const INVERSE_DELTA_MAX_DURATION: f64 = 12.0;
const INVERSE_DELTA_SLOPE: f64 = (INVERSE_DELTA_MIN_DURATION - INVERSE_DELTA_MAX_DURATION)
    / (INVERSE_DELTA_RAMP_END_PX - INVERSE_DELTA_RAMP_START_PX);
const INVERSE_DELTA_OFFSET: f64 = INVERSE_DELTA_MAX_DURATION - INVERSE_DELTA_RAMP_START_PX * INVERSE_DELTA_SLOPE;
/// `kProgrammaticScrollAnimationOverride`'s `max_animation_duration` off Android: 1500 ms.
const MAX_PROGRAMMATIC_SECONDS: f64 = 1.5;
/// Chromium's `kEpsilon`.
const EPSILON: f64 = 0.01;

/// What started a scroll animation; decides its duration and easing (Chromium's `ScrollType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A mouse-wheel notch.
    Wheel,
    /// Page Up/Down, Home/End.
    Keyboard,
    /// A scroll the app asks for: a link to a row, keyboard focus moving off screen.
    Programmatic,
}

impl Kind {
    /// The control points (`kEaseInOutControlPoints`; the programmatic override's defaults off Android).
    fn control_points(self) -> Bezier {
        match self {
            Kind::Wheel | Kind::Keyboard => Bezier::new(0.42, 0.0, 0.58, 1.0),
            Kind::Programmatic => Bezier::new(0.4, 0.0, 0.0, 1.0),
        }
    }

    /// `EaseInOutSegmentDuration` in seconds, for a segment of `delta` pixels.
    fn segment_duration(self, delta: f64) -> f64 {
        let frames = match self {
            Kind::Keyboard => CONSTANT_DURATION,
            Kind::Wheel => (INVERSE_DELTA_OFFSET + delta.abs() * INVERSE_DELTA_SLOPE)
                .clamp(INVERSE_DELTA_MIN_DURATION, INVERSE_DELTA_MAX_DURATION),
            Kind::Programmatic => delta.abs().sqrt().min(MAX_PROGRAMMATIC_SECONDS * DURATION_DIVISOR),
        };
        frames / DURATION_DIVISOR
    }
}

/// One scroll animation along one axis, in scroll positions (pixels from the top, growing downwards) and seconds
/// since it started.
///
/// Invariant: `last_retarget <= total` once a segment is set, and `value` reaches `target` at `total`.
#[derive(Clone, Debug)]
pub struct Curve {
    kind: Kind,
    easing: Bezier,
    initial: f64,
    target: f64,
    /// `total_animation_duration_`: when the animation ends, from its start.
    total: f64,
    /// `last_retarget_`: when the current segment started.
    last_retarget: f64,
}

impl Curve {
    /// An animation from `from` to `to` (`CreateEaseInOutAnimation` + `SetInitialValue`).
    pub fn new(kind: Kind, from: f64, to: f64) -> Self {
        Self {
            kind,
            easing: kind.control_points(),
            initial: from,
            target: to,
            total: kind.segment_duration(to - from),
            last_retarget: 0.0,
        }
    }

    pub fn target(&self) -> f64 {
        self.target
    }

    /// When the animation ends, in seconds from its start.
    #[cfg(test)]
    pub fn duration(&self) -> f64 {
        self.total
    }

    /// Whether the content has arrived by `t`.
    pub fn finished(&self, t: f64) -> bool {
        t >= self.total
    }

    /// The position at `t` (`GetValue`).
    pub fn value(&self, t: f64) -> f64 {
        let duration = self.total - self.last_retarget;
        let t = t - self.last_retarget;
        if duration <= 0.0 || t >= duration {
            return self.target;
        }
        if t <= 0.0 {
            return self.initial;
        }
        let progress = self.easing.solve(t / duration);
        self.initial + (self.target - self.initial) * progress
    }

    /// Pixels per second at `t` (`CalculateVelocity`).
    fn velocity(&self, t: f64) -> f64 {
        let duration = self.total - self.last_retarget;
        if duration <= 0.0 {
            return 0.0;
        }
        let slope = self.easing.slope((t - self.last_retarget) / duration);
        slope * ((self.target - self.initial) / duration)
    }

    /// Moves the target to `new_target` at `t`, keeping the current velocity (`UpdateTarget`).
    pub fn update_target(&mut self, t: f64, new_target: f64) {
        let t = t.max(self.last_retarget);
        if (self.target - new_target).abs() < EPSILON {
            // The same target: a new segment would only be longer.
            self.target = new_target;
            return;
        }
        let current = self.value(t);
        let new_delta = new_target - current;
        if new_delta.abs() < EPSILON {
            self.last_retarget = t;
            self.total = t;
            self.target = new_target;
            return;
        }
        if self.total - self.last_retarget <= 0.0 {
            // The last segment took no time.
            self.total = self.kind.segment_duration(new_delta);
            self.target = new_target;
            return;
        }
        let velocity = self.velocity(t);
        let new_duration = self.kind.segment_duration(new_delta).min(velocity_bound(velocity, new_delta));
        if new_duration < EPSILON {
            self.target = new_target;
            self.total = t;
            return;
        }
        let slope = velocity * (new_duration / new_delta);
        self.easing = self.kind.control_points().with_initial_slope(slope);
        self.initial = current;
        self.target = new_target;
        self.total = t + new_duration;
        self.last_retarget = t;
    }
}

/// `VelocityBasedDurationBound`: how long `new_delta` takes at `velocity`, with Chromium's 2.5× allowance for the
/// ease-out; unbounded when not moving or moving the other way.
fn velocity_bound(velocity: f64, new_delta: f64) -> f64 {
    if new_delta.abs() < EPSILON {
        return 0.0;
    }
    if velocity.abs() < EPSILON {
        return f64::INFINITY;
    }
    let bound = new_delta / velocity * 2.5;
    if bound < 0.0 { f64::INFINITY } else { bound }
}

/// A CSS `cubic-bezier` timing function with the slope Chromium's retargeting needs (`gfx::CubicBezier`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bezier {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

/// `gfx::CubicBezier`'s `kBezierEpsilon`.
const BEZIER_EPSILON: f64 = 1e-7;

impl Bezier {
    const fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// `EaseInOutWithInitialSlope`: the first control point raised so the curve starts at `slope`.
    fn with_initial_slope(self, slope: f64) -> Self {
        let slope = slope.clamp(-1000.0, 1000.0);
        Self { y1: self.x1 * slope, ..self }
    }

    fn coefficients(a: f64, b: f64) -> (f64, f64, f64) {
        let c = 3.0 * a;
        let b = 3.0 * (b - a) - c;
        (1.0 - c - b, b, c)
    }

    fn sample(a: f64, b: f64, s: f64) -> f64 {
        let (ca, cb, cc) = Self::coefficients(a, b);
        ((ca * s + cb) * s + cc) * s
    }

    fn derivative(a: f64, b: f64, s: f64) -> f64 {
        let (ca, cb, cc) = Self::coefficients(a, b);
        (3.0 * ca * s + 2.0 * cb) * s + cc
    }

    /// The curve parameter where x is `x` (`SolveCurveX`: Newton's method, then bisection).
    fn parameter(&self, x: f64) -> f64 {
        let mut s = x;
        for _ in 0..8 {
            let error = Self::sample(self.x1, self.x2, s) - x;
            if error.abs() < BEZIER_EPSILON {
                return s;
            }
            let slope = Self::derivative(self.x1, self.x2, s);
            if slope.abs() < BEZIER_EPSILON {
                break;
            }
            s -= error / slope;
        }
        let (mut low, mut high) = (0.0, 1.0);
        s = x.clamp(0.0, 1.0);
        for _ in 0..64 {
            let sampled = Self::sample(self.x1, self.x2, s);
            if (sampled - x).abs() < BEZIER_EPSILON {
                break;
            }
            if x > sampled {
                low = s;
            } else {
                high = s;
            }
            s = (low + high) / 2.0;
        }
        s
    }

    /// Progress (y) at time fraction `x` in 0…1.
    fn solve(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        Self::sample(self.y1, self.y2, self.parameter(x))
    }

    /// dy/dx at time fraction `x` (`gfx::CubicBezier::Slope`).
    fn slope(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        let s = self.parameter(x);
        let dx = Self::derivative(self.x1, self.x2, s);
        let dy = Self::derivative(self.y1, self.y2, s);
        if dx.abs() < BEZIER_EPSILON { 0.0 } else { dy / dx }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn wheel_durations_follow_chromium_inverse_delta() {
        // 12 frames up to 120 px, 6 frames from 480 px, linear between.
        assert!(close(Kind::Wheel.segment_duration(100.0), 0.2, 1e-9));
        assert!(close(Kind::Wheel.segment_duration(-120.0), 0.2, 1e-9));
        assert!(close(Kind::Wheel.segment_duration(300.0), 9.0 / 60.0, 1e-9));
        assert!(close(Kind::Wheel.segment_duration(480.0), 0.1, 1e-9));
        assert!(close(Kind::Wheel.segment_duration(2000.0), 0.1, 1e-9));
    }

    #[test]
    fn keyboard_and_programmatic_durations() {
        assert!(close(Kind::Keyboard.segment_duration(700.0), 0.15, 1e-9));
        assert!(close(Kind::Programmatic.segment_duration(400.0), 20.0 / 60.0, 1e-9));
        assert!(close(Kind::Programmatic.segment_duration(1e7), 1.5, 1e-9));
    }

    #[test]
    fn bezier_matches_css_ease_in_out() {
        let ease = Kind::Wheel.control_points();
        assert!(close(ease.solve(0.0), 0.0, 1e-9));
        assert!(close(ease.solve(1.0), 1.0, 1e-9));
        // Symmetric control points: halfway in time is halfway in distance.
        assert!(close(ease.solve(0.5), 0.5, 1e-6));
        // cubic-bezier(0.42, 0, 0.58, 1) solved independently by bisection.
        assert!(close(ease.solve(0.25), 0.129_161_9, 1e-4));
        assert!(close(ease.solve(0.75), 0.870_838_1, 1e-4));
        assert!(close(ease.slope(0.0), 0.0, 1e-6));
    }

    #[test]
    fn a_notch_starts_and_ends_at_rest() {
        let curve = Curve::new(Kind::Wheel, 0.0, 100.0);
        assert!(close(curve.duration(), 0.2, 1e-9));
        assert!(close(curve.value(0.0), 0.0, 1e-9));
        assert!(close(curve.value(0.1), 50.0, 1e-3));
        assert!(close(curve.value(0.2), 100.0, 1e-9));
        assert!(curve.finished(0.2) && !curve.finished(0.19));
        let mut last = 0.0;
        for step in 0..=20 {
            let value = curve.value(f64::from(step) / 100.0);
            assert!(value >= last - 1e-9, "monotonic");
            last = value;
        }
    }

    #[test]
    fn a_second_notch_keeps_moving_without_stopping() {
        let mut curve = Curve::new(Kind::Wheel, 0.0, 100.0);
        let before = curve.value(0.1);
        let speed_before = curve.velocity(0.1);
        curve.update_target(0.1, 200.0);
        assert!(close(curve.value(0.1), before, 1e-6), "no jump at the retarget");
        assert!(close(curve.velocity(0.1), speed_before, speed_before * 0.01), "velocity carries over");
        assert!(close(curve.target(), 200.0, 1e-9));
        let end = curve.duration();
        assert!(end > 0.1 && end <= 0.1 + 0.2 + 1e-9);
        assert!(close(curve.value(end), 200.0, 1e-9));
    }

    #[test]
    fn retargeting_to_the_same_place_changes_nothing() {
        let mut curve = Curve::new(Kind::Wheel, 0.0, 100.0);
        let duration = curve.duration();
        curve.update_target(0.05, 100.0);
        assert!(close(curve.duration(), duration, 1e-12));
    }

    #[test]
    fn reversing_shortens_nothing_into_a_jump() {
        let mut curve = Curve::new(Kind::Wheel, 0.0, 300.0);
        let at = curve.value(0.05);
        curve.update_target(0.05, 0.0);
        assert!(close(curve.value(0.05), at, 1e-6));
        let end = curve.duration();
        assert!(close(curve.value(end), 0.0, 1e-9));
    }

    #[test]
    fn programmatic_curve_decelerates() {
        let curve = Curve::new(Kind::Programmatic, 0.0, 400.0);
        // cubic-bezier(0.4, 0, 0, 1) is past halfway well before half the time.
        assert!(curve.value(curve.duration() / 2.0) > 300.0);
        assert!(close(curve.value(curve.duration()), 400.0, 1e-9));
    }
}
