//! ezgc's engine: a project (kind + settings) and a printer profile → toolpaths → G-code.
//!
//! This is the whole API the app uses (through `ezgc-wasm`), and the one a CLI or an agent
//! would use: every call takes and returns plain data (JSON text), nothing here knows about the
//! browser.
//!
//! - [`kinds`], [`schema`], [`printer_schema`]: what projects can be, the printers ezgc knows, and
//!   their settings windows.
//! - [`templates`]: ready-made projects (the salt & pepper shaker, …).
//! - [`new_project`], [`new_printer`]: a fresh project of a kind (every setting at its default) or
//!   from a template, a fresh printer profile of a machine.
//! - [`run`]: checks a project's and a printer's settings and builds the toolpaths, for the
//!   preview and the estimate. Never fails: problems come back as messages, next to the settings
//!   they're about.
//! - [`gcode`]: the G-code of one printed part, or why it can't be written.
//!
//! Inside: [`kinds`] compose [`toolbox`] steps into [`toolpath`]s; [`gcode`] writes any toolpath
//! for any [`printer`]; [`schema`] describes and checks settings. Rules (see `CLAUDE.md`): no UI,
//! no I/O, no panics. Bad input of any kind becomes a message.

pub mod bead;
pub mod gcode;
pub mod geom;
pub mod kinds;
pub mod num;
pub mod printer;
pub mod project;
pub mod schema;
pub mod toolbox;
pub mod toolpath;

#[cfg(test)]
mod snapshot_tests;

use kinds::{Hint, Info, Kind, TemplateInfo};
use printer::{Machine, Mode, Printer};
use project::Project;
use schema::{Group, Problem};
use serde::Serialize;
use serde_json::Value;
use toolpath::Stats;

/// Every kind of project.
pub fn kinds() -> Vec<Info> {
    kinds::all().iter().map(|k| k.info()).collect()
}

/// A kind's settings window and defaults.
#[derive(Debug, Clone, Serialize)]
pub struct KindSchema {
    pub info: Info,
    pub groups: Vec<Group>,
    pub defaults: Value,
}

pub fn schema(kind: &str) -> Result<KindSchema, String> {
    let k = find(kind)?;
    Ok(KindSchema {
        info: k.info(),
        groups: k.schema(),
        defaults: k.defaults(),
    })
}

/// The printer's settings window, and the machines a profile can be based on.
#[derive(Debug, Clone, Serialize)]
pub struct PrinterSchema {
    pub groups: Vec<Group>,
    pub machines: &'static [Machine],
}

pub fn printer_schema() -> PrinterSchema {
    PrinterSchema {
        groups: printer::schema(),
        machines: &printer::MACHINES,
    }
}

/// Every template, kind by kind.
pub fn templates() -> Vec<TemplateInfo> {
    kinds::all()
        .iter()
        .flat_map(|k| {
            let kind = k.info().id;
            k.templates().into_iter().map(move |t| TemplateInfo {
                id: t.id,
                kind,
                name: t.name,
                description: t.description,
            })
        })
        .collect()
}

/// A new project: of a kind, with every setting at its default, or from a template (by its id).
pub fn new_project(id: &str) -> Result<Project, String> {
    if let Some(k) = kinds::find(id) {
        let info = k.info();
        return Ok(Project {
            app: project::APP.into(),
            format: project::FORMAT,
            kind: info.id.into(),
            kind_version: info.version,
            name: format!("New {}", info.name.to_lowercase()),
            notes: String::new(),
            settings: k.defaults(),
        });
    }
    for k in kinds::all() {
        if let Some(t) = k.templates().into_iter().find(|t| t.id == id) {
            let info = k.info();
            let (settings, _) = schema::normalize(&k.schema(), &k.defaults(), &t.settings);
            return Ok(Project {
                app: project::APP.into(),
                format: project::FORMAT,
                kind: info.id.into(),
                kind_version: info.version,
                name: t.name.into(),
                notes: t.notes.into(),
                settings,
            });
        }
    }
    Err(unknown_kind(id))
}

/// A new printer profile based on a machine (`"g1"`, `"p1p"`), every setting at its default.
pub fn new_printer(model: &str) -> Result<Printer, String> {
    Printer::new(model).ok_or_else(|| unknown_model(model))
}

