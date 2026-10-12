//! Preserve explicitly admitted native leaves without rewriting or deleting the old TOML source.
use super::UserPreferencesScope;
use crate::{WindowsPrivateCache, WindowsShellSettings};
use orchbee_workbench_protocol::{PreferencesFailure, UserPreferencesPatch};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub(super) fn read(
    scope: &UserPreferencesScope,
) -> Result<(UserPreferencesPatch, String), PreferencesFailure> {
    let source =
        WindowsPrivateCache::read_configuration_existing_pinned(&scope.configuration, 64 * 1024)
            .map_err(|_| PreferencesFailure::InvalidSource)?;
    let Some(bytes) = source.bytes() else {
        return Ok((UserPreferencesPatch::default(), "missing".into()));
    };
    let mut digest = Sha256::new();
    digest.update(b"native-preferences-import-v1\0");
    digest.update(bytes);
    let fingerprint = format!("{:x}", digest.finalize());
    let text = std::str::from_utf8(bytes).map_err(|_| PreferencesFailure::InvalidSource)?;
    let document: toml::Table =
        toml::from_str(text).map_err(|_| PreferencesFailure::InvalidSource)?;
    let mut fields = Map::new();
    if let Some(theme) = document
        .get("theme")
        .and_then(|theme| theme.get("name"))
        .and_then(toml::Value::as_str)
    {
        let values = match theme {
            "terminal-native" => Some(("system", "host")),
            "one-light" => Some(("light", "light")),
            "one-dark" => Some(("dark", "dark")),
            _ => None,
        };
        if let Some((application, terminal)) = values {
            fields.insert("applicationTheme".into(), json!(application));
            fields.insert("terminalDisplayTheme".into(), json!(terminal));
        }
    }
    if let Some(ui) = document.get("ui") {
        if let Some(delivery) = ui
            .get("toast")
            .and_then(|toast| toast.get("agent_delivery"))
            .and_then(toml::Value::as_str)
            && matches!(
                delivery,
                "off" | "orchbee" | "system" | "terminal" | "herdr"
            )
        {
            fields.insert(
                "agentNotificationMode".into(),
                json!(if delivery == "off" { "never" } else { "hidden" }),
            );
        }
        if let Some(sound) = ui
            .get("sound")
            .and_then(|sound| sound.get("enabled"))
            .and_then(toml::Value::as_bool)
        {
            fields.insert("soundEnabled".into(), json!(sound));
        }
    }
    if let Some(terminal) = document.get("terminal") {
        let executable = terminal
            .get("default_shell")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        let mode = terminal
            .get("shell_mode")
            .and_then(toml::Value::as_str)
            .unwrap_or("auto");
        if let Ok(shell) = WindowsShellSettings::new(executable, mode)
            && (!shell.executable().is_empty() || mode == "login")
        {
            let command = if shell.executable().is_empty() {
                // Automatic Windows shells are PowerShell-family and do not have login mode.
                json!({"kind": "system-shell", "login": false})
            } else {
                let executable =
                    resolve(shell.executable()).ok_or(PreferencesFailure::InvalidSource)?;
                let name = executable
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let arguments = if mode == "login"
                    && matches!(
                        name.as_str(),
                        "sh" | "bash"
                            | "zsh"
                            | "ksh"
                            | "dash"
                            | "ash"
                            | "mksh"
                            | "fish"
                            | "csh"
                            | "tcsh"
                    ) {
                    vec!["-l"]
                } else {
                    Vec::new()
                };
                json!({"kind": "custom-executable", "executable": executable, "arguments": arguments})
            };
            fields.insert(
                "launchProfiles".into(),
                json!({"version": 1, "defaultProfileId": "system-default",
                "profiles": [{"id": "system-default", "name": null, "command": command}]}),
            );
        }
    }
    serde_json::from_value(Value::Object(fields))
        .map(|patch| (patch, fingerprint))
        .map_err(|_| PreferencesFailure::InvalidSource)
}

fn resolve(value: &str) -> Option<PathBuf> {
    let selected = Path::new(value);
    if selected.is_absolute() {
        return Some(selected.to_owned());
    }
    let paths = std::env::var_os("PATH")?;
    let names = if selected.extension().is_none() {
        vec![selected.to_owned(), selected.with_extension("exe")]
    } else {
        vec![selected.to_owned()]
    };
    std::env::split_paths(&paths)
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| {
            names
                .iter()
                .map(|name| directory.join(name))
                .find(|path| path.is_file())
        })
}
