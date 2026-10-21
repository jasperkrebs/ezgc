//! Vase mode: one continuous spiral up a wall (EasyGCode's coat hanger, without its extras).
//!
//! The outline is walked by arc length and Z rises one layer per revolution, with no seam and no
//! travel. The first revolution ramps up: it starts on the bed with no extrusion and climbs to a
//! full bead over one turn, each move extruding the average of the ramp at its two ends, through
//! a denser set of points (no gap over one line width) so the ramp climbs smoothly. The height
//! need not be a whole number of layers: the last revolution stops part way. A flat top then adds
//! one more revolution at the top height whose extrusion tapers to zero, so the rim finishes level;
//! an open top stops at full flow.
//!
//! Ported from `gcode.js` (`generate`, its vase spiral with pattern, hanger, brim and bottom fillet
//! off), and golden-tested against it.

use crate::geom::P2;
use crate::num::round_to;
use crate::toolbox::shapes::Sampler;
use crate::toolpath::{Event, MAX_MOVES, Toolpath};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Top {
    /// One more revolution at the top height, extrusion tapering to zero: a level rim.
    #[default]
    Flat,
    /// The spiral ends at full flow, a one-layer step at the seam (for open rims).
    Open,
}

/// One vase-mode wall.
pub struct Spiral<'a> {
    /// The outline, starting at the seam (see `shapes::seam`).
    pub base: &'a [P2],
    pub layer: f64,
    pub height: f64,
    /// The bead's cross-section, mm².
    pub area: f64,
    /// The line width, mm (the ramp's points are at most this far apart).
    pub width: f64,
    /// mm/min.
    pub feed: f64,
    pub top: Top,
}

