//! Layer heights up a wall that isn't straight: each turn of the spiral climbs one layer, or less
//! where the wall leans out, so an overhang gets closer layers that support each other (EasyGCode's
//! overhang-paced vessel, EasyCAD's `contour(…, pace:)`).

use crate::toolpath::MAX_MOVES;

/// The heights the spiral's turns start at, from `z0` up to exactly `z1`: each step as tall as it
/// is halfway up it, the steps then scaled a hair to end on `z1`.
///
/// A step at height z is `layer(z)`, closer where the wall leans: with `cos` the cosine of the
/// wall's lean from vertical there (`lean_cos(z, layer)`, no less than 0.05),
/// `layer · cos^(pace − 1) · (1 − min(pace, 1) · (1 − cos))` for `pace` above 1, so 1 (100 %) makes
/// each step `layer · cos` and 2.5 (250 %) `layer · cos^2.5`; a straight wall keeps full layers.
/// `points` (per turn) bounds the work: too many turns is a message.
pub fn paced(
    z0: f64,
    z1: f64,
    layer: &dyn Fn(f64) -> f64,
    lean_cos: &dyn Fn(f64, f64) -> f64,
    pace: f64,
    points: usize,
) -> Result<Vec<f64>, String> {
    let (pace_c, pace_x) = (pace.clamp(0.0, 1.0), (pace - 1.0).max(0.0));
    let step = |z: f64| -> f64 {
        let l = layer(z);
        if pace <= 0.0 {
            return l;
        }
        let c = lean_cos(z, l).clamp(0.05, 1.0);
        let closer = if pace_x > 0.0 {
            crate::num::pow(c, pace_x)
        } else {
            1.0
        };
        l * closer * (1.0 - pace_c * (1.0 - c))
    };
    let mut zs = vec![z0];
    let mut z = z0;
    while z < z1 - 1e-9 {
        let g = step(z);
        let s = step(z + g / 2.0);
        if !(s > 1e-6 && s.is_finite()) {
            return Err(format!(
                "The layer height must be more than 0, but it is {s} at {z:.2} mm."
            ));
        }
        z += s;
        zs.push(z);
        if zs.len().saturating_mul(points.max(1)) > MAX_MOVES {
            return Err(format!(
                "That's over {} turns of {points} points: too long to compute. Use a thicker layer, a coarser chord tolerance or fewer closer layers on overhangs.",
                zs.len()
            ));
        }
    }
    // Scaled to end exactly at the top.
    let k = (z1 - z0) / (z - z0).max(1e-12);
    Ok(zs.into_iter().map(|z| z0 + (z - z0) * k).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_straight_wall_keeps_full_layers() {
        let zs = paced(0.0, 6.0, &|_| 0.6, &|_, _| 1.0, 2.5, 100).unwrap();
        assert_eq!(zs.len(), 11);
        assert_eq!(zs[0], 0.0);
        assert_eq!(*zs.last().unwrap(), 6.0);
        for w in zs.windows(2) {
            assert!((w[1] - w[0] - 0.6).abs() < 1e-9);
        }
    }

    #[test]
    fn leaning_walls_get_closer_layers_up_to_cos_to_the_two_and_a_half() {
        // Leaning 60° everywhere: cos = 0.5.
        let steps = |pace: f64| {
            let zs = paced(0.0, 30.0, &|_| 0.6, &|_, _| 0.5, pace, 100).unwrap();
            30.0 / (zs.len() - 1) as f64
        };
        assert!((steps(0.0) - 0.6).abs() < 1e-9);
        assert!((steps(0.5) - 0.45).abs() < 0.01); // 0.6 · (1 − 0.5 · 0.5)
        assert!((steps(1.0) - 0.3).abs() < 0.01); // 0.6 · 0.5
        assert!((steps(2.5) - 0.6 * 0.5f64.powf(2.5)).abs() < 0.005);
    }

    #[test]
    fn layers_that_vary_step_by_their_middle_and_too_many_is_a_message() {
        let zs = paced(0.0, 10.0, &|z| 0.2 + 0.04 * z, &|_, _| 1.0, 0.0, 100).unwrap();
        assert!(zs[1] - zs[0] < zs[zs.len() - 1] - zs[zs.len() - 2]);
        assert!(paced(0.0, 1000.0, &|_| 0.01, &|_, _| 1.0, 0.0, 1000).is_err());
        assert!(paced(0.0, 1.0, &|_| 0.0, &|_, _| 1.0, 0.0, 10).is_err());
    }
}
