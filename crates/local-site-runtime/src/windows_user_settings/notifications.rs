//! Manual and automatic notification policy shared by startup, checking and live reload.

use super::{
    DiagnosticsCollector, WindowsSettingsDiagnosticDisposition, WindowsSettingsError,
    unknown_field, windows_user_config_has_child,
};

pub const MAXIMUM_NOTIFICATION_PROFILE_BYTES: usize = 16 * 1024;
pub const MAXIMUM_AGENT_SOUND_OVERRIDES: usize = 64;
pub const MAXIMUM_NOTIFICATION_DELAY_SECONDS: u64 = 3600;

pub use crate::notification_policy::{
    NativeAgentSoundSetting as WindowsAgentSoundSetting,
    NativeNotificationDelivery as WindowsNotificationDelivery,
    NativeNotificationSettings as WindowsNotificationSettings,
};

impl WindowsNotificationSettings {
    pub fn profile(&self) -> String {
        let mut profile = format!(
            "[ui.toast]\ndelivery='{}'\nagent_delivery='{}'\ndelay_seconds={}\n[ui.toast.overlay]\nposition='{}'\n[ui.sound]\nenabled={}\n",
            self.delivery.as_str(),
            self.agent_delivery.as_str(),
            self.delay_seconds,
            self.position,
            self.sound_enabled
        );
        if !self.agent_sounds.is_empty() {
            profile.push_str("[ui.sound.agents]\n");
            for (agent, setting) in &self.agent_sounds {
                profile.push_str(&format!("'{agent}'='{}'\n", setting.as_str()));
            }
        }
        profile
    }

    pub fn from_profile(profile: &str) -> Result<Self, WindowsSettingsError> {
        if profile.len() > MAXIMUM_NOTIFICATION_PROFILE_BYTES {
            return Err(toast_error("value-too-long"));
        }
        let raw: toml::Value = toml::from_str(profile).map_err(|_| toast_error("invalid-toml"))?;
        let root = raw.as_table().ok_or(toast_error("wrong-type"))?;
        if root.len() != 1 {
            return Err(toast_error("unknown-key"));
        }
        let ui = root
            .get("ui")
            .and_then(toml::Value::as_table)
            .ok_or(toast_error("wrong-type"))?;
        if ui.is_empty() {
            return Err(toast_error("wrong-type"));
        }
        if ui
            .keys()
            .any(|key| !matches!(key.as_str(), "toast" | "sound"))
        {
            return Err(toast_error("unknown-key"));
        }
        Self::parse(ui.get("toast"), ui.get("sound"))
    }

    pub(super) fn parse(
        toast: Option<&toml::Value>,
        sound: Option<&toml::Value>,
    ) -> Result<Self, WindowsSettingsError> {
        let mut settings = Self::default();
        if let Some(raw) = toast {
            settings.parse_toast(raw)?;
        }
        if let Some(raw) = sound {
            settings.parse_sound(raw)?;
        }
        Ok(settings)
    }

    pub(super) fn parse_lenient(
        toast: Option<&toml::Value>,
        sound: Option<&toml::Value>,
        diagnostics: &mut DiagnosticsCollector,
    ) -> Self {
        let mut settings = Self::default();
        if let Some(raw) = toast {
            settings.parse_toast_lenient(raw, diagnostics);
        }
        if let Some(raw) = sound {
            settings.parse_sound_lenient(raw, diagnostics);
        }
        settings
    }

