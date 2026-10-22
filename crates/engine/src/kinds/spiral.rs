//! Spiral: the base every ezgc project grows from. One continuous spiral, no travel: a shape
//! printed like a vase, its extrusion ramping up over the first turn so the spiral climbs off a
//! flat bed, and a level or open rim at the top.
//!
//! Features join it as settings that are off by default (so a project keeps printing as it did):
//! - a **body profile**: the outline scaled along the height, a smooth curve through points;
//! - a **flat bottom**: flat spiral layers (rings linked into a spiral, out and back in) that turn
//!   into the wall through a fillet, the bead easing from theirs into the wall's on the way up;
//! - **closer layers on overhangs**, and an **extrusion** factor that keeps the flow;
//! - a **lid** that screws onto a neck on top: a second part, a flat spiral with holes (turns that
//!   spread apart around a ring and interlock) turning into a skirt. Container + lid make the salt
//!   & pepper shaker (EasyGCode's container, with EasyCAD's lid).
//!
//! Patterns on the wall and a shaped top come later (docs/roadmap.md).

use super::{Generated, Hint, Info, Kind, Part, Template, to_value, typed};
use crate::bead::{bead_area, feed_for_flow};
use crate::num::{fixed, hypot2, js, round};
use crate::printer::Printer;
use crate::schema::{Axis, Group, choice, group, integer, number, points, toggle};
use crate::toolbox::disc::{self, Flat};
use crate::toolbox::layers;
use crate::toolbox::profile::{Graph, fade, smoothstep};
use crate::toolbox::shapes::{self, Sampler, SeamSide, Shape, ShapeKind};
use crate::toolbox::spiral::{self, Shaped, Top};
use crate::toolpath::Toolpath;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
    /// mm. With a lid: up to the neck.
    pub height: f64,
    pub layer_height: f64,
    pub line_width: f64,
    /// Volumetric flow, mm³/s: speeds are always set this way, and each move's feed follows from
    /// its bead.
    pub flow: f64,
    /// The wall's extrusion, %: more plastic per move at the same flow (the move slows to match).
    pub extrusion: f64,
    /// Closer layers on overhangs, % (0: every turn climbs a full layer).
    pub pace: f64,
    pub body: Body,
    pub bottom: Bottom,
    pub disc: Disc,
    pub lid: Lid,
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
            extrusion: 100.0,
            pace: 0.0,
            body: Body::default(),
            bottom: Bottom::default(),
            disc: Disc::default(),
            lid: Lid::default(),
            top: Top::Flat,
            seam: SeamSide::Back,
            direction: Direction::Ccw,
            tolerance: 0.05,
        }
    }
}

/// The outline's size along the height.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Body {
    pub on: bool,
    /// [height, scale], both %: 0 is the bottom of the wall (above a fillet), 100 its top (where a
    /// neck starts); 100 % is the shape's own size. A smooth curve through them, level at both ends.
    pub points: Vec<[f64; 2]>,
}

