use super::{
    ClientContextTabs, ClientProjectionDivergence, WorkbenchLabel, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId, validate_canonical_candidate,
};

/// Captured request context and known revision, not an echoed wire target or cleanup proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct RenamedTabExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    requested_label: WorkbenchLabel,
}

impl RenamedTabExpectation {
    pub(in crate::workbench_client) const fn new(
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
pub(in crate::workbench_client) struct ValidatedRenamedProjection {
    pub(in crate::workbench_client) candidate: ClientContextTabs,
    pub(in crate::workbench_client) renamed_index: Option<usize>,
}

pub(in crate::workbench_client) fn validate_renamed_tab(
    baseline: &ClientContextTabs,
    expected: &RenamedTabExpectation,
    candidate: ClientContextTabs,
) -> Result<ValidatedRenamedProjection, ClientProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    super::tab_metadata::validate_receipt(baseline, &candidate, expected.committed_revision)?;
    if baseline.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || Some(&expected.terminal_context_id) != baseline.terminal_context_id()
        || candidate.revision < expected.committed_revision
    {
        return Err(ClientProjectionDivergence);
    }
    let target = baseline
        .tab_index(&expected.tab_id)
        .ok_or(ClientProjectionDivergence)?;
    if !baseline.tabs[target].automatic_label
        && baseline.tabs[target].tab_label == expected.requested_label
    {
        return Err(ClientProjectionDivergence);
    }
    if candidate.revision == expected.committed_revision {
        let mut expected_tabs = baseline.tabs.clone();
        expected_tabs[target].tab_label = expected.requested_label.clone();
        expected_tabs[target].automatic_label = false;
        if candidate.tabs != expected_tabs {
            return Err(ClientProjectionDivergence);
        }
    }
    let renamed_index = candidate.tab_index(&expected.tab_id);
    Ok(ValidatedRenamedProjection {
        candidate,
        renamed_index,
    })
}
