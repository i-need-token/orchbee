//! Validates the captured Tab rename and committed revision against a canonical candidate
//! while preserving unrelated membership. It does not treat an echoed wire label as canonical
//! proof or infer attachment cleanup.

use super::{
    ClientContextTabs, ClientProjectionDivergence, WorkbenchLabel, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId, validate_canonical_candidate,
};

/// Captured request context and known revision, not an echoed wire target or cleanup proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenamedTabExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    requested_label: WorkbenchLabel,
}

impl RenamedTabExpectation {
    pub(crate) const fn new(
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
pub(crate) struct ValidatedRenamedProjection {
    pub(crate) candidate: ClientContextTabs,
    pub(crate) renamed_index: Option<usize>,
}

pub(crate) fn validate_renamed_tab(
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

/// A Panel rename changes only the primary Presentation's override, including when a child is shown.
pub(crate) fn validate_renamed_panel(
    baseline: &ClientContextTabs,
    revision: WorkbenchRevision,
    tab_id: &WorkbenchTabId,
    panel_id: &orchbee_workbench_protocol::WorkbenchPanelId,
    label: &orchbee_workbench_protocol::PanelLabelUpdate,
    candidate: ClientContextTabs,
) -> Result<ValidatedRenamedProjection, ClientProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    super::tab_metadata::validate_receipt(baseline, &candidate, revision)?;
    if baseline.revision.value().checked_add(1) != Some(revision.value())
        || candidate.revision < revision
    {
        return Err(ClientProjectionDivergence);
    }
    let target = baseline
        .tab_index(tab_id)
        .ok_or(ClientProjectionDivergence)?;
    let panel = baseline.tabs[target]
        .panel_index(panel_id)
        .ok_or(ClientProjectionDivergence)?;
    let requested = label.custom_label().cloned();
    if baseline.tabs[target].panels[panel].primary.custom_label == requested {
        return Err(ClientProjectionDivergence);
    }
    if candidate.revision == revision {
        let mut expected_tabs = baseline.tabs.clone();
        expected_tabs[target].panels[panel].primary.custom_label = requested;
        if candidate.tabs != expected_tabs {
            return Err(ClientProjectionDivergence);
        }
    }
    let renamed_index = candidate.tab_index(tab_id);
    Ok(ValidatedRenamedProjection {
        candidate,
        renamed_index,
    })
}
