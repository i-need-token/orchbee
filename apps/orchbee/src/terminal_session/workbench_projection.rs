use orchbee_workbench_protocol::{
    MAXIMUM_TABS_PER_CONTEXT, MAXIMUM_TERMINAL_CONTEXTS, PanelNode, PanelOrientation, SiteId,
    Snapshot, Tab, TerminalId, TerminalPresentationId, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchProjectId, WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId,
    WorkbenchTabMoveDirection, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
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
    panels: Box<[NativePanel]>,
    tree: NativePanelTree,
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

    #[cfg(test)]
    pub(super) fn binding(&self) -> &NativeTerminalBinding {
        &self.binding
    }

    pub(super) fn panel_count(&self) -> usize {
        self.panels.len()
    }

    pub(super) fn panel(&self, index: usize) -> Option<&NativePanel> {
        self.panels.get(index)
    }

    pub(super) fn panel_index(&self, panel_id: &WorkbenchPanelId) -> Option<usize> {
        self.panels
            .iter()
            .position(|panel| panel.panel_id() == panel_id)
    }

    pub(super) fn panel_tree(&self) -> &NativePanelTree {
        &self.tree
    }

    pub(super) fn panel_focus_target(
        &self,
        panel_index: usize,
        direction: crate::client_view::PanelDirection,
    ) -> Option<usize> {
        self.tree.neighbor(panel_index, direction)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativePanel {
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    presentation_label: WorkbenchLabel,
    binding: NativeTerminalBinding,
}

impl NativePanel {
    pub(super) fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }

    #[cfg(test)]
    pub(super) fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }

    pub(crate) fn presentation_label(&self) -> &WorkbenchLabel {
        &self.presentation_label
    }

    pub(super) fn binding(&self) -> &NativeTerminalBinding {
        &self.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativePanelTree {
    Panel {
        panel_index: usize,
    },
    Split {
        first: Box<Self>,
        second: Box<Self>,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        orientation: PanelOrientation,
        split_id: WorkbenchSplitId,
    },
}

#[derive(Clone, Copy)]
struct PanelBounds {
    panel_index: usize,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl NativePanelTree {
    pub(super) fn neighbor(
        &self,
        panel_index: usize,
        direction: crate::client_view::PanelDirection,
    ) -> Option<usize> {
        let mut bounds = Vec::new();
        self.collect_bounds(0.0, 0.0, 1.0, 1.0, &mut bounds);
        let current = *bounds
            .iter()
            .find(|candidate| candidate.panel_index == panel_index)?;
        let mut best: Option<(usize, f64, f64)> = None;
        for candidate in bounds {
            if candidate.panel_index == panel_index {
                continue;
            }
            let (gap, offset, overlaps) = match direction {
                crate::client_view::PanelDirection::Left => (
                    current.left - candidate.right,
                    ((candidate.top + candidate.bottom) - (current.top + current.bottom)).abs(),
                    candidate.bottom > current.top && candidate.top < current.bottom,
                ),
                crate::client_view::PanelDirection::Right => (
                    candidate.left - current.right,
                    ((candidate.top + candidate.bottom) - (current.top + current.bottom)).abs(),
                    candidate.bottom > current.top && candidate.top < current.bottom,
                ),
                crate::client_view::PanelDirection::Up => (
                    current.top - candidate.bottom,
                    ((candidate.left + candidate.right) - (current.left + current.right)).abs(),
                    candidate.right > current.left && candidate.left < current.right,
                ),
                crate::client_view::PanelDirection::Down => (
                    candidate.top - current.bottom,
                    ((candidate.left + candidate.right) - (current.left + current.right)).abs(),
                    candidate.right > current.left && candidate.left < current.right,
                ),
            };
            if !overlaps || gap < -f64::EPSILON {
                continue;
            }
            let is_better = best.is_none_or(|(best_index, best_gap, best_offset)| {
                gap < best_gap - f64::EPSILON
                    || ((gap - best_gap).abs() <= f64::EPSILON
                        && (offset < best_offset - f64::EPSILON
                            || ((offset - best_offset).abs() <= f64::EPSILON
                                && candidate.panel_index < best_index)))
            });
            if is_better {
                best = Some((candidate.panel_index, gap, offset));
            }
        }
        best.map(|(index, _, _)| index)
    }

    fn collect_bounds(
        &self,
        left: f64,
        top: f64,
        width: f64,
        height: f64,
        output: &mut Vec<PanelBounds>,
    ) {
        match self {
            Self::Panel { panel_index } => output.push(PanelBounds {
                panel_index: *panel_index,
                left,
                top,
                right: left + width,
                bottom: top + height,
            }),
            Self::Split {
                first,
                second,
                orientation,
                ..
            } => match orientation {
                PanelOrientation::Horizontal => {
                    let half = width / 2.0;
                    first.collect_bounds(left, top, half, height, output);
                    second.collect_bounds(left + half, top, half, height, output);
                }
                PanelOrientation::Vertical => {
                    let half = height / 2.0;
                    first.collect_bounds(left, top, width, half, output);
                    second.collect_bounds(left, top + half, width, half, output);
                }
            },
        }
    }

    fn split_ids(&self, output: &mut Vec<WorkbenchSplitId>) {
        if let Self::Split {
            first,
            second,
            split_id,
            ..
        } = self
        {
            output.push(split_id.clone());
            first.split_ids(output);
            second.split_ids(output);
        }
    }

    fn slot_ids(&self, output: &mut Vec<WorkbenchPanelSlotId>) {
        if let Self::Split {
            first,
            second,
            first_slot_id,
            second_slot_id,
            ..
        } = self
        {
            output.extend([first_slot_id.clone(), second_slot_id.clone()]);
            first.slot_ids(output);
            second.slot_ids(output);
        }
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
    let mut panels = Vec::new();
    let tree = project_panel_tree(&tab.root, &mut panels)?;
    let primary = panels.first().ok_or(NativeProjectionError)?;
    Ok(NativeSimpleTab {
        tab_id: tab.tab_id.clone(),
        tab_label: WorkbenchLabel::parse(&tab.label).map_err(|_error| NativeProjectionError)?,
        panel_id: primary.panel_id.clone(),
        presentation_id: primary.presentation_id.clone(),
        presentation_label: primary.presentation_label.clone(),
        binding: primary.binding.clone(),
        panels: panels.into_boxed_slice(),
        tree,
    })
}

fn project_panel_tree(
    node: &PanelNode,
    panels: &mut Vec<NativePanel>,
) -> Result<NativePanelTree, NativeProjectionError> {
    match node {
        PanelNode::Panel {
            child_presentations,
            panel_id,
            primary_presentation,
        } => {
            if !child_presentations.is_empty() {
                return Err(NativeProjectionError);
            }
            let binding = primary_presentation
                .binding
                .as_ref()
                .ok_or(NativeProjectionError)?;
            let panel_index = panels.len();
            panels.push(NativePanel {
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                presentation_label: WorkbenchLabel::parse(&primary_presentation.label)
                    .map_err(|_error| NativeProjectionError)?,
                binding: NativeTerminalBinding {
                    worker_id: binding.worker_id.clone(),
                    terminal_id: binding.terminal_id.clone(),
                },
            });
            Ok(NativePanelTree::Panel { panel_index })
        }
        PanelNode::Split {
            children,
            orientation,
            split_id,
        } => {
            let [first, second] = children.as_ref();
            let first_tree = project_panel_tree(&first.node, panels)?;
            let second_tree = project_panel_tree(&second.node, panels)?;
            Ok(NativePanelTree::Split {
                first: Box::new(first_tree),
                second: Box::new(second_tree),
                first_slot_id: first.slot_id.clone(),
                second_slot_id: second.slot_id.clone(),
                orientation: *orientation,
                split_id: split_id.clone(),
            })
        }
    }
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
                    && !panel_split_extension(current, candidate)
                    && !panel_close_contraction(current, candidate)
            })
        })
    {
        return Err(NativeProjectionDivergence);
    }
    Ok(())
}

