// SPDX-License-Identifier: AGPL-3.0-or-later
//! Continuous-curvature ("squircle") corners, as `packages/shared/src/squircle.ts` draws them.
//!
//! Plain rounded corners join a straight edge to a circular arc, and the jump in curvature is
//! visible as a faint "kink". Here each corner eases into a short arc through two cubic Béziers.
//! The construction must stay the same as the TypeScript one, so the web and the desktop app draw
//! the same shapes.

use std::f64::consts::SQRT_2;
use std::fmt::Write;

/// A point, with y pointing down.
pub type Point = (f64, f64);

/// One step of an outline, in absolute coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    Move(Point),
    Line(Point),
    Cubic(Point, Point, Point),
}

struct Corner {
    r: f64,
    p: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    arc: f64,
}

fn corner(radius: f64, smoothing: f64, max_p: f64) -> Corner {
    let r = radius.min(max_p);
    let mut s = smoothing;
    let mut p = (1.0 + s) * r;
    if p > max_p {
        s = s.min(max_p / r - 1.0).max(0.0);
        p = p.min(max_p);
    }
    let arc_measure = 90.0 * (1.0 - s);
    let arc = (arc_measure / 2.0).to_radians().sin() * r * SQRT_2;
    let alpha = (90.0 - arc_measure) / 2.0;
    let p3p4 = r * (alpha / 2.0).to_radians().tan();
    let beta = 45.0 * s;
    let c = p3p4 * beta.to_radians().cos();
    let d = c * beta.to_radians().tan();
    let b = (p - arc - c - d) / 3.0;
    let a = 2.0 * b;
    Corner { r, p, a, b, c, d, arc }
}

/// Rotates a vector 90° clockwise `turns` times (the y axis points down).
fn rot((mut x, mut y): Point, turns: u8) -> Point {
    for _ in 0..turns {
        (x, y) = (-y, x);
    }
    (x, y)
}

fn add((x, y): Point, (dx, dy): Point) -> Point {
    (x + dx, y + dy)
}

/// What a corner is made of, relative to where it starts, for the top-right corner turned `turns`
/// times: a cubic, a circular arc (radius, end) and another cubic.
struct Steps {
    c1: Point,
    c2: Point,
    e1: Point,
    arc: Point,
    c3: Point,
    c4: Point,
    e2: Point,
}

fn steps(k: &Corner, turns: u8) -> Steps {
    Steps {
        c1: rot((k.a, 0.0), turns),
        c2: rot((k.a + k.b, 0.0), turns),
        e1: rot((k.a + k.b + k.c, k.d), turns),
        arc: rot((k.arc, k.arc), turns),
        c3: rot((k.d, k.c), turns),
        c4: rot((k.d, k.b + k.c), turns),
        e2: rot((k.d, k.a + k.b + k.c), turns),
    }
}

/// Where each corner starts and where the straight edge after it ends.
fn edges(w: f64, h: f64, p: f64) -> [(Point, Point); 4] {
    [((w - p, 0.0), (w, h - p)), ((w, h - p), (p, h)), ((p, h), (0.0, p)), ((0.0, p), (w - p, 0.0))]
}

/// The outline of a `w`×`h` box with smoothed corners, as lines and cubics. `smoothing` 0 is a
/// plain rounded rectangle; 0.6 is iOS-like. Each corner's arc becomes a single cubic, which is
/// within a thousandth of a pixel for arcs of 90° or less.
pub fn outline(w: f64, h: f64, radius: f64, smoothing: f64) -> Vec<Segment> {
    if w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }
    let k = corner(radius, smoothing, w.min(h) / 2.0);
    if k.r <= 0.0 {
        return vec![
            Segment::Move((0.0, 0.0)),
            Segment::Line((w, 0.0)),
            Segment::Line((w, h)),
            Segment::Line((0.0, h)),
            Segment::Line((0.0, 0.0)),
        ];
    }

    let mut out = vec![Segment::Move((w - k.p, 0.0))];
    for (turns, (start, next)) in edges(w, h, k.p).into_iter().enumerate() {
        let s = steps(&k, turns as u8);
        let e1 = add(start, s.e1);
        out.push(Segment::Cubic(add(start, s.c1), add(start, s.c2), e1));

        let end = add(e1, s.arc);
        out.push(arc(e1, end, k.r));

        out.push(Segment::Cubic(add(end, s.c3), add(end, s.c4), add(end, s.e2)));
        out.push(Segment::Line(next));
    }
    out
}

/// A clockwise arc of a circle of `r` from `from` to `to`, shorter than a half turn, as one cubic.
fn arc(from: Point, to: Point, r: f64) -> Segment {
    let (vx, vy) = (to.0 - from.0, to.1 - from.1);
    let half = (vx * vx + vy * vy).sqrt() / 2.0;
    if half == 0.0 {
        return Segment::Line(to);
    }
    let (ux, uy) = (vx / (half * 2.0), vy / (half * 2.0));
    let reach = (r * r - half * half).max(0.0).sqrt();
    let center = (from.0 + vx / 2.0 - uy * reach, from.1 + vy / 2.0 + ux * reach);
    let angle = 2.0 * (half / r).min(1.0).asin();
    let k = 4.0 / 3.0 * (angle / 4.0).tan();
    // Tangents point along the direction of travel, a quarter turn from each radius.
    let tangent = |(x, y): Point| (-(y - center.1) / r, (x - center.0) / r);
    let (t0, t1) = (tangent(from), tangent(to));
    Segment::Cubic((from.0 + t0.0 * k * r, from.1 + t0.1 * k * r), (to.0 - t1.0 * k * r, to.1 - t1.1 * k * r), to)
}