impl Default for Body {
    fn default() -> Self {
        Body {
            on: false,
            points: vec![[0.0, 100.0], [100.0, 100.0]],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Bottom {
    pub on: bool,
    /// Radius of the fillet from the flat bottom up into the wall, mm (0: a sharp corner).
    pub fillet: f64,
}

impl Default for Bottom {
    fn default() -> Self {
        Bottom {
            on: false,
            fillet: 8.0,
        }
    }
}

/// The flat spirals: the bottom's and the lid's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Disc {
    /// Flat layers, odd (out, back in, out…), so the last ends at the edge where the wall starts.
    pub layers: f64,
    /// mm; 0 is the wall's.
    pub layer_height: f64,
    /// mm; 0 is the wall's.
    pub line_width: f64,
    /// %, keeping the flow.
    pub extrusion: f64,
}

impl Default for Disc {
    fn default() -> Self {
        Disc {
            layers: 3.0,
            layer_height: 0.0,
            line_width: 0.0,
            extrusion: 100.0,
        }
    }
}

/// A lid that screws onto a neck on top of the wall, printed upside down: a flat spiral with
/// holes, a fillet, a skirt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Lid {
    pub on: bool,
    /// The skirt's height, mm; the neck is as tall.
    pub skirt: f64,
    /// The lid's fillet, mm (0: a sharp corner).
    pub fillet: f64,
    /// Extra room between the neck and the skirt, mm (+ looser, − tighter).
    pub fit: f64,
    pub holes: Holes,
}

impl Default for Lid {
    fn default() -> Self {
        Lid {
            on: false,
            skirt: 20.0,
            fillet: 6.0,
            fit: 0.0,
            holes: Holes::default(),
        }
    }
}

/// Where the lid's spiral spreads apart: around a ring, its turns interlocking there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Holes {
    /// mm from the centre.
    pub ring: f64,
    /// The turns' spacing on the ring, % of the line width.
    pub open: f64,
    /// Full spacing over this width around the ring, mm.
    pub band: f64,
    /// The spacing fades back to one line over these distances inside and outside the ring, mm.
    pub fade_in: f64,
    pub fade_out: f64,
    /// Bumps per turn (odd).
    pub bumps: f64,
}

impl Default for Holes {
    fn default() -> Self {
        Holes {
            ring: 12.0,
            open: 250.0,
            band: 0.0,
            fade_in: 6.0,
            fade_out: 6.0,
            bumps: 9.0,
        }
    }
}

