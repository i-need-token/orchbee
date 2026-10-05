//! Bounded Windows key configuration. Semantic dispatch stays with the Client.
use super::{
    DiagnosticsCollector, WindowsSettingsDiagnosticDisposition, WindowsSettingsError, unknown_field,
};
mod commands;
pub use commands::{
    WindowsCommandSize, WindowsCustomCommand, WindowsCustomCommandAction, WindowsCustomCommandEntry,
};

pub(super) const ACTIONS: &[&str] = &[
    "resize_mode",
    "resize_pane_left",
    "resize_pane_right",
    "resize_pane_up",
    "resize_pane_down",
    "reload_config",
    "help",
    "new_workspace",
    "rename_workspace",
    "workspace_picker",
    "detach",
    "previous_workspace",
    "next_workspace",
    "previous_agent",
    "next_agent",
    "focus_agent",
    "new_tab",
    "rename_tab",
    "previous_tab",
    "next_tab",
    "move_tab_previous",
    "move_tab_next",
    "switch_tab",
    "close_tab",
    "focus_pane_left",
    "focus_pane_down",
    "focus_pane_up",
    "focus_pane_right",
    "split_vertical",
    "split_horizontal",
    "close_pane",
    "zoom",
    "toggle_sidebar",
];

const HERDR_ONLY_ACTIONS: &[&str] = &[
    "settings",
    "new_worktree",
    "open_worktree",
    "remove_worktree",
    "close_workspace",
    "goto",
    "navigate_workspace_up",
    "navigate_workspace_down",
    "navigate_pane_left",
    "navigate_pane_down",
    "navigate_pane_up",
    "navigate_pane_right",
    "open_notification_target",
    "remote_image_paste",
    "switch_workspace",
    "rename_pane",
    "edit_scrollback",
    "clear_pane",
    "copy_mode",
    "swap_pane_left",
    "swap_pane_down",
    "swap_pane_up",
    "swap_pane_right",
    "cycle_pane_next",
    "cycle_pane_previous",
    "last_pane",
    "indexed",
];

