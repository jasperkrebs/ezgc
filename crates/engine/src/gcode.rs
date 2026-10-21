//! G-code for a toolpath, in EasyGCode's dialect: the one writer every project kind shares.
//!
//! G-code drives real machines, so this follows EasyGCode's proven output exactly (`gcode.js`):
//! absolute positioning (G90) with relative extrusion (M83); every `G1 E` is the segment's
//! volume (bead area × length × flow) in mm³ for the pellet printer or in mm of filament for the
//! filament printer, times the extrusion multiplier; coordinates with 3 decimals and E with 5,
//! rounded like JavaScript's `toFixed`; F whenever the feed changes (before rounding, as
//! EasyGCode compares it); and the start and end blocks copied verbatim from EasyGCode. The golden
//! tests compare the output with G-code produced by EasyGCode itself (`fixtures/easygcode/`).
//!
//! Before writing anything, the toolpath is checked: every point must be on the bed and inside
//! the build height, every extrusion amount and feed sane, and every temperature one the machine
//! takes. A toolpath that fails gets a message instead of a file.

use crate::geom::dist3;
use crate::num::{f3, f5, fixed, js, round};
use crate::printer::{Filament, Mode, Pellet, Printer};
use crate::toolpath::{Event, MIN_MOVE, Toolpath};
use std::f64::consts::PI;
use std::fmt::Write as _;

/// The most a move may extrude, in beads.
const MAX_EXTRUSION: f64 = 4.0;
/// The largest bead cross-section a move may have, mm².
const MAX_AREA: f64 = 400.0;
/// The fastest an extruding move may be, mm/min.
const MAX_FEED: f64 = 60_000.0;

/// EasyGCode's filament (Marlin) start block.
fn marlin_start(f: &Filament) -> Vec<String> {
    let (bed, nozzle) = (js(f.bed), js(f.nozzle));
    vec![
        "; --- start G-code (filament / Marlin) ---".into(),
        format!("M140 S{bed} ; set bed temp"),
        format!("M104 S{nozzle} ; set hotend temp (heats during bed wait + homing)"),
        format!("M190 S{bed} ; wait for bed temp"),
        "G28 ; home all (incl. mesh bed level)".into(),
        "G90 ; absolute coordinates".into(),
        "G21 ; millimeter units".into(),
        "M83 ; relative extrusion".into(),
        "G0 F3000 X10.0 Y10.0 ; park".into(),
        format!("M109 S{nozzle} ; wait for hotend temp"),
        "G1 F250 E20.788 ; load / prime nozzle".into(),
        "G0 F8000 Z0.3".into(),
        "M220 S100 ; reset speed factor".into(),
        "M221 S100 ; reset extrude factor".into(),
        "; primer lines".into(),
        "G0 F8000 X50.0 Y14.0 Z0.2".into(),
        "G1 F500 X110.0 E12".into(),
        "G1 Y12.0 E0.5".into(),
        "G1 X50.0 E12".into(),
        "G1 Y14.0 E0.5".into(),
        "G1 Y16.0 E0.5".into(),
        "G1 X80.0 E4".into(),
        "G1 Y45.2 E3".into(),
        "M106 S0 ; fan off for the ramp loop".into(),
        "; --- end of start G-code ---".into(),
    ]
}

fn marlin_end(z_lift: f64) -> Vec<String> {
    vec![
        "; --- end G-code (filament / Marlin) ---".into(),
        "M83".into(),
        "G1 E-0.8 F3000 ; retract".into(),
        "G90 ; absolute coordinates".into(),
        format!(
            "G0 Z{} F8000 ; lift clear of the print for finishing",
            f3(z_lift)
        ),
        "M106 S0 ; fan off".into(),
        "M140 S0 ; bed off".into(),
        "M104 S0 ; hotend off".into(),
        "M221 S100 ; reset flow".into(),
        "M900 K0 ; reset linear advance".into(),
        "M84 ; disable steppers".into(),
    ]
}

