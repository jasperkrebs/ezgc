//! Flat spirals: the bottoms and lids that spirals start on, printed flat on the bed before the
//! wall climbs off their outer edge, with no travel in between.
//!
//! - [`ring_spiral`]: EasyGCode's ring fill, rings one line apart inside an outline, linked into one
//!   spiral from the centre out that ends on the outline's seam, where the wall starts.
//! - [`hole_spiral`] + [`interlock`] + [`stretch`]: EasyGCode's spiral disc. Turns one line apart
//!   that spread around a ring and wiggle so each outward bump meets the next turn's inward bump:
//!   the turns stay joined there and leave holes in between (the salt shaker's lid).
//! - [`stack`]: several flat layers of one spiral, every other one run back inward, as one line.
//!
//! Ported from EasyCAD (`rings.rs`, `spiral.rs`), which follows EasyGCode step by step.

use crate::geom::{P2, P3};
use crate::num::{cos, hypot2, sin};
use crate::toolbox::shapes::Sampler;
use crate::toolpath::{Event, MAX_MOVES, Toolpath};
use std::f64::consts::TAU;

/// The most rings or turns one flat spiral may have.
const MAX_TURNS: usize = 2000;
/// Integration steps per turn of a spiral whose spacing varies (as EasyGCode).
const SUB: usize = 720;

