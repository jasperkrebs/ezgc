//! Settings schemas: what a settings window shows, and the checks every value gets.
//!
//! A project kind (and the printer) describes its settings as groups of fields; the web app draws
//! the window from that description, so a new setting is one line in the kind, never new HTML.
//! Values live in a plain JSON object (the project file's `settings`), addressed by dotted keys
//! (`shape.circle.radius`). [`normalize`] fills in what a file leaves out from the defaults (so
//! files from before a setting existed keep printing as they did), reads numbers typed as text,
//! and checks every visible field against its range: bad input becomes a message, never a crash.

use serde::Serialize;
use serde_json::{Map, Value};

/// What kind of input a field is.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Input {
    /// A number in `unit`, between `min` and `max`; `integer` ones are rounded.
    Number {
        unit: &'static str,
        min: f64,
        max: f64,
        step: f64,
        integer: bool,
    },
    Toggle,
    Choice {
        options: Vec<Choice>,
    },
    /// A list of points (x, y), `min` to `max` of them, each value inside its axis' range: a curve
    /// drawn through them (a body profile).
    Points {
        x: Axis,
        y: Axis,
        min: usize,
        max: usize,
    },
}

/// One coordinate of a list of points.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Axis {
    pub label: &'static str,
    pub unit: &'static str,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

/// Shown only while the setting at `key` has one of these values.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct When {
    pub key: String,
    pub any_of: Vec<Value>,
    /// Or shown while this holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub or: Option<Box<When>>,
}

impl When {
    fn holds(&self, values: &Value) -> bool {
        get(values, &self.key).is_some_and(|v| self.any_of.contains(v))
            || self.or.as_ref().is_some_and(|w| w.holds(values))
    }

    fn on(key: impl Into<String>) -> When {
        When {
            key: key.into(),
            any_of: vec![Value::Bool(true)],
            or: None,
        }
    }

