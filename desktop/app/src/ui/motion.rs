// SPDX-License-Identifier: AGPL-3.0-or-later

//! Timing: the web's easing curves, and how far along something is.

use std::time::{Duration, Instant};

/// A CSS `cubic-bezier()` timing function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Curve(pub f32, pub f32, pub f32, pub f32);

pub const EASE_IN: Curve = Curve(0.42, 0.0, 1.0, 1.0);
pub const EASE_OUT: Curve = Curve(0.0, 0.0, 0.58, 1.0);
pub const EASE_IN_OUT: Curve = Curve(0.42, 0.0, 0.58, 1.0);
/// `cubic-bezier(0.16, 1, 0.3, 1)`: a fast start that settles slowly, for things arriving.
pub const SETTLE: Curve = Curve(0.16, 1.0, 0.3, 1.0);
/// `cubic-bezier(0.2, 0.8, 0.2, 1)`: things that move into place.
pub const GLIDE: Curve = Curve(0.2, 0.8, 0.2, 1.0);
/// `cubic-bezier(0.3, 1.35, 0.5, 1)`: overshoots, for a segmented thumb's leading edge.
pub const SPRING: Curve = Curve(0.3, 1.35, 0.5, 1.0);
/// `cubic-bezier(0.65, 0, 0.25, 1)`: a segmented thumb's trailing edge catching up.
pub const CATCH_UP: Curve = Curve(0.65, 0.0, 0.25, 1.0);

impl Curve {
    /// The eased value at `t` (0 to 1).
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t == 0.0 || t == 1.0 {
            return t;
        }
        let Curve(x1, y1, x2, y2) = self;
        let bezier = |a: f32, b: f32, s: f32| {
            let u = 1.0 - s;
            3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
        };
        // The curve's parameter for x = t, by bisection (x is monotonic for valid curves).
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if bezier(x1, x2, mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        bezier(y1, y2, (lo + hi) / 2.0)
    }
}

/// How far (0 to 1) something that started at `start` and lasts `duration` is at `now`.
pub fn progress(start: Instant, duration: Duration, now: Instant) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    (now.saturating_duration_since(start).as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves() {
        assert_eq!(EASE_OUT.at(0.0), 0.0);
        assert_eq!(EASE_OUT.at(1.0), 1.0);
        assert!((Curve(0.0, 0.0, 1.0, 1.0).at(0.3) - 0.3).abs() < 1e-3);
        assert!(SETTLE.at(0.2) > 0.6);
        assert!(EASE_IN.at(0.2) < 0.1);
    }
}
