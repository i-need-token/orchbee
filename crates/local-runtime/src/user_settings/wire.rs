//! The settings wire contract's edits and refusals, in the editor's terms.
use super::{SettingChange, SettingChanges, SettingsEditError};
use orchbee_workbench_protocol::{
    MAXIMUM_SETTINGS_TOKEN_BYTES, SettingChange as WireChange, SettingEntryValue, SettingPath,
    SettingValue as WireValue, SettingsOutcome, SettingsRefusal,
};

/// The editor's changes for one wire edit, or `None` when the edit names no change, too many, or
/// one key twice.
pub fn setting_changes(changes: &[WireChange]) -> Option<SettingChanges> {
    SettingChanges::new(
        changes
            .iter()
            .map(|change| {
                let key = change.key().segments().map(str::to_owned).collect();
                match change {
                    WireChange::Set { value, .. } => SettingChange::Set {
                        key,
                        value: toml_value(value),
                    },
                    WireChange::Restore { .. } => SettingChange::Restore { key },
                }
            })
            .collect(),
    )
}

fn toml_value(value: &WireValue) -> toml::Value {
    match value {
        WireValue::Boolean(value) => toml::Value::Boolean(*value),
        WireValue::Integer(value) => toml::Value::Integer(*value),
        WireValue::Text(value) => toml::Value::String(value.clone()),
        WireValue::Entries(entries) => toml::Value::Array(
            entries
                .iter()
                .map(|entry| {
                    toml::Value::Table(
                        entry
                            .iter()
                            .map(|(name, value)| {
                                let value = match value {
                                    SettingEntryValue::Boolean(value) => {
                                        toml::Value::Boolean(*value)
                                    }
                                    SettingEntryValue::Integer(value) => {
                                        toml::Value::Integer(*value)
                                    }
                                    SettingEntryValue::Text(value) => {
                                        toml::Value::String(value.clone())
                                    }
                                    SettingEntryValue::List(values) => toml::Value::Array(
                                        values.iter().cloned().map(toml::Value::String).collect(),
                                    ),
                                };
                                (name.clone(), value)
                            })
                            .collect(),
                    )
                })
                .collect(),
        ),
    }
}

/// The refusal an editor error answers with, one reason per error. `first` is the edit's first
/// key: an invalid value is charged to it when the editor names no key the wire can carry.
pub fn settings_refusal(error: &SettingsEditError, first: &SettingPath) -> SettingsOutcome {
    let charged = |key: &str| SettingPath::parse(key).unwrap_or_else(|_| first.clone());
    let detail = |reason: &str, fallback: &str| {
        Some(if token(reason) { reason } else { fallback }.to_owned())
    };
    let position = |value: usize| Some(u32::try_from(value).unwrap_or(u32::MAX).max(1));
    match error {
        SettingsEditError::UnknownKey { key } => SettingsOutcome::Refused {
            reason: SettingsRefusal::UnknownKey,
            key: Some(charged(key)),
            detail: None,
            line: None,
            column: None,
        },
        SettingsEditError::InvalidValue { key, reason, .. } => SettingsOutcome::Refused {
            reason: SettingsRefusal::InvalidValue,
            key: Some(charged(key)),
            detail: detail(reason, "invalid-value"),
            line: None,
            column: None,
        },
        SettingsEditError::Syntax(syntax) => SettingsOutcome::Refused {
            reason: SettingsRefusal::SyntaxError,
            key: None,
            detail: Some(
                if syntax.reason == "invalid-utf8" {
                    "invalid-utf8"
                } else {
                    "invalid-toml"
                }
                .to_owned(),
            ),
            line: position(syntax.line),
            column: position(syntax.column),
        },
        SettingsEditError::Source(source) => SettingsOutcome::Refused {
            reason: SettingsRefusal::UnusableFile,
            key: None,
            detail: detail(source.reason, "io-error"),
            line: None,
            column: None,
        },
        SettingsEditError::ReadOnly => SettingsOutcome::refused(SettingsRefusal::ReadOnly),
        SettingsEditError::InUse => SettingsOutcome::refused(SettingsRefusal::FileInUse),
        SettingsEditError::Locked => SettingsOutcome::refused(SettingsRefusal::Locked),
        SettingsEditError::Changed => SettingsOutcome::refused(SettingsRefusal::Changed),
        SettingsEditError::TooLarge => SettingsOutcome::refused(SettingsRefusal::TooLarge),
        SettingsEditError::Io(_) => SettingsOutcome::refused(SettingsRefusal::WriteFailed),
    }
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_SETTINGS_TOKEN_BYTES
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}
