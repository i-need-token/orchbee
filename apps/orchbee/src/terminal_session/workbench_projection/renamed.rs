use super::{
    NativeContextTabs, NativeProjectionDivergence, WorkbenchLabel, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId, validate_canonical_candidate,
};

/// Captured request context and known revision, not an echoed wire target or cleanup proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenamedTabExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    requested_label: WorkbenchLabel,
}

impl RenamedTabExpectation {
    pub(in crate::terminal_session) const fn new(
        committed_revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        requested_label: WorkbenchLabel,
    ) -> Self {
        Self {
            committed_revision,
            terminal_context_id,
            tab_id,
            requested_label,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct ValidatedRenamedProjection {
    pub(in crate::terminal_session) candidate: NativeContextTabs,
    pub(in crate::terminal_session) renamed_index: Option<usize>,
}

pub(in crate::terminal_session) fn validate_renamed_tab(
    baseline: &NativeContextTabs,
    expected: &RenamedTabExpectation,
    candidate: NativeContextTabs,
) -> Result<ValidatedRenamedProjection, NativeProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    if baseline.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || &expected.terminal_context_id != baseline.terminal_context_id()
        || candidate.revision < expected.committed_revision
    {
        return Err(NativeProjectionDivergence);
    }
    let target = baseline
        .tab_index(&expected.tab_id)
        .ok_or(NativeProjectionDivergence)?;
    if baseline.tabs[target].tab_label == expected.requested_label {
        return Err(NativeProjectionDivergence);
    }
    if candidate.revision == expected.committed_revision {
        let mut expected_tabs = baseline.tabs.clone();
        expected_tabs[target].tab_label = expected.requested_label.clone();
        if candidate.tabs != expected_tabs {
            return Err(NativeProjectionDivergence);
        }
    }
    let renamed_index = candidate.tab_index(&expected.tab_id);
    Ok(ValidatedRenamedProjection {
        candidate,
        renamed_index,
    })
}
