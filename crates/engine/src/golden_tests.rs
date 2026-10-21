//! Golden tests: EasyGCode 2's G-code against G-code produced by EasyGCode itself.
//!
//! The fixtures in `fixtures/easygcode/` come from running EasyGCode's own `gcode.js` headlessly
//! (`scripts/easygcode-fixtures.cjs`; the commit is in `SOURCE`): its coat hanger with every
//! extra off, which is the vase kind. Each case opens the same settings as a project file
//! (`<case>.project.json`), writes its G-code, and compares it with EasyGCode's command by command:
//! every command must be byte-identical (comments aside), and the start and end blocks too,
//! comments included.

use crate::gcode;

const CASES: [(&str, &str, &str); 6] = [
    (
        "vase-circle",
        include_str!("../../../fixtures/easygcode/vase-circle.project.json"),
        include_str!("../../../fixtures/easygcode/vase-circle.gcode"),
    ),
    (
        "vase-rect-filament",
        include_str!("../../../fixtures/easygcode/vase-rect-filament.project.json"),
        include_str!("../../../fixtures/easygcode/vase-rect-filament.gcode"),
    ),
    (
        "vase-star-bare",
        include_str!("../../../fixtures/easygcode/vase-star-bare.project.json"),
        include_str!("../../../fixtures/easygcode/vase-star-bare.gcode"),
    ),
    (
        "vase-ellipse-fan",
        include_str!("../../../fixtures/easygcode/vase-ellipse-fan.project.json"),
        include_str!("../../../fixtures/easygcode/vase-ellipse-fan.gcode"),
    ),
    (
        "vase-polygon",
        include_str!("../../../fixtures/easygcode/vase-polygon.project.json"),
        include_str!("../../../fixtures/easygcode/vase-polygon.gcode"),
    ),
    (
        "vase-squircle",
        include_str!("../../../fixtures/easygcode/vase-squircle.project.json"),
        include_str!("../../../fixtures/easygcode/vase-squircle.gcode"),
    ),
];

/// The commands, without comments.
fn commands(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| l.split(';').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect()
}

/// The lines from the one starting with `from` up to the one starting with `to` (inclusive), or
/// to the end.
fn block<'a>(text: &'a str, from: &str, to: Option<&str>) -> Vec<&'a str> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|l| l.starts_with(from)) else {
        return Vec::new();
    };
    let end = match to {
        Some(to) => lines
            .iter()
            .skip(start)
            .position(|l| l.starts_with(to))
            .map_or(lines.len(), |i| start + i + 1),
        None => lines.len(),
    };
    lines[start..end].to_vec()
}

#[test]
fn the_vase_prints_like_easygcodes_coat_hanger() {
    for (name, project, want) in CASES {
        let got = gcode(project, 0).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (g, w) = (commands(&got), commands(want));
        for (i, (a, b)) in g.iter().zip(&w).enumerate() {
            assert_eq!(a, b, "{name}: command {i} differs");
        }
        assert_eq!(g.len(), w.len(), "{name}: command count");
        // The start and end blocks, comments included.
        for (from, to) in [
            ("; --- start G-code", Some("; --- end of start G-code ---")),
            ("; --- end G-code", None),
        ] {
            assert_eq!(
                block(&got, from, to),
                block(want, from, to),
                "{name}: {from}"
            );
        }
    }
}

#[test]
fn every_case_covers_something() {
    // The cases between them use both printers, both speed modes, both tops, every seam side,
    // both directions and every shape, with and without start/end blocks.
    let all: String = CASES.iter().map(|(_, p, _)| *p).collect();
    for needle in [
        "\"p1p\"",
        "\"g1\"",
        "\"flow\"",
        "\"feed\"",
        "\"open\"",
        "\"flat\"",
        "\"front\"",
        "\"back\"",
        "\"left\"",
        "\"right\"",
        "\"cw\"",
        "\"ccw\"",
        "\"circle\"",
        "\"roundedRect\"",
        "\"ellipse\"",
        "\"polygon\"",
        "\"star\"",
        "\"squircle\"",
        "\"includeStartEnd\": false",
    ] {
        assert!(all.contains(needle), "no case has {needle}");
    }
}
