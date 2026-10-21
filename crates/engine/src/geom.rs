//! Points and distances. Plain arrays keep the engine dependency-free; distances use V8's
//! `Math.hypot` (see [`crate::num`]) so lengths match EasyGCode's to the last bit.

use crate::num::{hypot2, hypot3};

/// A point in the plane (mm).
pub type P2 = [f64; 2];
/// A point in space (mm), Z up.
pub type P3 = [f64; 3];

pub fn dist2(a: P2, b: P2) -> f64 {
    hypot2(b[0] - a[0], b[1] - a[1])
}

pub fn dist3(a: P3, b: P3) -> f64 {
    hypot3(b[0] - a[0], b[1] - a[1], b[2] - a[2])
}

/// Axis-aligned bounds of everything a toolpath visits.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Bounds {
    pub min: P3,
    pub max: P3,
}

impl Bounds {
    pub fn empty() -> Bounds {
        Bounds {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }

    pub fn add(&mut self, p: P3) {
        for ((lo, hi), v) in self.min.iter_mut().zip(self.max.iter_mut()).zip(p) {
            *lo = lo.min(v);
            *hi = hi.max(v);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min[0]
            .partial_cmp(&self.max[0])
            .is_none_or(|o| o.is_gt())
    }
}
