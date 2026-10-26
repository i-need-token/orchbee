//! Owns locale selection and the localized presentation boundary for this executable.
//! Feature modules own human-facing copy; domain and protocol identifiers stay unchanged.

mod action_menu;
pub(crate) use action_menu::{ActionSearchAlias, MenuLabel, MenuUnavailable};
mod parameters;
pub(crate) use parameters::{BackendAction, Notice, PanelDirection};
mod agent_projection;
mod control_takeover;
pub(crate) use control_takeover::TakeoverStatus;
mod directory_startup;
mod machines;
pub(crate) use machines::SidebarActionLabel;
pub(crate) use machines::{MachineControlLabel, SidebarPlacementLabel};
mod navigator;
mod notification;
mod operation;
mod presentation;
mod tab_menu;
pub(crate) use presentation::RecoveryVerb;
pub(crate) use tab_menu::{TabMenuPath, TabMenuScope};
mod plugins;
mod shortcut_help;
pub(crate) use shortcut_help::{ShortcutContext, ShortcutHelpGroup, ShortcutLabel};
mod capabilities;
mod runtime_configuration;
mod settings;
pub(crate) use settings::SettingsNotice;
mod status;
mod terminal_feedback;
pub(crate) use terminal_feedback::{CompletionReminder, TerminalEnd};
mod update;
mod window_title;
use std::ffi::OsString;

mod agent_manifests;
mod cli;
pub(crate) use cli::{HelpDetails, HelpText};
mod configured_commands;
mod copy_mode;
pub(crate) use copy_mode::{CopyHintLabel, CopyModeStatus};
mod events;
mod pane_graphics;
mod panel_reports;
pub(crate) use cli::{CliLabels, DetailLabel};
mod agent_board;
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
    ClientProjectionStatus, DisplayController, DisplayPhase, DisplayProvenance, DisplayRetirement,
    DuplicateProgress, DuplicateStop, LaunchFailure, LaunchProfile, NoticeContext, RefreshStatus,
    TerminalStatus, WorkbenchClientNotice,
};
mod shell;
mod startup;
#[cfg(any(unix, windows))]
mod tab_close;
pub(crate) use tab_close::{
    CloseConfirmationKind, CloseSetStatus, CloseStatusKind, CloseStatusText,
};
#[cfg(any(unix, windows))]
mod tab_rename;
mod terminal_history;
#[cfg(any(unix, windows))]
mod workspace_close;
pub(crate) use workspace_close::{WorkspaceCloseStatus, WorkspaceCloseText};
#[cfg(any(unix, windows))]
mod workspace_setup;
pub(crate) use workspace_setup::{SetupStage, SetupStatus, WorkspaceCreationPhase};
mod worktree_launch;
pub(crate) use shell::{ShellBindings, ShellText};
pub(crate) use worktree_launch::{WorktreeMode, WorktreeNotice, WorktreeStage};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    EnUs,
    ZhCn,
}

pub(crate) fn select_locale(mut read_environment: impl FnMut(&str) -> Option<OsString>) -> Locale {
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
pub(crate) fn locale_source() -> (&'static str, &'static str) {
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

#[cfg(test)]
mod tests;