pub(super) fn is_herdr_only_action(action: &str) -> bool {
    HERDR_ONLY_ACTIONS.contains(&action)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsKeyChord {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsKeyTrigger {
    pub prefix: bool,
    pub chord: WindowsKeyChord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsKeyOverride {
    pub action: &'static str,
    pub triggers: Vec<WindowsKeyTrigger>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsKeySettings {
    pub prefix: WindowsKeyChord,
    pub overrides: Vec<WindowsKeyOverride>,
    pub commands: Vec<WindowsCustomCommand>,
}

impl Default for WindowsKeySettings {
    fn default() -> Self {
        Self {
            prefix: WindowsKeyChord {
                key: "b".into(),
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
            },
            overrides: Vec::new(),
            commands: Vec::new(),
        }
    }
}

fn error(reason: &'static str) -> WindowsSettingsError {
    WindowsSettingsError::new("keys", reason)
}

impl WindowsKeySettings {
    pub(super) fn parse_lenient(
        value: &toml::Value,
        diagnostics: &mut DiagnosticsCollector,
    ) -> Self {
        let Some(table) = value.as_table() else {
            diagnostics.push(
                "keys",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            return Self::default();
        };
        let mut result = Self::default();
        if let Some(value) = table.get("prefix") {
            let candidate = value
                .as_str()
                .ok_or_else(|| error("wrong-type"))
                .and_then(parse_chord);
            match candidate {
                Ok(prefix) => result.prefix = prefix,
                Err(error) => diagnostics.push(
                    "keys.prefix",
                    error.reason,
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
        for key in table.keys().filter(|key| {
            !matches!(key.as_str(), "prefix" | "command" | "fullscreen")
                && !ACTIONS.contains(&key.as_str())
        }) {
            let unsupported = HERDR_ONLY_ACTIONS.contains(&key.as_str());
            diagnostics.push(
                unknown_field("keys", key),
                if unsupported {
                    "unsupported-herdr"
                } else {
                    "unknown-key"
                },
                if unsupported {
                    WindowsSettingsDiagnosticDisposition::Unsupported
                } else {
                    WindowsSettingsDiagnosticDisposition::Ignored
                },
            );
        }
        for action in ACTIONS {
            let (field, value) = match (table.get(*action), *action) {
                (Some(value), _) => (*action, Some(value)),
                (None, "zoom") => ("fullscreen", table.get("fullscreen")),
                (None, _) => (*action, None),
            };
            let Some(value) = value else {
                continue;
            };
            parse_action_lenient(action, field, value, &mut result, diagnostics, 16);
        }
        if table.contains_key("zoom") && table.contains_key("fullscreen") {
            diagnostics.push(
                "keys.fullscreen",
                "duplicate-binding",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        if let Some(commands) = table.get("command") {
            match commands::parse(Some(commands), &result) {
                Ok(commands) => result.commands = commands,
                Err(error) => diagnostics.push(
                    "keys.command",
                    error.reason,
                    WindowsSettingsDiagnosticDisposition::Disabled,
                ),
            }
        }
        if result.profile().len() > 64 * 1024 {
            for setting in &mut result.overrides {
                setting.triggers.clear();
            }
            result.commands.clear();
            diagnostics.push(
                "keys",
                "too-large",
                WindowsSettingsDiagnosticDisposition::Disabled,
            );
        }
        result
    }

    #[cfg(test)]
    pub(super) fn parse(value: &toml::Value) -> Result<Self, WindowsSettingsError> {
        Self::parse_with_limit(value, 16)
    }
    fn parse_with_limit(value: &toml::Value, limit: usize) -> Result<Self, WindowsSettingsError> {
        let table = value.as_table().ok_or(error("wrong-type"))?;
        let mut result = Self::default();
        if let Some(value) = table.get("prefix") {
            result.prefix = parse_chord(value.as_str().ok_or(error("wrong-type"))?)?;
        }
        for (field, value) in table {
            if matches!(field.as_str(), "prefix" | "command") {
                continue;
            }
            let field = if field == "fullscreen" {
                "zoom"
            } else {
                field.as_str()
            };
            let action = ACTIONS
                .iter()
                .find(|action| **action == field)
                .copied()
                .ok_or(error("unknown-key"))?;
            if result.overrides.iter().any(|entry| entry.action == action) {
                return Err(error("duplicate-binding"));
            }
            let values = match value {
                toml::Value::String(value) => vec![value.as_str()],
                toml::Value::Array(values) if values.len() <= limit => values
                    .iter()
                    .map(|value| value.as_str().ok_or(error("wrong-type")))
                    .collect::<Result<Vec<_>, _>>()?,
                toml::Value::Array(_) => return Err(error("too-many-bindings")),
                _ => return Err(error("wrong-type")),
            };
            let mut triggers = Vec::new();
            for raw in values {
                let raw = raw.trim();
                if raw.is_empty() {
                    continue;
                }
                let (prefix, body) = raw
                    .strip_prefix("prefix+")
                    .map_or((false, raw), |body| (true, body));
                let bodies = if body.split('+').any(|part| part.trim() == "1..9") {
                    if !matches!(action, "switch_tab" | "focus_agent") {
                        return Err(error("invalid-binding"));
                    }
                    (1..=9)
                        .map(|digit| body.replace("1..9", &digit.to_string()))
                        .collect()
                } else {
                    vec![body.to_owned()]
                };
                for body in bodies {
                    let chord = parse_chord(&body)?;
                    if matches!(action, "switch_tab" | "focus_agent")
                        && !(chord.key.len() == 1 && matches!(chord.key.as_bytes()[0], b'1'..=b'9'))
                    {
                        return Err(error("invalid-binding"));
                    }
                    if chord == result.prefix || (prefix && chord.key == "esc" && !chord.modified())
                    {
                        return Err(error("reserved-binding"));
                    }
                    if !prefix
                        && (!chord.safe_direct()
                            || (action.starts_with("resize_")
                                && !chord.modified()
                                && !chord.key.starts_with('f')))
                    {
                        return Err(error("unsafe-binding"));
                    }
                    let trigger = WindowsKeyTrigger { prefix, chord };
                    if triggers.contains(&trigger)
                        || result
                            .overrides
                            .iter()
                            .any(|entry| entry.triggers.contains(&trigger))
                    {
                        return Err(error("duplicate-binding"));
                    }
                    if triggers.len() >= 144 {
                        return Err(error("too-many-bindings"));
                    }
                    triggers.push(trigger);
                }
            }
            result
                .overrides
                .push(WindowsKeyOverride { action, triggers });
        }
        result.commands = commands::parse(table.get("command"), &result)?;
        if result.profile().len() > 64 * 1024 {
            return Err(error("too-large"));
        }
        Ok(result)
    }
}

pub(super) fn parse_action_lenient(
    action: &'static str,
    source_field: &str,
    value: &toml::Value,
    result: &mut WindowsKeySettings,
    diagnostics: &mut DiagnosticsCollector,
    limit: usize,
) {
    let field = format!("keys.{source_field}");
    let values: Vec<Option<&str>> = match value {
        toml::Value::String(value) => vec![Some(value)],
        toml::Value::Array(values) if values.is_empty() => {
            diagnostics.push(
                field.clone(),
                "invalid-binding",
                WindowsSettingsDiagnosticDisposition::Disabled,
            );
            Vec::new()
        }
        toml::Value::Array(values) if values.len() <= limit => values
            .iter()
            .map(|value| {
                let value = value.as_str();
                if value.is_none() {
                    diagnostics.push(
                        field.clone(),
                        "wrong-type",
                        WindowsSettingsDiagnosticDisposition::Disabled,
                    );
                }
                value
            })
            .collect(),
        toml::Value::Array(_) => {
            diagnostics.push(
                field.clone(),
                "too-many-bindings",
                WindowsSettingsDiagnosticDisposition::Disabled,
            );
            Vec::new()
        }
        _ => {
            diagnostics.push(
                field.clone(),
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Disabled,
            );
            Vec::new()
        }
    };
    let mut triggers = Vec::new();
    for raw in values.into_iter().flatten() {
        let raw = raw.trim();
        if raw.is_empty() {
            diagnostics.push(
                field.clone(),
                "invalid-binding",
                WindowsSettingsDiagnosticDisposition::Disabled,
            );
            continue;
        }
        let (prefix, body) = raw
            .strip_prefix("prefix+")
            .map_or((false, raw), |body| (true, body));
        let bodies = if body.split('+').any(|part| part.trim() == "1..9") {
            if !matches!(action, "switch_tab" | "focus_agent") {
                diagnostics.push(
                    field.clone(),
                    "invalid-binding",
                    WindowsSettingsDiagnosticDisposition::Disabled,
                );
                continue;
            }
            (1..=9)
                .map(|digit| body.replace("1..9", &digit.to_string()))
                .collect()
        } else {
            vec![body.to_owned()]
        };
        for body in bodies {
            let chord = match parse_chord(&body) {
                Ok(chord) => chord,
                Err(error) => {
                    diagnostics.push(
                        field.clone(),
                        error.reason,
                        WindowsSettingsDiagnosticDisposition::Disabled,
                    );
                    continue;
                }
            };
            let reason = if matches!(action, "switch_tab" | "focus_agent")
                && !(chord.key.len() == 1 && matches!(chord.key.as_bytes()[0], b'1'..=b'9'))
            {
                Some("invalid-binding")
            } else if chord == result.prefix || (prefix && chord.key == "esc" && !chord.modified())
            {
                Some("reserved-binding")
            } else if !prefix
                && (!chord.safe_direct()
                    || (action.starts_with("resize_")
                        && !chord.modified()
                        && !chord.key.starts_with('f')))
            {
                Some("unsafe-binding")
            } else {
                None
            };
            if let Some(reason) = reason {
                diagnostics.push(
                    field.clone(),
                    reason,
                    WindowsSettingsDiagnosticDisposition::Disabled,
                );
                continue;
            }
            let trigger = WindowsKeyTrigger { prefix, chord };
            if triggers.contains(&trigger)
                || result
                    .overrides
                    .iter()
                    .any(|entry| entry.triggers.contains(&trigger))
            {
                diagnostics.push(
                    field.clone(),
                    "duplicate-binding",
                    WindowsSettingsDiagnosticDisposition::Disabled,
                );
                continue;
            }
            if triggers.len() >= 144 {
                diagnostics.push(
                    field.clone(),
                    "too-many-bindings",
                    WindowsSettingsDiagnosticDisposition::Disabled,
                );
                break;
            }
            triggers.push(trigger);
        }
    }
    result
        .overrides
        .push(WindowsKeyOverride { action, triggers });
}

impl WindowsKeyChord {
    fn modified(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.super_key
    }
    fn safe_direct(&self) -> bool {
        if !self.alt && !self.super_key {
            if !self.ctrl && self.key.chars().count() == 1 {
                return false;
            }
            if self.ctrl && !self.shift && matches!(self.key.as_str(), "c" | "d" | "z" | "i" | "m")
            {
                return false;
            }
            if !self.modified() && matches!(self.key.as_str(), "esc" | "backspace") {
                return false;
            }
        }
        true
    }
}

fn parse_chord(raw: &str) -> Result<WindowsKeyChord, WindowsSettingsError> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 128 || raw.chars().any(char::is_control) {
        return Err(error("invalid-binding"));
    }
    let mut chord = WindowsKeyChord {
        key: String::new(),
        ctrl: false,
        alt: false,
        shift: false,
        super_key: false,
    };
    for part in raw.split('+').map(str::trim) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => chord.ctrl = true,
            "alt" | "option" | "meta" => chord.alt = true,
            "shift" => chord.shift = true,
            "super" | "cmd" | "command" => chord.super_key = true,
            key => {
                if !chord.key.is_empty() {
                    return Err(error("invalid-binding"));
                }
                chord.key = match key {
                    "escape" => "esc",
                    "return" => "enter",
                    "pgup" => "pageup",
                    "pgdn" => "pagedown",
                    "plus" => "+",
                    "minus" => "-",
                    "space" => " ",
                    "esc" | "enter" | "tab" | "backspace" | "delete" | "insert" | "home"
                    | "end" | "pageup" | "pagedown" | "up" | "down" | "left" | "right" => key,
                    _ if key
                        .strip_prefix('f')
                        .and_then(|number| number.parse::<u8>().ok())
                        .is_some_and(|number| (1..=24).contains(&number)) =>
                    {
                        key
                    }
                    _ if part.chars().count() == 1 => part,
                    _ => return Err(error("invalid-binding")),
                }
                .to_owned();
            }
        }
    }
    if chord.key.is_empty() {
        return Err(error("invalid-binding"));
    }
    if chord.key.len() == 1 && chord.key.as_bytes()[0].is_ascii_uppercase() {
        chord.key.make_ascii_lowercase();
        chord.shift = true;
    }
    if let Some(number) = chord
        .key
        .strip_prefix('f')
        .and_then(|number| number.parse::<u8>().ok())
    {
        chord.key = format!("f{number}");
    }
    match chord.key.as_str() {
        "enter" => {
            chord.key = "m".into();
            chord.ctrl = true;
        }
        "tab" => {
            chord.key = "i".into();
            chord.ctrl = true;
        }
        "5" if chord.ctrl => chord.key = "]".into(),
        _ => {}
    }
    Ok(chord)
}

#[cfg(test)]
mod tests;

impl WindowsKeySettings {
    pub fn command_catalog(&self) -> Vec<WindowsCustomCommandEntry> {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(self.profile().as_bytes());
        let namespace: String = hash[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.commands
            .iter()
            .enumerate()
            .map(|(index, definition)| WindowsCustomCommandEntry {
                id: format!("cmd_{namespace}_{index}"),
                definition: definition.clone(),
            })
            .collect()
    }
    pub fn from_profile(profile: &str) -> Result<Self, WindowsSettingsError> {
        if profile.len() > 64 * 1024 {
            return Err(error("too-large"));
        }
        let value: toml::Table = toml::from_str(profile).map_err(|_| error("invalid-toml"))?;
        if value.len() != 1 {
            return Err(error("unknown-key"));
        }
        Self::parse_with_limit(value.get("keys").ok_or(error("unknown-key"))?, 144)
    }
    pub fn profile(&self) -> String {
        fn chord(value: &WindowsKeyChord) -> String {
            let mut parts = Vec::new();
            for (enabled, name) in [
                (value.ctrl, "ctrl"),
                (value.alt, "alt"),
                (value.shift, "shift"),
                (value.super_key, "super"),
            ] {
                if enabled {
                    parts.push(name);
                }
            }
            parts.push(match value.key.as_str() {
                " " => "space",
                "+" => "plus",
                key => key,
            });
            parts.join("+")
        }
        let quoted = |value: &str| serde_json::to_string(value).expect("strings always encode");
        let mut text = format!("[keys]\nprefix={}\n", quoted(&chord(&self.prefix)));
        for setting in &self.overrides {
            let values: Vec<_> = setting
                .triggers
                .iter()
                .map(|trigger| {
                    quoted(&format!(
                        "{}{}",
                        if trigger.prefix { "prefix+" } else { "" },
                        chord(&trigger.chord)
                    ))
                })
                .collect();
            text.push_str(&format!("{}=[{}]\n", setting.action, values.join(",")));
        }
        for command in &self.commands {
            commands::write_profile(command, &mut text);
        }
        text
    }
}

pub fn trigger_label(trigger: &WindowsKeyTrigger) -> String {
    let mut parts = Vec::new();
    if trigger.prefix {
        parts.push("prefix");
    }
    for (enabled, name) in [
        (trigger.chord.ctrl, "ctrl"),
        (trigger.chord.alt, "alt"),
        (trigger.chord.shift, "shift"),
        (trigger.chord.super_key, "super"),
    ] {
        if enabled {
            parts.push(name);
        }
    }
    parts.push(match trigger.chord.key.as_str() {
        " " => "space",
        "+" => "plus",
        key => key,
    });
    parts.join("+")
}