fn unique_structural_ids(tabs: &[NativeSimpleTab]) -> bool {
    tabs.iter().enumerate().all(|(index, tab)| {
        tab.panels.iter().enumerate().all(|(panel_index, panel)| {
            tab.panels[..panel_index].iter().all(|prior| {
                prior.panel_id != panel.panel_id
                    && prior.presentation_id != panel.presentation_id
                    && prior.binding.terminal_id != panel.binding.terminal_id
            })
        }) && {
            let mut split_ids = Vec::new();
            tab.tree.split_ids(&mut split_ids);
            split_ids
                .iter()
                .enumerate()
                .all(|(id_index, id)| split_ids[..id_index].iter().all(|prior| prior != id))
        } && {
            let mut slot_ids = Vec::new();
            tab.tree.slot_ids(&mut slot_ids);
            slot_ids
                .iter()
                .enumerate()
                .all(|(id_index, id)| slot_ids[..id_index].iter().all(|prior| prior != id))
        } && tabs[..index].iter().all(|prior| {
            prior.tab_id != tab.tab_id
                && prior.panels.iter().all(|prior_panel| {
                    tab.panels.iter().all(|panel| {
                        prior_panel.panel_id != panel.panel_id
                            && prior_panel.presentation_id != panel.presentation_id
                            && prior_panel.binding.terminal_id != panel.binding.terminal_id
                    })
                })
                && {
                    let mut prior_split_ids = Vec::new();
                    let mut split_ids = Vec::new();
                    prior.tree.split_ids(&mut prior_split_ids);
                    tab.tree.split_ids(&mut split_ids);
                    prior_split_ids
                        .iter()
                        .all(|prior_id| split_ids.iter().all(|id| prior_id != id))
                }
                && {
                    let mut prior_slot_ids = Vec::new();
                    let mut slot_ids = Vec::new();
                    prior.tree.slot_ids(&mut prior_slot_ids);
                    tab.tree.slot_ids(&mut slot_ids);
                    prior_slot_ids
                        .iter()
                        .all(|prior_id| slot_ids.iter().all(|id| prior_id != id))
                }
        })
    })
}

