//! Snapshot tests: ezgc's own G-code, kept in `fixtures/snapshots/` and reviewed when it changes.
//!
//! G-code drives real machines, so any change to it should be one somebody meant. Each case is a
//! project and a printer profile (`<case>.project.json`, `<case>.printer.json`) and the G-code ezgc
//! writes for them (`<case>.gcode`); the test fails when the G-code differs. After an intended
//! change, rewrite the snapshots and review the diff before committing:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p ezgc-engine snapshots
//! ```
//!
//! The browser's engine is checked against the same files (`scripts/wasm.test.mjs`), so the
//! WebAssembly build is held to exactly the same output.

use crate::gcode;
use std::fs;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/snapshots")
}

#[test]
fn snapshots() {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let mut cases: Vec<String> = fs::read_dir(dir())
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".project.json").map(str::to_string)
        })
        .collect();
    cases.sort();
    assert!(cases.len() >= 4, "{cases:?}");
    let mut changed = Vec::new();
    for case in &cases {
        let read = |ext: &str| fs::read_to_string(dir().join(format!("{case}.{ext}"))).unwrap();
        let got = gcode(&read("project.json"), &read("printer.json"), 0)
            .unwrap_or_else(|e| panic!("{case}: {e}"));
        let path = dir().join(format!("{case}.gcode"));
        let want = fs::read_to_string(&path).unwrap_or_default();
        if got == want {
            continue;
        }
        if update {
            fs::write(&path, &got).unwrap();
        } else {
            let line = got
                .lines()
                .zip(want.lines())
                .position(|(a, b)| a != b)
                .unwrap_or(got.lines().count().min(want.lines().count()));
            changed.push(format!(
                "{case}: line {}: {:?} (snapshot: {:?})",
                line + 1,
                got.lines().nth(line).unwrap_or(""),
                want.lines().nth(line).unwrap_or("")
            ));
        }
    }
    assert!(
        changed.is_empty(),
        "the G-code changed; if that's meant, run UPDATE_SNAPSHOTS=1 cargo test -p ezgc-engine snapshots and review the diff:\n{}",
        changed.join("\n")
    );
}
