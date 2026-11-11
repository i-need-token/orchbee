//! Manual and automatic notification policy shared by startup, checking and live reload.
//!
//! The Server-to-Client profile uses the file's spelling: one `[notifications]` table with
//! `cli_delivery`, `agent_delivery` (profile only), `delay_seconds`, `position`, `sound` and a
//! `[notifications.sound_by_agent]` table. A profile still spelled `[ui.toast]` and `[ui.sound]`
//! stays readable; nothing writes it.

use super::{DiagnosticsCollector, SettingsDiagnosticDisposition, SettingsError, unknown_field};

pub const MAXIMUM_NOTIFICATION_PROFILE_BYTES: usize = 16 * 1024;
pub const MAXIMUM_AGENT_SOUND_OVERRIDES: usize = 64;
pub const MAXIMUM_NOTIFICATION_DELAY_SECONDS: u64 = 3600;

pub use crate::notification_policy::{
    AgentSoundSetting, NotificationDelivery, NotificationSettings,
};

impl NotificationSettings {
    pub fn profile(&self) -> String {
        let mut profile = format!(
            "[notifications]\ncli_delivery='{}'\nagent_delivery='{}'\ndelay_seconds={}\nposition='{}'\nsound={}\n",
            self.delivery.as_str(),
            self.agent_delivery.as_str(),
            self.delay_seconds,
            self.position,
            self.sound_enabled
        );
        if !self.agent_sounds.is_empty() {
            profile.push_str("[notifications.sound_by_agent]\n");
            for (agent, setting) in &self.agent_sounds {
                profile.push_str(&format!("'{agent}'='{}'\n", setting.as_str()));
            }
        }
        profile
    }

    /// Admit a profile in full; every refusal names `notifications`.
    pub fn from_profile(profile: &str) -> Result<Self, SettingsError> {
        if profile.len() > MAXIMUM_NOTIFICATION_PROFILE_BYTES {
            return Err(profile_error("value-too-long"));
        }
        let root: toml::Table =
            toml::from_str(profile).map_err(|_| profile_error("invalid-toml"))?;
        let mut entries = root.iter();
        let (Some((name, table)), None) = (entries.next(), entries.next()) else {
            return Err(profile_error("unknown-key"));
        };
        let table = table
            .as_table()
            .filter(|table| !table.is_empty())
            .ok_or(profile_error("wrong-type"))?;
        match name.as_str() {
            "notifications" => Self::parse_profile(table),
            "ui" => Self::parse_legacy_profile(table),
            _ => Err(profile_error("unknown-key")),
        }
    }

    fn parse_profile(table: &toml::Table) -> Result<Self, SettingsError> {
        let mut settings = Self::default();
        if table.keys().any(|key| {
            !matches!(
                key.as_str(),
                "cli_delivery"
                    | "agent_delivery"
                    | "delay_seconds"
                    | "position"
                    | "sound"
                    | "sound_by_agent"
            )
        }) {
            return Err(profile_error("unknown-key"));
        }
        if let Some(delivery) = table.get("cli_delivery") {
            settings.delivery = parse_delivery(delivery, profile_error)?;
        }
        if let Some(delivery) = table.get("agent_delivery") {
            settings.agent_delivery = parse_delivery(delivery, profile_error)?;
        }
        if let Some(delay) = table.get("delay_seconds") {
            settings.delay_seconds = parse_delay(delay)?;
        }
        if let Some(position) = table.get("position") {
            settings.position = parse_position(position)?;
        }
        if let Some(sound) = table.get("sound") {
            settings.sound_enabled = sound.as_bool().ok_or(profile_error("wrong-type"))?;
        }
        if let Some(agents) = table.get("sound_by_agent") {
            settings.parse_agent_sounds(agents)?;
        }
        Ok(settings)
    }

