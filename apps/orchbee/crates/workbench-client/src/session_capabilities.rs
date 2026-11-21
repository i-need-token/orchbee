//! The one table from a `client-server` capability to the TUI actions that need it. A Session whose
//! handshake answer lacks a capability disables only those actions, which show as unavailable with
//! their reason before invocation; every other action and the connection stay. The action's own
//! owner still checks the answer again at its effect boundary.

use orchbee_connection_handshake::{
    AGENT_CONFIRMATION, AGENT_INPUT_AREA, COMPATIBILITY_WINDOW, CONFIGURED_COMMAND_SOURCE_EXITED,
    CapabilitySet, PROMPT_GATED_INPUT, STREAM_OUTCOME_UNKNOWN, TERMINAL_LIMIT_REACHED,
    TERMINAL_RUNTIME, UPDATE_CHECK,
};

/// A TUI action that only a Session listing a certain capability can serve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionAction {
    /// The Start Agent picker's start in the focused Panel, which types the start line at a
    /// confirmed prompt. Its other places have the Server start the Agent in a new shell.
    StartInThisPanel,
    /// About's Check for updates, which asks the Session Server to run its update job now.
    CheckForUpdates,
}

/// Every capability this build's `client-server` offer lists, with the TUI actions that need it.
const NEEDED_BY: &[(&str, &[SessionAction])] = &[
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
    (PROMPT_GATED_INPUT, &[SessionAction::StartInThisPanel]),
    // The connection reads it itself: without it, an intent that fails after it ran ends the
    // connection instead of closing its own stream as unknown.
    (STREAM_OUTCOME_UNKNOWN, &[]),
    // The Server reads it itself: without it, a create past the terminal limit is refused as
    // server-busy instead of asking to close some terminals.
    (TERMINAL_LIMIT_REACHED, &[]),
    // The terminal-runtime follower reads it itself: without it, the TUI keeps its own recovery.
    (TERMINAL_RUNTIME, &[]),
    (UPDATE_CHECK, &[SessionAction::CheckForUpdates]),
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