/// EasyGCode's pellet (Klipper, Ginger G1) start block: it calls Ginger's own macros.
fn klipper_start(p: &Pellet) -> Vec<String> {
    // Bed wait window derived from the target (reproduces 40/90 at bed 50).
    let bed_min = round(p.bed - 10.0).max(0);
    let bed_max = round(p.bed + 40.0);
    let (up, mid, down) = (js(p.up), js(p.mid), js(p.down));
    vec![
        "; --- start G-code (pellet / Klipper) ---".into(),
        "SET_PRESSURE_ADVANCE EXTRUDER=extruder SMOOTH_TIME=0.04".into(),
        "SET_PRESSURE_ADVANCE EXTRUDER=extruder ADVANCE=0.0".into(),
        "_GINGER_BUZZER_TONE_INITIAL".into(),
        format!("_GINGER_BED_HEATING BED_TEMPERATURE={}", js(p.bed)),
        format!("_GINGER_EXTRUDER_SET_UP S={up}"),
        format!("_GINGER_EXTRUDER_SET_MID S={mid}"),
        format!("_GINGER_EXTRUDER_SET_DOWN S={down}"),
        "G28 ; home".into(),
        "BED_MESH_PROFILE LOAD=global".into(),
        "_GINGER_PURGE_PARKING PURGE_LAYER_HEIGHT=2 PURGE_PARKING_SPEED=10000".into(),
        format!("_GINGER_EXTRUDER_WAIT_UP S={up}"),
        format!("_GINGER_EXTRUDER_WAIT_MID S={mid}"),
        format!("_GINGER_EXTRUDER_WAIT_DOWN S={down}"),
        format!("_GINGER_BED_WAIT BED_TEMPERATURE_MIN={bed_min} BED_TEMPERATURE_MAX={bed_max}"),
        "_GINGER_EXTRUDER_MIXING_MULTIPLIER S=1".into(),
        "SET_EXTRUDER_ROTATION_DISTANCE EXTRUDER=extruder DISTANCE=456".into(),
        "SET_EXTRUDER_ROTATION_DISTANCE EXTRUDER=mixing_stepper DISTANCE=8000".into(),
        // PURGE_LENGHT [sic]: the parameter name must match the printer macro.
        format!(
            "_GINGER_PURGE PURGE_LENGHT=400 PURGE_SPEED=500 PURGE_MATERIAL_QUANTITY={}",
            js(p.purge)
        ),
        "G90 ; absolute coordinates".into(),
        "G92 E0".into(),
        "M83 ; relative extrusion".into(),
        "M220 S100 ; reset speed factor".into(),
        "M221 S100 ; reset extrude factor".into(),
        format!(
            "SET_PRESSURE_ADVANCE EXTRUDER=extruder ADVANCE={}",
            js(p.pa)
        ),
        "SET_PRESSURE_ADVANCE EXTRUDER=extruder SMOOTH_TIME=0.5".into(),
        "M106 S0 ; fan off for the ramp loop".into(),
        "_GINGER_BUZZER_TONE_INITIAL".into(),
        "; --- end of start G-code ---".into(),
    ]
}

fn klipper_end(z_lift: f64) -> Vec<String> {
    vec![
        "; --- end G-code (pellet / Klipper) ---".into(),
        "M83".into(),
        "G90 ; absolute coordinates".into(),
        format!(
            "G0 Z{} F3000 ; lift clear of the print for finishing",
            f3(z_lift)
        ),
        "TURN_OFF_HEATERS ; zones + bed off".into(),
        "M106 S0 ; fan off".into(),
        "M84 ; disable steppers".into(),
    ]
}