fn stable_member_except_tab_label(current: &NativeSimpleTab, candidate: &NativeSimpleTab) -> bool {
    current.tab_id == candidate.tab_id
        && current.panel_id == candidate.panel_id
        && current.presentation_id == candidate.presentation_id
        && current.presentation_label == candidate.presentation_label
        && current.binding == candidate.binding
        && current.panels == candidate.panels
        && current.tree == candidate.tree
}

fn panel_split_extension(current: &NativeSimpleTab, candidate: &NativeSimpleTab) -> bool {
    if current.tab_id != candidate.tab_id
        || candidate.panels.len() != current.panels.len() + 1
        || !current
            .panels
            .iter()
            .all(|panel| candidate.panels.iter().any(|other| other == panel))
        || candidate
            .panels
            .iter()
            .filter(|panel| !current.panels.iter().any(|other| other == *panel))
            .count()
            != 1
    {
        return false;
    }
    split_tree_extension(
        &current.tree,
        &candidate.tree,
        &current.panels,
        &candidate.panels,
    )
}

fn split_tree_extension(
    current: &NativePanelTree,
    candidate: &NativePanelTree,
    current_panels: &[NativePanel],
    candidate_panels: &[NativePanel],
) -> bool {
    match (current, candidate) {
        (
            NativePanelTree::Panel {
                panel_index: current_index,
            },
            NativePanelTree::Split { first, second, .. },
        ) => {
            let old_matches = |node: &NativePanelTree| {
                matches!(
                    node,
                    NativePanelTree::Panel {
                        panel_index: candidate_index,
                    } if current_panels.get(*current_index) == candidate_panels.get(*candidate_index)
                )
            };
            let new_leaf = |node: &NativePanelTree| {
                matches!(
                    node,
                    NativePanelTree::Panel {
                        panel_index: candidate_index,
                    } if candidate_panels
                        .get(*candidate_index)
                        .is_some_and(|candidate| {
                            !current_panels.iter().any(|current| current == candidate)
                        })
                )
            };
            (old_matches(first) && new_leaf(second)) || (old_matches(second) && new_leaf(first))
        }
        (
            NativePanelTree::Split {
                first: current_first,
                second: current_second,
                first_slot_id: current_first_slot,
                second_slot_id: current_second_slot,
                orientation: current_orientation,
                split_id: current_split_id,
            },
            NativePanelTree::Split {
                first: candidate_first,
                second: candidate_second,
                first_slot_id: candidate_first_slot,
                second_slot_id: candidate_second_slot,
                orientation: candidate_orientation,
                split_id: candidate_split_id,
            },
        ) if current_first_slot == candidate_first_slot
            && current_second_slot == candidate_second_slot
            && current_orientation == candidate_orientation
            && current_split_id == candidate_split_id =>
        {
            (split_tree_extension(
                current_first,
                candidate_first,
                current_panels,
                candidate_panels,
            ) && subtree_matches(
                current_second,
                candidate_second,
                current_panels,
                candidate_panels,
            )) || (split_tree_extension(
                current_second,
                candidate_second,
                current_panels,
                candidate_panels,
            ) && subtree_matches(
                current_first,
                candidate_first,
                current_panels,
                candidate_panels,
            ))
        }
        _ => false,
    }
}

