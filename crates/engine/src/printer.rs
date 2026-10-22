//! Printers: the machines ezgc writes for, and the printer profiles on a device.
//!
//! A machine is what ezgc knows about a printer model (mode, heater limits, a typical bed); what
//! only one machine has (the Ginger G1's macros) lives in its start and end blocks (`gcode.rs`).
//! A printer profile is a person's own printer: based on a machine, with its own bed, temperatures,
//! placement and material. Profiles live on the device, apart from projects, so one project prints
//! on any printer, and a project sent to someone else prints on theirs. Its settings are a plain
//! JSON object, edited through the same schema-driven settings window as a project's ([`schema`]).

use crate::schema::{Group, group, number, toggle};
use serde::{Deserialize, Serialize};

/// Which kind of machine, and so which start/end blocks and E units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Klipper, pellets: E in mm³.
    Pellet,
    /// Marlin-style filament printer: E in mm of filament.
    Filament,
}

/// A machine: what EasyGCode knows about a printer model.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub id: &'static str,
    pub name: &'static str,
    pub mode: Mode,
    /// Bed width, depth and build height in mm, for a new profile of this machine.
    pub bed: [f64; 3],
    /// The most volumetric flow it melts reliably (mm³/s), for a warning.
    pub max_flow: f64,
    /// Heater limits (°C) from its firmware config: G-code asking for more is refused.
    pub min_extrude_temp: f64,
    pub max_temp: f64,
    pub max_bed: f64,
}

/// The Ginger Additive G1 (pellets, Klipper) and the Bambu P1P (filament). Figures from the
/// G1's wiki and EasyCAD's `docs/printers.md`.
pub const MACHINES: [Machine; 2] = [
    Machine {
        id: "g1",
        name: "Ginger G1 (pellets)",
        mode: Mode::Pellet,
        bed: [1000.0, 1000.0, 1000.0],
        max_flow: 200.0,
        min_extrude_temp: 160.0,
        max_temp: 280.0,
        max_bed: 110.0,
    },
    Machine {
        id: "p1p",
        name: "Bambu P1P (filament)",
        mode: Mode::Filament,
        bed: [256.0, 256.0, 256.0],
        max_flow: 20.0,
        min_extrude_temp: 0.0,
        max_temp: 300.0,
        max_bed: 110.0,
    },
];

pub fn machine(id: &str) -> Option<&'static Machine> {
    MACHINES.iter().find(|m| m.id == id)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Filament {
    pub diameter: f64,
    pub nozzle: f64,
    pub bed: f64,
    /// Part fan, 0–100 %.
    pub fan: f64,
}

impl Default for Filament {
    fn default() -> Self {
        // EasyGCode's defaults.
        Filament {
            diameter: 1.75,
            nozzle: 205.0,
            bed: 60.0,
            fan: 100.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Pellet {
    /// Upper, middle and lower extruder zone temperatures.
    pub up: f64,
    pub mid: f64,
    pub down: f64,
    pub bed: f64,
    /// Pressure advance.
    pub pa: f64,
    /// Purge amount.
    pub purge: f64,
    /// Part fan, 0–100 %.
    pub fan: f64,
}

impl Default for Pellet {
    fn default() -> Self {
        // EasyGCode's defaults.
        Pellet {
            up: 210.0,
            mid: 210.0,
            down: 210.0,
            bed: 50.0,
            pa: 0.3,
            purge: 5000.0,
            fan: 0.0,
        }
    }
}

/// For the material estimate: 0 skips it (as in EasyGCode).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Material {
    /// g/cm³.
    pub density: f64,
    /// Price per kg.
    pub price: f64,
}

/// The bed, mm: every move must stay inside.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Bed {
    pub width: f64,
    pub depth: f64,
    /// Build height.
    pub height: f64,
}

impl Default for Bed {
    fn default() -> Self {
        let [width, depth, height] = MACHINES[0].bed;
        Bed {
            width,
            depth,
            height,
        }
    }
}

/// A printer profile's settings. Defaults are EasyGCode's, on the Ginger G1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Printer {
    /// The [`Machine`] it's based on (chosen when the profile is made).
    pub model: String,
    pub bed: Bed,
    /// Extrusion multiplier.
    pub multiplier: f64,
    /// Include the start and end G-code blocks.
    pub include_start_end: bool,
    /// How far above the print the nozzle lifts at the end, in mm.
    pub end_lift: f64,
    /// Travel feed in mm/min.
    pub travel_feed: f64,
    /// Where a part's origin sits on the bed, in mm.
    pub center_x: f64,
    pub center_y: f64,
    pub filament: Filament,
    pub pellet: Pellet,
    pub material: Material,
}