fn unknown_model(model: &str) -> String {
    format!("This ezgc doesn't know the printer \"{model}\" (reload the app to update it).")
}

fn find(kind: &str) -> Result<&'static dyn Kind, String> {
    kinds::find(kind).ok_or_else(|| unknown_kind(kind))
}

fn unknown_kind(kind: &str) -> String {
    format!("This ezgc doesn't know projects of kind \"{kind}\" (reload the app to update it).")
}

/// What [`run`] found.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// The kind, when the file names one this engine knows.
    pub kind: Option<Info>,
    /// The project's settings, complete (defaults filled in) and as the kind's current version.
    pub settings: Value,
    /// The printer profile's settings, complete.
    pub printer: Value,
    /// Settings that need fixing (nothing is built until they are).
    pub problems: Vec<Problem>,
    pub printer_problems: Vec<Problem>,
    /// Why nothing could be built, when it isn't one setting's fault.
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub hints: Vec<Hint>,
    pub parts: Vec<PartRun>,
}

/// One printed part, for the preview and the estimate.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartRun {
    pub name: String,
    pub stats: Stats,
    /// Grams and cost, when the printer profile gives a density and a price.
    pub mass_g: Option<f64>,
    pub cost: Option<f64>,
    /// Why its G-code can't be written (off the bed, …), if so.
    pub blocked: Option<String>,
    /// The moves' end points on the bed (x, y, z), for the preview.
    #[serde(skip)]
    pub positions: Vec<f32>,
    /// Each move's feed, mm/min (0 for travels).
    #[serde(skip)]
    pub feeds: Vec<f32>,
    /// 1 where a move is a travel.
    #[serde(skip)]
    pub travel: Vec<u8>,
}

struct Prepared {
    project: Project,
    kind: &'static dyn Kind,
    settings: Value,
    printer: Value,
    problems: Vec<Problem>,
    printer_problems: Vec<Problem>,
}

fn prepare(project: &str, printer: &str) -> Result<Prepared, String> {
    let project = project::parse(project)?;
    let kind = find(&project.kind)?;
    let mut given = project.settings.clone();
    let version = kind.info().version;
    if project.kind_version < version {
        kind.migrate(&mut given, project.kind_version);
    }
    let (settings, problems) = schema::normalize(&kind.schema(), &kind.defaults(), &given);
    let given = project::parse_json(printer, "printer")?;
    let model = given.get("model").and_then(Value::as_str).unwrap_or("g1");
    let defaults = kinds::to_value(&new_printer(model)?);
    let (printer, printer_problems) = schema::normalize(&printer::schema(), &defaults, &given);
    Ok(Prepared {
        project,
        kind,
        settings,
        printer,
        problems,
        printer_problems,
    })
}

/// Checks a project (the text of its file) and a printer profile's settings (JSON), and builds
/// the toolpaths.
pub fn run(project: &str, printer: &str) -> Run {
    let p = match prepare(project, printer) {
        Ok(p) => p,
        Err(e) => {
            return Run {
                error: Some(e),
                ..Run::default()
            };
        }
    };
    let mut out = Run {
        kind: Some(p.kind.info()),
        settings: p.settings.clone(),
        printer: p.printer.clone(),
        problems: p.problems.clone(),
        printer_problems: p.printer_problems.clone(),
        ..Run::default()
    };
    if !p.problems.is_empty() || !p.printer_problems.is_empty() {
        return out;
    }
    let printer: Printer = match kinds::typed(&p.printer) {
        Ok(v) => v,
        Err(e) => {
            out.error = Some(e);
            return out;
        }
    };
    let generated = match p.kind.generate(&p.settings, &printer) {
        Ok(g) => g,
        Err(e) => {
            out.error = Some(e);
            return out;
        }
    };
    out.warnings = generated.warnings;
    out.hints = generated.hints;
    for part in &generated.parts {
        let stats = part.path.stats(printer.travel_feed);
        out.warnings
            .extend(printer_warnings(&printer, &stats, &part.name));
        out.parts.push(part_run(part, stats, &printer));
    }
    out
}

