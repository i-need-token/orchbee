//! Manual and automatic notification policy shared by startup, checking and live reload.
use std::collections::BTreeMap;

use super::WindowsSettingsError;

pub const MAXIMUM_NOTIFICATION_PROFILE_BYTES: usize = 16 * 1024;
pub const MAXIMUM_AGENT_SOUND_OVERRIDES: usize = 64;
pub const MAXIMUM_NOTIFICATION_DELAY_SECONDS: u64 = 3600;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowsNotificationDelivery {
    #[default]
    Off,
    Tui,
    Terminal,
    System,
}

impl WindowsNotificationDelivery {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Tui => "orchbee",
            Self::Terminal => "terminal",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowsAgentSoundSetting {
    #[default]
    Default,
    On,
    Off,
}

impl WindowsAgentSoundSetting {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsNotificationSettings {
    /// Existing manual `notification show` visual delivery.
    pub delivery: WindowsNotificationDelivery,
    /// Client-local automatic physical-Agent popup delivery.
    pub agent_delivery: WindowsNotificationDelivery,
    pub delay_seconds: u64,
    pub sound_enabled: bool,
    agent_sounds: BTreeMap<String, WindowsAgentSoundSetting>,
    position: &'static str,
}

impl Default for WindowsNotificationSettings {
    fn default() -> Self {
        Self {
            delivery: WindowsNotificationDelivery::Off,
            agent_delivery: WindowsNotificationDelivery::Tui,
            delay_seconds: 1,
            sound_enabled: false,
            agent_sounds: BTreeMap::new(),
            position: "bottom-right",
        }
    }
}

impl WindowsNotificationSettings {
    pub const fn position(&self) -> &'static str {
        self.position
    }

    pub fn agent_sounds(&self) -> &BTreeMap<String, WindowsAgentSoundSetting> {
        &self.agent_sounds
    }

    pub fn agent_sound(&self, agent: &str) -> WindowsAgentSoundSetting {
        self.agent_sounds.get(agent).copied().unwrap_or_default()
    }

    pub fn profile(&self) -> String {
        let mut profile = format!(
            "[ui.toast]\ndelivery='{}'\nagent_delivery='{}'\ndelay_seconds={}\n[ui.toast.herdr]\nposition='{}'\n[ui.sound]\nenabled={}\n",
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

    fn parse_toast(&mut self, raw: &toml::Value) -> Result<(), WindowsSettingsError> {
        let table = raw.as_table().ok_or(toast_error("wrong-type"))?;
        if table.keys().any(|key| {
            !matches!(
                key.as_str(),
                "delivery" | "enabled" | "agent_delivery" | "delay_seconds" | "herdr"
            )
        }) {
            return Err(toast_error("unknown-key"));
        }
        if let Some(enabled) = table.get("enabled") {
            self.delivery = if enabled.as_bool().ok_or(toast_error("wrong-type"))? {
                WindowsNotificationDelivery::Tui
            } else {
                WindowsNotificationDelivery::Off
            };
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
        if let Some(herdr) = table.get("herdr") {
            let herdr = herdr.as_table().ok_or(toast_error("wrong-type"))?;
            if herdr.keys().any(|key| key != "position") {
                return Err(toast_error("unknown-key"));
            }
            if let Some(position) = herdr.get("position") {
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
            .any(|key| !matches!(key.as_str(), "enabled" | "agents"))
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
        "orchbee" | "herdr" => WindowsNotificationDelivery::Tui,
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
