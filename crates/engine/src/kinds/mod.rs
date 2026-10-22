//! Project kinds: what a project can be.
//!
//! There is one today, and on purpose: the [`spiral`]. Almost every project is one continuous
//! spiral, so they all grow from it, its settings switching rules and features on (a flat bottom
//! that turns into the wall, a body profile, patterns, a shaped top). A project that genuinely isn't
//! a spiral (the bend stool's concentric rings with legs) becomes a kind of its own here.
//!
//! A kind is a small Rust module: its settings (a struct with defaults), the settings window for
//! them ([`crate::schema`]), and a `generate` that composes toolbox steps into toolpaths. It never
//! writes G-code and never touches the UI. Every project file names its kind.

use crate::printer::Printer;
use crate::schema::Group;
use crate::toolpath::Toolpath;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub mod spiral;

/// What the app shows about a kind.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Bumped when the kind's settings change shape; [`Kind::migrate`] carries older files over.
    pub version: u32,
}

/// One printed part: its toolpath, and the comment lines its G-code starts with.
pub struct Part {
    /// Empty for a kind that prints one part.
    pub name: String,
    pub path: Toolpath,
    pub header: Vec<String>,
}

/// A live note under one setting, like EasyGCode's hints ("= 1108 mm/min at this bead").
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hint {
    pub key: String,
    pub text: String,
}

pub struct Generated {
    pub parts: Vec<Part>,
    pub warnings: Vec<String>,
    pub hints: Vec<Hint>,
}

pub trait Kind: Sync {
    fn info(&self) -> Info;
    /// The settings window.
    fn schema(&self) -> Vec<Group>;
    /// Every setting with its default (a file leaving one out gets this).
    fn defaults(&self) -> Value;
    /// Builds the toolpaths from complete, checked settings ([`crate::schema::normalize`]).
    fn generate(&self, settings: &Value, printer: &Printer) -> Result<Generated, String>;
    /// Carries settings saved by an older version of the kind over to this one.
    fn migrate(&self, _settings: &mut Value, _from_version: u32) {}
}

static KINDS: [&dyn Kind; 1] = [&spiral::Spiral];

pub fn all() -> &'static [&'static dyn Kind] {
    &KINDS
}

pub fn find(id: &str) -> Option<&'static dyn Kind> {
    KINDS.iter().copied().find(|k| k.info().id == id)
}

/// Settings as a kind's own struct.
pub fn typed<S: DeserializeOwned>(settings: &Value) -> Result<S, String> {
    serde_json::from_value(settings.clone())
        .map_err(|e| format!("internal: settings don't fit the kind ({e})"))
}

/// A struct's defaults as JSON.
pub fn to_value<S: Serialize>(settings: &S) -> Value {
    serde_json::to_value(settings).unwrap_or(Value::Null)
}
