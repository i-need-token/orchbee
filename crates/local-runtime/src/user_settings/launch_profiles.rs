//! Per-platform launch profiles in `config.toml`, admitted by the launch-profile catalog's rules.
//!
//! Each platform reads only its own `[[terminal.profiles.<platform>]]` table and
//! `terminal.default_profile.<platform>`, so one file synchronized between machines carries every
//! platform's shells. The built-in `system-default` profile is always present; a table entry with
//! that id replaces it. Every platform's table is checked, by that platform's launch rules, so a
//! mistake shows on any machine.
use super::{
    Fields, SettingsDiagnosticDisposition,
    registry::{SETTING_PLATFORMS, current_platform, user_config_has_child},
    unknown_field,
};
use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchProfiles};
use serde_json::{Map, Value, json};

const SYSTEM_DEFAULT: &str = "system-default";

/// Whether `platform` (`windows`, `macos` or `linux`) can start `command`: a custom executable
/// names an absolute path in that platform's form, and Windows has no login shell. The rule is
/// spelled out per platform rather than taken from this host's path rules, so every platform's
/// table is judged the same way on every machine.
pub(crate) fn launch_command_admitted(command: &UserLaunchCommand, platform: &str) -> bool {
    let windows = platform == "windows";
    match command {
        UserLaunchCommand::SystemShell { login } => !(windows && *login),
        UserLaunchCommand::CustomExecutable { executable, .. } => {
            let separator = |byte: Option<&u8>| matches!(byte, Some(b'\\' | b'/'));
            let bytes = executable.as_bytes();
            if windows {
                let drive = bytes.first().is_some_and(u8::is_ascii_alphabetic)
                    && bytes.get(1) == Some(&b':')
                    && separator(bytes.get(2));
                let shared = separator(bytes.first()) && separator(bytes.get(1));
                drive || shared
            } else {
                bytes.first() == Some(&b'/')
            }
        }
    }
}

/// The current platform's effective catalog; every platform's table is admitted for diagnostics.
pub(super) fn parse(fields: &mut Fields<'_>) -> UserLaunchProfiles {
    let mut effective = UserLaunchProfiles::default();
    for platform in SETTING_PLATFORMS {
        let catalog = platform_catalog(fields, platform);
        if platform == current_platform() {
            effective = catalog;
        }
    }
    effective
}

fn platform_catalog(fields: &mut Fields<'_>, platform: &str) -> UserLaunchProfiles {
    let table_path = format!("terminal.profiles.{platform}");
    let default_path = format!("terminal.default_profile.{platform}");
    let profiles = match fields.value(&table_path) {
        None => Some(Vec::new()),
        Some(value) => match entries(fields, value, &table_path, platform) {
            Ok(entries) => Some(entries),
            Err(reason) => {
                fields.push(table_path.clone(), reason);
                None
            }
        },
    };
    let mut profiles = profiles.unwrap_or_default();
    if !profiles
        .iter()
        .any(|profile| profile["id"].as_str() == Some(SYSTEM_DEFAULT))
    {
        profiles.insert(0, built_in());
    }
    let default_id = fields
        .text(&default_path)
        .unwrap_or_else(|| SYSTEM_DEFAULT.to_owned());
    if let Some(catalog) = catalog(&default_id, &profiles) {
        return catalog;
    }
    if !profiles
        .iter()
        .any(|profile| profile["id"].as_str() == Some(default_id.as_str()))
    {
        fields.push(default_path, "unknown-profile");
        if let Some(catalog) = catalog(SYSTEM_DEFAULT, &profiles) {
            return catalog;
        }
    }
    fields.push(table_path, "invalid-value");
    UserLaunchProfiles::default()
}

fn catalog(default_id: &str, profiles: &[Value]) -> Option<UserLaunchProfiles> {
    serde_json::from_value(json!({
        "version": 1,
        "defaultProfileId": default_id,
        "profiles": profiles,
    }))
    .ok()
}

fn built_in() -> Value {
    json!({
        "id": SYSTEM_DEFAULT,
        "name": null,
        "command": { "kind": "system-shell", "login": false },
    })
}

/// The table's entries in the catalog's wire form; any malformed entry, or one `platform` cannot
/// start, refuses the whole table. Unknown keys inside an entry are reported and ignored.
fn entries(
    fields: &mut Fields<'_>,
    value: &toml::Value,
    table_path: &str,
    platform: &str,
) -> Result<Vec<Value>, &'static str> {
    let entries = value.as_array().ok_or("wrong-type")?;
    let element_path = format!("{table_path}[]");
    let mut profiles = Vec::with_capacity(entries.len());
    for entry in entries {
        let entry = entry.as_table().ok_or("wrong-type")?;
        for key in entry
            .keys()
            .filter(|key| !user_config_has_child(&element_path, key))
        {
            fields.diagnostics.push(
                unknown_field(&element_path, key),
                "unknown-key",
                SettingsDiagnosticDisposition::Ignored,
            );
        }
        profiles.push(profile(entry, platform)?);
    }
    Ok(profiles)
}

fn profile(entry: &toml::Table, platform: &str) -> Result<Value, &'static str> {
    let text = |key: &str| -> Result<Option<String>, &'static str> {
        entry
            .get(key)
            .map(|value| value.as_str().map(str::to_owned).ok_or("wrong-type"))
            .transpose()
    };
    let id = text("id")?.ok_or("invalid-value")?;
    let name = match text("name")? {
        Some(name) => Value::String(name),
        None if id == SYSTEM_DEFAULT => Value::Null,
        None => Value::String(id.clone()),
    };
    let login = entry
        .get("login")
        .map(|value| value.as_bool().ok_or("wrong-type"))
        .transpose()?;
    let executable = text("executable")?;
    let arguments = entry
        .get("arguments")
        .map(|value| {
            value
                .as_array()
                .ok_or("wrong-type")?
                .iter()
                .map(|argument| {
                    argument
                        .as_str()
                        .map(|argument| Value::String(argument.to_owned()))
                        .ok_or("wrong-type")
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let mut command = Map::new();
    match text("kind")?.as_deref() {
        Some("system-shell") if executable.is_none() && arguments.is_none() => {
            command.insert("kind".into(), "system-shell".into());
            command.insert("login".into(), login.unwrap_or(false).into());
        }
        Some("custom-executable") if login.is_none() => {
            command.insert("kind".into(), "custom-executable".into());
            command.insert(
                "executable".into(),
                executable.ok_or("invalid-value")?.into(),
            );
            command.insert("arguments".into(), arguments.unwrap_or_default().into());
        }
        _ => return Err("invalid-value"),
    }
    let command = Value::Object(command);
    let typed: UserLaunchCommand =
        serde_json::from_value(command.clone()).map_err(|_| "invalid-value")?;
    if !launch_command_admitted(&typed, platform) {
        return Err("invalid-value");
    }
    Ok(json!({ "id": id, "name": name, "command": command }))
}