fn part_run(part: &kinds::Part, stats: Stats, printer: &Printer) -> PartRun {
    let (cx, cy) = (printer.center_x, printer.center_y);
    let n = part.path.moves.len();
    let mut positions = Vec::with_capacity(n * 3);
    let mut feeds = Vec::with_capacity(n);
    let mut travel = Vec::with_capacity(n);
    for m in &part.path.moves {
        positions.extend([(m.to[0] + cx) as f32, (m.to[1] + cy) as f32, m.to[2] as f32]);
        feeds.push(if m.travel { 0.0 } else { m.feed as f32 });
        travel.push(u8::from(m.travel));
    }
    let m = &printer.material;
    let mass_g = (m.density > 0.0).then(|| stats.volume_mm3 * m.density / 1000.0);
    let cost = mass_g
        .filter(|_| m.price > 0.0)
        .map(|g| g / 1000.0 * m.price);
    PartRun {
        name: part.name.clone(),
        blocked: gcode::check(&part.path, printer).err(),
        stats,
        mass_g,
        cost,
        positions,
        feeds,
        travel,
    }
}

/// What the machine itself would struggle with (EasyCAD's printer checks, from Ginger's wiki).
fn printer_warnings(printer: &Printer, stats: &Stats, part: &str) -> Vec<String> {
    let m = printer.machine();
    let of = if part.is_empty() {
        String::new()
    } else {
        format!(" ({part})")
    };
    let mut out = Vec::new();
    if let Some([lo, hi]) = stats.flow {
        // A flow set to exactly the limit comes back a hair over it (flow → feed → flow).
        if hi > m.max_flow * (1.0 + 1e-9) {
            out.push(format!(
                "The flow reaches {} mm³/s{of}: more than the {} melts reliably (about {} mm³/s).",
                num::fixed(hi, 1),
                m.name,
                num::js(m.max_flow)
            ));
        }
        let p = &printer.pellet;
        if m.mode == Mode::Pellet && lo < 150.0 && p.up > p.mid.min(p.down) - 10.0 {
            out.push(format!(
                "The flow is {} mm³/s{of}: below about 150 mm³/s the G1's barrel inlet can plug. Run the upper zone at least 10 °C cooler than the other two.",
                num::fixed(lo, 1)
            ));
        }
    }
    if m.mode == Mode::Pellet && printer.pellet.fan > 70.0 {
        out.push("The part fan is over 70 %: on the G1 that can pull the nozzle zone into a heating error.".into());
    }
    out
}