/// The outline as points, each cubic cut into straight pieces no longer than `step`. It's convex,
/// so it can be filled as a fan from its centre.
pub fn polygon(w: f64, h: f64, radius: f64, smoothing: f64, step: f64) -> Vec<Point> {
    let mut points = Vec::new();
    let mut at = (0.0, 0.0);
    for segment in outline(w, h, radius, smoothing) {
        match segment {
            Segment::Move(p) => {
                points.push(p);
                at = p;
            },
            Segment::Line(p) => {
                if p != at {
                    points.push(p);
                }
                at = p;
            },
            Segment::Cubic(c1, c2, p) => {
                let length = dist(at, c1) + dist(c1, c2) + dist(c2, p);
                let n = (length / step.max(0.01)).ceil().max(1.0) as usize;
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    let mix = |a: f64, b: f64, c: f64, d: f64| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    points.push((mix(at.0, c1.0, c2.0, p.0), mix(at.1, c1.1, c2.1, p.1)));
                }
                at = p;
            },
        }
    }
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    points
}

fn dist(a: Point, b: Point) -> f64 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Rounded to thousandths and printed the way JavaScript prints numbers.
fn f(n: f64) -> String {
    let n = (n * 1000.0 + 0.5).floor() / 1000.0;
    format!("{}", if n == 0.0 { 0.0 } else { n })
}

/// The SVG path, exactly as `squirclePath` makes it.
pub fn svg(w: f64, h: f64, radius: f64, smoothing: f64) -> String {
    if w <= 0.0 || h <= 0.0 {
        return String::new();
    }
    let k = corner(radius, smoothing, w.min(h) / 2.0);
    if k.r <= 0.0 {
        return format!("M0 0H{}V{}H0Z", f(w), f(h));
    }

    let mut out = format!("M{} 0", f(w - k.p));
    for (turns, (_, next)) in edges(w, h, k.p).into_iter().enumerate() {
        let s = steps(&k, turns as u8);
        let _ = write!(
            out,
            "c{} {} {} {} {} {}a{} {} 0 0 1 {} {}c{} {} {} {} {} {}",
            f(s.c1.0),
            f(s.c1.1),
            f(s.c2.0),
            f(s.c2.1),
            f(s.e1.0),
            f(s.e1.1),
            f(k.r),
            f(k.r),
            f(s.arc.0),
            f(s.arc.1),
            f(s.c3.0),
            f(s.c3.1),
            f(s.c4.0),
            f(s.c4.1),
            f(s.e2.0),
            f(s.e2.1),
        );
        if turns < 3 {
            let _ = write!(out, "L{} {}", f(next.0), f(next.1));
        }
    }
    out.push('Z');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degenerate_boxes() {
        assert_eq!(svg(0.0, 10.0, 4.0, 0.6), "");
        assert_eq!(svg(10.0, 10.0, 0.0, 0.6), "M0 0H10V10H0Z");
    }

    #[test]
    fn corners_never_overlap_however_big_the_radius() {
        let path = svg(40.0, 20.0, 100.0, 0.6);
        let start: f64 = path[1..].split(' ').next().unwrap().parse().unwrap();
        assert!(start >= 30.0, "{path}");
    }

    /// From `squirclePath` itself (bun), so the construction can't drift from the web's.
    #[test]
    fn matches_the_web() {
        for (args, path) in VECTORS {
            assert_eq!(svg(args.0, args.1, args.2, args.3), *path, "{args:?}");
        }
    }

    #[test]
    fn outline_follows_the_path() {
        for (args, _) in VECTORS {
            let (w, h, r, s) = *args;
            let points = polygon(w, h, r, s, 0.5);
            assert!(points.iter().all(|&(x, y)| (-1e-9..=w + 1e-9).contains(&x) && (-1e-9..=h + 1e-9).contains(&y)));
            // Convex, and clockwise: every turn goes the same way.
            for i in 0..points.len() {
                let (a, b, c) = (points[i], points[(i + 1) % points.len()], points[(i + 2) % points.len()]);
                let cross = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
                assert!(cross >= -1e-6, "{args:?} turns back at {b:?}");
            }
        }
    }

    #[test]
    fn arcs_stay_on_their_circle() {
        let k = corner(20.0, 0.6, 50.0);
        let from = (0.0, 0.0);
        let Segment::Cubic(_, _, to) = arc(from, (k.arc, k.arc), k.r) else { panic!("a cubic") };
        assert_eq!(to, (k.arc, k.arc));
    }

    const VECTORS: &[((f64, f64, f64, f64), &str)] = &include!("vectors.in");
}
