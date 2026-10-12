//! Platform-independent native notification policy; platform adapters own persistence and delivery.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NativeNotificationDelivery {
    #[default]
    Off,
    Tui,
    Terminal,
    System,
}

impl NativeNotificationDelivery {
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
pub enum NativeAgentSoundSetting {
    #[default]
    Default,
    On,
    Off,
}

impl NativeAgentSoundSetting {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeNotificationSettings {
    /// Existing manual `notification show` visual delivery.
    pub delivery: NativeNotificationDelivery,
    /// Client-local automatic physical-Agent popup delivery.
    pub agent_delivery: NativeNotificationDelivery,
    pub delay_seconds: u64,
    pub sound_enabled: bool,
    pub(crate) agent_sounds: BTreeMap<String, NativeAgentSoundSetting>,
    pub(crate) position: &'static str,
}

impl Default for NativeNotificationSettings {
    fn default() -> Self {
        Self {
            delivery: NativeNotificationDelivery::Off,
            agent_delivery: NativeNotificationDelivery::Tui,
            delay_seconds: 1,
            sound_enabled: false,
            agent_sounds: BTreeMap::new(),
            position: "bottom-right",
        }
    }
}

impl NativeNotificationSettings {
    pub const fn position(&self) -> &'static str {
        self.position
    }

    pub fn agent_sounds(&self) -> &BTreeMap<String, NativeAgentSoundSetting> {
        &self.agent_sounds
    }

    pub fn agent_sound(&self, agent: &str) -> NativeAgentSoundSetting {
        self.agent_sounds.get(agent).copied().unwrap_or_default()
    }
}
