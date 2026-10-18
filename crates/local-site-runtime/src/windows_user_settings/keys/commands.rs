//! Immutable configured actions; the settings reader never executes them.
use super::{
    WindowsKeySettings, WindowsKeyTrigger, WindowsSettingsError, parse_chord,
    windows_user_config_has_child,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsCustomCommandAction {
    Shell,
    Pane,
    Popup,
    PluginAction,
}
impl WindowsCustomCommandAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Pane => "pane",
            Self::Popup => "popup",
            Self::PluginAction => "plugin_action",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsCommandSize {
    Cells(u16),
    Percent(u8),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsCustomCommand {
    pub command: String,
    pub action: WindowsCustomCommandAction,
    pub triggers: Vec<WindowsKeyTrigger>,
    pub description: Option<String>,
    pub width: Option<WindowsCommandSize>,
    pub height: Option<WindowsCommandSize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsCustomCommandEntry {
    pub id: String,
    pub definition: WindowsCustomCommand,
}
fn error(reason: &'static str) -> WindowsSettingsError {
    WindowsSettingsError::new("keys", reason)
}
fn string(value: &toml::Value, maximum: usize) -> Result<String, WindowsSettingsError> {
    let text = value.as_str().ok_or(error("wrong-type"))?;
    if text.len() > maximum {
        return Err(error("too-large"));
    }
    if text.contains('\0') {
        return Err(error("invalid-binding"));
    }
    Ok(text.to_owned())
}
fn size(value: &toml::Value) -> Result<WindowsCommandSize, WindowsSettingsError> {
    if let Some(cells) = value.as_integer()
        && (1..=4096).contains(&cells)
    {
        return Ok(WindowsCommandSize::Cells(cells as u16));
    }
    if let Some(percent) = value
        .as_str()
        .and_then(|s| s.strip_suffix('%'))
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|p| (1..=100).contains(p))
    {
        return Ok(WindowsCommandSize::Percent(percent));
    }
    Err(error("invalid-binding"))
}
pub(super) fn parse(
    value: Option<&toml::Value>,
    settings: &WindowsKeySettings,
) -> Result<Vec<WindowsCustomCommand>, WindowsSettingsError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value.as_array().ok_or(error("wrong-type"))?;
    if entries.len() > 64 {
        return Err(error("too-many-bindings"));
    }
    let mut result: Vec<WindowsCustomCommand> = Vec::new();
    for value in entries {
        let table = value.as_table().ok_or(error("wrong-type"))?;
        if table
            .keys()
            .any(|key| !windows_user_config_has_child("keys.command[]", key))
        {
            return Err(error("unknown-key"));
        }
        let command = string(table.get("command").ok_or(error("invalid-binding"))?, 4096)?;
        if command.trim().is_empty() {
            return Err(error("invalid-binding"));
        }
        let action = match table.get("type") {
            None => WindowsCustomCommandAction::Shell,
            Some(value) => match value.as_str().ok_or(error("wrong-type"))? {
                "shell" => WindowsCustomCommandAction::Shell,
                "pane" => WindowsCustomCommandAction::Pane,
                "popup" => WindowsCustomCommandAction::Popup,
                "plugin_action" => WindowsCustomCommandAction::PluginAction,
                _ => return Err(error("invalid-binding")),
            },
        };
        let width = table.get("width").map(size).transpose()?;
        if action == WindowsCustomCommandAction::PluginAction
            && (command.len() > 241
                || !command
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte)))
        {
            return Err(error("invalid-binding"));
        }
        let height = table.get("height").map(size).transpose()?;
        if action != WindowsCustomCommandAction::Popup && (width.is_some() || height.is_some()) {
            return Err(error("invalid-binding"));
        }
        let keys = match table.get("key") {
            None => Vec::new(),
            Some(toml::Value::String(key)) => vec![key.as_str()],
            Some(toml::Value::Array(keys)) if keys.len() <= 16 => keys
                .iter()
                .map(|k| k.as_str().ok_or(error("wrong-type")))
                .collect::<Result<Vec<_>, _>>()?,
            Some(toml::Value::Array(_)) => return Err(error("too-many-bindings")),
            Some(_) => return Err(error("wrong-type")),
        };
        let mut triggers = Vec::new();
        for raw in keys {
            let (prefix, body) = raw
                .trim()
                .strip_prefix("prefix+")
                .map_or((false, raw.trim()), |s| (true, s));
            let chord = parse_chord(body)?;
            if chord == settings.prefix || (prefix && chord.key == "esc" && !chord.modified()) {
                return Err(error("reserved-binding"));
            }
            if !prefix && !chord.safe_direct() {
                return Err(error("unsafe-binding"));
            }
            let trigger = WindowsKeyTrigger { prefix, chord };
            if triggers.contains(&trigger)
                || settings
                    .overrides
                    .iter()
                    .any(|e| e.triggers.contains(&trigger))
                || result.iter().any(|c| c.triggers.contains(&trigger))
            {
                return Err(error("duplicate-binding"));
            }
            triggers.push(trigger);
        }
        result.push(WindowsCustomCommand {
            command,
            action,
            triggers,
            description: table
                .get("description")
                .map(|v| string(v, 512))
                .transpose()?,
            width,
            height,
        });
    }
    Ok(result)
}

pub(super) fn write_profile(command: &WindowsCustomCommand, text: &mut String) {
    let quoted = |v: &str| serde_json::to_string(v).expect("strings always encode");
    text.push_str(&format!(
        "\n[[keys.command]]\ncommand={}\ntype={}\n",
        quoted(&command.command),
        quoted(command.action.as_str())
    ));
    if !command.triggers.is_empty() {
        let keys: Vec<_> = command
            .triggers
            .iter()
            .map(|t| quoted(&super::trigger_label(t)))
            .collect();
        text.push_str(&format!("key=[{}]\n", keys.join(",")));
    }
    if let Some(description) = &command.description {
        text.push_str(&format!("description={}\n", quoted(description)));
    }
    for (name, size) in [("width", command.width), ("height", command.height)] {
        if let Some(size) = size {
            let value = match size {
                WindowsCommandSize::Cells(n) => n.to_string(),
                WindowsCommandSize::Percent(n) => quoted(&format!("{n}%")),
            };
            text.push_str(&format!("{name}={value}\n"));
        }
    }
}

#[cfg(test)]
mod tests;
