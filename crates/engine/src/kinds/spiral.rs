//! Spiral: the base every ezgc project grows from. One continuous spiral, no travel: a shape
//! printed like a vase, its extrusion ramping up over the first turn so the spiral climbs off a
//! flat bed, and a level or open rim at the top.
//!
//! Features join it as settings that are off by default (so a project keeps printing as it did):
//! a flat spiral bottom that turns into the wall through a fillet, a body profile, layer heights
//! that follow the overhang, patterns on the wall, a shaped top (docs/roadmap.md).

use super::{Generated, Hint, Info, Kind, Part, to_value, typed};
use crate::bead::{bead_area, feed_for_flow};
use crate::num::{fixed, js, round};
use crate::printer::Printer;
use crate::schema::{Group, choice, group, number};
use crate::toolbox::shapes::{self, SeamSide, Shape};
use crate::toolbox::spiral::{self, Top};
use crate::toolpath::Toolpath;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct Spiral;

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
    /// Volumetric flow, mm³/s: speeds are always set this way, and each move's feed follows from
    /// its bead.
    pub flow: f64,
    pub top: Top,
    pub seam: SeamSide,
    pub direction: Direction,
    /// Chord tolerance of the outline, mm.
    pub tolerance: f64,
}

impl Default for Settings {
    fn default() -> Self {
        // EasyGCode's coat hanger defaults, at its flow-mode default.
        Settings {
            shape: Shape::default(),
            height: 60.0,
            layer_height: 0.6,
            line_width: 2.0,
            flow: 20.0,
            top: Top::Flat,
            seam: SeamSide::Back,
            direction: Direction::Ccw,
            tolerance: 0.05,
        }
    }
}

impl Kind for Spiral {
    fn info(&self) -> Info {
        Info {
            id: "spiral",
            name: "Spiral",
            description: "One continuous spiral, no travel: a shape printed like a vase, ramping up off the bed over the first turn, with a level rim.",
            version: 1,
        }
    }

    fn schema(&self) -> Vec<Group> {
        vec![
            group("Shape", shapes::fields("shape")).open(),
            group("Size", vec![number("height", "Height", "mm", 0.1, 1000.0)]).open(),
            group(
                "Bead and flow",
                vec![
                    number("layerHeight", "Layer height", "mm", 0.05, 10.0),
                    number("lineWidth", "Line width", "mm", 0.1, 20.0),
                    number("flow", "Flow", "mm³/s", 0.1, 1000.0),
                ],
            )
            .open(),
            group(
                "Top",
                vec![choice("top", "Rim", &[("flat", "Flat (level rim)"), ("open", "Open spiral")])],
            ),
            group(
                "Path",
                vec![
                    choice(
                        "seam",
                        "Start",
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
        let feed = feed_for_flow(s.flow, area);
        let mut path = Toolpath::new();
        spiral::wall(
            &mut path,
            &spiral::Wall {
                base: &base,
                layer: s.layer_height,
                height: s.height,
                area,
                width: s.line_width,
                feed,
                top: s.top,
            },
        )?;

        let header = vec![
            format!(
                "spiral: shape={} tolerance={}mm start={} direction={} top={}",
                name(&s.shape.kind),
                js(s.tolerance),
                name(&s.seam),
                name(&s.direction).to_uppercase(),
                name(&s.top)
            ),
            format!(
                "height={} layerHeight={} lineWidth={} flow={} mm3/s -> feed {} mm/min (bead area {} mm2)",
                js(s.height),
                js(s.layer_height),
                js(s.line_width),
                js(s.flow),
                round(feed),
                fixed(area, 3)
            ),
        ];
        let hints = vec![
            // Speeds are set as flow; the feed it makes is shown right next to it.
            Hint {
                key: "flow".into(),
                text: format!(
                    "= {} mm/min with this bead ({} mm²)",
                    round(feed),
                    fixed(area, 3)
                ),
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
        let mut fields = keys(&Spiral.schema());
        let mut leaves = leaf_keys(&Spiral.defaults());
        fields.sort();
        leaves.sort();
        assert_eq!(fields, leaves);
    }

    #[test]
    fn the_defaults_print_at_the_flow_they_say() {
        let g = Spiral
            .generate(&Spiral.defaults(), &Printer::default())
            .unwrap();
        assert_eq!(g.parts.len(), 1);
        assert!(g.warnings.is_empty());
        let stats = g.parts[0].path.stats(6000.0);
        assert_eq!(stats.layers, 100.0);
        // Every full bead melts exactly the 20 mm³/s set; the feed is shown next to it.
        let [lo, hi] = stats.flow.unwrap();
        assert!((lo - 20.0).abs() < 1e-9 && (hi - 20.0).abs() < 1e-9);
        let hint = g.hints.iter().find(|h| h.key == "flow").unwrap();
        assert_eq!(hint.text, "= 1069 mm/min with this bead (1.123 mm²)");
    }
}
