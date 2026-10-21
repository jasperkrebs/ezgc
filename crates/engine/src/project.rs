//! The project file: one JSON document per project, as EasyGCode saved its settings.
//!
//! ```json
//! { "app": "easygcode", "format": 2, "kind": "vase", "kindVersion": 1,
//!   "name": "Tall vase", "notes": "",
//!   "settings": { "shape": { "kind": "circle", "circle": { "radius": 30 } }, "height": 120 },
//!   "printer": { "model": "g1", "pellet": { "up": 200 } } }
//! ```
//!
//! `settings` holds the kind's settings and `printer` the printer's, each as complete or as
//! partial as you like: what's missing takes its default ([`crate::schema::normalize`]), so a
//! project saved before a setting existed opens with it at the value that prints as before. The
//! app adds its own bookkeeping (an id, dates) next to these; the engine ignores it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const APP: &str = "easygcode";
/// The file format. EasyGCode 1 wrote `{ "app": "easygcode", "version": 1, "settings": … }`.
pub const FORMAT: u32 = 2;
/// Project files are small; anything bigger isn't one.
pub const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    #[serde(default = "app")]
    pub app: String,
    #[serde(default = "format")]
    pub format: u32,
    pub kind: String,
    #[serde(default)]
    pub kind_version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub settings: Value,
    #[serde(default)]
    pub printer: Value,
}

fn app() -> String {
    APP.into()
}

fn format() -> u32 {
    FORMAT
}

/// Reads a project file. Every way it can be wrong becomes a message.
pub fn parse(text: &str) -> Result<Project, String> {
    if text.len() > MAX_BYTES {
        return Err("That file is too big to be a project.".into());
    }
    let v: Value =
        serde_json::from_str(text).map_err(|e| format!("That isn't a project file ({e})."))?;
    if !v.is_object() {
        return Err("That isn't a project file.".into());
    }
    if v.get("format").is_none() && v.get("version").is_some() {
        return Err("This is an EasyGCode 1 settings file (the old app's tabs). Opening those isn't built yet.".into());
    }
    match v.get("app").and_then(Value::as_str) {
        None | Some(APP) => {}
        Some(other) => return Err(format!("That file is from another app ({other}).")),
    }
    let p: Project =
        serde_json::from_value(v).map_err(|e| format!("That project file is damaged ({e})."))?;
    if p.format > FORMAT {
        return Err(
            "That project was saved by a newer EasyGCode: reload the app to update it.".into(),
        );
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_file_opens() {
        let p = parse(r#"{ "kind": "vase" }"#).unwrap();
        assert_eq!(p.app, "easygcode");
        assert_eq!(p.format, FORMAT);
        assert_eq!(p.settings, Value::Null);
    }

    #[test]
    fn wrong_files_are_messages() {
        assert!(parse("not json").unwrap_err().contains("isn't a project"));
        assert!(parse("[1, 2]").unwrap_err().contains("isn't a project"));
        assert!(
            parse(r#"{ "app": "easygcode", "version": 1, "settings": {} }"#)
                .unwrap_err()
                .contains("EasyGCode 1")
        );
        assert!(
            parse(r#"{ "app": "other", "kind": "vase" }"#)
                .unwrap_err()
                .contains("another app")
        );
        assert!(
            parse(r#"{ "format": 99, "kind": "vase" }"#)
                .unwrap_err()
                .contains("newer")
        );
        assert!(
            parse(r#"{ "name": "no kind" }"#)
                .unwrap_err()
                .contains("damaged")
        );
        assert!(
            parse(&" ".repeat(MAX_BYTES + 1))
                .unwrap_err()
                .contains("too big")
        );
    }
}
