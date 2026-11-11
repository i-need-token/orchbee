//! Settings as a bundle carries them: the same keys, with commands, arguments,
//! environment values, credentials and unrecognized settings values replaced, then every string
//! scrubbed.
use super::{
    ARGUMENT_PLACEHOLDER, COMMAND_PLACEHOLDER, CREDENTIAL_PLACEHOLDER, DiagnosticRedactor,
    ENVIRONMENT_PLACEHOLDER, UNRECOGNIZED_PLACEHOLDER,
};
use serde_json::{Map, Value};

/// Keys whose value names a program to run.
const COMMAND_KEYS: [&str; 3] = ["command", "executable", "program"];
/// Keys whose value is a list of program arguments.
const ARGUMENT_KEYS: [&str; 2] = ["args", "arguments"];
/// Keys whose value maps environment variable names to values.
const ENVIRONMENT_KEYS: [&str; 3] = ["env", "environment", "environmentVariables"];
/// Fragments of a key name that hold a secret.
const CREDENTIAL_FRAGMENTS: [&str; 6] = [
    "token",
    "secret",
    "password",
    "apikey",
    "api_key",
    "credential",
];

/// `config.toml` written again from its values, or `None` when it is not valid TOML. Its comments
/// and formatting are not carried.
pub fn redact_settings(text: &str, redactor: &DiagnosticRedactor) -> Option<String> {
    let table: toml::Table = text.parse().ok()?;
    let Value::Object(mut values) = from_toml(toml::Value::Table(table)) else {
        return None;
    };
    for (key, value) in &mut values {
        redact(value, key, key, redactor);
    }
    let mut document = toml_edit::DocumentMut::new();
    *document.as_table_mut() = to_table(&values);
    Some(document.to_string())
}

/// Redacts `value` at `path`. A settings path is recognized when it or an ancestor is a
/// registered key, with `[]` marking an array element.
fn redact(value: &mut Value, key: &str, path: &str, redactor: &DiagnosticRedactor) {
    let scalar = |value: &Value| !matches!(value, Value::Object(_) | Value::Array(_));
    let lowered = key.to_ascii_lowercase();
    if !crate::user_settings::recognized_path(path) && !matches!(value, Value::Object(_)) {
        *value = UNRECOGNIZED_PLACEHOLDER.into();
    } else if CREDENTIAL_FRAGMENTS
        .iter()
        .any(|fragment| lowered.contains(fragment))
    {
        replace_leaves(value, CREDENTIAL_PLACEHOLDER);
    } else if ENVIRONMENT_KEYS.contains(&key) {
        replace_leaves(value, ENVIRONMENT_PLACEHOLDER);
    } else if COMMAND_KEYS.contains(&key) && scalar(value) {
        *value = COMMAND_PLACEHOLDER.into();
    } else if ARGUMENT_KEYS.contains(&key) {
        replace_leaves(value, ARGUMENT_PLACEHOLDER);
    } else {
        match value {
            Value::Object(map) => {
                for (child, value) in map.iter_mut() {
                    redact(value, child, &format!("{path}.{child}"), redactor);
                }
            }
            Value::Array(items) => {
                for item in items {
                    redact(item, key, &format!("{path}[]"), redactor);
                }
            }
            Value::String(text) => *text = redactor.scrub(text),
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
}

/// Replaces every scalar under `value`, keeping object keys and list lengths.
fn replace_leaves(value: &mut Value, placeholder: &str) {
    match value {
        Value::Object(map) => map
            .values_mut()
            .for_each(|value| replace_leaves(value, placeholder)),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| replace_leaves(item, placeholder)),
        Value::Null => {}
        Value::Bool(_) | Value::Number(_) | Value::String(_) => *value = placeholder.into(),
    }
}

fn from_toml(value: toml::Value) -> Value {
    match value {
        toml::Value::String(text) => Value::String(text),
        toml::Value::Integer(number) => Value::from(number),
        toml::Value::Float(number) => Value::from(number),
        toml::Value::Boolean(flag) => Value::Bool(flag),
        toml::Value::Datetime(datetime) => Value::String(datetime.to_string()),
        toml::Value::Array(items) => Value::Array(items.into_iter().map(from_toml).collect()),
        toml::Value::Table(table) => Value::Object(
            table
                .into_iter()
                .map(|(key, value)| (key, from_toml(value)))
                .collect(),
        ),
    }
}

fn to_table(values: &Map<String, Value>) -> toml_edit::Table {
    let mut table = toml_edit::Table::new();
    let mut only_tables = true;
    for (key, value) in values {
        let item = match value {
            Value::Object(map) => toml_edit::Item::Table(to_table(map)),
            Value::Array(items) if !items.is_empty() && items.iter().all(Value::is_object) => {
                let mut tables = toml_edit::ArrayOfTables::new();
                for item in items.iter().filter_map(Value::as_object) {
                    tables.push(to_table(item));
                }
                toml_edit::Item::ArrayOfTables(tables)
            }
            other => {
                only_tables = false;
                match to_value(other) {
                    Some(value) => toml_edit::Item::Value(value),
                    None => continue,
                }
            }
        };
        table.insert(key, item);
    }
    table.set_implicit(only_tables);
    table
}

fn to_value(value: &Value) -> Option<toml_edit::Value> {
    Some(match value {
        Value::Null => return None,
        Value::Bool(flag) => (*flag).into(),
        Value::Number(number) => match number.as_i64() {
            Some(integer) => integer.into(),
            None => number.as_f64()?.into(),
        },
        Value::String(text) => text.as_str().into(),
        Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items.iter().filter_map(to_value) {
                array.push(item);
            }
            array.into()
        }
        Value::Object(map) => {
            let mut inline = toml_edit::InlineTable::new();
            for (key, value) in map {
                if let Some(value) = to_value(value) {
                    inline.insert(key, value);
                }
            }
            inline.into()
        }
    })
}
