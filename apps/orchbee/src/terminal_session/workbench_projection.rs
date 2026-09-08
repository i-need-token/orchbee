use orchbee_workbench_protocol::{
    MAXIMUM_TABS_PER_CONTEXT, PanelNode, SiteId, Snapshot, Tab, TerminalId, TerminalPresentationId,
    WorkbenchLabel, WorkbenchPanelId, WorkbenchRevision, WorkbenchTabId, WorkbenchTabMoveDirection,
    WorkbenchTerminalContextId, WorkerId,
};

mod closed;
pub(super) mod renamed;
pub(super) use closed::{ClosedTabExpectation, validate_closed_tab};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeStandaloneTabs {
    site_id: SiteId,
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tabs: Vec<NativeSimpleTab>,
}

impl NativeStandaloneTabs {
    pub(super) fn site_id(&self) -> &SiteId {
        &self.site_id
    }

    pub(super) const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    pub(super) fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }

    pub(super) fn tabs(&self) -> &[NativeSimpleTab] {
        &self.tabs
    }

    pub(super) fn tab_index(&self, tab_id: &WorkbenchTabId) -> Option<usize> {
        self.tabs.iter().position(|tab| &tab.tab_id == tab_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeSimpleTab {
    tab_id: WorkbenchTabId,
    tab_label: WorkbenchLabel,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    presentation_label: WorkbenchLabel,
    binding: NativeTerminalBinding,
}

impl NativeSimpleTab {
    pub(super) fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }

    pub(super) fn tab_label(&self) -> &WorkbenchLabel {
        &self.tab_label
    }

    #[cfg(test)]
    pub(super) fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }

    #[cfg(test)]
    pub(super) fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }

    #[cfg(test)]
    pub(super) fn presentation_label(&self) -> &WorkbenchLabel {
        &self.presentation_label
    }

    pub(super) fn binding(&self) -> &NativeTerminalBinding {
        &self.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeTerminalBinding {
    worker_id: WorkerId,
    terminal_id: TerminalId,
}

impl NativeTerminalBinding {
    pub(super) fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    pub(super) fn terminal_id(&self) -> &TerminalId {
        &self.terminal_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CreatedTabExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    requested_label: WorkbenchLabel,
}

impl CreatedTabExpectation {
    pub(super) fn new(
        committed_revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        requested_label: WorkbenchLabel,
    ) -> Self {
        Self {
            committed_revision,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            requested_label,
        }
    }

    #[cfg(test)]
    pub(super) const fn committed_revision(&self) -> WorkbenchRevision {
        self.committed_revision
    }

    #[cfg(test)]
    pub(super) fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }

    #[cfg(test)]
    pub(super) fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }

    #[cfg(test)]
    pub(super) fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }

    #[cfg(test)]
    pub(super) fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }

    #[cfg(test)]
    pub(super) fn requested_label(&self) -> &WorkbenchLabel {
        &self.requested_label
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeProjectionError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MovedTabExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    direction: WorkbenchTabMoveDirection,
}