    /// The `[ui.toast]` and `[ui.sound]` spelling profiles had before they took the file's.
    fn parse_legacy_profile(ui: &toml::Table) -> Result<Self, SettingsError> {
        if ui
            .keys()
            .any(|key| !matches!(key.as_str(), "toast" | "sound"))
        {
            return Err(profile_error("unknown-key"));
        }
        let mut settings = Self::default();
        if let Some(raw) = ui.get("toast") {
            settings.parse_toast(raw)?;
        }
        if let Some(raw) = ui.get("sound") {
            settings.parse_sound(raw)?;
        }
        Ok(settings)
    }

    /// Admit the `[notifications]` keys this policy owns. Unknown keys are reported by the
    /// reader's registry walk; the common keys in the same table feed the typed values instead.
    pub(super) fn parse_lenient(
        table: &toml::Table,
        diagnostics: &mut DiagnosticsCollector,
    ) -> Self {
        let mut settings = Self::default();
        if let Some(delivery) = table.get("cli_delivery") {
            match parse_delivery(delivery, profile_error) {
                Ok(value) => settings.delivery = value,
                Err(error) => diagnostics.push(
                    "notifications.cli_delivery",
                    error.reason,
                    SettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
        if let Some(delay) = table.get("delay_seconds") {
            match delay.as_integer().map(u64::try_from) {
                Some(Ok(value)) if value <= MAXIMUM_NOTIFICATION_DELAY_SECONDS => {
                    settings.delay_seconds = value;
                }
                Some(_) => diagnostics.push(
                    "notifications.delay_seconds",
                    "invalid-delay",
                    SettingsDiagnosticDisposition::Defaulted,
                ),
                None => diagnostics.push(
                    "notifications.delay_seconds",
                    "wrong-type",
                    SettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
        if let Some(position) = table.get("position") {
            match position.as_str().map(corner) {
                Some(Some(corner)) => settings.position = corner,
                Some(None) => diagnostics.push(
                    "notifications.position",
                    "invalid-mode",
                    SettingsDiagnosticDisposition::Defaulted,
                ),
                None => diagnostics.push(
                    "notifications.position",
                    "wrong-type",
                    SettingsDiagnosticDisposition::Defaulted,
                ),
            }
        }
        if let Some(agents) = table.get("sound_by_agent") {
            settings.parse_agent_sounds_lenient(agents, diagnostics);
        }
        settings
    }

    fn parse_agent_sounds_lenient(
        &mut self,
        agents: &toml::Value,
        diagnostics: &mut DiagnosticsCollector,
    ) {
        const FIELD: &str = "notifications.sound_by_agent";
        let Some(agents) = agents.as_table() else {
            diagnostics.push(
                FIELD,
                "wrong-type",
                SettingsDiagnosticDisposition::Defaulted,
            );
            return;
        };
        if agents.len() > MAXIMUM_AGENT_SOUND_OVERRIDES {
            diagnostics.push(
                FIELD,
                "too-many-agents",
                SettingsDiagnosticDisposition::Defaulted,
            );
            return;
        }
        for (agent, value) in agents {
            let field = unknown_field(FIELD, agent);
            if !valid_agent_id(agent) {
                diagnostics.push(
                    field,
                    "invalid-agent",
                    SettingsDiagnosticDisposition::Ignored,
                );
                continue;
            }
            let setting = match value.as_str() {
                Some("default") => AgentSoundSetting::Default,
                Some("on") => AgentSoundSetting::On,
                Some("off") => AgentSoundSetting::Off,
                Some(_) => {
                    diagnostics.push(
                        field,
                        "invalid-mode",
                        SettingsDiagnosticDisposition::Defaulted,
                    );
                    continue;
                }
                None => {
                    diagnostics.push(
                        field,
                        "wrong-type",
                        SettingsDiagnosticDisposition::Defaulted,
                    );
                    continue;
                }
            };
            self.agent_sounds.insert(agent.clone(), setting);
        }
    }

    fn parse_toast(&mut self, raw: &toml::Value) -> Result<(), SettingsError> {
        let table = raw.as_table().ok_or(profile_error("wrong-type"))?;
        if table.keys().any(|key| {
            !matches!(
                key.as_str(),
                "delivery" | "agent_delivery" | "delay_seconds" | "overlay"
            )
        }) {
            return Err(profile_error("unknown-key"));
        }
        if let Some(delivery) = table.get("delivery") {
            self.delivery = parse_delivery(delivery, profile_error)?;
        }
        if let Some(delivery) = table.get("agent_delivery") {
            self.agent_delivery = parse_delivery(delivery, profile_error)?;
        }
        if let Some(delay) = table.get("delay_seconds") {
            self.delay_seconds = parse_delay(delay)?;
        }
        if let Some(overlay) = table.get("overlay") {
            let overlay = overlay.as_table().ok_or(profile_error("wrong-type"))?;
            if overlay.keys().any(|key| key != "position") {
                return Err(profile_error("unknown-key"));
            }
            if let Some(position) = overlay.get("position") {
                self.position = parse_position(position)?;
            }
        }
        Ok(())
    }

    fn parse_sound(&mut self, raw: &toml::Value) -> Result<(), SettingsError> {
        let table = raw.as_table().ok_or(profile_error("wrong-type"))?;
        if table
            .keys()
            .any(|key| !matches!(key.as_str(), "enabled" | "agents"))
        {
            return Err(profile_error("unknown-key"));
        }
        if let Some(enabled) = table.get("enabled") {
            self.sound_enabled = enabled.as_bool().ok_or(profile_error("wrong-type"))?;
        }
        match table.get("agents") {
            Some(agents) => self.parse_agent_sounds(agents),
            None => Ok(()),
        }
    }

    fn parse_agent_sounds(&mut self, agents: &toml::Value) -> Result<(), SettingsError> {
        let agents = agents.as_table().ok_or(profile_error("wrong-type"))?;
        if agents.len() > MAXIMUM_AGENT_SOUND_OVERRIDES {
            return Err(profile_error("too-many-agents"));
        }
        for (agent, value) in agents {
            if !valid_agent_id(agent) {
                return Err(profile_error("invalid-agent"));
            }
            let setting = match value.as_str().ok_or(profile_error("wrong-type"))? {
                "default" => AgentSoundSetting::Default,
                "on" => AgentSoundSetting::On,
                "off" => AgentSoundSetting::Off,
                _ => return Err(profile_error("invalid-mode")),
            };
            self.agent_sounds.insert(agent.clone(), setting);
        }
        Ok(())
    }
}

fn parse_delay(value: &toml::Value) -> Result<u64, SettingsError> {
    let delay: u64 = value
        .as_integer()
        .ok_or(profile_error("wrong-type"))?
        .try_into()
        .map_err(|_| profile_error("invalid-delay"))?;
    if delay > MAXIMUM_NOTIFICATION_DELAY_SECONDS {
        return Err(profile_error("invalid-delay"));
    }
    Ok(delay)
}

fn parse_position(value: &toml::Value) -> Result<&'static str, SettingsError> {
    corner(value.as_str().ok_or(profile_error("wrong-type"))?).ok_or(profile_error("invalid-mode"))
}

fn parse_delivery(
    value: &toml::Value,
    error: fn(&'static str) -> SettingsError,
) -> Result<NotificationDelivery, SettingsError> {
    Ok(match value.as_str().ok_or(error("wrong-type"))? {
        "off" => NotificationDelivery::Off,
        "orchbee" => NotificationDelivery::Tui,
        "terminal" => NotificationDelivery::Terminal,
        "system" => NotificationDelivery::System,
        _ => return Err(error("invalid-mode")),
    })
}

fn corner(value: &str) -> Option<&'static str> {
    ["top-left", "top-right", "bottom-left", "bottom-right"]
        .into_iter()
        .find(|corner| *corner == value)
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

fn profile_error(reason: &'static str) -> SettingsError {
    SettingsError::new("notifications", reason)
}

#[cfg(test)]
mod tests;
