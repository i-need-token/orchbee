//! The native feature projection for the complete shared preference inventory.
//!
//! Workbench Protocol remains the value, default, validation, and persistence owner. This table
//! owns only the TUI composition facts that every native Settings consumer must share.

use super::rows::Row;
use orchbee_workbench_protocol::PreferenceKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingCategory {
    Language,
    ApplicationAppearance,
    TerminalColors,
    SelectionAndClipboard,
    History,
    ShellIntegration,
    LaunchProfiles,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingUnit {
    Seconds,
    Bytes,
}

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingMetadata {
    /// Stable machine identity and localization descriptor identity.
    pub(crate) key: PreferenceKey,
    pub(crate) row: Row,
    pub(crate) category: SettingCategory,
    pub(crate) control: SettingControl,
    pub(crate) availability: SettingAvailability,
    pub(crate) effect: SettingEffect,
}

pub(crate) static SETTINGS: [SettingMetadata; 13] = [
    SettingMetadata {
        key: PreferenceKey::Locale,
        row: Row::Language,
        category: SettingCategory::Language,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::CurrentApplication,
    },
    SettingMetadata {
        key: PreferenceKey::ApplicationTheme,
        row: Row::Theme,
        category: SettingCategory::ApplicationAppearance,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::CurrentApplication,
    },
    SettingMetadata {
        key: PreferenceKey::TerminalDisplayTheme,
        row: Row::TerminalTheme,
        category: SettingCategory::TerminalColors,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::CurrentTerminalPresentation,
    },
    SettingMetadata {
        key: PreferenceKey::CustomForeground,
        row: Row::Foreground,
        category: SettingCategory::TerminalColors,
        control: SettingControl::Color,
        availability: SettingAvailability::TerminalDisplayThemeCustom,
        effect: SettingEffect::CurrentTerminalPresentation,
    },
    SettingMetadata {
        key: PreferenceKey::CustomBackground,
        row: Row::Background,
        category: SettingCategory::TerminalColors,
        control: SettingControl::Color,
        availability: SettingAvailability::TerminalDisplayThemeCustom,
        effect: SettingEffect::CurrentTerminalPresentation,
    },
    SettingMetadata {
        key: PreferenceKey::CopyOnSelect,
        row: Row::CopyOnSelect,
        category: SettingCategory::SelectionAndClipboard,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::SubsequentSelection,
    },
    SettingMetadata {
        key: PreferenceKey::TrimTrailingBlankRowsForAgents,
        row: Row::TrimAgents,
        category: SettingCategory::History,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::ExistingAgentPresentation,
    },
    SettingMetadata {
        key: PreferenceKey::AgentNotificationMode,
        row: Row::Reminders,
        category: SettingCategory::ShellIntegration,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::FutureNotification,
    },
    SettingMetadata {
        key: PreferenceKey::CompletionReminderMode,
        row: Row::CompletionReminders,
        category: SettingCategory::ShellIntegration,
        control: SettingControl::FiniteChoice,
        availability: SettingAvailability::Always,
        effect: SettingEffect::FutureNotification,
    },
    SettingMetadata {
        key: PreferenceKey::CompletionReminderMinimumSeconds,
        row: Row::CompletionThreshold,
        category: SettingCategory::ShellIntegration,
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
        key: PreferenceKey::SoundEnabled,
        row: Row::Sound,
        category: SettingCategory::ShellIntegration,
        control: SettingControl::Boolean,
        availability: SettingAvailability::Always,
        effect: SettingEffect::FutureNotification,
    },
    SettingMetadata {
        key: PreferenceKey::ScrollbackBytes,
        row: Row::Scrollback,
        category: SettingCategory::History,
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
        key: PreferenceKey::LaunchProfiles,
        row: Row::LaunchProfiles,
        category: SettingCategory::LaunchProfiles,
        control: SettingControl::Catalog,
        availability: SettingAvailability::Always,
        effect: SettingEffect::NewDefaultLaunch,
    },
];

pub(crate) fn setting(key: PreferenceKey) -> &'static SettingMetadata {
    SETTINGS
        .iter()
        .find(|metadata| metadata.key == key)
        .expect("complete PreferenceKey metadata")
}

pub(crate) fn setting_for_row(row: Row) -> Option<&'static SettingMetadata> {
    SETTINGS.iter().find(|metadata| metadata.row == row)
}
