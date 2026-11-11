//! The native feature projection for the common settings both Clients present.
//!
//! The settings registry in Local Runtime remains the key, default, validation, and persistence
//! owner. This table owns only the TUI composition facts that every native Settings consumer must
//! share.

use super::rows::Row;
use orchbee_local_runtime::UserSettingsField;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingCategory {
    Appearance,
    Terminal,
    Notifications,
    LaunchProfiles,
    /// Holds the entry into Agent integrations; no preference belongs to it.
    AgentIntegrations,
    /// Section headings on the Agent integrations page.
    LocalAgentAdapters,
    RemoteAgentHosts,
}

impl SettingCategory {
    /// The General page's categories in order.
    pub(crate) const ALL: &'static [Self] = &[
        Self::Appearance,
        Self::Terminal,
        Self::Notifications,
        Self::LaunchProfiles,
        Self::AgentIntegrations,
    ];
}

pub(crate) use crate::localization::SettingUnit;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingControl {
    FiniteChoice,
    Boolean,
    Color,
    BoundedInteger {
        minimum: u64,
        maximum: u64,
        small_step: u64,
        large_step: u64,
        unit: SettingUnit,
    },
    Catalog,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingAvailability {
    Always,
    TerminalDisplayThemeCustom,
    /// A truecolor host; fewer colors collapse a named theme's canvas tint and interaction mixes.
    NamedThemes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingEffect {
    CurrentApplication,
    CurrentTerminalPresentation,
    SubsequentSelection,
    ExistingAgentPresentation,
    FutureNotification,
    NewTerminal,
    NewDefaultLaunch,
    /// Applies when the Server next restarts from its checkpoint.
    NextRestart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingMetadata {
    /// Stable machine identity and localization descriptor identity.
    pub(crate) key: UserSettingsField,
    pub(crate) row: Row,
    pub(crate) category: SettingCategory,
    pub(crate) control: SettingControl,
    pub(crate) availability: SettingAvailability,
    pub(crate) effect: SettingEffect,
}

pub(crate) static SETTINGS: [SettingMetadata; 18] = [
    SettingMetadata {
        key: UserSettingsField::Locale,
        row: Row::Language,
        category: SettingCategory::Appearance,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::CurrentApplication,
    },
    SettingMetadata {
        key: UserSettingsField::ApplicationTheme,
        row: Row::Theme,
        category: SettingCategory::Appearance,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::CurrentApplication,
    },
    SettingMetadata {
        key: UserSettingsField::LightTheme,
        row: Row::LightTheme,
        category: SettingCategory::Appearance,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::NamedThemes,
        effect: SettingEffect::CurrentApplication,
    },
    SettingMetadata {
        key: UserSettingsField::DarkTheme,
        row: Row::DarkTheme,
        category: SettingCategory::Appearance,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::NamedThemes,
        effect: SettingEffect::CurrentApplication,
    },
    SettingMetadata {
        key: UserSettingsField::TuiColorSource,
        row: Row::ColorSource,
        category: SettingCategory::Appearance,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::NamedThemes,
        effect: SettingEffect::CurrentApplication,
    },
    SettingMetadata {
        key: UserSettingsField::TerminalDisplayTheme,
        row: Row::TerminalTheme,
        category: SettingCategory::Appearance,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::CurrentTerminalPresentation,
    },
    SettingMetadata {
        key: UserSettingsField::CustomForeground,
        row: Row::Foreground,
        category: SettingCategory::Appearance,
        control: SettingControl::Color,
        availability: SettingAvailability::TerminalDisplayThemeCustom,
        effect: SettingEffect::CurrentTerminalPresentation,
    },
    SettingMetadata {
        key: UserSettingsField::CustomBackground,
        row: Row::Background,
        category: SettingCategory::Appearance,
        control: SettingControl::Color,
        availability: SettingAvailability::TerminalDisplayThemeCustom,
        effect: SettingEffect::CurrentTerminalPresentation,
    },
    SettingMetadata {
        key: UserSettingsField::CopyOnSelect,
        row: Row::CopyOnSelect,
        category: SettingCategory::Terminal,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::SubsequentSelection,
    },
    SettingMetadata {
        key: UserSettingsField::TrimTrailingBlankRowsForAgents,
        row: Row::TrimAgents,
        category: SettingCategory::Terminal,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::ExistingAgentPresentation,
    },
    SettingMetadata {
        key: UserSettingsField::ResumeAgentsOnRestore,
        row: Row::ResumeAgents,
        category: SettingCategory::Terminal,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::NextRestart,
    },
    SettingMetadata {
        key: UserSettingsField::RestoreTerminalOutput,
        row: Row::RestoreOutput,
        category: SettingCategory::Terminal,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::NextRestart,
    },
    SettingMetadata {
        key: UserSettingsField::AgentNotificationMode,
        row: Row::Reminders,
        category: SettingCategory::Notifications,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::FutureNotification,
    },
    SettingMetadata {
        key: UserSettingsField::CompletionReminderMode,
        row: Row::CompletionReminders,
        category: SettingCategory::Notifications,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::FutureNotification,
    },
    SettingMetadata {
        key: UserSettingsField::CompletionReminderMinimumSeconds,
        row: Row::CompletionThreshold,
        category: SettingCategory::Notifications,
        control: SettingControl::BoundedInteger {
            minimum: 1,
            maximum: 3_600,
            small_step: 1,
            large_step: 10,
            unit: SettingUnit::Seconds,
        },
        availability: SettingAvailability::Always,
        effect: SettingEffect::FutureNotification,
    },
    SettingMetadata {
        key: UserSettingsField::SoundEnabled,
        row: Row::Sound,
        category: SettingCategory::Notifications,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::FutureNotification,
    },
    SettingMetadata {
        key: UserSettingsField::ScrollbackBytes,
        row: Row::Scrollback,
        category: SettingCategory::Terminal,
        control: SettingControl::BoundedInteger {
            minimum: 0,
            maximum: 1_000_000_000,
            small_step: 1_000_000,
            large_step: 10_000_000,
            unit: SettingUnit::Bytes,
        },
        availability: SettingAvailability::Always,
        effect: SettingEffect::NewTerminal,
    },
    SettingMetadata {
        key: UserSettingsField::LaunchProfiles,
        row: Row::LaunchProfiles,
        category: SettingCategory::LaunchProfiles,
        control: SettingControl::Catalog,
        availability: SettingAvailability::Always,
        effect: SettingEffect::NewDefaultLaunch,
    },
];

pub(crate) fn setting(key: UserSettingsField) -> &'static SettingMetadata {
    SETTINGS
        .iter()
        .find(|metadata| metadata.key == key)
        .expect("complete UserSettingsField metadata")
}

pub(crate) fn setting_for_row(row: Row) -> Option<&'static SettingMetadata> {
    SETTINGS.iter().find(|metadata| metadata.row == row)
}

impl Row {
    /// The longest text draft this row admits: a metadata control's own bound, or the editor
    /// bound of a launch-profile text row.
    pub(crate) fn text_limit(self) -> Option<usize> {
        if let Some(metadata) = setting_for_row(self) {
            return match metadata.control {
                SettingControl::Color => Some(7),
                SettingControl::BoundedInteger { maximum, .. } => Some(maximum.to_string().len()),
                _ => None,
            };
        }
        match self {
            Self::ProfileName => Some(512),
            Self::Executable | Self::ArgumentValue => Some(32 * 1024),
            _ => None,
        }
    }
}
