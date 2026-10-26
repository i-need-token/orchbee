//! Validates a captured Tab's canonical removal at or after its known revision within the
//! exact Session and context. It proves membership absence only, never physical terminal
//! disposal or viewer and controller cleanup.

use super::{
    ClientContextTabs, ClientProjectionDivergence, SessionId, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId, validate_canonical_candidate,
};

/// Canonical membership evidence only; this never proves viewer or controller cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct ClosedTabExpectation {
    session_id: SessionId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    minimum_revision: WorkbenchRevision,
}

impl ClosedTabExpectation {
    pub(in crate::workbench_client) const fn new(
        session_id: SessionId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        minimum_revision: WorkbenchRevision,
    ) -> Self {
        Self {
            session_id,
            terminal_context_id,
            tab_id,
            minimum_revision,
        }
    }

    pub(in crate::workbench_client) const fn session_id(&self) -> &SessionId {
        &self.session_id
    }
    pub(in crate::workbench_client) const fn terminal_context_id(
        &self,
    ) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }
    pub(in crate::workbench_client) const fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }
    pub(in crate::workbench_client) const fn minimum_revision(&self) -> WorkbenchRevision {
        self.minimum_revision
    }
}

pub(in crate::workbench_client) fn validate_closed_tab(
    current: &ClientContextTabs,
    expected: &ClosedTabExpectation,
    candidate: ClientContextTabs,
) -> Result<ClientContextTabs, ClientProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    if candidate.session_id() != expected.session_id()
        || candidate.terminal_context_id() != Some(expected.terminal_context_id())
        || candidate.revision() < expected.minimum_revision()
        || candidate.tab_index(expected.tab_id()).is_some()
    {
        return Err(ClientProjectionDivergence);
    }
    Ok(candidate)
}