/// An odd count from a setting (even rounds up): 1, 3, 5…
fn odd(n: f64, max: f64) -> usize {
    let n = n.round().clamp(1.0, max) as usize;
    2 * (n / 2) + 1
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
        let pct = |label| Axis {
            label,
            unit: "%",
            min: 0.0,
            max: 100.0,
        };
        vec![
            group("Shape", shapes::fields("shape")).open(),
            group(
                "Size",
                vec![number("height", "Height", "mm", 0.1, 1000.0)
                    .help("With a lid: up to the neck (the neck is as tall as the lid's skirt).")],
            )
            .open(),
            group(
                "Bead and flow",
                vec![
                    number("layerHeight", "Layer height", "mm", 0.05, 10.0),
                    number("lineWidth", "Line width", "mm", 0.1, 20.0),
                    number("flow", "Flow", "mm³/s", 0.1, 1000.0),
                    number("extrusion", "Extrusion", "%", 50.0, 200.0)
                        .step(5.0)
                        .help("More or less plastic per move, at the same flow: the moves slow down or speed up to match."),
                    number("pace", "Closer layers on overhangs", "%", 0.0, 250.0)
                        .step(10.0)
                        .help("Where the wall leans out, each turn climbs less (each still a full layer's bead): at 100 % the layer height × the cosine of the lean, above 100 % even less (250 %: × cos²·⁵)."),
                ],
            )
            .open(),
            group(
                "Body profile",
                vec![
                    toggle("body.on", "Size along the height"),
                    points(
                        "body.points",
                        "Profile",
                        pct("Height"),
                        Axis {
                            label: "Scale",
                            unit: "%",
                            min: 10.0,
                            max: 400.0,
                        },
                        1,
                        12,
                    )
                    .when_on("body.on")
                    .help("The shape's size (100 %: as set above) at heights from 0 % (the bottom, above a fillet) to 100 % (the top, or the neck), along a smooth curve that leaves and meets them level."),
                ],
            ),
            group(
                "Flat bottom",
                vec![
                    toggle("bottom.on", "Flat bottom"),
                    number("bottom.fillet", "Fillet", "mm", 0.0, 100.0)
                        .when_on("bottom.on")
                        .help("From the flat bottom up into the wall: a quarter circle, the bead easing from the flat spiral's into the wall's (0: a sharp corner)."),
                ],
            ),
            group(
                "Lid",
                vec![
                    toggle("lid.on", "Screw-on lid with holes"),
                    number("lid.skirt", "Skirt height", "mm", 1.0, 200.0)
                        .when_on("lid.on")
                        .help("A second print. The wall gets a straight neck as tall; both spirals turn the same way, so the lid, turned over, screws onto it."),
                    number("lid.fillet", "Lid fillet", "mm", 0.0, 60.0).when_on("lid.on"),
                    number("lid.fit", "Fit", "mm", -2.0, 2.0)
                        .step(0.05)
                        .when_on("lid.on")
                        .help("The skirt sits one line width outside the neck, plus this (+ looser, − tighter)."),
                ],
            ),
            group(
                "Holes in the lid",
                vec![
                    number("lid.holes.ring", "Ring radius", "mm", 0.0, 500.0),
                    number("lid.holes.open", "Spacing at the ring", "%", 100.0, 400.0)
                        .step(10.0)
                        .help("Of the line width: around the ring the turns spread apart and wiggle into each other, leaving the holes."),
                    number("lid.holes.band", "Full-spacing band", "mm", 0.0, 100.0),
                    number("lid.holes.fadeIn", "Fade inside the ring", "mm", 0.0, 100.0),
                    number("lid.holes.fadeOut", "Fade outside the ring", "mm", 0.0, 100.0),
                    integer("lid.holes.bumps", "Bumps per turn", 1.0, 49.0)
                        .step(2.0)
                        .help("Odd, so each turn's outward bump meets the next turn's inward one."),
                ],
            )
            .when_on("lid.on"),
            group(
                "Flat spirals",
                vec![
                    integer("disc.layers", "Flat spiral layers", 1.0, 9.0)
                        .step(2.0)
                        .help("Out, back in, out…: an odd number, so the last one ends at the edge, where the wall starts. The fillet and the wall start on top of them."),
                    number("disc.layerHeight", "Layer height", "mm", 0.0, 10.0)
                        .help("0: the wall's."),
                    number("disc.lineWidth", "Line width", "mm", 0.0, 20.0).help("0: the wall's."),
                    number("disc.extrusion", "Extrusion", "%", 50.0, 200.0)
                        .step(5.0)
                        .help("At the same flow, as the wall's."),
                ],
            )
            .when_either_on("bottom.on", "lid.on"),
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

    fn templates(&self) -> Vec<Template> {
        vec![Template {
            id: "salt-shaker",
            name: "Salt & pepper shaker",
            description: "A container with a flat bottom and a lid that screws on, with holes to shake through: two prints, each one continuous spiral.",
            notes: "EasyGCode's container with EasyCAD's lid. The container: flat spiral layers from the centre out and back, a fillet up into a bellied wall, its layers closer where it leans out, and a neck. The lid, printed upside down: a flat spiral whose turns spread apart and interlock around a ring (the holes), a fillet and a skirt that screws onto the neck.",
            settings: json!({
                "shape": { "kind": "circle", "circle": { "radius": 33 } },
                "height": 60,
                "pace": 100,
                "body": { "on": true, "points": [[0, 100], [45, 115], [100, 97]] },
                "bottom": { "on": true, "fillet": 8 },
                "lid": { "on": true },
                "top": "open"
            }),
        }]
    }

    fn generate(&self, settings: &Value, _printer: &Printer) -> Result<Generated, String> {
        let s: Settings = typed(settings)?;
        let mut warnings = Vec::new();
        let mut hints = Vec::new();
        if s.line_width < s.layer_height {
            warnings.push("The line width is less than the layer height: the bead's width is taken as the layer height.".into());
        }
        let (layer, line) = (s.layer_height, s.line_width);

        // The body: the outline's scale at heights 0..1 of the wall.
        let profile = if s.body.on {
            let pts: Vec<[f64; 2]> = s
                .body
                .points
                .iter()
                .map(|p| [p[0] / 100.0, p[1] / 100.0])
                .collect();
            Some(Graph::smooth(&pts)?)
        } else {
            None
        };
        let p_at = |t: f64| profile.as_ref().map_or(1.0, |g| g.at(t));
        let p_max = profile.as_ref().map_or(1.0, |g| g.range().1);

        let lid_on = s.lid.on && s.shape.kind == ShapeKind::Circle;
        if s.lid.on && !lid_on {
            warnings.push(
                "The lid screws onto a round neck: choose the circle shape to print one.".into(),
            );
        }

        // The outline, within the chord tolerance at its largest.
        let mut base = shapes::outline(&s.shape, s.tolerance / p_max.max(1.0));
        if s.direction == Direction::Cw {
            base.reverse();
        }
        let base = shapes::seam(&base, s.seam);
        let sampler = Sampler::new(&base);
        let r_max = base.iter().map(|p| hypot2(p[0], p[1])).fold(0.0, f64::max);
        if !(r_max > 1e-6 && r_max.is_finite()) {
            return Err("The shape has zero size: check its dimensions.".into());
        }

        // The wall's bead at a layer and line, with the extrusion factor, at the set flow.
        let e = s.extrusion / 100.0;
        let wall_bead = |line: f64, layer: f64| {
            let a = bead_area(line, layer);
            (a * e, feed_for_flow(s.flow, a) / e)
        };
        let (area, feed) = wall_bead(line, layer);

        // The flat spirals (the bottom's, the lid's): their bead, and how many layers.
        let d_layer = if s.disc.layer_height > 0.0 {
            s.disc.layer_height
        } else {
            layer
        };
        let d_line = if s.disc.line_width > 0.0 {
            s.disc.line_width
        } else {
            line
        };
        let d_count = odd(s.disc.layers, 9.0);
        let flat = {
            let a = bead_area(d_line, d_layer);
            let e = s.disc.extrusion / 100.0;
            Flat {
                area: a * e,
                feed: feed_for_flow(s.flow, a) / e,
                layer: d_layer,
            }
        };
        let z_flat = d_count as f64 * d_layer;
        if (s.bottom.on || lid_on) && s.disc.layers.round() as usize != d_count {
            hints.push(Hint {
                key: "disc.layers".into(),
                text: format!("printing {d_count}: an odd number ends at the edge"),
            });
        }

        let neck_h = if lid_on { s.lid.skirt } else { 0.0 };
        let wall_top = s.height + neck_h;
        let pace = s.pace / 100.0;
        // Points per turn, to bound the work of the paced layers.
        let per_turn = base.len() + 1;

        // ---- The container (the only part without a lid).
        let mut container = Toolpath::new();
        let shaped = s.body.on || s.bottom.on;
        let mut wall_turns = s.height / layer;
        let mut fillet = 0.0;
        if !shaped {
            spiral::wall(
                &mut container,
                &spiral::Wall {
                    base: &base,
                    layer,
                    height: wall_top,
                    area,
                    width: line,
                    feed,
                    top: s.top,
                    shaped: None,
                },
            )?;
        } else {
            let z0 = if s.bottom.on { z_flat } else { 0.0 };
            if s.height <= z0 + layer {
                return Err(format!(
                    "The flat spiral layers ({} mm) leave no room for the wall: make it taller, or use fewer or thinner flat layers.",
                    fixed(z0, 2)
                ));
            }
            let r0 = r_max * p_at(0.0);
            if s.bottom.on && s.bottom.fillet > 0.0 {
                fillet = layer.max(
                    s.bottom
                        .fillet
                        .min(((s.height - z0) * 0.95).min(r0 - 2.0 * d_line)),
                );
                if fillet < s.bottom.fillet - 1e-9 {
                    hints.push(Hint {
                        key: "bottom.fillet".into(),
                        text: format!("{} mm: as much as fits", fixed(fillet, 2)),
                    });
                }
            }
            let body0 = z0 + fillet;
            let scale = |z: f64| -> f64 {
                if fillet > 0.0 && z < body0 {
                    quarter(r0 - fillet, fillet, z - z0) / r_max
                } else if z < s.height {
                    let span = s.height - body0;
                    p_at(if span > 1e-9 { (z - body0) / span } else { 1.0 })
                } else {
                    p_at(1.0)
                }
            };
            // Through the fillet the bead eases from the flat spiral's into the wall's.
            let blend = |z: f64| {
                if fillet > 0.0 {
                    smoothstep((z - z0) / fillet)
                } else {
                    1.0
                }
            };
            let layer_at = |z: f64| {
                if s.bottom.on {
                    d_layer + (layer - d_layer) * blend(z)
                } else {
                    layer
                }
            };
            let line_at = |z: f64| {
                if s.bottom.on {
                    d_line + (line - d_line) * blend(z)
                } else {
                    line
                }
            };
            let lean = |z: f64, l: f64| lean_cos(&scale, z, l, z0, wall_top, r_max);
            let levels = layers::paced(z0, wall_top, &layer_at, &lean, pace, per_turn)?;
            let bead = |z: f64| wall_bead(line_at(z), layer_at(z));
            if s.bottom.on {
                let spiral = disc::ring_spiral(&sampler, r_max, scale(z0), d_line)?;
                disc::stack(&mut container, &spiral, d_count, &flat)?;
            }
            spiral::wall(
                &mut container,
                &spiral::Wall {
                    base: &base,
                    layer,
                    height: wall_top,
                    area,
                    width: line,
                    feed,
                    top: s.top,
                    shaped: Some(Shaped {
                        levels: &levels,
                        scale: &scale,
                        bead: &bead,
                        ramp: !s.bottom.on,
                    }),
                },
            )?;
            wall_turns = container.layers;
            if s.bottom.on {
                container.layers += d_count as f64;
            }
        }

        let mut header = vec![
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
                js(layer),
                js(line),
                js(s.flow),
                round(feed),
                fixed(area, 3)
            ),
        ];
        if s.extrusion != 100.0 || s.pace != 0.0 {
            header.push(format!(
                "extrusion={}% closer layers on overhangs={}%",
                js(s.extrusion),
                js(s.pace)
            ));
        }
        if let Some(g) = &profile {
            let pts: Vec<String> = s
                .body
                .points
                .iter()
                .map(|p| format!("{}%:{}%", js(p[0]), js(p[1])))
                .collect();
            header.push(format!(
                "body profile (height:scale): {} (scale {}..{})",
                pts.join(" "),
                fixed(g.range().0, 3),
                fixed(g.range().1, 3)
            ));
        }
        if s.bottom.on || lid_on {
            header.push(format!(
                "flat spirals: {d_count} layers of {} x {} mm at {}% extrusion",
                js(d_line),
                js(d_layer),
                js(s.disc.extrusion)
            ));
        }
        if s.bottom.on {
            header.push(format!("flat bottom: fillet {} mm", fixed(fillet, 3)));
        }

        // ---- The lid: a flat spiral with holes, a fillet and a skirt, printed upside down.
        let mut parts = Vec::new();
        if lid_on {
            let neck = r_max * p_at(1.0);
            let lid_r = neck + line + s.lid.fit;
            let lid_fillet = if s.lid.fillet > 0.0 {
                layer.max(s.lid.fillet.min((neck_h * 0.95).min(lid_r - 2.0 * d_line)))
            } else {
                0.0
            };
            let lid_flat = lid_r - lid_fillet;
            if lid_flat < 2.0 * d_line {
                return Err(
                    "The lid is too small for its fillet and line: make the neck wider.".into(),
                );
            }
            let z0 = z_flat;
            let lid_top = z0 + lid_fillet + neck_h;
            let scale = |z: f64| -> f64 {
                if lid_fillet > 0.0 && z < z0 + lid_fillet {
                    quarter(lid_flat, lid_fillet, z - z0) / r_max
                } else {
                    lid_r / r_max
                }
            };
            let blend = |z: f64| {
                if lid_fillet > 0.0 {
                    smoothstep((z - z0) / lid_fillet)
                } else {
                    1.0
                }
            };
            let layer_at = |z: f64| d_layer + (layer - d_layer) * blend(z);
            let line_at = |z: f64| d_line + (line - d_line) * blend(z);
            let lean = |z: f64, l: f64| lean_cos(&scale, z, l, z0, lid_top, r_max);
            let levels = layers::paced(z0, lid_top, &layer_at, &lean, pace, per_turn)?;
            let bead = |z: f64| wall_bead(line_at(z), layer_at(z));

            // Turns one line apart, spreading around the ring and interlocking there (the holes),
            // stretched after the wiggles to end exactly where the skirt starts, on the seam.
            let h = &s.lid.holes;
            let open = h.open / 100.0;
            let spacing = |x: f64, y: f64| {
                d_line
                    * (1.0
                        + (open - 1.0) * fade(hypot2(x, y) - h.ring, h.fade_in, h.fade_out, h.band))
            };
            let bumps = odd(h.bumps, 49.0);
            if h.bumps.round() as usize != bumps {
                hints.push(Hint {
                    key: "lid.holes.bumps".into(),
                    text: format!("printing {bumps}: odd, so each bump meets a dip"),
                });
            }
            let winding = disc::hole_spiral(lid_flat, &spacing, s.tolerance, 16 * bumps)?;
            let (mut holes, info) = disc::interlock(&winding, bumps, d_line);
            let b0 = base.first().copied().unwrap_or([0.0, 1.0]);
            let b0_len = hypot2(b0[0], b0[1]).max(1e-9);
            disc::stretch(&mut holes, b0_len * lid_flat / r_max);
            // The spiral turns counter-clockwise and ends on +X: turned the wall's way, then
            // rotated onto its seam.
            let (c, sn) = (b0[0] / b0_len, b0[1] / b0_len);
            let flip = if s.direction == Direction::Cw {
                -1.0
            } else {
                1.0
            };
            for p in holes.iter_mut() {
                let (x, y) = (p[0], p[1] * flip);
                *p = [x * c - y * sn, x * sn + y * c];
            }
            let mut lid = Toolpath::new();
            disc::stack(&mut lid, &holes, d_count, &flat)?;
            spiral::wall(
                &mut lid,
                &spiral::Wall {
                    base: &base,
                    layer,
                    height: lid_top,
                    area,
                    width: line,
                    feed,
                    top: s.top,
                    shaped: Some(Shaped {
                        levels: &levels,
                        scale: &scale,
                        bead: &bead,
                        ramp: false,
                    }),
                },
            )?;
            lid.layers += d_count as f64;

            hints.push(Hint {
                key: "lid.fit".into(),
                text: format!(
                    "the neck {} mm across outside, the lid {} mm inside",
                    fixed(2.0 * (neck + line / 2.0), 1),
                    fixed(2.0 * (lid_r - line / 2.0), 1)
                ),
            });
            hints.push(Hint {
                key: "lid.holes.open".into(),
                text: format!(
                    "turns up to {} mm apart; wiggles up to {} mm",
                    fixed(d_line * open, 1),
                    fixed(info.max_amplitude, 2)
                ),
            });
            let mut lid_header = header.clone();
            lid_header.push(format!(
                "lid: radius {} (neck {} + line + fit {}), fillet {}, skirt {} mm",
                fixed(lid_r, 3),
                fixed(neck, 3),
                js(s.lid.fit),
                fixed(lid_fillet, 3),
                js(neck_h)
            ));
            lid_header.push(format!(
                "holes: ring {} mm, spacing {}% (band {}, fade {} in / {} out mm), {} bumps per turn",
                js(h.ring),
                js(h.open),
                js(h.band),
                js(h.fade_in),
                js(h.fade_out),
                bumps
            ));
            header.push(format!("neck {} mm for a lid", js(neck_h)));
            parts.push(Part {
                name: "Container".into(),
                path: container,
                header,
            });
            parts.push(Part {
                name: "Lid".into(),
                path: lid,
                header: lid_header,
            });
        } else {
            parts.push(Part {
                name: String::new(),
                path: container,
                header,
            });
        }

        // Speeds are set as flow; the feed it makes is shown right next to it.
        hints.insert(
            0,
            Hint {
                key: "flow".into(),
                text: format!(
                    "= {} mm/min with this bead ({} mm²)",
                    round(feed),
                    fixed(area, 3)
                ),
            },
        );
        hints.insert(
            1,
            Hint {
                key: "height".into(),
                text: format!("{} revolutions", fixed(wall_turns, 2)),
            },
        );
        Ok(Generated {
            parts,
            warnings,
            hints,
        })
    }
}