/// The ring fill of an outline (centred on the origin, starting at its seam) scaled by `scale`:
/// copies of it scaled toward the centre one `spacing` apart, from the outline inward (exact for
/// circles), linked into one spiral that opens with a turn out of the middle, blends each ring
/// into the next and ends on the outline's own seam point. Every ring gets the same number of
/// points, half a millimetre apart on the outline (48 at least), starting on the seam.
pub fn ring_spiral(
    base: &Sampler,
    r_max: f64,
    scale: f64,
    spacing: f64,
) -> Result<Vec<P2>, String> {
    let r_out = r_max * scale;
    if !(r_out > 1e-6 && r_out.is_finite() && spacing > 0.0) {
        return Err("The flat bottom has no size: check the shape and the fillet.".into());
    }
    // Ring k sits k spacings in from the outline, while it's at least half a spacing out.
    let mut factors = vec![1.0];
    let mut k = 1;
    while (k as f64) * spacing <= r_out - spacing / 2.0 && k < MAX_TURNS {
        factors.push((r_out - k as f64 * spacing) / r_out);
        k += 1;
    }
    factors.reverse(); // from the centre out
    let n = 48usize.max((base.perimeter * scale / 0.5).ceil() as usize);
    if n.saturating_mul(factors.len() + 1) > MAX_MOVES {
        return Err("The flat bottom needs too many points: use a wider line.".into());
    }
    let ring: Vec<P2> = (0..n)
        .map(|j| {
            let (p, _) = base.at(j as f64 / n as f64);
            [p[0] * scale, p[1] * scale]
        })
        .collect();
    let at = |f: f64, j: usize| {
        let p = ring[j % n];
        [p[0] * f, p[1] * f]
    };
    let mut out: Vec<P2> = Vec::with_capacity(n * (factors.len() + 1) + 1);
    // An opening turn out of the middle onto the innermost ring.
    let f0 = factors[0];
    for j in 0..=n {
        let p = at(f0, j);
        let t = j as f64 / n as f64;
        out.push([p[0] * t, p[1] * t]);
    }
    for w in factors.windows(2) {
        for j in 1..=n {
            let t = j as f64 / n as f64;
            let (a, b) = (at(w[0], j), at(w[1], j));
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out.dedup_by(|x, y| hypot2(x[0] - y[0], x[1] - y[1]) < 1e-9);
    Ok(out)
}

/// A flat spiral around the origin, from the centre out, counter-clockwise from +X.
#[derive(Debug, Clone, PartialEq)]
pub struct Winding {
    pub points: Vec<P2>,
    /// How many turns from the centre each point is (0 at the centre).
    pub turn: Vec<f64>,
    /// Points per turn (they're spaced evenly in angle).
    pub per_turn: usize,
}

/// A flat spiral whose turns sit one local `spacing(x, y)` apart (dr/dθ = spacing / 2π,
/// integrated with RK4, 720 steps a turn), ending on the whole turn whose end is nearest `outer`.
/// Points evenly in angle: enough per turn to stay within `tolerance` of the true curve, 32 at
/// least, and at least `min_samples`.
pub fn hole_spiral(
    outer: f64,
    spacing: &dyn Fn(f64, f64) -> f64,
    tolerance: f64,
    min_samples: usize,
) -> Result<Winding, String> {
    if !(outer > 0.0 && outer.is_finite()) {
        return Err("The lid's spiral has no size: check the lid's fillet.".into());
    }
    let h = TAU / SUB as f64;
    let drdt = |th: f64, r: f64| -> Result<f64, String> {
        let s = spacing(r * cos(th), r * sin(th));
        if !(s.is_finite() && s > 0.0) {
            return Err(format!(
                "The lid's spiral spacing must be more than 0, but it is {s} at r = {r:.2} mm."
            ));
        }
        Ok(s / TAU)
    };
    // Integrate r(θ), turn by turn, until a turn ends past `outer`.
    let mut rs: Vec<f64> = vec![0.0];
    let mut turn_r: Vec<f64> = vec![0.0];
    let mut n = 0;
    loop {
        if n >= MAX_TURNS {
            return Err(format!(
                "The lid's spiral would need more than {MAX_TURNS} turns: use a wider line."
            ));
        }
        for i in 0..SUB {
            let r = rs.last().copied().unwrap_or(0.0);
            let th = h * (n * SUB + i) as f64;
            let k1 = drdt(th, r)?;
            let k2 = drdt(th + h / 2.0, r + (h / 2.0) * k1)?;
            let k3 = drdt(th + h / 2.0, r + (h / 2.0) * k2)?;
            let k4 = drdt(th + h, r + h * k3)?;
            rs.push(r + (h / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4));
        }
        n += 1;
        let end = rs.last().copied().unwrap_or(0.0);
        turn_r.push(end);
        if end >= outer {
            break;
        }
    }
    // Snap: the whole number of turns ending closest to `outer`.
    let at = |k: usize| turn_r.get(k).copied().unwrap_or(0.0);
    let mut turns = n;
    if n > 1 && (at(n - 1) - outer).abs() <= (at(n) - outer).abs() {
        turns = n - 1;
    }
    let turns = turns.max(1);
    let th_end = TAU * turns as f64;
    let last = turns * SUB;
    let r_at = |th: f64| -> f64 {
        let x = (th / h).clamp(0.0, last as f64);
        let i = (x.floor() as usize).min(last - 1);
        let (a, b) = (
            rs.get(i).copied().unwrap_or(0.0),
            rs.get(i + 1).copied().unwrap_or(0.0),
        );
        a + (b - a) * (x - i as f64)
    };
    let r_end = at(turns);
    let mut dth = 2.0 * libm::acos((1.0 - tolerance / r_end.max(spacing(0.0, 0.0))).max(-1.0));
    if !dth.is_finite() || dth <= 0.0 {
        dth = 0.2;
    }
    let per_turn = 32.max((TAU / dth).ceil() as usize).max(min_samples);
    let total = turns.saturating_mul(per_turn);
    if total >= MAX_MOVES {
        return Err("The lid's spiral needs too many points: use a larger chord tolerance.".into());
    }
    let mut points = Vec::with_capacity(total + 1);
    let mut turn = Vec::with_capacity(total + 1);
    for i in 0..=total {
        let th = (i as f64 / total as f64) * th_end;
        let r = r_at(th);
        points.push([r * cos(th), r * sin(th)]);
        turn.push((i as f64 / total as f64) * turns as f64);
    }
    Ok(Winding {
        points,
        turn,
        per_turn,
    })
}

/// What [`interlock`] did, for the hints.
#[derive(Debug, Clone, PartialEq)]
pub struct Interlock {
    /// The widest wiggle, mm.
    pub max_amplitude: f64,
}

/// Wiggles a spiral's turns in and out, `A · sin(B·θ/2)`, so that where turns are spread more
/// than `width` apart, each turn's outward peak meets the next turn's inward dip with exactly one
/// line width between their centrelines. `bumps` (B, each outward and each inward bump counted) is
/// odd so a peak always faces a dip; an even count is rounded up.
///
/// At each angle the turns form a chain where `A_k + A_(k+1)` must equal the extra gap `g_k`;
/// solved in the least-squares sense (a tridiagonal solve per angle, with a hair of damping),
/// turns whose neighbouring gaps are all closed held at zero. As EasyGCode's spiral disc.
pub fn interlock(w: &Winding, bumps: usize, width: f64) -> (Vec<P2>, Interlock) {
    let spt = w.per_turn.max(1);
    let total = w.points.len().saturating_sub(1);
    let b = if bumps % 2 == 1 { bumps } else { bumps + 1 };
    let radii: Vec<f64> = w.points.iter().map(|p| hypot2(p[0], p[1])).collect();
    let r = |i: usize| radii.get(i).copied().unwrap_or(0.0);
    let mut amps = vec![0.0; total + 1];
    for j in 0..spt {
        let idx: Vec<usize> = (j..=total).step_by(spt).collect();
        let k_count = idx.len();
        if k_count < 2 {
            continue;
        }
        let g: Vec<f64> = (0..k_count - 1)
            .map(|k| (r(idx[k + 1]) - r(idx[k]) - width).max(0.0))
            .collect();
        let gap = |k: usize| g.get(k).copied().unwrap_or(0.0);
        let eps = width * 1e-4;
        let free: Vec<usize> = (0..k_count)
            .filter(|&k| (k > 0 && gap(k - 1) > eps) || (k < k_count - 1 && gap(k) > eps))
            .collect();
        if free.is_empty() {
            continue;
        }
        let mut is_free = vec![false; k_count];
        for &k in &free {
            is_free[k] = true;
        }
        let m = free.len();
        let (mut dg, mut off, mut rhs) = (vec![0.0; m], vec![0.0; m], vec![0.0; m]);
        for (n2, &k) in free.iter().enumerate() {
            dg[n2] = 1e-6;
            if k > 0 {
                dg[n2] += 1.0;
                rhs[n2] += gap(k - 1);
            }
            if k < k_count - 1 {
                dg[n2] += 1.0;
                rhs[n2] += gap(k);
                if is_free.get(k + 1).copied().unwrap_or(false) {
                    off[n2] = 1.0;
                }
            }
        }
        // Thomas algorithm on the symmetric tridiagonal (dg, off).
        let (mut cp, mut dp) = (vec![0.0; m], vec![0.0; m]);
        cp[0] = off[0] / dg[0];
        dp[0] = rhs[0] / dg[0];
        for n2 in 1..m {
            let den = dg[n2] - off[n2 - 1] * cp[n2 - 1];
            cp[n2] = off[n2] / den;
            dp[n2] = (rhs[n2] - off[n2 - 1] * dp[n2 - 1]) / den;
        }
        let mut sol = vec![0.0; m];
        sol[m - 1] = dp[m - 1];
        for n2 in (0..m - 1).rev() {
            sol[n2] = dp[n2] - cp[n2] * sol[n2 + 1];
        }
        for (n2, &k) in free.iter().enumerate() {
            if let Some(a) = amps.get_mut(idx[k]) {
                *a = sol[n2].max(0.0);
            }
        }
    }
    let mut max_amplitude: f64 = 0.0;
    let points = w
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let a = amps.get(i).copied().unwrap_or(0.0);
            max_amplitude = max_amplitude.max(a);
            let th = w.turn.get(i).copied().unwrap_or(0.0) * TAU;
            let ri = r(i);
            let rr = (ri + a * sin((b as f64 * th) / 2.0)).max(0.0);
            // Turns that don't wiggle keep their exact points.
            if a > 0.0 && ri > 0.0 {
                [p[0] / ri * rr, p[1] / ri * rr]
            } else {
                *p
            }
        })
        .collect();
    (points, Interlock { max_amplitude })
}

