//! Vase: one shape printed as a continuous vase-mode spiral.
//!
//! The base of EasyGCode's coat hanger (and of its vessel), without the coat hanger's extras
//! (patterns, the wall hanger, brim, bottom fillet): those come back as toolbox steps when the
//! coat hanger is ported. With them off, its G-code matches EasyGCode's coat hanger command for
//! command (`golden_tests.rs`).

use super::{Generated, Hint, Info, Kind, Part, to_value, typed};
use crate::bead::{bead_area, feed_for_flow};
use crate::num::{fixed, js, round};
use crate::printer::Printer;
use crate::schema::{Group, choice, group, number};
use crate::toolbox::shapes::{self, SeamSide, Shape};
use crate::toolbox::vase::{self, Top};
use crate::toolpath::Toolpath;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct Vase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SpeedMode {
    /// Volumetric flow (mm³/s): the feed follows from the bead.
    #[default]
    Flow,
    /// A fixed feed (mm/min).
    Feed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Speed {
    pub mode: SpeedMode,
    /// mm³/s.
    pub flow: f64,
    /// mm/min.
    pub feed: f64,
}

impl Default for Speed {
    fn default() -> Self {
        // EasyGCode's coat hanger: 1800 mm/min, or 20 mm³/s in its flow mode.
        Speed {
            mode: SpeedMode::Flow,
            flow: 20.0,
            feed: 1800.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Counter-clockwise, seen from above (EasyGCode's default).
    #[default]
    Ccw,
    Cw,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub shape: Shape,
    /// mm.
    pub height: f64,
    pub layer_height: f64,
    pub line_width: f64,
    pub speed: Speed,
    pub top: Top,
    pub seam: SeamSide,
    pub direction: Direction,
    /// Chord tolerance of the outline, mm.
    pub tolerance: f64,
}

impl Default for Settings {
    fn default() -> Self {
        // EasyGCode's coat hanger defaults.
        Settings {
            shape: Shape::default(),
            height: 60.0,
            layer_height: 0.6,
            line_width: 2.0,
            speed: Speed::default(),
            top: Top::Flat,
            seam: SeamSide::Back,
            direction: Direction::Ccw,
            tolerance: 0.05,
        }
    }
}

impl Kind for Vase {
    fn info(&self) -> Info {
        Info {
            id: "vase",
            name: "Vase",
            description: "A shape printed as one continuous spiral (vase mode), with a ramped first turn and a level rim.",
            version: 1,
        }
    }

    fn schema(&self) -> Vec<Group> {
        vec![
            group("Shape", shapes::fields("shape")).open(),
            group(
                "Size",
                vec![
                    number("height", "Height", "mm", 0.1, 1000.0),
                    number("layerHeight", "Layer height", "mm", 0.05, 10.0),
                    number("lineWidth", "Line width", "mm", 0.1, 20.0),
                ],
            )
            .open(),
            group(
                "Speed",
                vec![
                    choice("speed.mode", "Set the speed as", &[("flow", "Volumetric flow"), ("feed", "Feed")]),
                    number("speed.flow", "Flow", "mm³/s", 0.1, 1000.0).when("speed.mode", &["flow"]),
                    number("speed.feed", "Feed", "mm/min", 1.0, 60_000.0).when("speed.mode", &["feed"]),
                ],
            )
            .open(),
            group(
                "Spiral",
                vec![
                    choice("top", "Top", &[("flat", "Flat cap (level rim)"), ("open", "Open spiral")]),
                    choice(
                        "seam",
                        "Seam",
                        &[("back", "Back (+Y)"), ("front", "Front (−Y)"), ("right", "Right (+X)"), ("left", "Left (−X)")],
                    ),
                    choice("direction", "Direction", &[("ccw", "Counter-clockwise"), ("cw", "Clockwise")]),
                    number("tolerance", "Chord tolerance", "mm", 0.001, 5.0)
                        .help("How closely the outline's points follow the shape: smaller is smoother, with more points."),
                ],
            ),
        ]
    }

    fn defaults(&self) -> Value {
        to_value(&Settings::default())
    }

    fn generate(&self, settings: &Value, _printer: &Printer) -> Result<Generated, String> {
        let s: Settings = typed(settings)?;
        let mut warnings = Vec::new();
        if s.line_width < s.layer_height {
            warnings.push("The line width is less than the layer height: the bead's width is taken as the layer height.".into());
        }
        let mut base = shapes::outline(&s.shape, s.tolerance);
        if s.direction == Direction::Cw {
            base.reverse();
        }
        let base = shapes::seam(&base, s.seam);
        let area = bead_area(s.line_width, s.layer_height);
        let feed = match s.speed.mode {
            SpeedMode::Flow => feed_for_flow(s.speed.flow, area),
            SpeedMode::Feed => s.speed.feed,
        };
        let mut path = Toolpath::new();
        vase::spiral(
            &mut path,
            &vase::Spiral {
                base: &base,
                layer: s.layer_height,
                height: s.height,
                area,
                width: s.line_width,
                feed,
                top: s.top,
            },
        )?;

        let flow = feed * area / 60.0;
        let header = vec![
            format!(
                "vase: shape={} tolerance={}mm seam={}",
                name(&s.shape.kind),
                js(s.tolerance),
                name(&s.seam)
            ),
            format!(
                "layerHeight={} lineWidth={} height={} direction={} top={}",
                js(s.layer_height),
                js(s.line_width),
                js(s.height),
                name(&s.direction).to_uppercase(),
                name(&s.top)
            ),
            format!(
                "feed {} mm/min = flow {} mm3/s (bead area {} mm2)",
                round(feed),
                fixed(flow, 2),
                fixed(area, 3)
            ),
        ];
        let hints = vec![
            Hint {
                key: match s.speed.mode {
                    SpeedMode::Flow => "speed.flow".into(),
                    SpeedMode::Feed => "speed.feed".into(),
                },
                text: match s.speed.mode {
                    SpeedMode::Flow => format!(
                        "= {} mm/min with this bead ({} mm²)",
                        round(feed),
                        fixed(area, 3)
                    ),
                    SpeedMode::Feed => format!(
                        "= {} mm³/s with this bead ({} mm²)",
                        fixed(flow, 2),
                        fixed(area, 3)
                    ),
                },
            },
            Hint {
                key: "height".into(),
                text: format!("{} revolutions", fixed(s.height / s.layer_height, 2)),
            },
        ];
        Ok(Generated {
            parts: vec![Part {
                name: String::new(),
                path,
                header,
            }],
            warnings,
            hints,
        })
    }
}

/// A setting's value as the settings file writes it ("roundedRect", "ccw").
fn name<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{keys, leaf_keys};

    #[test]
    fn every_setting_has_a_field_and_every_field_a_setting() {
        let mut fields = keys(&Vase.schema());
        let mut leaves = leaf_keys(&Vase.defaults());
        fields.sort();
        leaves.sort();
        assert_eq!(fields, leaves);
    }

    #[test]
    fn the_defaults_print() {
        let g = Vase
            .generate(&Vase.defaults(), &Printer::default())
            .unwrap();
        assert_eq!(g.parts.len(), 1);
        assert!(g.warnings.is_empty());
        let stats = g.parts[0].path.stats(6000.0);
        assert_eq!(stats.layers, 100.0);
        // Flow mode: every full bead melts 20 mm³/s.
        let [lo, hi] = stats.flow.unwrap();
        assert!((lo - 20.0).abs() < 1e-9 && (hi - 20.0).abs() < 1e-9);
    }
}
