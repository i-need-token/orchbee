//! Owns locale selection and the localized presentation boundary for this executable.
//! Feature modules own human-facing copy; domain and protocol identifiers stay unchanged.

mod action_menu;
pub(crate) use action_menu::{
    ActionSearchAlias, MenuHeading, MenuLabel, MenuRowNote, MenuUnavailable,
};
mod parameters;
pub use parameters::Notice;
pub(crate) use parameters::{BackendAction, PanelDirection};
mod control_takeover;
mod data_versions;
pub(crate) use control_takeover::TakeoverStatus;
mod directory_startup;
mod environment;
mod layout_history;
pub(crate) use layout_history::EarlierLayoutNotice;
mod machines;
pub(crate) use machines::SidebarActionLabel;
pub(crate) use machines::{MachineControlLabel, SidebarPlacementLabel};
mod navigator;
mod notification;
mod presentation;
mod tab_menu;
pub(crate) use presentation::RecoveryVerb;
pub(crate) use tab_menu::TabMenuScope;
mod plugins;
mod shortcut_help;
pub(crate) use shortcut_help::{ShortcutContext, ShortcutHelpGroup, ShortcutLabel};
mod capabilities;
mod settings;
pub(crate) use settings::{
    IntegrationAdapter, IntegrationBlock, IntegrationFactLabel, IntegrationFailure,
    IntegrationFinding, IntegrationStatus, IntegrationStep, IntegrationUnknown, SettingUnit,
    SettingsCategory, SettingsChoice, SettingsDetail, SettingsDetailKind, SettingsFieldRequirement,
    SettingsFileFact, SettingsGroup, SettingsHintLabel, SettingsNotice, SettingsProfileLabel,
    SettingsRow, SettingsValue,
};
mod status;
mod terminal_feedback;
pub(crate) use status::PersistentStatus;
use std::ffi::OsString;
pub(crate) use terminal_feedback::{CompletionReminder, HeldRestoreAction, TerminalEnd};

mod copy_mode;
pub(crate) use copy_mode::{CopyHintLabel, CopyModeStatus};
mod agent_board;
mod panel_reports;
pub(crate) use agent_board::AgentStartStatus;
pub(crate) use agent_board::{
    AgentBoardRead, AgentBoardReadFailure, AgentBoardSummary, JumpFailure,
};
#[cfg(any(unix, windows))]
mod location_labels;
pub(crate) use location_labels::{LocationLabelStatus, LocationLabelTarget};
mod onboarding;
pub(crate) use onboarding::OnboardingLabel;
#[cfg(any(unix, windows))]
mod panel_close;
mod prefix;
pub(crate) use prefix::PrefixDesc;
mod session_status;
pub(crate) use session_status::{
    ClientProjectionStatus, DisplayController, DisplayProvenance, DisplayRetirement,
    DuplicateProgress, DuplicateStop, LaunchFailure, LaunchProfile, NoticeContext, RefreshStatus,
    WorkbenchClientNotice,
};
pub use session_status::{DisplayPhase, TerminalStatus};
mod shell;
mod startup;
#[cfg(any(unix, windows))]
mod tab_close;
pub(crate) use tab_close::{
    CloseConfirmationKind, CloseImpact, CloseSetStatus, CloseStatusKind, CloseStatusText,
};
#[cfg(any(unix, windows))]
mod tab_rename;
mod terminal_history;
mod terminal_title;
mod tooltip;
pub(crate) use tooltip::TooltipLabel;
#[cfg(any(unix, windows))]
mod workspace_close;
pub(crate) use workspace_close::{WorkspaceCloseStatus, WorkspaceCloseText};
#[cfg(any(unix, windows))]
mod workspace_setup;
pub(crate) use workspace_setup::{FieldError, SetupStage, SetupStatus, WorkspaceCreationPhase};
mod worktree_launch;
pub(crate) use shell::{ShellBindings, ShellText};
pub(crate) use worktree_launch::{WorktreeFieldError, WorktreeMode, WorktreeNotice, WorktreeStage};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
    EnUs,
    ZhCn,
}

pub fn select_locale(mut read_environment: impl FnMut(&str) -> Option<OsString>) -> Locale {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = read_environment(name) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        return locale_from_value(&value);
    }
    Locale::EnUs
}

/// Effective locale plus the variable that selected it (`default` when none),
/// reading only LC_ALL/LC_MESSAGES/LANG.
pub fn locale_source() -> (&'static str, &'static str) {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = std::env::var_os(name) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        return (locale_name(locale_from_value(&value)), name);
    }
    ("en-US", "default")
}

pub(crate) const fn locale_name(locale: Locale) -> &'static str {
    match locale {
        Locale::EnUs => "en-US",
        Locale::ZhCn => "zh-CN",
    }
}

fn locale_from_value(value: &OsString) -> Locale {
    let Some(value) = value.to_str() else {
        return Locale::EnUs;
    };
    let bytes = value.as_bytes();
    match (bytes.first(), bytes.get(1)) {
        (Some(first), Some(second))
            if first.eq_ignore_ascii_case(&b'z') && second.eq_ignore_ascii_case(&b'h') =>
        {
            Locale::ZhCn
        }
        _ => Locale::EnUs,
    }
}

/// Crate-internal `Locale` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait LocaleTestAccess {
    fn machine_picker_text(self, key: &str) -> &'static str;
    fn shell(self) -> ShellText<'static>;
}

#[cfg(any(test, feature = "test-support"))]
impl LocaleTestAccess for Locale {
    fn machine_picker_text(self, key: &str) -> &'static str {
        Locale::machine_picker_text(self, key)
    }
    fn shell(self) -> ShellText<'static> {
        Locale::shell(self)
    }
}

/// Crate-internal `ShellText` copy the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait ShellTextTestAccess {
    fn too_small_notice(&self) -> &'static str;
}

#[cfg(any(test, feature = "test-support"))]
impl<'a> ShellTextTestAccess for ShellText<'a> {
    fn too_small_notice(&self) -> &'static str {
        self.too_small_notice
    }
}

#[cfg(test)]
mod tests;