/// The G-code of part `part` of a project (the text of its file) for a printer profile's settings
/// (JSON).
pub fn gcode(project: &str, printer: &str, part: usize) -> Result<String, String> {
    let p = prepare(project, printer)?;
    if let Some(first) = p.problems.first().or(p.printer_problems.first()) {
        return Err(format!("Fix the settings first: {}", first.message));
    }
    let printer: Printer = kinds::typed(&p.printer)?;
    let generated = p.kind.generate(&p.settings, &printer)?;
    let Some(one) = generated.parts.get(part) else {
        return Err("That part doesn't exist.".into());
    };
    let mut title = format!("{}: {}", p.kind.info().name, p.project.name);
    if !one.name.is_empty() {
        title = format!("{title} ({})", one.name);
    }
    gcode::write(&one.path, &printer, &title, &one.header).map_err(|e| {
        let mut chars = e.chars();
        match chars.next() {
            Some(c) => format!("No G-code: {}{}.", c.to_lowercase(), chars.as_str()),
            None => "No G-code.".into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const G1: &str = r#"{ "model": "g1" }"#;
    const P1P: &str = r#"{ "model": "p1p" }"#;

    fn text<T: Serialize>(v: &T) -> String {
        serde_json::to_string(v).unwrap()
    }

    #[test]
    fn a_new_project_runs_and_writes_gcode() {
        let p = text(&new_project("spiral").unwrap());
        for printer in [
            text(&new_printer("g1").unwrap()),
            text(&new_printer("p1p").unwrap()),
        ] {
            let r = run(&p, &printer);
            assert!(r.error.is_none() && r.problems.is_empty(), "{r:?}");
            assert_eq!(r.parts.len(), 1);
            assert_eq!(r.parts[0].positions.len(), 3 * r.parts[0].feeds.len());
            assert!(r.parts[0].blocked.is_none());
            let g = gcode(&p, &printer, 0).unwrap();
            assert!(g.starts_with("; ezgc — Spiral: New spiral\n"));
            assert!(gcode(&p, &printer, 1).is_err());
        }
    }

    #[test]
    fn one_project_prints_on_any_printer() {
        let p = text(&new_project("spiral").unwrap());
        assert!(
            gcode(&p, G1, 0)
                .unwrap()
                .contains("_GINGER_EXTRUDER_SET_DOWN S=210")
        );
        assert!(gcode(&p, P1P, 0).unwrap().contains("M104 S205"));
        // A partial profile takes its machine's defaults: the P1P's bed is 256 mm.
        let r = run(&p, P1P);
        assert_eq!(
            schema::get(&r.printer, "bed.width"),
            Some(&serde_json::json!(256.0))
        );
    }

    #[test]
    fn problems_come_back_by_setting_and_nothing_is_built() {
        let r = run(
            r#"{ "kind": "spiral", "settings": { "layerHeight": 0 } }"#,
            r#"{ "pellet": { "up": 400 } }"#,
        );
        assert_eq!(r.problems[0].key, "layerHeight");
        assert_eq!(r.printer_problems[0].key, "pellet.up");
        assert!(r.parts.is_empty());
        // The settings come back complete anyway, so the window can show them.
        assert_eq!(
            schema::get(&r.settings, "shape.circle.radius"),
            Some(&serde_json::json!(30.0))
        );
        let err = gcode(
            r#"{ "kind": "spiral", "settings": { "layerHeight": 0 } }"#,
            G1,
            0,
        )
        .unwrap_err();
        assert!(
            err.starts_with("Fix the settings first: Layer height"),
            "{err}"
        );
    }

    #[test]
    fn bad_files_kinds_and_printers_are_messages() {
        assert!(run("{", G1).error.unwrap().contains("isn't a project"));
        assert!(
            run(r#"{ "kind": "spaceship" }"#, G1)
                .error
                .unwrap()
                .contains("spaceship")
        );
        assert!(
            run(r#"{ "kind": "spiral" }"#, "[]")
                .error
                .unwrap()
                .contains("isn't a printer")
        );
        let r = run(r#"{ "kind": "spiral" }"#, r#"{ "model": "x9" }"#);
        assert!(r.error.unwrap().contains("printer \"x9\""));
        assert!(schema("spaceship").is_err());
        assert!(new_project("spaceship").is_err());
        assert!(new_printer("x9").is_err());
    }

    #[test]
    fn a_part_off_the_bed_is_blocked_with_a_reason() {
        let off = r#"{ "model": "p1p", "centerX": 10 }"#;
        let r = run(r#"{ "kind": "spiral" }"#, off);
        assert!(
            r.parts[0]
                .blocked
                .as_ref()
                .unwrap()
                .contains("leaves the bed")
        );
        let err = gcode(r#"{ "kind": "spiral" }"#, off, 0).unwrap_err();
        assert!(
            err.starts_with("No G-code: the toolpath leaves the bed"),
            "{err}"
        );
    }

    #[test]
    fn printer_warnings() {
        // 32 mm³/s is too much for the P1P.
        let fast = r#"{ "kind": "spiral", "settings": { "flow": 32 } }"#;
        let r = run(fast, P1P);
        assert!(
            r.warnings.iter().any(|w| w.contains("P1P")),
            "{:?}",
            r.warnings
        );
        // Exactly the P1P's 20 mm³/s is fine.
        let r = run(r#"{ "kind": "spiral", "settings": { "flow": 20 } }"#, P1P);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        // 20 mm³/s on the G1: low flow, unless the upper zone runs cooler.
        let r = run(r#"{ "kind": "spiral" }"#, G1);
        assert!(
            r.warnings.iter().any(|w| w.contains("barrel inlet")),
            "{:?}",
            r.warnings
        );
        let r = run(r#"{ "kind": "spiral" }"#, r#"{ "pellet": { "up": 195 } }"#);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn the_material_estimate() {
        let r = run(
            r#"{ "kind": "spiral" }"#,
            r#"{ "material": { "density": 1.24, "price": 20 } }"#,
        );
        let part = &r.parts[0];
        let g = part.mass_g.unwrap();
        assert!((g - part.stats.volume_mm3 * 1.24e-3).abs() < 1e-9);
        assert!((part.cost.unwrap() - g * 0.02).abs() < 1e-9);
    }
}
