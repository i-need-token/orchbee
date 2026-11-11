//! The settings snapshot a Server publishes: the whole file of its machine, key by key, in the
//! file's own spelling.
use super::{
    DiagnosticsCollector, Scalar, SettingsDiagnosticDisposition, SettingsDiagnostics,
    UserSettingsLoad,
    registry::{SETTING_KEYS, SETTING_PLATFORMS, SettingDefault, SettingValue, current_platform},
    user_config_has_child,
};
use orchbee_workbench_protocol::{
    ConfigurationDiagnostic, ConfigurationDiagnosticDisposition, ConfigurationDiagnostics,
    MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED, MAXIMUM_SETTING_TEXT_BYTES,
    MAXIMUM_SETTINGS_FILE_PATH_BYTES, MAXIMUM_SNAPSHOT_SETTING_KEYS, SettingPath, SettingScalar,
    SettingsFile, SettingsPlatform, SettingsSnapshot, SettingsSyntaxReason, WorkbenchRevision,
};
use std::collections::{BTreeMap, BTreeSet};

/// What the file itself says: each scalar key it sets that the reader took as written, in the
/// file's spelling, and every registered key it sets. Keys outside the snapshot's key grammar,
/// such as an Agent name with a hyphen, are left out.
pub(super) fn file_keys(
    table: &toml::Table,
    diagnostics: &DiagnosticsCollector,
) -> (BTreeMap<String, Scalar>, BTreeSet<String>) {
    let mut values = BTreeMap::new();
    let mut set = BTreeSet::new();
    walk(table, &mut Vec::new(), diagnostics, &mut values, &mut set);
    while set.len() > MAXIMUM_SNAPSHOT_SETTING_KEYS {
        set.pop_last();
    }
    (values, set)
}

fn walk<'a>(
    table: &'a toml::Table,
    parent: &mut Vec<&'a str>,
    diagnostics: &DiagnosticsCollector,
    values: &mut BTreeMap<String, Scalar>,
    set: &mut BTreeSet<String>,
) {
    for (key, value) in table {
        if !user_config_has_child(&parent.join("."), key) {
            continue;
        }
        parent.push(key);
        let path = parent.join(".");
        match super::registry::find_setting(parent) {
            Some(setting) => {
                if SettingPath::parse(&path).is_ok() {
                    set.insert(path.clone());
                }
                if concrete(setting.path)
                    && !diagnostics.reported(&path)
                    && let Ok(scalar) = setting.admit(value)
                {
                    values.insert(path, scalar);
                }
            }
            None => {
                if let Some(child) = value.as_table() {
                    walk(child, parent, diagnostics, values, set);
                }
            }
        }
        parent.pop();
    }
}

/// A key the snapshot lists in `values`: a scalar one whose path names no open set of names.
fn concrete(pattern: &str) -> bool {
    !pattern.contains('*') && !pattern.contains("<action>") && !pattern.contains("[]")
}

/// Every concrete path of a registered scalar key, `<platform>` spelled out for each platform.
fn scalar_paths() -> impl Iterator<Item = (String, &'static SettingDefault)> {
    SETTING_KEYS
        .iter()
        .filter(|setting| {
            concrete(setting.path) && !matches!(setting.value, SettingValue::Structured(_))
        })
        .flat_map(|setting| {
            let platforms: &[&str] = if setting.path.contains("<platform>") {
                &SETTING_PLATFORMS
            } else {
                &[""]
            };
            platforms.iter().map(move |platform| {
                (
                    setting.path.replace("<platform>", platform),
                    &setting.default,
                )
            })
        })
}

fn wire_scalar(scalar: &Scalar) -> SettingScalar {
    match scalar {
        Scalar::Boolean(value) => SettingScalar::Boolean(*value),
        Scalar::Integer(value) => SettingScalar::Integer(*value),
        Scalar::Text(value) => SettingScalar::Text(value.clone()),
    }
}

/// The scalar a wire value carries, for typed views built from a snapshot.
pub(super) fn scalar(value: &SettingScalar) -> Scalar {
    match value {
        SettingScalar::Boolean(value) => Scalar::Boolean(*value),
        SettingScalar::Integer(value) => Scalar::Integer(*value),
        SettingScalar::Text(value) => Scalar::Text(value.clone()),
    }
}

