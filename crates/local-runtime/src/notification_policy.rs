//! Platform-independent native notification policy; platform adapters own persistence and delivery.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotificationDelivery {
    #[default]
    Off,
    Tui,
    Terminal,
    System,
}

impl NotificationDelivery {
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
pub enum AgentSoundSetting {
    #[default]
    Default,
    On,
    Off,
}

impl AgentSoundSetting {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationSettings {
    /// Existing manual `notification show` visual delivery.
    pub delivery: NotificationDelivery,
    /// Client-local automatic physical-Agent popup delivery.
    pub agent_delivery: NotificationDelivery,
    pub delay_seconds: u64,
    pub sound_enabled: bool,
    pub(crate) agent_sounds: BTreeMap<String, AgentSoundSetting>,
    pub(crate) position: &'static str,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            delivery: NotificationDelivery::Off,
            agent_delivery: NotificationDelivery::Tui,
            delay_seconds: 1,
            sound_enabled: false,
            agent_sounds: BTreeMap::new(),
            position: "bottom-right",
        }
    }
}

impl NotificationSettings {
    pub const fn position(&self) -> &'static str {
        self.position
    }

    pub fn agent_sounds(&self) -> &BTreeMap<String, AgentSoundSetting> {
        &self.agent_sounds
    }

    pub fn agent_sound(&self, agent: &str) -> AgentSoundSetting {
        self.agent_sounds.get(agent).copied().unwrap_or_default()
    }
}
