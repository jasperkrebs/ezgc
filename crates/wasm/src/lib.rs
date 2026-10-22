//! WebAssembly facade for the web app (`web/js/engine.worker.js` loads it in a Web Worker).
//!
//! Deliberately thin: JSON text in, JSON text (and typed arrays for the preview) out. Every call
//! maps to one function of `ezgc_engine`; nothing here makes decisions.

use ezgc_engine as engine;
use js_sys::{Array, Float32Array, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

/// Engine version, shown in the app.
#[wasm_bindgen(js_name = engineVersion)]
pub fn engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Every kind of project: `[{ id, name, description, version }]` as JSON.
#[wasm_bindgen]
pub fn kinds() -> Result<String, JsError> {
    json(&engine::kinds())
}

/// A kind's settings window and defaults: `{ info, groups, defaults }` as JSON.
#[wasm_bindgen]
pub fn schema(kind: &str) -> Result<String, JsError> {
    engine::schema(kind)
        .map_err(|e| JsError::new(&e))
        .and_then(|s| json(&s))
}

/// The printer's settings window and the machines a profile can be based on, as JSON.
#[wasm_bindgen(js_name = printerSchema)]
pub fn printer_schema() -> Result<String, JsError> {
    json(&engine::printer_schema())
}

/// A new project of a kind (a project file, as JSON).
#[wasm_bindgen(js_name = newProject)]
pub fn new_project(kind: &str) -> Result<String, JsError> {
    engine::new_project(kind)
        .map_err(|e| JsError::new(&e))
        .and_then(|p| json(&p))
}

/// Checks a project (the text of its file) and a printer profile's settings (JSON), and builds
/// the toolpaths. Returns `{ meta, parts }`:
/// `meta` is the engine's `Run` as JSON (settings, problems, warnings, hints, each part's stats),
/// and `parts` has each part's preview as typed arrays: `{ positions, feeds, travel }`. Never
/// throws for a bad project: its problems are in `meta`.
#[wasm_bindgen]
pub fn run(project: &str, printer: &str) -> Result<Object, JsError> {
    let r = engine::run(project, printer);
    let out = Object::new();
    set(
        &out,
        "meta",
        &JsValue::from_str(&serde_json::to_string(&r).map_err(internal)?),
    )?;
    let parts = Array::new();
    for p in &r.parts {
        let part = Object::new();
        set(
            &part,
            "positions",
            &Float32Array::from(p.positions.as_slice()),
        )?;
        set(&part, "feeds", &Float32Array::from(p.feeds.as_slice()))?;
        set(&part, "travel", &Uint8Array::from(p.travel.as_slice()))?;
        parts.push(&part);
    }
    set(&out, "parts", &parts)?;
    Ok(out)
}

/// A new printer profile's settings, based on a machine (`"g1"`, `"p1p"`), as JSON.
#[wasm_bindgen(js_name = newPrinter)]
pub fn new_printer(model: &str) -> Result<String, JsError> {
    engine::new_printer(model)
        .map_err(|e| JsError::new(&e))
        .and_then(|p| json(&p))
}

/// The G-code of one part of a project for a printer profile. Throws a readable message when it
/// can't be written.
#[wasm_bindgen]
pub fn gcode(project: &str, printer: &str, part: usize) -> Result<String, JsError> {
    engine::gcode(project, printer, part).map_err(|e| JsError::new(&e))
}

fn json<T: serde::Serialize>(v: &T) -> Result<String, JsError> {
    serde_json::to_string(v).map_err(internal)
}

fn internal(e: serde_json::Error) -> JsError {
    JsError::new(&format!("internal: {e}"))
}

fn set(target: &Object, key: &str, value: &JsValue) -> Result<(), JsError> {
    Reflect::set(target, &JsValue::from_str(key), value)
        .map(|_| ())
        .map_err(|_| JsError::new("internal: could not build the result object"))
}
