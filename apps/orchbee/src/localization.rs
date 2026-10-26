//! Owns locale selection and the localized presentation boundary for this executable.
//! Feature modules own human-facing copy; domain and protocol identifiers stay unchanged.

#[cfg(windows)]
mod action_menu;
#[cfg(windows)]
mod agent_projection;
mod control_takeover;
mod directory_startup;
#[cfg(windows)]
mod machines;
#[cfg(windows)]
mod navigator;
mod notification;
mod operation;
mod presentation;
#[cfg(windows)]
mod tab_menu;
pub(crate) use presentation::RecoveryVerb;
#[cfg(windows)]
mod plugins;
mod shortcut_help;
pub(crate) use shortcut_help::{ShortcutHelpEntry, shortcut_filter_matches};
mod runtime_configuration;
mod settings;
mod status;
mod terminal_feedback;
#[cfg(windows)]
mod update;
mod window_title;
use std::ffi::OsString;

mod agent_manifests;
mod cli;
pub(crate) use cli::{HelpDetails, HelpText};
#[cfg(windows)]
mod configured_commands;
#[cfg(windows)]
mod copy_mode;
#[cfg(windows)]
mod events;
#[cfg(windows)]
mod pane_graphics;
#[cfg(windows)]
mod panel_reports;
pub(crate) use cli::{CliLabels, DetailLabel};
mod agent_board;
#[cfg(any(unix, windows))]
mod location_labels;
mod onboarding;
#[cfg(any(unix, windows))]
mod panel_close;
mod prefix;
mod session_status;
mod shell;
mod startup;
#[cfg(any(unix, windows))]
mod tab_close;
#[cfg(any(unix, windows))]
mod tab_rename;
mod terminal_history;
#[cfg(any(unix, windows))]
mod workspace_close;
#[cfg(any(unix, windows))]
mod workspace_setup;
#[cfg(windows)]
mod worktree_launch;
pub(crate) use shell::ShellText;

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
