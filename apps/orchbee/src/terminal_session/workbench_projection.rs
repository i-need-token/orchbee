use orchbee_workbench_protocol::{
    MAXIMUM_TABS_PER_CONTEXT, MAXIMUM_TERMINAL_CONTEXTS, PanelNode, SiteId, Snapshot, Tab,
    TerminalId, TerminalPresentationId, WorkbenchLabel, WorkbenchPanelId, WorkbenchProjectId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, WorkerId,
};

mod closed;
pub(super) mod renamed;
pub(super) use closed::{ClosedTabExpectation, validate_closed_tab};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeContextTabs {
    site_id: SiteId,
    revision: WorkbenchRevision,
    tabs: Vec<NativeSimpleTab>,
    location: NativeContextLocation,
    navigation: std::sync::Arc<[NativeContext]>,
    projects: std::sync::Arc<[orchbee_workbench_protocol::Project]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NativeContextLocation {
    Standalone(WorkbenchTerminalContextId),
    Workspace {
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
}

impl NativeContextLocation {
    pub(super) fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        match self {
            Self::Standalone(id) => id,
            Self::Workspace {
                terminal_context_id,
                ..
            } => terminal_context_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeContext {
    pub(super) location: NativeContextLocation,
    pub(super) project_label: Option<WorkbenchLabel>,
    pub(super) workspace_label: Option<WorkbenchLabel>,
}

impl NativeContextTabs {
    pub(super) fn projects(&self) -> &[orchbee_workbench_protocol::Project] {
        &self.projects
    }

    pub(super) fn directory(&self) -> Option<&orchbee_workbench_protocol::Directory> {
        let NativeContextLocation::Workspace {
            project_id,
            workspace_id,
            ..
        } = &self.location
        else {
            return None;
        };
        let project = self.projects.iter().find(|p| &p.project_id == project_id)?;
        let directory = project
            .workspaces
            .iter()
            .find(|w| &w.workspace_id == workspace_id)?
            .primary_directory_id
            .as_ref()?;
        project
            .directories
            .iter()
            .find(|entry| &entry.directory_id == directory)
    }

    pub(super) fn location(&self) -> &NativeContextLocation {
        &self.location
    }

    pub(super) fn navigation(&self) -> &[NativeContext] {
        &self.navigation
    }

    pub(super) fn project_snapshot(
        &self,
        snapshot: &Snapshot,
    ) -> Result<Self, NativeProjectionError> {
        project_native_context_tabs(snapshot, &self.location)
    }
    pub(super) fn site_id(&self) -> &SiteId {
        &self.site_id
    }

    pub(super) const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    pub(super) fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        self.location.terminal_context_id()
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
    pub(super) candidate: NativeContextTabs,
    pub(super) created_index: Option<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValidatedMovedProjection {
    pub(super) candidate: NativeContextTabs,
    pub(super) moved_index: Option<usize>,
}

pub(super) fn project_native_standalone_tabs(
    snapshot: &Snapshot,
) -> Result<NativeContextTabs, NativeProjectionError> {
    project_native_context_tabs(
        snapshot,
        &NativeContextLocation::Standalone(
            snapshot.navigation.standalone_terminal_context_id.clone(),
        ),
    )
}

pub(super) fn project_native_context_tabs(
    snapshot: &Snapshot,
    location: &NativeContextLocation,
) -> Result<NativeContextTabs, NativeProjectionError> {
    let mut navigation = vec![NativeContext {
        location: NativeContextLocation::Standalone(
            snapshot.navigation.standalone_terminal_context_id.clone(),
        ),
        project_label: None,
        workspace_label: None,
    }];
    for project in &snapshot.navigation.projects {
        for workspace in &project.workspaces {
            if workspace.primary_directory_id.as_ref().is_some_and(|id| {
                project
                    .directories
                    .iter()
                    .filter(|directory| &directory.directory_id == id)
                    .count()
                    != 1
            }) {
                return Err(NativeProjectionError);
            }
            if navigation.len() == MAXIMUM_TERMINAL_CONTEXTS {
                return Err(NativeProjectionError);
            }
            navigation.push(NativeContext {
                location: NativeContextLocation::Workspace {
                    project_id: project.project_id.clone(),
                    workspace_id: workspace.workspace_id.clone(),
                    terminal_context_id: workspace.terminal_context_id.clone(),
                },
                project_label: Some(
                    WorkbenchLabel::parse(&project.label).map_err(|_| NativeProjectionError)?,
                ),
                workspace_label: Some(
                    WorkbenchLabel::parse(&workspace.label).map_err(|_| NativeProjectionError)?,
                ),
            });
        }
    }
    if !navigation.iter().any(|entry| &entry.location == location)
        || navigation.iter().enumerate().any(|(index, entry)| {
            navigation[..index].iter().any(|prior| {
                prior.location.terminal_context_id() == entry.location.terminal_context_id()
            })
        })
    {
        return Err(NativeProjectionError);
    }
    let mut contexts = snapshot
        .terminal_contexts
        .iter()
        .filter(|context| &context.terminal_context_id == location.terminal_context_id());
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
    Ok(NativeContextTabs {
        site_id: snapshot.site_id.clone(),
        revision: snapshot.revision,
        tabs,
        location: location.clone(),
        navigation: navigation.into(),
        projects: snapshot.navigation.projects.clone().into(),
    })
}

pub(super) fn validate_canonical_refresh(
    current: &NativeContextTabs,
    candidate: NativeContextTabs,
) -> Result<NativeContextTabs, NativeProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    Ok(candidate)
}

pub(super) fn validate_created_tab(
    current: &NativeContextTabs,
    expected: &CreatedTabExpectation,
    candidate: NativeContextTabs,
) -> Result<ValidatedCreatedProjection, NativeProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    if current.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || current.tab_index(&expected.tab_id).is_some()
        || candidate.revision < expected.committed_revision
        || candidate.terminal_context_id() != &expected.terminal_context_id
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
    baseline: &NativeContextTabs,
    expected: &MovedTabExpectation,
    candidate: NativeContextTabs,
) -> Result<ValidatedMovedProjection, NativeProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    if baseline.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || &expected.terminal_context_id != baseline.terminal_context_id()
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

pub(super) fn validate_canonical_candidate(
    current: &NativeContextTabs,
    candidate: &NativeContextTabs,
) -> Result<(), NativeProjectionDivergence> {
    validate_retained_context(
        &current.site_id,
        &current.location,
        current.revision,
        &current.tabs,
        candidate,
    )?;
    if candidate.revision == current.revision
        && (candidate.navigation != current.navigation || candidate.projects != current.projects)
    {
        return Err(NativeProjectionDivergence);
    }
    Ok(())
}

/// Historical membership retains no navigation labels; the active snapshot validates navigation.
pub(super) fn validate_retained_context(
    site_id: &SiteId,
    location: &NativeContextLocation,
    revision: WorkbenchRevision,
    tabs: &[NativeSimpleTab],
    candidate: &NativeContextTabs,
) -> Result<(), NativeProjectionDivergence> {
    if &candidate.site_id != site_id
        || &candidate.location != location
        || candidate.revision < revision
    {
        return Err(NativeProjectionDivergence);
    }
    if candidate.revision == revision {
        return (candidate.tabs.as_slice() == tabs)
            .then_some(())
            .ok_or(NativeProjectionDivergence);
    }
    if candidate.tabs.len() > MAXIMUM_TABS_PER_CONTEXT
        || !unique_structural_ids(&candidate.tabs)
        || tabs.iter().any(|current| {
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