/// The radius of a quarter-circle fillet `dz` above its foot: from `flat` at the foot out to
/// `flat + f` at its top, `f` up.
fn quarter(flat: f64, f: f64, dz: f64) -> f64 {
    let dz = dz.clamp(0.0, f);
    flat + (f * f - (f - dz) * (f - dz)).max(0.0).sqrt()
}

/// The cosine of the wall's lean from vertical at height `z`, where it leans most (the outline's
/// farthest point), over a rise a little more than a hundredth of the layer `l`.
fn lean_cos(scale: &dyn Fn(f64) -> f64, z: f64, l: f64, z0: f64, z1: f64, r_max: f64) -> f64 {
    let eps = (0.02 * l).max(1e-3);
    let (a, b) = (
        scale((z - eps).clamp(z0, z1)),
        scale((z + eps).clamp(z0, z1)),
    );
    let run = (b - a).abs() * r_max;
    2.0 * eps / hypot2(2.0 * eps, run)
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
    use serde_json::json;

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

    fn shaker(changes: Value) -> Generated {
        let mut settings = Spiral.defaults();
        let template = Spiral.templates().remove(0).settings;
        for patch in [template, changes] {
            merge(&mut settings, &patch);
        }
        let (settings, problems) =
            crate::schema::normalize(&Spiral.schema(), &Spiral.defaults(), &settings);
        assert!(problems.is_empty(), "{problems:?}");
        Spiral.generate(&settings, &Printer::default()).unwrap()
    }

    fn merge(a: &mut Value, b: &Value) {
        match (a, b) {
            (Value::Object(a), Value::Object(b)) => {
                for (k, v) in b {
                    merge(a.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
            (a, b) => *a = b.clone(),
        }
    }

    fn top(path: &Toolpath) -> f64 {
        path.moves.iter().map(|m| m.to[2]).fold(0.0, f64::max)
    }

    #[test]
    fn the_shaker_is_a_container_and_a_lid_each_one_line() {
        let g = shaker(json!({}));
        let names: Vec<&str> = g.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Container", "Lid"]);
        for part in &g.parts {
            let m = &part.path.moves;
            // One travel to the start, then no travel and no jump: the flat spiral layers run
            // straight into the fillet and the wall.
            assert!(m[0].travel);
            assert!(m[1..].iter().all(|m| !m.travel), "{}", part.name);
            for w in m.windows(2) {
                let d = crate::geom::dist3(w[0].to, w[1].to);
                assert!(d < 4.0, "{}: a {d} mm move at {:?}", part.name, w[1].to);
            }
            let [lo, hi] = part.path.stats(6000.0).flow.unwrap();
            assert!(
                (lo - 20.0).abs() < 1e-6 && (hi - 20.0).abs() < 1e-6,
                "{lo}..{hi}"
            );
        }
        // The container goes up to the neck (60) and the neck as tall as the skirt (20); the lid
        // is its 3 flat layers, its fillet and the skirt.
        assert!((top(&g.parts[0].path) - 80.0).abs() < 1e-9);
        assert!((top(&g.parts[1].path) - (1.8 + 6.0 + 20.0)).abs() < 1e-9);
        // The bulge: 115 % of 33 mm.
        let b = g.parts[0].path.stats(6000.0).bounds.unwrap();
        assert!((b.max[0] - 33.0 * 1.15).abs() < 0.05, "{:?}", b.max);
        assert!(g.warnings.is_empty(), "{:?}", g.warnings);
    }

    #[test]
    fn extrusion_adds_plastic_but_keeps_the_flow() {
        let plain = shaker(json!({}));
        let more = shaker(json!({ "extrusion": 120, "disc": { "extrusion": 125 } }));
        for (a, b) in plain.parts.iter().zip(&more.parts) {
            let (sa, sb) = (a.path.stats(6000.0), b.path.stats(6000.0));
            let [lo, hi] = sb.flow.unwrap();
            assert!(
                (lo - 20.0).abs() < 1e-6 && (hi - 20.0).abs() < 1e-6,
                "{lo}..{hi}"
            );
            // Same path, more plastic, slower.
            assert_eq!(sa.length_mm, sb.length_mm);
            assert!(sb.volume_mm3 > sa.volume_mm3 * 1.19 && sb.volume_mm3 < sa.volume_mm3 * 1.26);
            assert!(sb.time_min > sa.time_min * 1.19);
        }
    }

    #[test]
    fn flat_spiral_layers_are_odd_and_lift_the_fillet_and_the_lid() {
        let one = shaker(json!({ "disc": { "layers": 1, "layerHeight": 0.5 } }));
        let five = shaker(json!({ "disc": { "layers": 4, "layerHeight": 0.5 } }));
        assert_eq!(
            five.hints
                .iter()
                .find(|h| h.key == "disc.layers")
                .unwrap()
                .text,
            "printing 5: an odd number ends at the edge"
        );
        // The container keeps its height to the neck; the lid grows by the extra layers.
        assert_eq!(top(&one.parts[0].path), top(&five.parts[0].path));
        assert!((top(&five.parts[1].path) - top(&one.parts[1].path) - 4.0 * 0.5).abs() < 1e-9);
        // Out, in, out…: the flat layers end at the edge, where the wall starts, on the seam (+Y).
        let m = &five.parts[0].path.moves;
        let first_wall = m.iter().position(|m| m.to[2] > 2.5 + 1e-9).unwrap();
        let edge = m[first_wall - 1].to;
        assert!(
            edge[0].abs() < 1e-6 && (edge[1] - (33.0 - 8.0)).abs() < 0.05,
            "{edge:?}"
        );
        let middle = m.iter().filter(|m| m.to[0].hypot(m.to[1]) < 1e-9).count();
        assert_eq!(middle, 3); // the start, and the two turns back out of the centre
    }

    #[test]
    fn closer_layers_on_overhangs_up_to_two_and_a_half() {
        let turns = |pace: f64| shaker(json!({ "pace": pace })).parts[0].path.layers;
        let (none, full, more) = (turns(0.0), turns(100.0), turns(250.0));
        assert!(none < full && full < more, "{none} {full} {more}");
        // A straight wall keeps its layers.
        let straight = |pace: f64| {
            shaker(json!({ "pace": pace, "body": { "on": false }, "bottom": { "on": false }, "lid": { "on": false } }))
                .parts[0]
                .path
                .layers
        };
        assert_eq!(straight(0.0), straight(250.0));
    }

    #[test]
    fn the_lid_needs_a_round_neck() {
        let g = shaker(json!({ "shape": { "kind": "star" } }));
        assert_eq!(g.parts.len(), 1);
        assert!(g.warnings[0].contains("round neck"));
    }
}