impl MovedTabExpectation {
    pub(super) const fn new(
        committed_revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        direction: WorkbenchTabMoveDirection,
    ) -> Self {
        Self {
            committed_revision,
            terminal_context_id,
            tab_id,
            direction,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeProjectionDivergence;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValidatedCreatedProjection {
    pub(super) candidate: NativeStandaloneTabs,
    pub(super) created_index: Option<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValidatedMovedProjection {
    pub(super) candidate: NativeStandaloneTabs,
    pub(super) moved_index: Option<usize>,
}

pub(super) fn project_native_standalone_tabs(
    snapshot: &Snapshot,
) -> Result<NativeStandaloneTabs, NativeProjectionError> {
    let mut contexts = snapshot.terminal_contexts.iter().filter(|context| {
        context.terminal_context_id == snapshot.navigation.standalone_terminal_context_id
    });
    let context = contexts.next().ok_or(NativeProjectionError)?;
    if contexts.next().is_some()
        || !context.groups.is_empty()
        || context.tabs.len() > MAXIMUM_TABS_PER_CONTEXT
    {
        return Err(NativeProjectionError);
    }

    let tabs = context
        .tabs
        .iter()
        .map(project_tab)
        .collect::<Result<Vec<_>, _>>()?;
    if !unique_structural_ids(&tabs) {
        return Err(NativeProjectionError);
    }
    Ok(NativeStandaloneTabs {
        site_id: snapshot.site_id.clone(),
        revision: snapshot.revision,
        terminal_context_id: context.terminal_context_id.clone(),
        tabs,
    })
}

pub(super) fn validate_canonical_refresh(
    current: &NativeStandaloneTabs,
    candidate: NativeStandaloneTabs,
) -> Result<NativeStandaloneTabs, NativeProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    Ok(candidate)
}

pub(super) fn validate_created_tab(
    current: &NativeStandaloneTabs,
    expected: &CreatedTabExpectation,
    candidate: NativeStandaloneTabs,
) -> Result<ValidatedCreatedProjection, NativeProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    if current.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || current.tab_index(&expected.tab_id).is_some()
        || candidate.revision < expected.committed_revision
        || candidate.terminal_context_id != expected.terminal_context_id
        || (candidate.revision == expected.committed_revision
            && (candidate.tabs.len() != current.tabs.len() + 1
                || current
                    .tabs
                    .iter()
                    .zip(&candidate.tabs)
                    .any(|(current, candidate)| current != candidate)))
    {
        return Err(NativeProjectionDivergence);
    }

    let Some(created_index) = candidate.tab_index(&expected.tab_id) else {
        if candidate.revision == expected.committed_revision
            || candidate.tabs.iter().any(|tab| {
                tab.panel_id == expected.panel_id || tab.presentation_id == expected.presentation_id
            })
        {
            return Err(NativeProjectionDivergence);
        }
        return Ok(ValidatedCreatedProjection {
            candidate,
            created_index: None,
        });
    };
    let created = &candidate.tabs[created_index];
    if created.tab_id != expected.tab_id
        || created.panel_id != expected.panel_id
        || created.presentation_id != expected.presentation_id
        || created.presentation_label != expected.requested_label
        || (candidate.revision == expected.committed_revision
            && created.tab_label != expected.requested_label)
    {
        return Err(NativeProjectionDivergence);
    }
    Ok(ValidatedCreatedProjection {
        candidate,
        created_index: Some(created_index),
    })
}

pub(super) fn validate_moved_tab(
    baseline: &NativeStandaloneTabs,
    expected: &MovedTabExpectation,
    candidate: NativeStandaloneTabs,
) -> Result<ValidatedMovedProjection, NativeProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    if baseline.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || expected.terminal_context_id != baseline.terminal_context_id
        || candidate.revision < expected.committed_revision
    {
        return Err(NativeProjectionDivergence);
    }
    let from = baseline
        .tab_index(&expected.tab_id)
        .ok_or(NativeProjectionDivergence)?;
    let to = match expected.direction {
        WorkbenchTabMoveDirection::Previous => from.saturating_sub(1),
        WorkbenchTabMoveDirection::Next => (from + 1).min(baseline.tabs.len() - 1),
        WorkbenchTabMoveDirection::First => 0,
        WorkbenchTabMoveDirection::Last => baseline.tabs.len() - 1,
    };
    if from == to {
        return Err(NativeProjectionDivergence);
    }
    if candidate.revision == expected.committed_revision {
        let mut expected_tabs = baseline.tabs.clone();
        let moved = expected_tabs.remove(from);
        expected_tabs.insert(to, moved);
        if candidate.tabs != expected_tabs {
            return Err(NativeProjectionDivergence);
        }
    }
    let moved_index = candidate.tab_index(&expected.tab_id);
    Ok(ValidatedMovedProjection {
        candidate,
        moved_index,
    })
}

fn project_tab(tab: &Tab) -> Result<NativeSimpleTab, NativeProjectionError> {
    if tab.group_id.is_some() || tab.pinned {
        return Err(NativeProjectionError);
    }
    let PanelNode::Panel {
        child_presentations,
        panel_id,
        primary_presentation,
    } = &tab.root
    else {
        return Err(NativeProjectionError);
    };
    if !child_presentations.is_empty() {
        return Err(NativeProjectionError);
    }
    let binding = primary_presentation
        .binding
        .as_ref()
        .ok_or(NativeProjectionError)?;
    Ok(NativeSimpleTab {
        tab_id: tab.tab_id.clone(),
        tab_label: WorkbenchLabel::parse(&tab.label).map_err(|_error| NativeProjectionError)?,
        panel_id: panel_id.clone(),
        presentation_id: primary_presentation.presentation_id.clone(),
        presentation_label: WorkbenchLabel::parse(&primary_presentation.label)
            .map_err(|_error| NativeProjectionError)?,
        binding: NativeTerminalBinding {
            worker_id: binding.worker_id.clone(),
            terminal_id: binding.terminal_id.clone(),
        },
    })
}

fn validate_canonical_candidate(
    current: &NativeStandaloneTabs,
    candidate: &NativeStandaloneTabs,
) -> Result<(), NativeProjectionDivergence> {
    if candidate.site_id != current.site_id
        || candidate.terminal_context_id != current.terminal_context_id
        || candidate.revision < current.revision
    {
        return Err(NativeProjectionDivergence);
    }
    if candidate.revision == current.revision {
        return (candidate == current)
            .then_some(())
            .ok_or(NativeProjectionDivergence);
    }
    if candidate.tabs.len() > MAXIMUM_TABS_PER_CONTEXT
        || !unique_structural_ids(&candidate.tabs)
        || current.tabs.iter().any(|current| {
            candidate.tabs.iter().any(|candidate| {
                (current.tab_id == candidate.tab_id
                    || current.panel_id == candidate.panel_id
                    || current.presentation_id == candidate.presentation_id
                    || current.binding.terminal_id == candidate.binding.terminal_id)
                    && !stable_member_except_tab_label(current, candidate)
            })
        })
    {
        return Err(NativeProjectionDivergence);
    }
    Ok(())
}

fn unique_structural_ids(tabs: &[NativeSimpleTab]) -> bool {
    tabs.iter().enumerate().all(|(index, tab)| {
        tabs[..index].iter().all(|prior| {
            prior.tab_id != tab.tab_id
                && prior.panel_id != tab.panel_id
                && prior.presentation_id != tab.presentation_id
                && prior.binding.terminal_id != tab.binding.terminal_id
        })
    })
}

fn stable_member_except_tab_label(current: &NativeSimpleTab, candidate: &NativeSimpleTab) -> bool {
    current.tab_id == candidate.tab_id
        && current.panel_id == candidate.panel_id
        && current.presentation_id == candidate.presentation_id
        && current.presentation_label == candidate.presentation_label
        && current.binding == candidate.binding
}

#[cfg(test)]
mod tests;
