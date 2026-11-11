// `schemas/workbench/v1/setting-keys-v1.json` is the registry as data for the TypeScript bindings
// and other readers outside Rust. It is generated here, and this check fails while the tracked
// file differs from the registry. `ORCHBEE_REGENERATE_SETTING_KEYS=1` rewrites it instead. The
// output is laid out the way the repository formatter lays out JSON, so the check and the
// formatter agree.
use super::*;
use crate::{MAXIMUM_STRUCTURED_ENTRIES, MAXIMUM_STRUCTURED_VALUE_BYTES};
use serde_json::{Value, json};
use std::path::PathBuf;

/// The formatter's line width; a tab counts as two columns.
const WIDTH: usize = 100;

#[test]
fn setting_key_file_matches_the_registry() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/workbench/v1/setting-keys-v1.json");
    let generated = document();
    if std::env::var_os("ORCHBEE_REGENERATE_SETTING_KEYS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let tracked = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        tracked == generated,
        "{} is stale; rerun this test with ORCHBEE_REGENERATE_SETTING_KEYS=1",
        path.display()
    );
}

fn document() -> String {
    let mut out = String::from("{\n");
    field(&mut out, 1, "contractVersion", &json!(1), false);
    field(&mut out, 1, "platforms", &json!(SETTING_PLATFORMS), false);
    out.push_str("\t\"keys\": [\n");
    for (index, setting) in SETTING_KEYS.iter().enumerate() {
        out.push_str("\t\t{\n");
        let fields = entry(setting);
        for (position, (name, value)) in fields.iter().enumerate() {
            field(&mut out, 3, name, value, position + 1 == fields.len());
        }
        out.push_str(if index + 1 == SETTING_KEYS.len() {
            "\t\t}\n"
        } else {
            "\t\t},\n"
        });
    }
    out.push_str("\t]\n}\n");
    out
}

/// One key's fields in their published order.
fn entry(setting: &SettingKey) -> Vec<(&'static str, Value)> {
    let mut fields = vec![("path", json!(setting.path))];
    match setting.value {
        V::Boolean => fields.push(("type", json!("boolean"))),
        V::Integer {
            minimum,
            maximum,
            step,
        } => fields.extend([
            ("type", json!("integer")),
            ("minimum", json!(minimum)),
            ("maximum", json!(maximum)),
            ("step", json!(step)),
        ]),
        V::Choice(choices) => {
            fields.extend([("type", json!("choice")), ("choices", json!(choices))])
        }
        V::Text { maximum_bytes, .. } => fields.extend([
            ("type", json!("text")),
            ("maximumBytes", json!(maximum_bytes)),
        ]),
        V::Color => fields.push(("type", json!("color"))),
        V::Theme => fields.extend([
            ("type", json!("theme")),
            ("maximumBytes", json!(THEME_ID_BYTES)),
        ]),
        V::Path => fields.push(("type", json!("path"))),
        V::Structured(_) => fields.extend([
            ("type", json!("structured")),
            ("maximumEntries", json!(MAXIMUM_STRUCTURED_ENTRIES)),
            ("maximumBytes", json!(MAXIMUM_STRUCTURED_VALUE_BYTES)),
        ]),
    }
    let default = match setting.default {
        D::Boolean(value) => json!(value),
        D::Integer(value) => json!(value),
        D::Text(value) => json!(value),
        D::Toml(value) => {
            let table: toml::Table = toml::from_str(&format!("value = {value}")).unwrap();
            serde_json::to_value(&table["value"]).unwrap()
        }
        D::Unset { .. } => Value::Null,
    };
    fields.extend([
        ("default", default),
        ("scope", json!(setting.scope.id())),
        ("clients", json!(setting.clients.id())),
        ("effect", json!(setting.effect.id())),
        ("category", json!(setting.category.id())),
        ("group", json!(setting.group.id())),
        ("page", json!(setting.page)),
    ]);
    fields
}

/// `"name": value` on its own line at `depth` tabs. An array that does not fit on the line is
/// written one element per line.
fn field(out: &mut String, depth: usize, name: &str, value: &Value, last: bool) {
    let indent = "\t".repeat(depth);
    let comma = if last { "" } else { "," };
    let head = format!("{indent}{}: ", quoted(name));
    let flat = inline(value);
    match value {
        Value::Array(items)
            if !items.is_empty()
                && depth * 2 + head.len() - depth + flat.len() + comma.len() > WIDTH =>
        {
            out.push_str(&head);
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                let separator = if index + 1 == items.len() { "" } else { "," };
                out.push_str(&format!("{indent}\t{}{separator}\n", inline(item)));
            }
            out.push_str(&format!("{indent}]{comma}\n"));
        }
        _ => out.push_str(&format!("{head}{flat}{comma}\n")),
    }
}

/// A value on one line, spaced as the formatter spaces it.
fn inline(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(inline).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(entries) if entries.is_empty() => "{}".into(),
        Value::Object(entries) => format!(
            "{{ {} }}",
            entries
                .iter()
                .map(|(name, value)| format!("{}: {}", quoted(name), inline(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

fn quoted(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}