impl Default for Printer {
    fn default() -> Self {
        Printer {
            model: MACHINES[0].id.into(),
            bed: Bed::default(),
            multiplier: 1.0,
            include_start_end: true,
            end_lift: 50.0,
            travel_feed: 6000.0,
            center_x: 125.0,
            center_y: 125.0,
            filament: Filament::default(),
            pellet: Pellet::default(),
            material: Material::default(),
        }
    }
}

impl Printer {
    /// A new profile of a machine, with its own bed.
    pub fn new(model: &str) -> Option<Printer> {
        let m = machine(model)?;
        let [width, depth, height] = m.bed;
        Some(Printer {
            model: m.id.into(),
            bed: Bed {
                width,
                depth,
                height,
            },
            ..Printer::default()
        })
    }

    /// The machine, falling back to the first one for an unknown id (`run` reports it).
    pub fn machine(&self) -> &'static Machine {
        machine(&self.model).unwrap_or(&MACHINES[0])
    }

    pub fn mode(&self) -> Mode {
        self.machine().mode
    }

    /// The part fan setting of the printer's mode, 0–100 %.
    pub fn fan(&self) -> f64 {
        match self.mode() {
            Mode::Filament => self.filament.fan,
            Mode::Pellet => self.pellet.fan,
        }
    }
}

/// The printer's settings window: the same field kinds as a project's. The machine it's based on
/// isn't a field: it's chosen when the profile is made.
pub fn schema() -> Vec<Group> {
    let g1 = &MACHINES[0];
    vec![
        group(
            "Temperatures",
            vec![
                number("pellet.up", "Upper zone (feeding)", "°C", g1.min_extrude_temp, g1.max_temp),
                number("pellet.mid", "Middle zone (melting)", "°C", g1.min_extrude_temp, g1.max_temp),
                number("pellet.down", "Lower zone (nozzle)", "°C", g1.min_extrude_temp, g1.max_temp),
                number("pellet.bed", "Bed", "°C", 0.0, g1.max_bed),
                number("pellet.pa", "Pressure advance", "", 0.0, 5.0)
                    .help("0 for vase mode, otherwise 0.2–0.7 on the G1."),
                number("pellet.purge", "Purge amount", "mm³", 0.0, 100_000.0),
                number("pellet.fan", "Part fan", "%", 0.0, 100.0)
                    .help("Keep it under about 70 % on the G1: more can pull the nozzle zone into a heating error."),
            ],
        )
        .when("model", &["g1"])
        .open(),
        group(
            "Temperatures",
            vec![
                number("filament.nozzle", "Nozzle", "°C", 0.0, 300.0),
                number("filament.bed", "Bed", "°C", 0.0, 110.0),
                number("filament.diameter", "Filament diameter", "mm", 0.6, 4.9),
                number("filament.fan", "Part fan", "%", 0.0, 100.0),
            ],
        )
        .when("model", &["p1p"])
        .open(),
        group(
            "Extrusion",
            vec![
                number("multiplier", "Extrusion multiplier", "×", 0.01, 5.0)
                    .help("Scales every E value, for fine-tuning a material."),
                toggle("includeStartEnd", "Start and end G-code"),
            ],
        )
        .open(),
        group(
            "Bed and placement",
            vec![
                number("bed.width", "Bed width (X)", "mm", 10.0, 5000.0),
                number("bed.depth", "Bed depth (Y)", "mm", 10.0, 5000.0),
                number("bed.height", "Build height", "mm", 1.0, 5000.0),
                number("centerX", "Part centre X", "mm", 0.0, 5000.0),
                number("centerY", "Part centre Y", "mm", 0.0, 5000.0),
            ],
        ),
        group(
            "Moves",
            vec![
                number("travelFeed", "Travel feed", "mm/min", 60.0, 18_000.0),
                number("endLift", "Lift at the end", "mm", 0.0, 1000.0)
                    .help("Above the tallest point printed, to reach in and finish the part by hand."),
            ],
        ),
        group(
            "Material estimate",
            vec![
                number("material.density", "Density (0: skip)", "g/cm³", 0.0, 20.0),
                number("material.price", "Price per kg (0: skip)", "", 0.0, 10_000.0),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{keys, leaf_keys};

    #[test]
    fn every_setting_but_the_model_has_a_field() {
        let mut fields = keys(&schema());
        let mut leaves = leaf_keys(&serde_json::to_value(Printer::default()).unwrap());
        leaves.retain(|k| k != "model");
        fields.sort();
        leaves.sort();
        assert_eq!(fields, leaves);
    }

    #[test]
    fn a_new_profile_takes_its_machines_bed() {
        let p = Printer::new("p1p").unwrap();
        assert_eq!(p.bed.width, 256.0);
        assert_eq!(p.mode(), Mode::Filament);
        assert_eq!(Printer::new("g1").unwrap(), Printer::default());
        assert!(Printer::new("nope").is_none());
    }
}
