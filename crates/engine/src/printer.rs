//! Printers: the machines EasyGCode writes for, and the per-project printer settings.
//!
//! A machine is data (bed, mode, limits); what only one machine has (the Ginger G1's macros) lives
//! in its start and end blocks (`gcode.rs`). The settings a project keeps for its printer are a
//! plain JSON object, edited through the same schema-driven settings window as the project's own
//! settings ([`schema`]), so adding a field is one line here and one in [`Printer`].

use crate::schema::{Group, choice, group, number, toggle};
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
    /// Bed width, depth and build height in mm: every move must stay inside.
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

/// A project's printer settings. Defaults are EasyGCode's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Printer {
    /// A [`Machine`] id.
    pub model: String,
    /// Extrusion multiplier.
    pub multiplier: f64,
    /// Include the start and end G-code blocks.
    pub include_start_end: bool,
    /// How far above the print the nozzle lifts at the end, in mm.
    pub end_lift: f64,
    /// Travel feed in mm/min.
    pub travel_feed: f64,
    /// Where the part's origin sits on the bed, in mm.
    pub center_x: f64,
    pub center_y: f64,
    pub filament: Filament,
    pub pellet: Pellet,
    pub material: Material,
}

impl Default for Printer {
    fn default() -> Self {
        Printer {
            model: "g1".into(),
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
    /// The machine, falling back to the first one for an unknown id (the schema check reports it).
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

/// The printer's settings window: the same field kinds as a project's.
pub fn schema() -> Vec<Group> {
    let models: Vec<(&'static str, &'static str)> =
        MACHINES.iter().map(|m| (m.id, m.name)).collect();
    let g1 = &MACHINES[0];
    vec![
        group(
            "Printer",
            vec![
                choice("model", "Printer", &models),
                toggle("includeStartEnd", "Start and end G-code"),
                number("multiplier", "Extrusion multiplier", "×", 0.01, 5.0)
                    .help("Scales every E value, for fine-tuning a material."),
            ],
        )
        .open(),
        group(
            "Temperatures (pellets)",
            vec![
                number(
                    "pellet.up",
                    "Upper zone (feeding)",
                    "°C",
                    g1.min_extrude_temp,
                    g1.max_temp,
                ),
                number(
                    "pellet.mid",
                    "Middle zone (melting)",
                    "°C",
                    g1.min_extrude_temp,
                    g1.max_temp,
                ),
                number(
                    "pellet.down",
                    "Lower zone (nozzle)",
                    "°C",
                    g1.min_extrude_temp,
                    g1.max_temp,
                ),
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
            "Temperatures (filament)",
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
            "Placement and moves",
            vec![
                number("centerX", "Part centre X", "mm", 0.0, 1000.0),
                number("centerY", "Part centre Y", "mm", 0.0, 1000.0),
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