    fn parse_toast_lenient(&mut self, raw: &toml::Value, diagnostics: &mut DiagnosticsCollector) {
        let Some(table) = raw.as_table() else {
            diagnostics.push(
                "ui.toast",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            return;
        };
        for key in table
            .keys()
            .filter(|key| !windows_user_config_has_child("ui.toast", key))
        {
            diagnostics.push(
                unknown_field("ui.toast", key),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        if let Some(delivery) = table.get("delivery") {
            match parse_delivery(delivery, toast_error) {
                Ok(value) => self.delivery = value,
                Err(error) => {
                    self.delivery = WindowsNotificationDelivery::Off;
                    diagnostics.push(
                        "ui.toast.delivery",
                        error.reason,
                        WindowsSettingsDiagnosticDisposition::Defaulted,
                    );
                }
            }
        }
        if let Some(delivery) = table.get("agent_delivery") {
            match parse_delivery(delivery, toast_error) {
                Ok(value) => self.agent_delivery = value,
                Err(error) => diagnostics.push(
                    "ui.toast.agent_delivery",
                    error.reason,
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
        if let Some(delay) = table.get("delay_seconds") {
            match delay.as_integer().and_then(|value| value.try_into().ok()) {
                Some(value) if value <= MAXIMUM_NOTIFICATION_DELAY_SECONDS => {
                    self.delay_seconds = value;
                }
                Some(_) => diagnostics.push(
                    "ui.toast.delay_seconds",
                    "invalid-delay",
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                ),
                None => diagnostics.push(
                    "ui.toast.delay_seconds",
                    if delay.as_integer().is_some() {
                        "invalid-delay"
                    } else {
                        "wrong-type"
                    },
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
        let Some(overlay) = table.get("overlay") else {
            return;
        };
        let Some(overlay) = overlay.as_table() else {
            diagnostics.push(
                "ui.toast.overlay",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            return;
        };
        for key in overlay
            .keys()
            .filter(|key| !windows_user_config_has_child("ui.toast.overlay", key))
        {
            diagnostics.push(
                unknown_field("ui.toast.overlay", key),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        if let Some(position) = overlay.get("position") {
            match position.as_str() {
                Some("top-left") => self.position = "top-left",
                Some("top-right") => self.position = "top-right",
                Some("bottom-left") => self.position = "bottom-left",
                Some("bottom-right") => self.position = "bottom-right",
                Some(_) => diagnostics.push(
                    "ui.toast.overlay.position",
                    "invalid-mode",
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                ),
                None => diagnostics.push(
                    "ui.toast.overlay.position",
                    "wrong-type",
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
    }

    fn parse_sound_lenient(&mut self, raw: &toml::Value, diagnostics: &mut DiagnosticsCollector) {
        let Some(table) = raw.as_table() else {
            diagnostics.push(
                "ui.sound",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            return;
        };
        for key in table
            .keys()
            .filter(|key| !windows_user_config_has_child("ui.sound", key))
        {
            diagnostics.push(
                unknown_field("ui.sound", key),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        if let Some(enabled) = table.get("enabled") {
            match enabled.as_bool() {
                Some(value) => self.sound_enabled = value,
                None => diagnostics.push(
                    "ui.sound.enabled",
                    "wrong-type",
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
        let Some(agents) = table.get("agents") else {
            return;
        };
        let Some(agents) = agents.as_table() else {
            diagnostics.push(
                "ui.sound.agents",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            return;
        };
        if agents.len() > MAXIMUM_AGENT_SOUND_OVERRIDES {
            diagnostics.push(
                "ui.sound.agents",
                "too-many-agents",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            return;
        }
        for (agent, value) in agents {
            let field = unknown_field("ui.sound.agents", agent);
            if !valid_agent_id(agent) {
                diagnostics.push(
                    field,
                    "invalid-agent",
                    WindowsSettingsDiagnosticDisposition::Ignored,
                );
                continue;
            }
            let setting = match value.as_str() {
                Some("default") => WindowsAgentSoundSetting::Default,
                Some("on") => WindowsAgentSoundSetting::On,
                Some("off") => WindowsAgentSoundSetting::Off,
                Some(_) => {
                    diagnostics.push(
                        field,
                        "invalid-mode",
                        WindowsSettingsDiagnosticDisposition::Defaulted,
                    );
                    continue;
                }
                None => {
                    diagnostics.push(
                        field,
                        "wrong-type",
                        WindowsSettingsDiagnosticDisposition::Defaulted,
                    );
                    continue;
                }
            };
            self.agent_sounds.insert(agent.clone(), setting);
        }
    }

    fn parse_toast(&mut self, raw: &toml::Value) -> Result<(), WindowsSettingsError> {
        let table = raw.as_table().ok_or(toast_error("wrong-type"))?;
        if table
            .keys()
            .any(|key| !windows_user_config_has_child("ui.toast", key))
        {
            return Err(toast_error("unknown-key"));
        }
        if let Some(delivery) = table.get("delivery") {
            self.delivery = parse_delivery(delivery, toast_error)?;
        }
        if let Some(delivery) = table.get("agent_delivery") {
            self.agent_delivery = parse_delivery(delivery, toast_error)?;
        }
        if let Some(delay) = table.get("delay_seconds") {
            self.delay_seconds = delay
                .as_integer()
                .ok_or(toast_error("wrong-type"))?
                .try_into()
                .map_err(|_| toast_error("invalid-delay"))?;
            if self.delay_seconds > MAXIMUM_NOTIFICATION_DELAY_SECONDS {
                return Err(toast_error("invalid-delay"));
            }
        }
        if let Some(overlay) = table.get("overlay") {
            let overlay = overlay.as_table().ok_or(toast_error("wrong-type"))?;
            if overlay
                .keys()
                .any(|key| !windows_user_config_has_child("ui.toast.overlay", key))
            {
                return Err(toast_error("unknown-key"));
            }
            if let Some(position) = overlay.get("position") {
                self.position = match position.as_str().ok_or(toast_error("wrong-type"))? {
                    "top-left" => "top-left",
                    "top-right" => "top-right",
                    "bottom-left" => "bottom-left",
                    "bottom-right" => "bottom-right",
                    _ => return Err(toast_error("invalid-mode")),
                };
            }
        }
        Ok(())
    }

    fn parse_sound(&mut self, raw: &toml::Value) -> Result<(), WindowsSettingsError> {
        let table = raw.as_table().ok_or(sound_error("wrong-type"))?;
        if table
            .keys()
            .any(|key| !windows_user_config_has_child("ui.sound", key))
        {
            return Err(sound_error("unknown-key"));
        }
        if let Some(enabled) = table.get("enabled") {
            self.sound_enabled = enabled.as_bool().ok_or(sound_error("wrong-type"))?;
        }
        let Some(agents) = table.get("agents") else {
            return Ok(());
        };
        let agents = agents.as_table().ok_or(sound_error("wrong-type"))?;
        if agents.len() > MAXIMUM_AGENT_SOUND_OVERRIDES {
            return Err(sound_error("too-many-agents"));
        }
        for (agent, value) in agents {
            if !valid_agent_id(agent) {
                return Err(sound_error("invalid-agent"));
            }
            let setting = match value.as_str().ok_or(sound_error("wrong-type"))? {
                "default" => WindowsAgentSoundSetting::Default,
                "on" => WindowsAgentSoundSetting::On,
                "off" => WindowsAgentSoundSetting::Off,
                _ => return Err(sound_error("invalid-mode")),
            };
            self.agent_sounds.insert(agent.clone(), setting);
        }
        Ok(())
    }
}

fn parse_delivery(
    value: &toml::Value,
    error: fn(&'static str) -> WindowsSettingsError,
) -> Result<WindowsNotificationDelivery, WindowsSettingsError> {
    Ok(match value.as_str().ok_or(error("wrong-type"))? {
        "off" => WindowsNotificationDelivery::Off,
        "orchbee" => WindowsNotificationDelivery::Tui,
        "terminal" => WindowsNotificationDelivery::Terminal,
        "system" => WindowsNotificationDelivery::System,
        _ => return Err(error("invalid-mode")),
    })
}

fn valid_agent_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte))
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn toast_error(reason: &'static str) -> WindowsSettingsError {
    WindowsSettingsError::new("ui.toast", reason)
}

fn sound_error(reason: &'static str) -> WindowsSettingsError {
    WindowsSettingsError::new("ui.sound", reason)
}

#[cfg(test)]
mod tests;