/// Writes the toolpath as G-code for `printer`. `title` names the project in the first line, and
/// `header` adds the kind's own comment lines (its settings, as EasyGCode lists them).
pub fn write(
    path: &Toolpath,
    printer: &Printer,
    title: &str,
    header: &[String],
) -> Result<String, String> {
    check(path, printer)?;
    let mode = printer.mode();
    let mult = if printer.multiplier > 0.0 {
        printer.multiplier
    } else {
        1.0
    };
    let dia = if printer.filament.diameter > 0.0 {
        printer.filament.diameter
    } else {
        1.75
    };
    let e_factor = mult
        / match mode {
            Mode::Filament => PI * (dia / 2.0) * (dia / 2.0),
            Mode::Pellet => 1.0,
        };
    let fan_pwm = round(printer.fan().clamp(0.0, 100.0) * 2.55);
    let travel_feed = printer.travel_feed;
    let (cx, cy) = (printer.center_x, printer.center_y);
    let start_end = printer.include_start_end;

    let mut lines: Vec<String> = Vec::with_capacity(path.moves.len() + 80);
    lines.push(format!(
        "; EasyGCode 2 — {}",
        title.replace(['\n', '\r'], " ")
    ));
    lines.extend(
        header
            .iter()
            .map(|l| format!("; {}", l.replace(['\n', '\r'], " "))),
    );
    lines.push(match mode {
        Mode::Filament => format!(
            "; printer=filament multiplier={} filamentDiameter={} (E in mm of filament)",
            js(mult),
            js(dia)
        ),
        Mode::Pellet => format!(
            "; printer=pellet multiplier={} (E in mm^3, volumetric)",
            js(mult)
        ),
    });
    if start_end {
        lines.extend(match mode {
            Mode::Filament => marlin_start(&printer.filament),
            Mode::Pellet => klipper_start(&printer.pellet),
        });
    }
    lines.push("G90 ; absolute positioning".into());
    lines.push("M83 ; relative extrusion".into());

    let mut events = path.events.iter().peekable();
    let mut flush = |lines: &mut Vec<String>, upto: usize| {
        while let Some((_, e)) = events.next_if(|(at, _)| *at <= upto) {
            match e {
                Event::Comment(text) => {
                    lines.push(format!("; {}", text.replace(['\n', '\r'], " ")))
                }
                // Only with start/end blocks: they turn the fan off again.
                Event::FanOn if start_end && fan_pwm > 0 => {
                    lines.push(format!("M106 S{fan_pwm} ; part cooling fan on"));
                }
                Event::FanOn => {}
                Event::Fan(level) if start_end => {
                    let pwm = round(level.clamp(0.0, 100.0) * 2.55);
                    lines.push(if pwm > 0 {
                        format!("M106 S{pwm} ; part fan {} %", js(*level))
                    } else {
                        "M107 ; part fan off".into()
                    });
                }
                Event::Fan(_) => {}
                Event::Dwell(seconds) => {
                    lines.push(format!("G4 P{} ; dwell", round(seconds * 1000.0)))
                }
            }
        }
    };

    let mut prev: Option<[f64; 3]> = None;
    // The feed as it was last written, before rounding: like EasyGCode, F is written whenever
    // the feed changes, even when it rounds to the same number.
    let mut last_feed: Option<f64> = None;
    let mut first_extrude = true;
    let mut max_z = 0.0f64;
    for (i, m) in path.moves.iter().enumerate() {
        flush(&mut lines, i);
        let cur = [m.to[0] + cx, m.to[1] + cy, m.to[2]];
        match prev {
            Some(from) if !m.travel => {
                let seg = dist3(from, cur);
                prev = Some(cur);
                if seg < MIN_MOVE {
                    continue;
                }
                // Same operation order as EasyGCode (area · length · ramp), so both round alike.
                let volume = m.area * seg * m.flow;
                let mut line = format!(
                    "G1 X{} Y{} Z{} E{}",
                    f3(cur[0]),
                    f3(cur[1]),
                    f3(cur[2]),
                    f5(volume * e_factor)
                );
                if last_feed != Some(m.feed) || first_extrude {
                    let _ = write!(line, " F{}", round(m.feed));
                    last_feed = Some(m.feed);
                }
                lines.push(line);
                first_extrude = false;
                max_z = max_z.max(cur[2]);
            }
            // A travel, or the very first move (always reached by travelling).
            _ => {
                lines.push(format!(
                    "G0 X{} Y{} Z{} F{}",
                    f3(cur[0]),
                    f3(cur[1]),
                    f3(cur[2]),
                    round(travel_feed)
                ));
                last_feed = Some(travel_feed);
                prev = Some(cur);
            }
        }
    }
    flush(&mut lines, usize::MAX);
    if start_end {
        // Lift clear of the print, but never past the build height (Klipper refuses such a move).
        let lift = (max_z + printer.end_lift.max(0.0)).min(printer.machine().bed[2].max(max_z));
        lines.extend(match mode {
            Mode::Filament => marlin_end(lift),
            Mode::Pellet => klipper_end(lift),
        });
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

/// Refuses toolpaths a machine shouldn't run: off the bed, out of reach, with nonsense
/// extrusion or feeds, or with temperatures the machine refuses.
pub fn check(path: &Toolpath, p: &Printer) -> Result<(), String> {
    if path.moves.is_empty() {
        return Err("the toolpath is empty".into());
    }
    let machine = p.machine();
    let settings = [
        p.multiplier,
        p.end_lift,
        p.travel_feed,
        p.center_x,
        p.center_y,
        p.filament.diameter,
        p.filament.nozzle,
        p.filament.bed,
        p.filament.fan,
        p.pellet.up,
        p.pellet.mid,
        p.pellet.down,
        p.pellet.bed,
        p.pellet.pa,
        p.pellet.purge,
        p.pellet.fan,
    ];
    if !settings.iter().all(|v| v.is_finite()) {
        return Err("a printer setting is not a number".into());
    }
    if !(p.travel_feed > 0.0 && p.travel_feed <= MAX_FEED) {
        return Err(format!(
            "the travel feed must be between 0 and {MAX_FEED} mm/min"
        ));
    }
    if !(p.multiplier > 0.0 && p.multiplier <= 5.0) {
        return Err("the extrusion multiplier must be between 0 and 5".into());
    }
    match machine.mode {
        Mode::Filament => {
            if !(p.filament.diameter > 0.5 && p.filament.diameter < 5.0) {
                return Err("the filament diameter must be between 0.5 and 5 mm".into());
            }
            if !(0.0..=machine.max_temp).contains(&p.filament.nozzle) {
                return Err(format!(
                    "the nozzle is set to {} °C; the {} goes up to {} °C",
                    js(p.filament.nozzle),
                    machine.name,
                    js(machine.max_temp)
                ));
            }
            if !(0.0..=machine.max_bed).contains(&p.filament.bed) {
                return Err(format!(
                    "the bed is set to {} °C; the bed goes up to {} °C",
                    js(p.filament.bed),
                    js(machine.max_bed)
                ));
            }
        }
        Mode::Pellet => {
            let (lo, hi) = (machine.min_extrude_temp, machine.max_temp);
            for (name, t) in [
                ("upper", p.pellet.up),
                ("middle", p.pellet.mid),
                ("lower", p.pellet.down),
            ] {
                if !(lo..=hi).contains(&t) {
                    return Err(format!(
                        "the {name} zone is set to {} °C; the printer takes {} to {} °C",
                        js(t),
                        js(lo),
                        js(hi)
                    ));
                }
            }
            if !(0.0..=machine.max_bed).contains(&p.pellet.bed) {
                return Err(format!(
                    "the bed is set to {} °C; the bed goes up to {} °C",
                    js(p.pellet.bed),
                    js(machine.max_bed)
                ));
            }
        }
    }
    let [bx, by, bz] = machine.bed;
    for (i, m) in path.moves.iter().enumerate() {
        let (x, y, z) = (m.to[0] + p.center_x, m.to[1] + p.center_y, m.to[2]);
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err("the toolpath has a point that is not a number".into());
        }
        if x < 0.0 || x > bx || y < 0.0 || y > by {
            return Err(format!(
                "the toolpath leaves the bed at X{} Y{} (the bed is {} × {} mm, the part's centre at X{} Y{}); move it or make it smaller",
                f3(x),
                f3(y),
                js(bx),
                js(by),
                js(p.center_x),
                js(p.center_y)
            ));
        }
        if z < 0.0 || z > bz {
            return Err(format!(
                "the toolpath goes to Z{} (the build height is {} mm)",
                f3(z),
                js(bz)
            ));
        }
        if m.travel {
            continue;
        }
        if !(m.flow.is_finite() && (0.0..=MAX_EXTRUSION).contains(&m.flow)) {
            return Err(format!(
                "move {i} extrudes {} beads; it must be between 0 and {MAX_EXTRUSION}",
                m.flow
            ));
        }
        if !(m.area.is_finite() && (0.0..=MAX_AREA).contains(&m.area)) {
            return Err(format!("move {i} has a bead of {} mm²", fixed(m.area, 3)));
        }
        if !(m.feed.is_finite() && m.feed > 0.0 && m.feed <= MAX_FEED) {
            return Err(format!(
                "move {i} has a feed of {} mm/min; it must be between 0 and {MAX_FEED}",
                m.feed
            ));
        }
    }
    for (_, e) in &path.events {
        match e {
            Event::Dwell(s) if !(s.is_finite() && (0.0..=600.0).contains(s)) => {
                return Err(format!("a dwell of {s} s; it must be between 0 and 600 s"));
            }
            Event::Fan(f) if !(f.is_finite() && (0.0..=100.0).contains(f)) => {
                return Err(format!(
                    "a fan level of {f} %; it must be between 0 and 100"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bead::bead_area;

    /// A square ring of side 10·√2 around the origin, 0.6 mm up.
    fn ring(area: f64, feed: f64) -> Toolpath {
        let mut p = Toolpath::new();
        let pts = [
            [10.0, 0.0],
            [0.0, 10.0],
            [-10.0, 0.0],
            [0.0, -10.0],
            [10.0, 0.0],
        ];
        p.travel([10.0, 0.0, 0.6]);
        for q in &pts[1..] {
            p.extrude([q[0], q[1], 0.6], area, 1.0, feed);
        }
        p
    }

    fn bare() -> Printer {
        Printer {
            include_start_end: false,
            ..Printer::default()
        }
    }

    #[test]
    fn a_ring_prints_with_volumetric_e_and_travel() {
        let area = bead_area(2.0, 0.6);
        let out = write(&ring(area, 1200.0), &bare(), "ring", &[]).unwrap();
        let body: Vec<&str> = out.lines().filter(|l| !l.starts_with(';')).collect();
        assert_eq!(body[0], "G90 ; absolute positioning");
        assert_eq!(body[2], "G0 X135.000 Y125.000 Z0.600 F6000");
        // A side of the square: 10·√2 mm × bead area (2 − 0.6)·0.6 + π·0.3².
        let e = 10.0 * 2f64.sqrt() * ((2.0 - 0.6) * 0.6 + PI * 0.3 * 0.3);
        assert_eq!(
            body[3],
            format!("G1 X125.000 Y135.000 Z0.600 E{} F1200", fixed(e, 5))
        );
        assert!(!body[4].contains('F'));
        assert_eq!(body.len(), 3 + 4);
    }

    #[test]
    fn filament_e_is_in_mm_of_filament() {
        let printer = Printer {
            model: "p1p".into(),
            ..bare()
        };
        let area = bead_area(0.6, 0.3);
        let out = write(&ring(area, 1800.0), &printer, "ring", &[]).unwrap();
        let g1 = out.lines().find(|l| l.starts_with("G1")).unwrap();
        let e = area * 10.0 * 2f64.sqrt() / (PI * 0.875 * 0.875);
        assert!(g1.contains(&format!("E{}", fixed(e, 5))), "{g1}");
    }

    #[test]
    fn unsafe_toolpaths_are_refused() {
        let area = bead_area(2.0, 0.6);
        let off_bed = Printer {
            center_x: 5.0,
            ..bare()
        };
        let err = write(&ring(area, 1200.0), &off_bed, "x", &[]).unwrap_err();
        assert!(err.contains("leaves the bed"), "{err}");
        let p1p = Printer {
            model: "p1p".into(),
            center_x: 250.0,
            ..bare()
        };
        assert!(
            write(&ring(area, 1200.0), &p1p, "x", &[])
                .unwrap_err()
                .contains("leaves the bed")
        );
        assert!(write(&ring(area, f64::NAN), &bare(), "x", &[]).is_err());
        assert!(write(&ring(f64::INFINITY, 1200.0), &bare(), "x", &[]).is_err());
        let hot = Printer {
            pellet: Pellet {
                down: 300.0,
                ..Pellet::default()
            },
            ..bare()
        };
        assert!(
            write(&ring(area, 1200.0), &hot, "x", &[])
                .unwrap_err()
                .contains("lower zone")
        );
        assert!(write(&Toolpath::new(), &bare(), "x", &[]).is_err());
    }

    #[test]
    fn start_and_end_blocks_are_easygcodes() {
        let area = bead_area(2.0, 0.6);
        let pellet = write(&ring(area, 1200.0), &Printer::default(), "x", &[]).unwrap();
        assert!(
            pellet.contains("_GINGER_BED_WAIT BED_TEMPERATURE_MIN=40 BED_TEMPERATURE_MAX=90\n")
        );
        assert!(pellet.ends_with(
            "G0 Z50.600 F3000 ; lift clear of the print for finishing\nTURN_OFF_HEATERS ; zones + bed off\nM106 S0 ; fan off\nM84 ; disable steppers\n"
        ));
        let filament = Printer {
            model: "p1p".into(),
            ..Printer::default()
        };
        let mut path = ring(area, 1200.0);
        path.events.push((2, Event::FanOn));
        let text = write(&path, &filament, "x", &[]).unwrap();
        assert!(text.contains("M104 S205 ; set hotend temp (heats during bed wait + homing)\n"));
        assert!(text.contains("M106 S255 ; part cooling fan on\n"));
        // The end lift stops at the build height (the P1P's is 256 mm).
        let tall = Printer {
            model: "p1p".into(),
            end_lift: 1000.0,
            ..Printer::default()
        };
        let text = write(&ring(area, 1200.0), &tall, "x", &[]).unwrap();
        assert!(text.contains("G0 Z256.000 F8000 ; lift clear of the print for finishing\n"));
    }

    #[test]
    fn events_land_between_the_right_moves() {
        let mut p = Toolpath::new();
        p.comment("start");
        p.travel([0.0, 0.0, 0.3]);
        p.extrude([5.0, 0.0, 0.3], 0.5, 1.0, 600.0);
        p.event(Event::Dwell(1.5));
        p.event(Event::Fan(40.0));
        p.extrude([5.0, 5.0, 0.3], 0.5, 1.0, 600.0);
        p.comment("done");
        let text = write(&p, &Printer::default(), "x", &["kind line".into()]).unwrap();
        let body: Vec<&str> = text.lines().collect();
        let at = |s: &str| body.iter().position(|l| l.starts_with(s)).unwrap();
        assert!(at("; start") < at("G0 X125.000"));
        assert!(at("G1 X130.000 Y125.000") < at("G4 P1500"));
        assert!(at("G4 P1500") < at("M106 S102 ; part fan 40 %"));
        assert!(at("M106 S102") < at("G1 X130.000 Y130.000"));
        assert!(at("G1 X130.000 Y130.000") < at("; done"));
        assert!(text.contains("\n; kind line\n"));
    }
}
