//! Agent completions the File Bar passes on, so the serving process refreshes the Git state of a
//! Workspace whose Agent finished a turn at once. Each completion reaches the process once, and
//! only a process that serves and heard of the Workspace; one that finishes while no process
//! serves goes nowhere, since a process reads every repository afresh when it starts.

use super::link::Link;
use orchbee_files_protocol::{LinkMessage, WorkspaceId};
use orchbee_server::SessionEventJournal;

/// Tells `link`, when a process serves, of each Agent completion `journal` recorded after
/// position `after`, and returns the position to read after next.
pub(super) fn settle(journal: &SessionEventJournal, link: Option<&Link>, after: u64) -> u64 {
    let (latest, workspaces) = journal.agent_completions(after);
    if let Some(link) = link {
        let scopes = link.scopes();
        for workspace in workspaces {
            let Ok(workspace) = WorkspaceId::parse(workspace.as_str()) else {
                continue;
            };
            if scopes.holds(workspace) {
                link.send(&LinkMessage::AgentSettled { workspace });
            }
        }
    }
    latest
}

#[cfg(test)]
mod tests;
