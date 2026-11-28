//! The one table from a `client-server` capability to the TUI actions that need it. A Session whose
//! handshake answer lacks a capability disables only those actions, which show as unavailable with
//! their reason before invocation; every other action and the connection stay. The action's own
//! owner still checks the answer again at its effect boundary.

use orchbee_connection_handshake::{
    AGENT_COMPLETION_ACKNOWLEDGE, AGENT_CONFIRMATION, AGENT_INPUT_AREA, COMPATIBILITY_WINDOW,
    CONFIGURED_COMMAND_SOURCE_EXITED, CapabilitySet, FILES, FILES_EDIT, FILES_SEARCH,
    PROJECT_IDENTITY, PROMPT_GATED_INPUT, RECONNECT, STREAM_OUTCOME_UNKNOWN,
    TERMINAL_LIMIT_REACHED, TERMINAL_RUNTIME, UPDATE_CHECK, UPDATE_PHASES, WORKSPACE_IDLE,
};

/// A TUI action that only a Session listing a certain capability can serve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionAction {
    /// The Start Agent picker's start in the focused Panel, which types the start line at a
    /// confirmed prompt. Its other places have the Server start the Agent in a new shell.
    StartInThisPanel,
    /// About's Check for updates, which asks the Session Server to run its update job now.
    CheckForUpdates,
    /// A double-click on a finished Agent's sidebar row, which tells the Session the person saw it
    /// finish, as typing into its terminal does.
    AcknowledgeAgentCompletion,
}

/// Every capability this build's `client-server` offer lists, with the TUI actions that need it.
const NEEDED_BY: &[(&str, &[SessionAction])] = &[
    // Without it, a double-click on a finished Agent's row only opens the Agent; typing into its
    // terminal still acknowledges it.
    (
        AGENT_COMPLETION_ACKNOWLEDGE,
        &[SessionAction::AcknowledgeAgentCompletion],
    ),
    // The confirmations follower reads it itself: without it, the TUI asks for no confirmation
    // rows and shows no offer, menu entry or reason for them.
    (AGENT_CONFIRMATION, &[]),
    // Only the CLI's recent Agent read asks for the input area, and it reads without one.
    (AGENT_INPUT_AREA, &[]),
    // Update reads it itself: without it, a press waits instead of leaving for the new version.
    (COMPATIBILITY_WINDOW, &[]),
    // The Server reads it itself: without it, a command from an exited Panel is refused as
    // unavailable instead of naming the exited terminal.
    (CONFIGURED_COMMAND_SOURCE_EXITED, &[]),
    // The TUI has no File Bar and opens no File Bar stream, with or without them.
    (FILES, &[]),
    (FILES_EDIT, &[]),
    (FILES_SEARCH, &[]),
    // Only a random Project identity joins Projects of different Sessions, and a Session without
    // it allocates none; the TUI names no Project identity of its own.
    (PROJECT_IDENTITY, &[]),
    (PROMPT_GATED_INPUT, &[SessionAction::StartInThisPanel]),
    // The Server reads it itself: without it, a restart refuses while this Client is connected.
    (RECONNECT, &[]),
    // The connection reads it itself: without it, an intent that fails after it ran ends the
    // connection instead of closing its own stream as unknown.
    (STREAM_OUTCOME_UNKNOWN, &[]),
    // The Server reads it itself: without it, a create past the terminal limit is refused as
    // server-busy instead of asking to close some terminals.
    (TERMINAL_LIMIT_REACHED, &[]),
    // The terminal-runtime follower reads it itself: without it, the TUI keeps its own recovery.
    (TERMINAL_RUNTIME, &[]),
    (UPDATE_CHECK, &[SessionAction::CheckForUpdates]),
    // About's check reads it itself: without it, a run shows Checking until it ends and a failure
    // names no phase.
    (UPDATE_PHASES, &[]),
    // The command line reads it for a Workspace's close or move: without it, the move says it
    // could not check what runs in the Workspace and asks.
    (WORKSPACE_IDLE, &[]),
];

/// Whether `action` is available under the Session's handshake answer. Without an answer, as
/// before the first connection is promoted, nothing is known to be missing.
pub(crate) fn serves(answer: Option<&CapabilitySet>, action: SessionAction) -> bool {
    answer.is_none_or(|answer| {
        NEEDED_BY
            .iter()
            .filter(|(_, actions)| actions.contains(&action))
            .all(|(capability, _)| answer.contains(capability))
    })
}

#[cfg(test)]
mod tests;
