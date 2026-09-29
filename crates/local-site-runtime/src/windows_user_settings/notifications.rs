//! Custom-notification policy shared by startup, configuration checking and live reload.
use super::WindowsSettingsError;
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsNotificationSettings {
    pub delivery: WindowsNotificationDelivery,
    position: &'static str,
}
impl Default for WindowsNotificationSettings {
    fn default() -> Self {
        Self {
            delivery: WindowsNotificationDelivery::Off,
            position: "bottom-right",
        }
    }
}
impl WindowsNotificationSettings {
    pub const fn position(&self) -> &'static str {
        self.position
    }
    pub fn profile(&self) -> String {
        format!(
            "[ui.toast]\ndelivery='{}'\n[ui.toast.herdr]\nposition='{}'\n",
            self.delivery.as_str(),
            self.position
        )
    }
    pub fn from_profile(profile: &str) -> Result<Self, WindowsSettingsError> {
        if profile.len() > 1024 {
            return Err(error("value-too-long"));
        }
        let raw: toml::Value = toml::from_str(profile).map_err(|_| error("invalid-toml"))?;
        let root = raw.as_table().ok_or(error("wrong-type"))?;
        if root.len() != 1 {
            return Err(error("unknown-key"));
        }
        let ui = root
            .get("ui")
            .and_then(toml::Value::as_table)
            .ok_or(error("wrong-type"))?;
        if ui.len() != 1 {
            return Err(error("unknown-key"));
        }
        Self::parse(ui.get("toast").ok_or(error("wrong-type"))?)
    }
    pub(super) fn parse(raw: &toml::Value) -> Result<Self, WindowsSettingsError> {
        let table = raw.as_table().ok_or(error("wrong-type"))?;
        if table
            .keys()
            .any(|key| !matches!(key.as_str(), "delivery" | "enabled" | "herdr"))
        {
            return Err(error("unknown-key"));
        }
        let mut settings = Self::default();
        if let Some(enabled) = table.get("enabled") {
            settings.delivery = if enabled.as_bool().ok_or(error("wrong-type"))? {
                WindowsNotificationDelivery::Tui
            } else {
                WindowsNotificationDelivery::Off
            };
        }
        if let Some(delivery) = table.get("delivery") {
            settings.delivery = match delivery.as_str().ok_or(error("wrong-type"))? {
                "off" => WindowsNotificationDelivery::Off,
                "orchbee" | "herdr" => WindowsNotificationDelivery::Tui,
                "terminal" => WindowsNotificationDelivery::Terminal,
                "system" => WindowsNotificationDelivery::System,
                _ => return Err(error("invalid-mode")),
            };
        }
        if let Some(herdr) = table.get("herdr") {
            let herdr = herdr.as_table().ok_or(error("wrong-type"))?;
            if herdr.keys().any(|key| key != "position") {
                return Err(error("unknown-key"));
            }
            if let Some(position) = herdr.get("position") {
                settings.position = match position.as_str().ok_or(error("wrong-type"))? {
                    "top-left" => "top-left",
                    "top-right" => "top-right",
                    "bottom-left" => "bottom-left",
                    "bottom-right" => "bottom-right",
                    _ => return Err(error("invalid-mode")),
                };
            }
        }
        Ok(settings)
    }
}
fn error(reason: &'static str) -> WindowsSettingsError {
    WindowsSettingsError::new("ui.toast", reason)
}
#[cfg(test)]
mod tests;