/// Adds the spiral to `path`. Fails with a message when the outline has no size or the spiral
/// would be too long to compute.
pub fn spiral(path: &mut Toolpath, s: &Spiral) -> Result<(), String> {
    let sampler = Sampler::new(s.base);
    let perim = sampler.perimeter;
    if !(perim > 1e-6 && perim.is_finite()) {
        return Err("The shape has zero size: check its dimensions.".into());
    }
    let lh = s.layer;
    // Revolutions: may end part way through one.
    let turns = s.height / lh;
    let whole = (turns - 1e-9).ceil().max(0.0) as usize;

    // The outline's own points, as fractions of the way round (rounded like EasyGCode, so points
    // a hair apart count as one).
    let mut u_set: Vec<f64> = (0..s.base.len())
        .map(|i| round_to(sampler.u_of(i), 9))
        .collect();
    u_set.sort_by(f64::total_cmp);
    u_set.dedup();
    if u_set.first().is_none_or(|u| *u > 1e-9) {
        u_set.insert(0, 0.0);
    }
    // The ramps' points: no gap wider than a line width (0.5 mm at least), so the extrusion
    // climbs in small steps even along a straight side.
    let max_du = s.width.max(0.5) / perim;
    let mut u_ramp = Vec::with_capacity(u_set.len());
    for (i, &a) in u_set.iter().enumerate() {
        let b = u_set.get(i + 1).copied().unwrap_or(1.0);
        u_ramp.push(a);
        let gap = b - a;
        if gap > max_du {
            let n = (gap / max_du).ceil() as usize;
            u_ramp.extend((1..n).map(|k| a + (gap * k as f64) / n as f64));
        }
    }
    if whole.saturating_mul(u_set.len() + 1) + 2 * u_ramp.len() > MAX_MOVES {
        return Err(format!(
            "That's about {} revolutions of {} points: too long to compute. Use a thicker layer or a coarser chord tolerance.",
            whole,
            u_set.len()
        ));
    }

    path.comment("--- vase spiral ---");
    let (start, _) = sampler.at(0.0);
    path.travel([start[0], start[1], 0.0]);
    let mut prev_u = 0.0;
    for l in 0..whole {
        let u_end = (turns - l as f64).min(1.0);
        if l == 1 {
            path.event(Event::FanOn);
        }
        let first = l == 0;
        let set = if first { &u_ramp } else { &u_set };
        let mut step = |path: &mut Toolpath, u: f64| {
            let (p, _) = sampler.at(u);
            let z = (lh * (l as f64 + u)).min(s.height).max(0.0);
            let ramp = if first {
                ((prev_u + u) / 2.0).clamp(0.0, 1.0)
            } else {
                1.0
            };
            path.extrude([p[0], p[1], z], s.area, ramp, s.feed);
            prev_u = u;
        };
        for &u in set {
            if (l > 0 && u <= 1e-9) || u >= u_end - 1e-9 {
                continue;
            }
            step(path, u);
        }
        step(path, u_end);
    }

    match s.top {
        Top::Flat => {
            // Where the spiral ended, as a fraction of a revolution.
            let f0 = (turns - (whole as f64 - 1.0)).min(1.0) % 1.0;
            path.comment("flat top: no z gain, extrusion ramps to zero for a clean rim");
            let seq = u_ramp
                .iter()
                .filter(|u| **u > f0 + 1e-9)
                .chain(u_ramp.iter().filter(|u| **u <= f0 + 1e-9))
                .copied()
                .chain(std::iter::once(f0));
            let mut pf = f0;
            let mut trav = 0.0f64;
            for u in seq {
                let mut d = u - pf;
                if d <= 1e-9 {
                    d += 1.0; // forward-wrap the fraction
                }
                trav = (trav + d).min(1.0);
                let (p, _) = sampler.at(u);
                path.extrude(
                    [p[0], p[1], s.height],
                    s.area,
                    (1.0 - trav).max(0.0),
                    s.feed,
                );
                pf = u;
            }
        }
        Top::Open => path.comment("open spiral top: wall ends at full flow, no cap loop"),
    }
    path.layers = turns;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bead::bead_area;
    use crate::toolbox::shapes::{SeamSide, Shape, outline, seam};

    fn wall(height: f64, top: Top) -> Toolpath {
        let base = seam(&outline(&Shape::default(), 0.05), SeamSide::Back);
        let mut path = Toolpath::new();
        spiral(
            &mut path,
            &Spiral {
                base: &base,
                layer: 0.6,
                height,
                area: bead_area(2.0, 0.6),
                width: 2.0,
                feed: 1800.0,
                top,
            },
        )
        .unwrap();
        path
    }

    #[test]
    fn rises_one_layer_per_revolution_and_ends_level() {
        let path = wall(3.0, Top::Flat);
        let first = path.moves[0];
        assert!(first.travel && first.to[2] == 0.0);
        let top = path.moves.iter().map(|m| m.to[2]).fold(0.0, f64::max);
        assert_eq!(top, 3.0);
        // The ramp starts from nothing; the flat top tapers back to nothing.
        assert!(path.moves[2].flow < 0.05);
        assert_eq!(path.moves.last().unwrap().flow, 0.0);
        assert_eq!(path.layers, 5.0);
    }

    #[test]
    fn a_part_revolution_and_an_open_top() {
        let path = wall(3.3, Top::Open);
        assert!((path.layers - 5.5).abs() < 1e-12);
        let last = path.moves.last().unwrap();
        assert_eq!(last.flow, 1.0);
        assert!((last.to[2] - 3.3).abs() < 1e-12);
        // Half way round from the seam (+Y), on a circle of 30: at −Y.
        assert!((last.to[1] + 30.0).abs() < 0.01, "{:?}", last.to);
    }

    #[test]
    fn zero_size_is_a_message() {
        let base = vec![[0.0, 0.0]; 4];
        let mut path = Toolpath::new();
        let err = spiral(
            &mut path,
            &Spiral {
                base: &base,
                layer: 0.6,
                height: 3.0,
                area: 1.0,
                width: 2.0,
                feed: 1800.0,
                top: Top::Flat,
            },
        )
        .unwrap_err();
        assert!(err.contains("zero size"));
    }
}
