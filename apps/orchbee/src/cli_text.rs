//! Owns CLI help and human-result formatting over the lower locale value.
//! Grammar, failure details and terminal escaping stay here; machine envelope fields are unchanged.

mod agent_view_spec;
mod client_control;
mod client_layout;
mod help;
mod help_arguments;
mod help_options;
pub(crate) use help::{HelpDetails, HelpText};
mod config_reset;
mod destructive_request;
mod details;
mod diagnostics;
pub(crate) mod escape;
mod layouts;
mod named_sessions;
mod organization;
mod panel_agent;
mod panel_metadata;
mod panel_relocation;
mod panel_terminal_creation;
mod readable;
mod receipt;
pub(crate) use details::DetailLabel;
mod runtime_status;
mod schema_export;
mod workspace_metadata;
use crate::cli_outcome::Failure;
use crate::localization::Locale;
mod agent;
mod agent_changes;
mod agent_diagnostics;
mod agent_names;
mod agent_prompt;
mod agent_start;
mod command_help;
mod configuration;
#[cfg(test)]
pub(crate) use configuration::CONFIGURATION_FILE_PLACEHOLDER;
pub(crate) use configuration::fill_environment_placeholders;
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
mod terminal_clear;
mod terminal_copy;
mod terminal_editor;
mod terminal_links;
mod usage;
mod workbench;
mod workspace;
mod workspace_creation;
mod workspace_group_close;
mod worktree;
pub(crate) use labels::CliLabels;

/// Each failure `resource` token with the word text names it by and the
/// command that lists its valid targets; an unnamed resource is a Project.
const FAILURE_RESOURCES: [(&str, &str, &str); 8] = [
    ("project", "project", "orchbee project list"),
    ("workspace", "workspace", "orchbee workspace list"),
    ("tab", "tab", "orchbee tab list"),
    ("panel", "panel", "orchbee panel list"),
    ("terminal", "terminal", "orchbee terminal list"),
    ("schema", "schema", "orchbee api schema"),
    ("directory", "directory", "orchbee project get <project>"),
    (
        "launch-profile",
        "launch profile",
        "orchbee launch-profile list",
    ),
];

fn failure_resource(failure: &Failure) -> (&'static str, &'static str) {
    let token = failure
        .details
        .as_ref()
        .and_then(|details| details.get("resource"))
        .and_then(|value| value.as_str());
    let (_, word, list) = FAILURE_RESOURCES
        .iter()
        .find(|(known, ..)| Some(*known) == token)
        .unwrap_or(&FAILURE_RESOURCES[0]);
    (word, list)
}

/// The command a not-found refusal of this failure's resource points to.
fn failure_list_command(failure: &Failure) -> &'static str {
    failure_resource(failure).1
}

fn failure_target(failure: &Failure) -> (&'static str, String) {
    let resource = failure_resource(failure).0;
    let target = failure
        .details
        .as_ref()
        .and_then(|details| details.get("target"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_owned();
    (resource, target)
}

mod agent_manifests;
mod agent_projection;
mod configured_commands;
mod events;
mod machines;
mod notification;
mod operation;
mod pane_graphics;
mod plugins;
mod runtime_configuration;
mod update;
mod window_title;

/// CLI presentation context. It owns grammar/result formatting without extending the lower locale type.
#[derive(Clone, Copy)]
pub(crate) struct CliText {
    locale: Locale,
}

impl CliText {
    /// Bind existing CLI formatters to a locale without reading environment or performing effects.
    pub(crate) const fn new(locale: Locale) -> Self {
        Self { locale }
    }
}

#[cfg(test)]
mod tests;