    fn any(key: impl Into<String>, values: &[&'static str]) -> When {
        When {
            key: key.into(),
            any_of: values.iter().map(|v| Value::from(*v)).collect(),
            or: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    pub key: String,
    pub label: &'static str,
    pub input: Input,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
}

impl Field {
    pub fn help(mut self, text: &'static str) -> Field {
        self.help = Some(text);
        self
    }

    /// Shown only while the choice at `key` is one of `values`.
    pub fn when(mut self, key: impl Into<String>, values: &[&'static str]) -> Field {
        self.when = Some(When::any(key, values));
        self
    }

    /// Shown only while the toggle at `key` is on.
    pub fn when_on(mut self, key: impl Into<String>) -> Field {
        self.when = Some(When::on(key));
        self
    }

    pub fn step(mut self, step: f64) -> Field {
        if let Input::Number { step: s, .. } = &mut self.input {
            *s = step;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    pub title: &'static str,
    pub fields: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
    /// Opened when the window is first drawn (the others start folded).
    pub open: bool,
}

impl Group {
    pub fn open(mut self) -> Group {
        self.open = true;
        self
    }

    pub fn when(mut self, key: impl Into<String>, values: &[&'static str]) -> Group {
        self.when = Some(When::any(key, values));
        self
    }

    pub fn when_on(mut self, key: impl Into<String>) -> Group {
        self.when = Some(When::on(key));
        self
    }

    /// Shown while the toggle at `a` or the one at `b` is on.
    pub fn when_either_on(mut self, a: impl Into<String>, b: impl Into<String>) -> Group {
        self.when = Some(When {
            or: Some(Box::new(When::on(b))),
            ..When::on(a)
        });
        self
    }
}

pub fn group(title: &'static str, fields: Vec<Field>) -> Group {
    Group {
        title,
        fields,
        when: None,
        open: false,
    }
}

/// A slider step for a range: about a hundredth of it, rounded down to a power of ten.
fn auto_step(min: f64, max: f64) -> f64 {
    let span = (max - min).abs();
    if span > 0.0 {
        10f64.powi((span / 100.0).log10().floor() as i32)
    } else {
        1.0
    }
}

fn field(key: impl Into<String>, label: &'static str, input: Input) -> Field {
    Field {
        key: key.into(),
        label,
        input,
        help: None,
        when: None,
    }
}

pub fn number(
    key: impl Into<String>,
    label: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
) -> Field {
    field(
        key,
        label,
        Input::Number {
            unit,
            min,
            max,
            step: auto_step(min, max),
            integer: false,
        },
    )
}

pub fn integer(key: impl Into<String>, label: &'static str, min: f64, max: f64) -> Field {
    field(
        key,
        label,
        Input::Number {
            unit: "",
            min,
            max,
            step: 1.0,
            integer: true,
        },
    )
}

pub fn toggle(key: impl Into<String>, label: &'static str) -> Field {
    field(key, label, Input::Toggle)
}

pub fn choice(
    key: impl Into<String>,
    label: &'static str,
    options: &[(&'static str, &'static str)],
) -> Field {
    field(
        key,
        label,
        Input::Choice {
            options: options
                .iter()
                .map(|(value, label)| Choice { value, label })
                .collect(),
        },
    )
}

/// A list of points (see [`Input::Points`]); values are `[[x, y], …]`.
pub fn points(
    key: impl Into<String>,
    label: &'static str,
    x: Axis,
    y: Axis,
    min: usize,
    max: usize,
) -> Field {
    field(key, label, Input::Points { x, y, min, max })
}

/// A problem with one setting: shown next to the field, and nothing is generated until it's fixed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    pub key: String,
    pub message: String,
}

/// The value at a dotted key.
pub fn get<'a>(values: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(values, |v, part| v.get(part))
}

fn get_mut<'a>(values: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    key.split('.').try_fold(values, |v, part| v.get_mut(part))
}

/// `given` laid over `defaults`: objects merge key by key; a value replaces the default only if
/// it's the same kind (a number for a number, …), or a number written as text ("1,5" too, as
/// EasyGCode's own settings files and phones in comma-decimal languages write them). Keys the
/// defaults don't have are dropped.
fn merge(defaults: &Value, given: &Value) -> Value {
    match (defaults, given) {
        (Value::Object(d), Value::Object(g)) => {
            let mut out = Map::new();
            for (k, dv) in d {
                let v = match g.get(k) {
                    Some(gv) => merge(dv, gv),
                    None => dv.clone(),
                };
                out.insert(k.clone(), v);
            }
            Value::Object(out)
        }
        (Value::Number(_), Value::Number(_)) => given.clone(),
        (Value::Number(_), Value::String(s)) => parse_number(s)
            .and_then(serde_json::Number::from_f64)
            .map_or_else(|| defaults.clone(), Value::Number),
        (Value::Bool(_), Value::Bool(_))
        | (Value::String(_), Value::String(_))
        | (Value::Array(_), Value::Array(_)) => given.clone(),
        _ => defaults.clone(),
    }
}

/// A list of points as `[[x, y], …]` numbers (typed as text too); rows that aren't two numbers are
/// dropped. `None` when it isn't a list at all.
fn clean_points(v: &Value) -> Option<Value> {
    let num = |v: &Value| match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => parse_number(s),
        _ => None,
    };
    let rows = v.as_array()?;
    Some(Value::Array(
        rows.iter()
            .filter_map(|row| match row.as_array().map(Vec::as_slice) {
                Some([x, y]) => Some(Value::from(vec![num(x)?, num(y)?])),
                _ => None,
            })
            .collect(),
    ))
}

/// A number typed by a person: a decimal comma works as well as a point.
pub fn parse_number(text: &str) -> Option<f64> {
    let t: String = text
        .trim()
        .replace('\u{2212}', "-")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let t = if t.contains(',') {
        if t.contains('.') {
            t.replace(',', "")
        } else {
            t.replacen(',', ".", 1)
        }
    } else {
        t
    };
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Whether a field is shown for these values (its group's condition and its own).
pub fn visible(group: &Group, field: &Field, values: &Value) -> bool {
    group.when.as_ref().is_none_or(|w| w.holds(values))
        && field.when.as_ref().is_none_or(|w| w.holds(values))
}

/// Fills in defaults, reads numbers typed as text, rounds integer fields, and checks every
/// visible field. Returns the complete settings and the problems found.
pub fn normalize(groups: &[Group], defaults: &Value, given: &Value) -> (Value, Vec<Problem>) {
    let mut values = merge(defaults, given);
    let mut problems = Vec::new();
    for g in groups {
        for f in &g.fields {
            if let Input::Number { integer: true, .. } = f.input
                && let Some(v) = get_mut(&mut values, &f.key)
                && let Some(x) = v.as_f64()
                && x.is_finite()
            {
                *v = Value::from(crate::num::round(x));
            }
            if let Input::Points { .. } = f.input
                && let Some(v) = get_mut(&mut values, &f.key)
            {
                *v = clean_points(v)
                    .unwrap_or_else(|| get(defaults, &f.key).cloned().unwrap_or(Value::Null));
            }
        }
    }
    for g in groups {
        for f in &g.fields {
            if !visible(g, f, &values) {
                continue;
            }
            if let Some(message) = check(f, get(&values, &f.key)) {
                problems.push(Problem {
                    key: f.key.clone(),
                    message,
                });
            }
        }
    }
    (values, problems)
}

fn check(f: &Field, v: Option<&Value>) -> Option<String> {
    match &f.input {
        Input::Number { unit, min, max, .. } => {
            let unit = if unit.is_empty() {
                String::new()
            } else {
                format!(" {unit}")
            };
            match v.and_then(Value::as_f64) {
                None => Some(format!("{}: enter a number", f.label)),
                Some(x) if x < *min || x > *max => Some(format!(
                    "{}: {} is outside {}–{}{unit}",
                    f.label,
                    crate::num::js(x),
                    crate::num::js(*min),
                    crate::num::js(*max)
                )),
                Some(_) => None,
            }
        }
        Input::Toggle => match v {
            Some(Value::Bool(_)) => None,
            _ => Some(format!("{}: must be on or off", f.label)),
        },
        Input::Choice { options } => match v.and_then(Value::as_str) {
            Some(s) if options.iter().any(|o| o.value == s) => None,
            _ => Some(format!("{}: choose one of the options", f.label)),
        },
        Input::Points { x, y, min, max } => {
            let rows = v.and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
            if rows.len() < *min || rows.len() > *max {
                return Some(format!("{}: give {min} to {max} points", f.label));
            }
            for (i, row) in rows.iter().enumerate() {
                for (axis, value) in [x, y].into_iter().zip([row.get(0), row.get(1)]) {
                    match value.and_then(Value::as_f64) {
                        Some(a) if a >= axis.min && a <= axis.max => {}
                        _ => {
                            return Some(format!(
                                "{}: point {}'s {} must be {}–{}{}",
                                f.label,
                                i + 1,
                                axis.label.to_lowercase(),
                                crate::num::js(axis.min),
                                crate::num::js(axis.max),
                                if axis.unit.is_empty() {
                                    String::new()
                                } else {
                                    format!(" {}", axis.unit)
                                }
                            ));
                        }
                    }
                }
            }
            None
        }
    }
}

/// Every key the schema has, for tests that keep schemas and settings structs in step.
pub fn keys(groups: &[Group]) -> Vec<String> {
    groups
        .iter()
        .flat_map(|g| g.fields.iter().map(|f| f.key.clone()))
        .collect()
}

/// Every leaf key of a settings object (dotted), for the same tests.
pub fn leaf_keys(values: &Value) -> Vec<String> {
    fn walk(prefix: &str, v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    let key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    walk(&key, v, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }
    let mut out = Vec::new();
    walk("", values, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn groups() -> Vec<Group> {
        vec![
            group(
                "Shape",
                vec![
                    choice(
                        "shape.kind",
                        "Shape",
                        &[("circle", "Circle"), ("star", "Star")],
                    ),
                    number("shape.radius", "Radius", "mm", 1.0, 500.0)
                        .when("shape.kind", &["circle"]),
                    integer("shape.points", "Points", 2.0, 50.0).when("shape.kind", &["star"]),
                ],
            ),
            group(
                "Brim",
                vec![number("brim.width", "Brim width", "mm", 0.1, 10.0)],
            )
            .when_on("brim.on"),
        ]
    }

    fn defaults() -> Value {
        json!({ "shape": { "kind": "circle", "radius": 30, "points": 5 }, "brim": { "on": false, "width": 2 } })
    }

    #[test]
    fn missing_values_come_from_the_defaults() {
        let (v, problems) = normalize(
            &groups(),
            &defaults(),
            &json!({ "shape": { "radius": 12 } }),
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            v,
            json!({ "shape": { "kind": "circle", "radius": 12, "points": 5 }, "brim": { "on": false, "width": 2 } })
        );
    }

    #[test]
    fn numbers_typed_as_text_are_read() {
        let (v, _) = normalize(
            &groups(),
            &defaults(),
            &json!({ "shape": { "radius": "12,5" } }),
        );
        assert_eq!(get(&v, "shape.radius"), Some(&json!(12.5)));
        let (v, _) = normalize(
            &groups(),
            &defaults(),
            &json!({ "shape": { "radius": "abc" } }),
        );
        assert_eq!(get(&v, "shape.radius"), Some(&json!(30)));
    }

    #[test]
    fn wrong_kinds_and_unknown_keys_are_dropped() {
        let (v, _) = normalize(
            &groups(),
            &defaults(),
            &json!({ "shape": { "kind": 3, "radius": true }, "extra": 1 }),
        );
        assert_eq!(v, defaults());
    }

    #[test]
    fn only_visible_fields_are_checked() {
        let bad_hidden = json!({ "shape": { "kind": "star", "radius": -5 } });
        let (_, problems) = normalize(&groups(), &defaults(), &bad_hidden);
        assert!(problems.is_empty(), "{problems:?}");
        let bad_shown = json!({ "shape": { "kind": "circle", "radius": -5 } });
        let (_, problems) = normalize(&groups(), &defaults(), &bad_shown);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].key, "shape.radius");
        assert_eq!(problems[0].message, "Radius: -5 is outside 1–500 mm");
        let brim = json!({ "brim": { "on": true, "width": 0 } });
        let (_, problems) = normalize(&groups(), &defaults(), &brim);
        assert_eq!(problems[0].key, "brim.width");
    }

    #[test]
    fn integers_are_rounded_and_choices_checked() {
        let (v, problems) = normalize(
            &groups(),
            &defaults(),
            &json!({ "shape": { "kind": "star", "points": 6.6 } }),
        );
        assert!(problems.is_empty());
        assert_eq!(get(&v, "shape.points"), Some(&json!(7)));
        let (_, problems) = normalize(
            &groups(),
            &defaults(),
            &json!({ "shape": { "kind": "blob" } }),
        );
        assert_eq!(problems[0].message, "Shape: choose one of the options");
    }

    #[test]
    fn points_are_cleaned_and_checked() {
        let axis = |label| Axis {
            label,
            unit: "%",
            min: 0.0,
            max: 100.0,
        };
        let groups = vec![group(
            "Body",
            vec![
                toggle("on", "Profile"),
                points("points", "Profile", axis("Height"), axis("Size"), 1, 3).when_on("on"),
            ],
        )];
        let defaults = json!({ "on": true, "points": [[0, 50]] });
        let (v, problems) = normalize(
            &groups,
            &defaults,
            &json!({ "points": [[0, "12,5"], [1], "x", [50, 60]] }),
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(get(&v, "points"), Some(&json!([[0.0, 12.5], [50.0, 60.0]])));
        let (v, _) = normalize(&groups, &defaults, &json!({ "points": 3 }));
        assert_eq!(get(&v, "points"), Some(&json!([[0.0, 50.0]])));
        let (_, problems) = normalize(&groups, &defaults, &json!({ "points": [[0, 150]] }));
        assert_eq!(
            problems[0].message,
            "Profile: point 1's size must be 0–100 %"
        );
        let (_, problems) = normalize(&groups, &defaults, &json!({ "points": [] }));
        assert_eq!(problems[0].message, "Profile: give 1 to 3 points");
        // Shown when either toggle is on.
        let g = group("Flat", vec![]).when_either_on("a", "b");
        let f = number("x", "X", "", 0.0, 1.0);
        assert!(!visible(&g, &f, &json!({ "a": false, "b": false })));
        assert!(visible(&g, &f, &json!({ "a": false, "b": true })));
        assert!(visible(&g, &f, &json!({ "a": true, "b": false })));
    }

    #[test]
    fn comma_decimals() {
        assert_eq!(parse_number("1,5"), Some(1.5));
        assert_eq!(parse_number("1,000.5"), Some(1000.5));
        assert_eq!(parse_number("\u{2212}2"), Some(-2.0));
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("inf"), None);
    }
}
