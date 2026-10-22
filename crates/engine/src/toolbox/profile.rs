//! Curves along a height and smooth blends: the body profile (a smooth curve through points), the
//! S-curve beads and spacings ease along, and the fade around a ring that opens the lid's holes.
//! From EasyCAD's `graph(…, smooth: true)`, `smoothstep` and `fade`.

/// A smooth curve through points (x, y), read at any x and level beyond its ends: a monotone cubic
/// (Fritsch and Carlson's) that never overshoots its points and leaves both ends level, so a body
/// profile leaves the bottom and meets the top straight up.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    xs: Vec<f64>,
    ys: Vec<f64>,
    /// The curve's slope at each point.
    slopes: Vec<f64>,
}

impl Graph {
    /// At least one point, in any order (sorted by x; points with the same x keep their order, a
    /// step). Points that aren't numbers are a message.
    pub fn smooth(points: &[[f64; 2]]) -> Result<Graph, String> {
        if points.is_empty() {
            return Err("The profile needs at least one point.".into());
        }
        if points.iter().flatten().any(|v| !v.is_finite()) {
            return Err("The profile has a point that is not a number.".into());
        }
        let mut pairs = points.to_vec();
        pairs.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let xs: Vec<f64> = pairs.iter().map(|p| p[0]).collect();
        let ys: Vec<f64> = pairs.iter().map(|p| p[1]).collect();
        let n = xs.len();
        let h: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
        let d: Vec<f64> = (0..n - 1)
            .map(|k| {
                if h[k] > 1e-12 {
                    (ys[k + 1] - ys[k]) / h[k]
                } else {
                    0.0
                }
            })
            .collect();
        // Each point's slope: the weighted harmonic mean of the slopes on either side (zero at a
        // peak, a dip or a flat stretch, so the curve never overshoots); the ends are level.
        let mut slopes = vec![0.0; n];
        for k in 1..n.saturating_sub(1) {
            let (a, b) = (d[k - 1], d[k]);
            if h[k - 1] > 1e-12 && h[k] > 1e-12 && a * b > 0.0 {
                let w1 = 2.0 * h[k] + h[k - 1];
                let w2 = h[k] + 2.0 * h[k - 1];
                slopes[k] = (w1 + w2) / (w1 / a + w2 / b);
            }
        }
        Ok(Graph { xs, ys, slopes })
    }

    pub fn at(&self, x: f64) -> f64 {
        let n = self.xs.len();
        let (Some(&first), Some(&last)) = (self.xs.first(), self.xs.last()) else {
            return f64::NAN;
        };
        if !x.is_finite() {
            return f64::NAN;
        }
        if x <= first {
            return self.ys[0];
        }
        if x >= last {
            return self.ys[n - 1];
        }
        // The last point at or below x, and the one after it.
        let (mut lo, mut hi) = (0, n - 1);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.xs[mid] <= x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let span = self.xs[hi] - self.xs[lo];
        if span < 1e-9 {
            return self.ys[hi];
        }
        // Cubic Hermite between the two points, with their slopes.
        let f = ((x - self.xs[lo]) / span).clamp(0.0, 1.0);
        let (f2, f3) = (f * f, f * f * f);
        (2.0 * f3 - 3.0 * f2 + 1.0) * self.ys[lo]
            + (f3 - 2.0 * f2 + f) * span * self.slopes[lo]
            + (-2.0 * f3 + 3.0 * f2) * self.ys[hi]
            + (f3 - f2) * span * self.slopes[hi]
    }

    /// The smallest and largest value the curve takes (at its points: it never overshoots them).
    pub fn range(&self) -> (f64, f64) {
        self.ys
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| {
                (lo.min(*y), hi.max(*y))
            })
    }
}

/// 0 below 0, 1 above 1, and an S-curve between that leaves and arrives level: `t²(3 − 2t)`.
pub fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How near a point is to a ring, 0..1, from its signed distance `d` to the ring (negative
/// inside): 1 on the ring and within half the `band` around it, easing (an S-curve) to 0 over
/// `inside` mm inward and `outside` mm outward. EasyGCode's spiral-disc attractor.
pub fn fade(d: f64, inside: f64, outside: f64, band: f64) -> f64 {
    let half = band.max(0.0) / 2.0;
    let d = if d < 0.0 {
        (d + half).min(0.0)
    } else {
        (d - half).max(0.0)
    };
    let t = if d < 0.0 {
        if inside > 0.0 { 1.0 + d / inside } else { 0.0 }
    } else if outside > 0.0 {
        1.0 - d / outside
    } else if d == 0.0 {
        1.0
    } else {
        0.0
    };
    smoothstep(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_smooth_profile_passes_its_points_level_at_both_ends() {
        let g = Graph::smooth(&[[1.0, 0.97], [0.0, 1.0], [0.45, 1.15]]).unwrap();
        assert_eq!(g.at(0.0), 1.0);
        assert_eq!(g.at(0.45), 1.15);
        assert_eq!(g.at(1.0), 0.97);
        assert_eq!(g.at(-1.0), 1.0);
        assert_eq!(g.at(2.0), 0.97);
        // Level at the ends: barely moves over the first and last thousandth.
        assert!((g.at(0.001) - 1.0).abs() < 1e-5);
        assert!((g.at(0.999) - 0.97).abs() < 1e-5);
        // Never above its highest point, a peak at 0.45.
        for i in 0..=1000 {
            assert!(g.at(i as f64 / 1000.0) <= 1.15 + 1e-12);
        }
        assert_eq!(g.range(), (0.97, 1.15));
    }

    #[test]
    fn one_point_is_a_constant_and_none_is_a_message() {
        let g = Graph::smooth(&[[0.3, 1.2]]).unwrap();
        assert_eq!(g.at(0.0), 1.2);
        assert_eq!(g.at(1.0), 1.2);
        assert!(Graph::smooth(&[]).is_err());
        assert!(Graph::smooth(&[[0.0, f64::NAN]]).is_err());
    }

    #[test]
    fn fades_around_a_ring() {
        assert_eq!(fade(0.0, 6.0, 6.0, 0.0), 1.0);
        assert_eq!(fade(-6.0, 6.0, 6.0, 0.0), 0.0);
        assert_eq!(fade(9.0, 6.0, 6.0, 0.0), 0.0);
        assert_eq!(fade(-3.0, 6.0, 6.0, 0.0), 0.5);
        // A band holds full strength over its width.
        assert_eq!(fade(1.9, 6.0, 6.0, 4.0), 1.0);
        assert_eq!(smoothstep(0.25), 0.15625);
    }
}
