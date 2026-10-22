//! Outlines: EasyGCode's shape library (`geometry.js`), ported point for point.
//!
//! Every shape is a dense, closed, counter-clockwise point list centred on the origin, then
//! simplified to a chord tolerance (Ramer–Douglas–Peucker), so flat stretches get few points and
//! tight curves many. [`seam`] turns the loop to start where an axis crosses it, and [`Sampler`]
//! walks it by arc length. The same functions build every vase, disc and brim outline.

use crate::geom::{P2, dist2};
use crate::num::{cos, hypot2, pow, round, sin};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Which outline, with its own dimensions (all kept, so switching shape loses nothing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shape {
    pub kind: ShapeKind,
    pub circle: Circle,
    pub rounded_rect: RoundedRect,
    pub ellipse: Ellipse,
    pub polygon: Polygon,
    pub star: Star,
    pub squircle: Squircle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ShapeKind {
    #[default]
    Circle,
    RoundedRect,
    Ellipse,
    Polygon,
    Star,
    Squircle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Circle {
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoundedRect {
    /// X extent.
    pub width: f64,
    /// Y extent.
    pub length: f64,
    /// Corner radius.
    pub fillet: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Ellipse {
    pub rx: f64,
    pub ry: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Polygon {
    pub radius: f64,
    pub sides: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Star {
    pub outer: f64,
    pub inner: f64,
    pub points: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Squircle {
    pub size: f64,
    /// The superellipse exponent: 2 is a circle, higher is squarer.
    pub n: f64,
}

// EasyGCode's defaults (index.html).
impl Default for Circle {
    fn default() -> Self {
        Circle { radius: 30.0 }
    }
}
impl Default for RoundedRect {
    fn default() -> Self {
        RoundedRect {
            width: 60.0,
            length: 40.0,
            fillet: 8.0,
        }
    }
}
impl Default for Ellipse {
    fn default() -> Self {
        Ellipse { rx: 40.0, ry: 25.0 }
    }
}
impl Default for Polygon {
    fn default() -> Self {
        Polygon {
            radius: 30.0,
            sides: 6.0,
        }
    }
}
impl Default for Star {
    fn default() -> Self {
        Star {
            outer: 35.0,
            inner: 18.0,
            points: 5.0,
        }
    }
}
impl Default for Squircle {
    fn default() -> Self {
        Squircle { size: 30.0, n: 4.0 }
    }
}
impl Default for Shape {
    fn default() -> Self {
        Shape {
            kind: ShapeKind::Circle,
            circle: Circle::default(),
            rounded_rect: RoundedRect::default(),
            ellipse: Ellipse::default(),
            polygon: Polygon::default(),
            star: Star::default(),
            squircle: Squircle::default(),
        }
    }
}

/// The schema fields for a [`Shape`] stored at `prefix` (e.g. "shape").
pub fn fields(prefix: &str) -> Vec<crate::schema::Field> {
    use crate::schema::{choice, integer, number};
    let key = |k: &str| format!("{prefix}.{k}");
    let kind = key("kind");
    let kind = kind.as_str();
    vec![
        choice(
            kind,
            "Type",
            &[
                ("circle", "Circle"),
                ("roundedRect", "Rounded rectangle"),
                ("ellipse", "Ellipse"),
                ("polygon", "Polygon"),
                ("star", "Star"),
                ("squircle", "Squircle"),
            ],
        ),
        number(key("circle.radius"), "Radius", "mm", 1.0, 500.0).when(kind, &["circle"]),
        number(key("roundedRect.width"), "Width (X)", "mm", 1.0, 1000.0)
            .when(kind, &["roundedRect"]),
        number(key("roundedRect.length"), "Length (Y)", "mm", 1.0, 1000.0)
            .when(kind, &["roundedRect"]),
        number(key("roundedRect.fillet"), "Corner radius", "mm", 0.0, 500.0)
            .when(kind, &["roundedRect"]),
        number(key("ellipse.rx"), "Radius X", "mm", 1.0, 500.0).when(kind, &["ellipse"]),
        number(key("ellipse.ry"), "Radius Y", "mm", 1.0, 500.0).when(kind, &["ellipse"]),
        number(key("polygon.radius"), "Radius", "mm", 1.0, 500.0).when(kind, &["polygon"]),
        integer(key("polygon.sides"), "Sides", 3.0, 100.0).when(kind, &["polygon"]),
        number(key("star.outer"), "Outer radius", "mm", 1.0, 500.0).when(kind, &["star"]),
        number(key("star.inner"), "Inner radius", "mm", 0.5, 500.0).when(kind, &["star"]),
        integer(key("star.points"), "Points", 2.0, 100.0).when(kind, &["star"]),
        number(key("squircle.size"), "Size", "mm", 1.0, 500.0).when(kind, &["squircle"]),
        number(key("squircle.n"), "Squareness (2: circle)", "", 0.2, 20.0)
            .when(kind, &["squircle"]),
    ]
}

fn signed_area(pts: &[P2]) -> f64 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

/// Counter-clockwise (positive signed area).
fn ensure_ccw(mut pts: Vec<P2>) -> Vec<P2> {
    if signed_area(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

fn circle(r: f64, steps: usize) -> Vec<P2> {
    ellipse(r, r, steps)
}

fn ellipse(rx: f64, ry: f64, steps: usize) -> Vec<P2> {
    (0..steps)
        .map(|i| {
            let a = (2.0 * PI * i as f64) / steps as f64;
            [rx * cos(a), ry * sin(a)]
        })
        .collect()
}

fn rounded_rect(width: f64, length: f64, fillet: f64) -> Vec<P2> {
    let (hw, hl) = (width / 2.0, length / 2.0);
    let rf = fillet.min(hw.min(hl)).max(0.0);
    // Bottom-right, top-right, top-left, bottom-left corner arcs.
    let corners = [
        (hw - rf, -hl + rf, -PI / 2.0, 0.0),
        (hw - rf, hl - rf, 0.0, PI / 2.0),
        (-hw + rf, hl - rf, PI / 2.0, PI),
        (-hw + rf, -hl + rf, PI, 1.5 * PI),
    ];
    const ARC_STEPS: usize = 24;
    let mut pts = Vec::with_capacity(4 * (ARC_STEPS + 1));
    for (x, y, a0, a1) in corners {
        for s in 0..=ARC_STEPS {
            let a = a0 + ((a1 - a0) * s as f64) / ARC_STEPS as f64;
            pts.push([x + rf * cos(a), y + rf * sin(a)]);
        }
    }
    pts
}

fn polygon(r: f64, sides: f64) -> Vec<P2> {
    let n = round(sides).max(3) as usize;
    (0..n)
        .map(|i| {
            let a = (2.0 * PI * i as f64) / n as f64 + PI / 2.0; // point-up
            [r * cos(a), r * sin(a)]
        })
        .collect()
}

fn star(outer: f64, inner: f64, points: f64) -> Vec<P2> {
    let p = round(points).max(2) as usize;
    (0..p * 2)
        .map(|i| {
            let r = if i % 2 == 0 { outer } else { inner };
            let a = (PI * i as f64) / p as f64 + PI / 2.0;
            [r * cos(a), r * sin(a)]
        })
        .collect()
}

fn squircle(size: f64, n: f64, steps: usize) -> Vec<P2> {
    let exp = n.max(0.2);
    let sgn = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
    (0..steps)
        .map(|i| {
            let t = (2.0 * PI * i as f64) / steps as f64;
            let (c, s) = (cos(t), sin(t));
            [
                size * sgn(c) * pow(c.abs(), 2.0 / exp),
                size * sgn(s) * pow(s.abs(), 2.0 / exp),
            ]
        })
        .collect()
}

/// The dense outline of a shape (EasyGCode's `makeShape`).
pub fn dense(shape: &Shape) -> Vec<P2> {
    let pts = match shape.kind {
        ShapeKind::Circle => circle(shape.circle.radius, 720),
        ShapeKind::Ellipse => ellipse(shape.ellipse.rx, shape.ellipse.ry, 720),
        ShapeKind::RoundedRect => {
            let r = &shape.rounded_rect;
            rounded_rect(r.width, r.length, r.fillet)
        }
        ShapeKind::Polygon => polygon(shape.polygon.radius, shape.polygon.sides),
        ShapeKind::Star => star(shape.star.outer, shape.star.inner, shape.star.points),
        ShapeKind::Squircle => squircle(shape.squircle.size, shape.squircle.n, 720),
    };
    ensure_ccw(pts)
}

/// Distance from `p` to the segment `a`–`b`.
fn seg_dist(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-12 {
        return hypot2(p[0] - a[0], p[1] - a[1]);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
    hypot2(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy))
}

/// Ramer–Douglas–Peucker on an open polyline (keeps both ends). Iterative, with the same choices
/// as EasyGCode's recursive version (the first farthest point splits each stretch).
fn rdp(points: &[P2], eps: f64) -> Vec<P2> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((lo, hi)) = stack.pop() {
        if hi < lo + 2 {
            continue;
        }
        let (a, b) = (points[lo], points[hi]);
        let mut idx = lo;
        let mut maxd = 0.0;
        for (i, p) in points.iter().enumerate().take(hi).skip(lo + 1) {
            let d = seg_dist(*p, a, b);
            if d > maxd {
                maxd = d;
                idx = i;
            }
        }
        if maxd > eps {
            keep[idx] = true;
            stack.push((lo, idx));
            stack.push((idx, hi));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// Simplifies a closed outline to within `eps` (chord tolerance), still closed and CCW, with no
/// repeated closing point (EasyGCode's `rdpClosed`).
pub fn simplify(closed: &[P2], eps: f64) -> Vec<P2> {
    if closed.len() < 4 {
        return closed.to_vec();
    }
    let start = closed[0];
    let mut idx = 0;
    let mut maxd = -1.0;
    for (i, p) in closed.iter().enumerate().skip(1) {
        let d = dist2(*p, start);
        if d > maxd {
            maxd = d;
            idx = i;
        }
    }
    let arc1 = &closed[..=idx];
    let mut arc2 = closed[idx..].to_vec();
    arc2.push(start);
    let r1 = rdp(arc1, eps);
    let r2 = rdp(&arc2, eps);
    let mut out = r1[..r1.len() - 1].to_vec();
    out.extend_from_slice(&r2[..r2.len() - 1]);
    out
}

/// The shape simplified to a chord tolerance in mm (EasyGCode's `adaptiveShape`).
pub fn outline(shape: &Shape, tolerance: f64) -> Vec<P2> {
    simplify(&dense(shape), tolerance.max(1e-4))
}

/// Where the seam goes: where an axis crosses the outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SeamSide {
    /// +Y
    #[default]
    Back,
    /// −Y
    Front,
    /// +X
    Right,
    /// −X
    Left,
}

/// Turns a closed outline so it starts exactly where the chosen axis crosses it: the crossing
/// point is inserted, so the seam lands mid-edge and stays put whatever the shape's points
/// (EasyGCode's `rotateToSeam`). The result repeats no point, but starts with the new one.
pub fn seam(base: &[P2], side: SeamSide) -> Vec<P2> {
    let n = base.len();
    if n == 0 {
        return Vec::new();
    }
    let horiz = matches!(side, SeamSide::Right | SeamSide::Left); // cross the X axis (y = 0)
    let want_pos = matches!(side, SeamSide::Back | SeamSide::Right);
    let mut best: Option<(P2, usize)> = None;
    let mut best_side = if want_pos {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };
    for i in 0..n {
        let (a, b) = (base[i], base[(i + 1) % n]);
        let (ca, cb) = if horiz { (a[1], b[1]) } else { (a[0], b[0]) };
        if (ca <= 0.0 && cb > 0.0) || (ca >= 0.0 && cb < 0.0) {
            let t = ca / (ca - cb);
            let pt = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let side_val = if horiz { pt[0] } else { pt[1] };
            let on_wanted = if want_pos {
                side_val > 0.0
            } else {
                side_val < 0.0
            };
            if on_wanted
                && (if want_pos {
                    side_val > best_side
                } else {
                    side_val < best_side
                })
            {
                best_side = side_val;
                best = Some((pt, i));
            }
        }
    }
    match best {
        Some((pt, i)) => {
            let mut out = Vec::with_capacity(n + 1);
            out.push(pt);
            out.extend((1..=n).map(|k| base[(i + k) % n]));
            out
        }
        None => {
            // Fallback: the most extreme point in the wanted direction.
            let mut best = 0;
            let mut best_main = if want_pos {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
            for (i, p) in base.iter().enumerate() {
                let main = if horiz { p[0] } else { p[1] };
                if if want_pos {
                    main > best_main
                } else {
                    main < best_main
                } {
                    best_main = main;
                    best = i;
                }
            }
            let mut out = base[best..].to_vec();
            out.extend_from_slice(&base[..best]);
            out
        }
    }
}

/// Walks a closed outline by arc length (EasyGCode's `makeSampler`).
pub struct Sampler<'a> {
    base: &'a [P2],
    cum: Vec<f64>,
    pub perimeter: f64,
}

impl<'a> Sampler<'a> {
    pub fn new(base: &'a [P2]) -> Sampler<'a> {
        let n = base.len();
        let mut cum = Vec::with_capacity(n + 1);
        cum.push(0.0);
        for i in 0..n {
            let d = dist2(base[i], base[(i + 1) % n]);
            cum.push(cum[i] + d);
        }
        let perimeter = cum.last().copied().unwrap_or(0.0);
        Sampler {
            base,
            cum,
            perimeter,
        }
    }

    /// The fraction of the way round at which point `i` sits.
    pub fn u_of(&self, i: usize) -> f64 {
        self.cum.get(i).copied().unwrap_or(0.0) / self.perimeter
    }

    /// The point at fraction `u` of the way round (wrapping), and the unit tangent there.
    pub fn at(&self, u: f64) -> (P2, P2) {
        let n = self.base.len();
        if n == 0 {
            return ([0.0, 0.0], [1.0, 0.0]);
        }
        let uu = u - u.floor();
        let target = uu * self.perimeter;
        let mut seg = 0;
        while seg < n - 1 && self.cum[seg + 1] <= target {
            seg += 1;
        }
        let (a, b) = (self.base[seg], self.base[(seg + 1) % n]);
        let seg_len = match self.cum[seg + 1] - self.cum[seg] {
            l if l != 0.0 => l,
            _ => 1e-9,
        };
        let t = (target - self.cum[seg]) / seg_len;
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = match hypot2(dx, dy) {
            l if l != 0.0 => l,
            _ => 1e-9,
        };
        ([a[0] + dx * t, a[1] + dy * t], [dx / l, dy / l])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(kind: ShapeKind) -> Shape {
        Shape {
            kind,
            ..Shape::default()
        }
    }

    #[test]
    fn every_shape_is_closed_ccw_and_simplified() {
        for kind in [
            ShapeKind::Circle,
            ShapeKind::RoundedRect,
            ShapeKind::Ellipse,
            ShapeKind::Polygon,
            ShapeKind::Star,
            ShapeKind::Squircle,
        ] {
            let pts = outline(&shape(kind), 0.05);
            assert!(pts.len() >= 3, "{kind:?}");
            assert!(signed_area(&pts) > 0.0, "{kind:?}");
            assert!(pts.len() <= dense(&shape(kind)).len(), "{kind:?}");
        }
        // A polygon keeps exactly its corners.
        assert_eq!(outline(&shape(ShapeKind::Polygon), 0.05).len(), 6);
    }

    #[test]
    fn simplification_stays_within_the_tolerance() {
        let s = shape(ShapeKind::Circle);
        let pts = outline(&s, 0.05);
        // Every dense point is within the tolerance of the simplified outline.
        for p in dense(&s) {
            let n = pts.len();
            let d = (0..n)
                .map(|i| seg_dist(p, pts[i], pts[(i + 1) % n]))
                .fold(f64::INFINITY, f64::min);
            assert!(d <= 0.05 + 1e-12, "{d}");
        }
    }

    #[test]
    fn the_seam_is_where_the_axis_crosses() {
        let pts = outline(&shape(ShapeKind::RoundedRect), 0.05);
        let back = seam(&pts, SeamSide::Back);
        assert_eq!(back.len(), pts.len() + 1);
        // On the axis, and on the outline (the simplified one: within the chord tolerance).
        assert!(
            back[0][0].abs() < 1e-12 && (back[0][1] - 20.0).abs() < 0.05,
            "{:?}",
            back[0]
        );
        let front = seam(&pts, SeamSide::Front);
        assert!(front[0][0].abs() < 1e-12 && (front[0][1] + 20.0).abs() < 0.05);
        let right = seam(&pts, SeamSide::Right);
        assert!((right[0][0] - 30.0).abs() < 0.05 && right[0][1].abs() < 1e-12);
    }

    #[test]
    fn the_sampler_walks_by_arc_length() {
        let pts = seam(
            &outline(&shape(ShapeKind::RoundedRect), 0.05),
            SeamSide::Back,
        );
        let s = Sampler::new(&pts);
        let (p0, t0) = s.at(0.0);
        assert_eq!(p0, pts[0]);
        assert!(t0[0] < -0.99); // CCW from +Y: heading −X
        let (p1, _) = s.at(1.0);
        assert_eq!(p1, p0);
        let (half, _) = s.at(0.5);
        assert!(
            (half[1] + 20.0).abs() < 0.05 && half[0].abs() < 0.5,
            "{half:?}"
        );
    }

    #[test]
    fn degenerate_input_does_not_panic() {
        assert!(seam(&[], SeamSide::Back).is_empty());
        let s = Sampler::new(&[]);
        assert_eq!(s.at(0.3).0, [0.0, 0.0]);
        let tiny = Shape {
            kind: ShapeKind::Circle,
            circle: Circle { radius: 0.0 },
            ..Shape::default()
        };
        let pts = outline(&tiny, 0.05);
        let _ = seam(&pts, SeamSide::Back);
    }
}
