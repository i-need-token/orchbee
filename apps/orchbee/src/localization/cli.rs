//! Owns the CLI localization module boundary and its shared presentation types.
//! Feature modules format human output; machine envelope fields remain unchanged.

mod agent_view_spec;
mod client_control;
mod client_layout;
mod help;
mod help_arguments;
mod help_options;
pub(crate) use help::{HelpDetails, HelpText};
#[cfg(windows)]
mod config_reset;
mod details;
#[cfg(windows)]
mod layouts;
#[cfg(windows)]
mod named_sessions;
mod organization;
mod panel_agent;
mod panel_metadata;
#[cfg(windows)]
mod panel_relocation;
#[cfg(windows)]
mod panel_terminal_creation;
mod readable;
mod receipt;
pub(crate) use details::DetailLabel;
mod runtime_status;
#[cfg(windows)]
mod schema_export;
mod workspace_metadata;
use super::Locale;
use crate::cli::Failure;
mod agent;
mod agent_changes;
mod agent_diagnostics;
mod agent_names;
mod agent_prompt;
mod agent_start;
mod command_help;
mod configuration;
mod failure;
mod integration;
mod labels;
mod launch_profile;
mod panel;
mod project;
mod server;
mod tab;
mod terminal;
mod terminal_attach;
#[cfg(windows)]
mod terminal_clear;
#[cfg(windows)]
mod terminal_copy;
#[cfg(windows)]
mod terminal_editor;
#[cfg(windows)]
mod terminal_links;
mod usage;
mod workbench;
mod workspace;
#[cfg(windows)]
mod workspace_creation;
mod workspace_group_close;
mod worktree;
pub(crate) use labels::CliLabels;

fn failure_target(failure: &Failure) -> (&'static str, String) {
    let resource = failure
        .details
        .as_ref()
        .and_then(|details| details.get("resource"))
        .and_then(|value| value.as_str())
        .unwrap_or("project");
    let resource = match resource {
        "workspace" => "workspace",
        "tab" => "tab",
        "panel" => "panel",
        "terminal" => "terminal",
        "schema" => "schema",
        "directory" => "directory",
        "launch-profile" => "launch profile",
        _ => "project",
    };
    let target = failure
        .details
        .as_ref()
        .and_then(|details| details.get("target"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_owned();
    (resource, target)
}
