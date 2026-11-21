//! The facts a row declares about the common setting it edits.
//!
//! The settings registry in Local Runtime remains the key, default, validation, and persistence
//! owner. A row's declaration in `rows` carries only the TUI composition facts that every native
//! Settings consumer must share.

use orchbee_local_runtime::UserSettingsField;

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
    FutureNotification,
    NewTerminal,
    NewDefaultLaunch,
    /// Applies when the Server next restarts from its checkpoint.
    NextRestart,
    /// Applies from the Server's next background update check.
    NextUpdateCheck,
    /// Applies to the confirmations Agents show from now on; turning it off also stops an
    /// answer already decided.
    AgentConfirmations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingMetadata {
    /// Stable machine identity and localization descriptor identity.
    pub(crate) key: UserSettingsField,
    pub(crate) control: SettingControl,
    pub(crate) availability: SettingAvailability,
    pub(crate) effect: SettingEffect,
}