/// The spiral stretched, every radius alike, to end exactly `outer` from its centre, so it runs
/// straight into a wall that starts there. Done after the wiggles are solved: stretching first
/// opens a sliver between touching turns and the wiggles then alternate big and small.
pub fn stretch(points: &mut [P2], outer: f64) {
    let end = points.last().map_or(0.0, |p| hypot2(p[0], p[1]));
    if end > 0.0 && outer > 0.0 {
        let k = outer / end;
        for p in points.iter_mut() {
            *p = [p[0] * k, p[1] * k];
        }
    }
}

/// One flat layer's bead.
pub struct Flat {
    /// mm² (times the extrusion factor).
    pub area: f64,
    /// mm/min.
    pub feed: f64,
    /// Layer height, mm: layer k prints at (k + 1) × this.
    pub layer: f64,
}

/// `layers` flat layers of a spiral that runs from the centre out, each one layer higher than the
/// one before; layers 0, 2, 4… run outward and 1, 3… back inward, joined into one line (each layer
/// climbs onto its height over its first move). An odd count ends at the outer end, where the wall
/// starts. The fan comes on as the second layer starts. Starts with a travel to the first point.
pub fn stack(path: &mut Toolpath, spiral: &[P2], layers: usize, bead: &Flat) -> Result<(), String> {
    if spiral.len() < 2 {
        return Err("The flat spiral has nothing to print: it's narrower than the line.".into());
    }
    if spiral.len().saturating_mul(layers) > MAX_MOVES {
        return Err(
            "The flat layers need too many points: use a wider line or fewer layers.".into(),
        );
    }
    for k in 0..layers {
        let z = bead.layer * (k + 1) as f64;
        let to = |p: &P2| -> P3 { [p[0], p[1], z] };
        if k == 0 {
            path.comment(format!("--- flat spiral: {layers} layers ---"));
            path.travel(to(&spiral[0]));
        } else if k == 1 {
            path.event(Event::FanOn);
        }
        // Each layer starts where the last one ended (or the travel went): skip that point, so a
        // later layer's first move climbs onto its height as it moves on.
        let points: Box<dyn Iterator<Item = &P2>> = if k % 2 == 0 {
            Box::new(spiral.iter())
        } else {
            Box::new(spiral.iter().rev())
        };
        for p in points.skip(1) {
            path.extrude(to(p), bead.area, 1.0, bead.feed);
        }
    }
    if layers == 1 {
        path.event(Event::FanOn);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbox::profile::fade;
    use crate::toolbox::shapes::{SeamSide, Shape, outline, seam};

    fn circle() -> Vec<P2> {
        seam(&outline(&Shape::default(), 0.05), SeamSide::Back)
    }

    #[test]
    fn a_ring_fill_spirals_out_to_the_seam_one_line_apart() {
        let base = circle();
        let sampler = Sampler::new(&base);
        let r_max = base.iter().map(|p| hypot2(p[0], p[1])).fold(0.0, f64::max);
        let s = ring_spiral(&sampler, r_max, 0.5, 2.0).unwrap();
        // From the centre out to the seam (+Y) of the outline at half size (radius 15).
        assert_eq!(s[0], [0.0, 0.0]);
        let end = *s.last().unwrap();
        assert!(
            end[0].abs() < 1e-9 && (end[1] - 15.0).abs() < 1e-9,
            "{end:?}"
        );
        // Rings at 15, 13, …, 1 mm (k · 2 ≤ 15 − 1): 8 rings, an opening turn and 7 blends.
        let n = 48usize.max((sampler.perimeter * 0.5 / 0.5).ceil() as usize);
        assert_eq!(s.len(), 8 * n + 1);
        // Never more than a line width further out per turn.
        for w in s.windows(2) {
            assert!(hypot2(w[1][0] - w[0][0], w[1][1] - w[0][1]) < 1.0);
        }
    }

    #[test]
    fn the_hole_spiral_spreads_around_its_ring_and_ends_on_its_edge() {
        let ring = 12.0;
        let spacing = |x: f64, y: f64| 2.0 * (1.0 + 1.5 * fade(hypot2(x, y) - ring, 6.0, 6.0, 0.0));
        let w = hole_spiral(30.0, &spacing, 0.05, 16 * 9).unwrap();
        assert!(w.per_turn >= 144);
        assert_eq!((w.points.len() - 1) % w.per_turn, 0);
        let (mut points, info) = interlock(&w, 9, 2.0);
        // Turns spread to 5 mm around the ring wiggle into the 3 mm extra between them.
        assert!(
            info.max_amplitude > 1.0 && info.max_amplitude < 3.0,
            "{info:?}"
        );
        stretch(&mut points, 30.0);
        let end = *points.last().unwrap();
        assert!(
            (end[0] - 30.0).abs() < 1e-9 && end[1].abs() < 1e-6,
            "{end:?}"
        );
        // Far from the ring the turns stay about one line apart and don't wiggle.
        let spt = w.per_turn;
        let r = |i: usize| hypot2(points[i][0], points[i][1]);
        let last = points.len() - 1;
        let d = r(last) - r(last - spt);
        assert!(d > 1.8 && d < 2.2, "{d}");
    }

    #[test]
    fn layers_alternate_out_and_in_as_one_line() {
        let spiral = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]];
        let mut path = Toolpath::new();
        let bead = Flat {
            area: 1.0,
            feed: 600.0,
            layer: 0.5,
        };
        stack(&mut path, &spiral, 3, &bead).unwrap();
        let to: Vec<P3> = path.moves.iter().map(|m| m.to).collect();
        assert!(path.moves[0].travel);
        assert_eq!(
            to,
            vec![
                [0.0, 0.0, 0.5],
                [1.0, 0.0, 0.5],
                [2.0, 0.0, 0.5],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.5],
                [2.0, 0.0, 1.5],
            ]
        );
        assert!(path.moves[1..].iter().all(|m| !m.travel));
    }
}
