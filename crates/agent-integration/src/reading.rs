//! What an operation starts and what its plan does, as every Client states it: which integrations
//! are read by starting the Agent's own program, when a report left a row not checked, and what a
//! planned provider command is for. The writer follows the same rule when it inspects.

use crate::{Availability, IntegrationId, ManagementMode, PlannedStep, SnapshotItem, State};

impl IntegrationId {
    /// Whether reading this integration's state starts the Agent's own program, which may create
    /// that Agent's files when it starts. `explain`, `enable` and `disable` start it; `list` never
    /// does and reports such a row as not checked.
    pub const fn read_by_starting_agent(self) -> bool {
        matches!(
            self,
            Self::ClaudeCode | Self::Grok | Self::GitHubCopilot | Self::Hermes
        )
    }
}

/// Whether an `explain` of this row starts the Agent's program: the Agent must be managed here and
/// not known to be missing.
pub const fn explain_starts_agent(
    id: IntegrationId,
    availability: Availability,
    management_mode: ManagementMode,
) -> bool {
    id.read_by_starting_agent()
        && matches!(management_mode, ManagementMode::ManagedLocal)
        && !matches!(availability, Availability::NotFound)
}

impl SnapshotItem {
    /// A found, managed Agent read only through its own program whose reported state is unknown
    /// with no issue and no timeout: no check read it, as `list` reports it. A failed or timed-out
    /// read always carries an issue.
    pub const fn not_checked(&self) -> bool {
        self.integration_id.read_by_starting_agent()
            && matches!(self.availability, Availability::Available)
            && matches!(self.management_mode, ManagementMode::ManagedLocal)
            && matches!(self.state, State::Unknown)
            && self.issue.is_none()
            && !self.timed_out
    }
}

/// What one planned provider command does, read from its argument vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandPurpose {
    AddPluginSource,
    RemovePluginSource,
    InstallPlugin,
    RemovePlugin,
    EnablePlugin,
    ConfirmPlugin,
}

impl PlannedStep {
    /// The purpose of a provider command, or `None` for another step or an argument vector
    /// outside the plugin commands the writer plans.
    pub fn command_purpose(&self) -> Option<CommandPurpose> {
        let Self::Command { argv, .. } = self else {
            return None;
        };
        command_purpose(argv)
    }
}

fn command_purpose(argv: &[String]) -> Option<CommandPurpose> {
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["plugin" | "plugins", "marketplace", "add", ..] => Some(CommandPurpose::AddPluginSource),
        ["plugin" | "plugins", "marketplace", "remove", ..] => {
            Some(CommandPurpose::RemovePluginSource)
        }
        ["plugin" | "plugins", "install" | "add", ..] => Some(CommandPurpose::InstallPlugin),
        ["plugin" | "plugins", "uninstall" | "remove", ..] => Some(CommandPurpose::RemovePlugin),
        ["plugin" | "plugins", "enable", ..] => Some(CommandPurpose::EnablePlugin),
        ["plugin" | "plugins", "list", ..] => Some(CommandPurpose::ConfirmPlugin),
        _ => None,
    }
}
