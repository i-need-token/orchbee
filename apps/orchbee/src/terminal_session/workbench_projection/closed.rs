use super::{
    NativeProjectionDivergence, NativeStandaloneTabs, SiteId, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId, validate_canonical_candidate,
};

/// Canonical membership evidence only; this never proves viewer or controller cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct ClosedTabExpectation {
    site_id: SiteId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    minimum_revision: WorkbenchRevision,
}

impl ClosedTabExpectation {
    pub(in crate::terminal_session) const fn new(
        site_id: SiteId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        minimum_revision: WorkbenchRevision,
    ) -> Self {
        Self {
            site_id,
            terminal_context_id,
            tab_id,
            minimum_revision,
        }
    }

    pub(in crate::terminal_session) const fn site_id(&self) -> &SiteId {
        &self.site_id
    }
    pub(in crate::terminal_session) const fn terminal_context_id(
        &self,
    ) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }
    pub(in crate::terminal_session) const fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }
    pub(in crate::terminal_session) const fn minimum_revision(&self) -> WorkbenchRevision {
        self.minimum_revision
    }
}

pub(in crate::terminal_session) fn validate_closed_tab(
    current: &NativeStandaloneTabs,
    expected: &ClosedTabExpectation,
    candidate: NativeStandaloneTabs,
) -> Result<NativeStandaloneTabs, NativeProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    if candidate.site_id() != expected.site_id()
        || candidate.terminal_context_id() != expected.terminal_context_id()
        || candidate.revision() < expected.minimum_revision()
        || candidate.tab_index(expected.tab_id()).is_some()
    {
        return Err(NativeProjectionDivergence);
    }
    Ok(candidate)
}