fn default_value(default: &SettingDefault) -> Option<SettingScalar> {
    match default {
        SettingDefault::Boolean(value) => Some(SettingScalar::Boolean(*value)),
        SettingDefault::Integer(value) => Some(SettingScalar::Integer(*value)),
        SettingDefault::Text(value) => Some(SettingScalar::Text((*value).to_owned())),
        SettingDefault::Toml(_) | SettingDefault::Unset { .. } => None,
    }
}

/// The diagnostics in the bounded wire form; an omitted count beyond the wire bound is clamped.
pub fn configuration_diagnostics(diagnostics: &SettingsDiagnostics) -> ConfigurationDiagnostics {
    ConfigurationDiagnostics {
        items: diagnostics
            .items()
            .iter()
            .map(|item| ConfigurationDiagnostic {
                field: item.field.clone(),
                reason: item.reason.into(),
                disposition: match item.disposition {
                    SettingsDiagnosticDisposition::Defaulted => {
                        ConfigurationDiagnosticDisposition::Defaulted
                    }
                    SettingsDiagnosticDisposition::Disabled => {
                        ConfigurationDiagnosticDisposition::Disabled
                    }
                    SettingsDiagnosticDisposition::Ignored => {
                        ConfigurationDiagnosticDisposition::Ignored
                    }
                },
            })
            .collect(),
        omitted_count: u32::try_from(diagnostics.omitted_count())
            .unwrap_or(u32::MAX)
            .min(MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED),
    }
}

/// The snapshot of `load` at `revision`. A load that kept its last valid settings because the file
/// stopped parsing ([`UserSettingsLoad::or_last_valid`]) publishes those values with the syntax
/// error, and a startup load that could not read the file publishes the defaults as
/// [`SettingsFile::Unreadable`]. A later read that fails is the caller's to report: it replaces
/// `file` with [`SettingsFile::Unreadable`] on the snapshot of the last valid load.
pub fn settings_snapshot(load: &UserSettingsLoad, revision: u64) -> SettingsSnapshot {
    let settings = &load.settings;
    let file = match (load.unreadable, load.syntax) {
        (Some(reason), _) => SettingsFile::Unreadable {
            reason: reason.into(),
        },
        (None, Some(syntax)) => SettingsFile::SyntaxError {
            reason: if syntax.reason == "invalid-utf8" {
                SettingsSyntaxReason::InvalidUtf8
            } else {
                SettingsSyntaxReason::InvalidToml
            },
            line: position(syntax.line),
            column: position(syntax.column),
        },
        (None, None) if settings.exists => SettingsFile::Ok {},
        (None, None) => SettingsFile::Missing {},
    };
    let values = scalar_paths()
        .filter_map(|(path, default)| {
            let key = SettingPath::parse(&path).ok()?;
            let value = settings
                .file_values
                .get(&path)
                .map(wire_scalar)
                .filter(|value| {
                    !matches!(value, SettingScalar::Text(text) if text.len() > MAXIMUM_SETTING_TEXT_BYTES)
                })
                .or_else(|| default_value(default));
            Some((key, value))
        })
        .collect();
    SettingsSnapshot {
        revision: WorkbenchRevision::new(revision),
        file_path: settings
            .path
            .as_deref()
            .and_then(std::path::Path::to_str)
            .filter(|path| {
                !path.is_empty()
                    && path.len() <= MAXIMUM_SETTINGS_FILE_PATH_BYTES
                    && !path.chars().any(char::is_control)
            })
            .map(str::to_owned),
        file,
        platform: match current_platform() {
            "windows" => SettingsPlatform::Windows,
            "macos" => SettingsPlatform::Macos,
            _ => SettingsPlatform::Linux,
        },
        values,
        set: settings
            .file_keys
            .iter()
            .filter_map(|key| SettingPath::parse(key).ok())
            .collect(),
        launch_profiles: settings.values.launch_profiles.clone(),
        diagnostics: configuration_diagnostics(&load.diagnostics),
    }
}

fn position(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX).max(1)
}