/// `candidate` is `current` with exactly one leaf Panel closed: the Panel set loses one member and
/// the leaf's parent Split collapses into the sibling subtree, retaining the sibling's identities.
fn panel_close_contraction(current: &NativeSimpleTab, candidate: &NativeSimpleTab) -> bool {
    if current.tab_id != candidate.tab_id
        || candidate.panels.len().checked_add(1) != Some(current.panels.len())
        || !candidate
            .panels
            .iter()
            .all(|panel| current.panels.iter().any(|other| other == panel))
        || current
            .panels
            .iter()
            .filter(|panel| !candidate.panels.iter().any(|other| other == *panel))
            .count()
            != 1
    {
        return false;
    }
    close_tree_contraction(
        &current.tree,
        &candidate.tree,
        &current.panels,
        &candidate.panels,
    )
}

fn close_tree_contraction(
    current: &NativePanelTree,
    candidate: &NativePanelTree,
    current_panels: &[NativePanel],
    candidate_panels: &[NativePanel],
) -> bool {
    let NativePanelTree::Split {
        first: current_first,
        second: current_second,
        first_slot_id: current_first_slot,
        second_slot_id: current_second_slot,
        orientation: current_orientation,
        split_id: current_split_id,
    } = current
    else {
        return false;
    };
    let removed_leaf = |node: &NativePanelTree| {
        matches!(
            node,
            NativePanelTree::Panel { panel_index } if current_panels
                .get(*panel_index)
                .is_some_and(|panel| {
                    !candidate_panels.iter().any(|candidate| candidate == panel)
                })
        )
    };
    // The closed leaf hung directly under this Split; the sibling subtree became the candidate.
    if (removed_leaf(current_second)
        && subtree_matches(current_first, candidate, current_panels, candidate_panels))
        || (removed_leaf(current_first)
            && subtree_matches(current_second, candidate, current_panels, candidate_panels))
    {
        return true;
    }
    // A deeper leaf closed: this Split survives and exactly one child contains the contraction.
    let NativePanelTree::Split {
        first: candidate_first,
        second: candidate_second,
        first_slot_id: candidate_first_slot,
        second_slot_id: candidate_second_slot,
        orientation: candidate_orientation,
        split_id: candidate_split_id,
    } = candidate
    else {
        return false;
    };
    current_first_slot == candidate_first_slot
        && current_second_slot == candidate_second_slot
        && current_orientation == candidate_orientation
        && current_split_id == candidate_split_id
        && ((close_tree_contraction(
            current_first,
            candidate_first,
            current_panels,
            candidate_panels,
        ) && subtree_matches(
            current_second,
            candidate_second,
            current_panels,
            candidate_panels,
        )) || (close_tree_contraction(
            current_second,
            candidate_second,
            current_panels,
            candidate_panels,
        ) && subtree_matches(
            current_first,
            candidate_first,
            current_panels,
            candidate_panels,
        )))
}

/// Structural equality across two tabs' Panel arrays; leaves compare by Panel value.
fn subtree_matches(
    current: &NativePanelTree,
    candidate: &NativePanelTree,
    current_panels: &[NativePanel],
    candidate_panels: &[NativePanel],
) -> bool {
    match (current, candidate) {
        (
            NativePanelTree::Panel {
                panel_index: current_index,
            },
            NativePanelTree::Panel {
                panel_index: candidate_index,
            },
        ) => current_panels.get(*current_index) == candidate_panels.get(*candidate_index),
        (
            NativePanelTree::Split {
                first: current_first,
                second: current_second,
                first_slot_id: current_first_slot,
                second_slot_id: current_second_slot,
                orientation: current_orientation,
                split_id: current_split_id,
            },
            NativePanelTree::Split {
                first: candidate_first,
                second: candidate_second,
                first_slot_id: candidate_first_slot,
                second_slot_id: candidate_second_slot,
                orientation: candidate_orientation,
                split_id: candidate_split_id,
            },
        ) => {
            current_first_slot == candidate_first_slot
                && current_second_slot == candidate_second_slot
                && current_orientation == candidate_orientation
                && current_split_id == candidate_split_id
                && subtree_matches(
                    current_first,
                    candidate_first,
                    current_panels,
                    candidate_panels,
                )
                && subtree_matches(
                    current_second,
                    candidate_second,
                    current_panels,
                    candidate_panels,
                )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
