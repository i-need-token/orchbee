use orchbee_workbench_protocol::{
    MAXIMUM_TABS_PER_CONTEXT, MAXIMUM_TERMINAL_CONTEXTS, PanelNode, PanelOrientation, SiteId,
    Snapshot, Tab, TerminalId, TerminalPresentationId, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchPanelSplitDirection, WorkbenchProjectId, WorkbenchRevision,
    WorkbenchSplitId, WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, WorkerId,
};
use std::collections::HashMap;

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
pub(crate) enum NativeContextLocation {
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

    /// The visible leaf panel ids in canonical tree order — the same leaf set
    /// the layout projection draws and the pane sync reconciles.
    pub(super) fn leaf_panel_ids(&self) -> Vec<WorkbenchPanelId> {
        self.tree
            .leaf_panel_indices()
            .into_iter()
            .filter_map(|index| self.panel(index).map(|panel| panel.panel_id().clone()))
            .collect()
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
    pub(crate) fn panel_id(&self) -> &WorkbenchPanelId {
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
    /// The leaf panel indices in tree order — canonical for layout and sync.
    pub(super) fn leaf_panel_indices(&self) -> Vec<usize> {
        let mut leaves = Vec::new();
        self.collect_leaves(&mut leaves);
        leaves
    }

    fn collect_leaves(&self, leaves: &mut Vec<usize>) {
        match self {
            Self::Panel { panel_index } => leaves.push(*panel_index),
            Self::Split { first, second, .. } => {
                first.collect_leaves(leaves);
                second.collect_leaves(leaves);
            }
        }
    }

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SplitPanelExpectation {
    pub(super) committed_revision: WorkbenchRevision,
    pub(super) terminal_context_id: WorkbenchTerminalContextId,
    pub(super) tab_id: WorkbenchTabId,
    pub(super) panel_id: WorkbenchPanelId,
    pub(super) new_panel_id: WorkbenchPanelId,
    pub(super) split_id: WorkbenchSplitId,
    pub(super) first_slot_id: WorkbenchPanelSlotId,
    pub(super) second_slot_id: WorkbenchPanelSlotId,
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) direction: WorkbenchPanelSplitDirection,
    pub(super) requested_label: WorkbenchLabel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MovedPanelExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    target_panel_id: WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
}

impl MovedPanelExpectation {
    pub(super) const fn new(
        committed_revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        target_panel_id: WorkbenchPanelId,
        direction: WorkbenchPanelSplitDirection,
    ) -> Self {
        Self {
            committed_revision,
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
            direction,
        }
    }
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

pub(super) fn validate_split_panel(
    baseline: &NativeContextTabs,
    expected: &SplitPanelExpectation,
    candidate: NativeContextTabs,
) -> Result<NativeContextTabs, NativeProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    if baseline.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || &expected.terminal_context_id != baseline.terminal_context_id()
        || candidate.revision < expected.committed_revision
    {
        return Err(NativeProjectionDivergence);
    }
    if candidate.revision == expected.committed_revision {
        let index = baseline
            .tab_index(&expected.tab_id)
            .ok_or(NativeProjectionDivergence)?;
        let baseline_tab = &baseline.tabs[index];
        let candidate_tab = candidate
            .tabs
            .get(index)
            .ok_or(NativeProjectionDivergence)?;
        if candidate.tabs.len() != baseline.tabs.len()
            || candidate_tab.tab_id != expected.tab_id
            || candidate_tab.tab_label != baseline_tab.tab_label
            || baseline_tab.panel_index(&expected.panel_id).is_none()
            || !split_graft_matches(baseline_tab, candidate_tab, expected)
            || candidate
                .tabs
                .iter()
                .zip(&baseline.tabs)
                .enumerate()
                .any(|(position, (candidate, baseline))| position != index && candidate != baseline)
        {
            return Err(NativeProjectionDivergence);
        }
    }
    Ok(candidate)
}

pub(super) fn validate_moved_panel(
    baseline: &NativeContextTabs,
    expected: &MovedPanelExpectation,
    candidate: NativeContextTabs,
) -> Result<NativeContextTabs, NativeProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    if baseline.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || &expected.terminal_context_id != baseline.terminal_context_id()
        || candidate.revision < expected.committed_revision
    {
        return Err(NativeProjectionDivergence);
    }
    if candidate.revision == expected.committed_revision {
        let index = baseline
            .tab_index(&expected.tab_id)
            .ok_or(NativeProjectionDivergence)?;
        let baseline_tab = &baseline.tabs[index];
        let candidate_tab = candidate
            .tabs
            .get(index)
            .ok_or(NativeProjectionDivergence)?;
        if candidate.tabs.len() != baseline.tabs.len()
            || candidate_tab.tab_id != expected.tab_id
            || candidate_tab.tab_label != baseline_tab.tab_label
            || baseline_tab.panel_index(&expected.panel_id).is_none()
            || baseline_tab
                .panel_index(&expected.target_panel_id)
                .is_none()
            || !move_graft_matches(baseline_tab, candidate_tab, expected)
            || candidate
                .tabs
                .iter()
                .zip(&baseline.tabs)
                .enumerate()
                .any(|(position, (candidate, baseline))| position != index && candidate != baseline)
        {
            return Err(NativeProjectionDivergence);
        }
    }
    Ok(candidate)
}

/// A comparable tree form keyed on leaf member tuples rather than DFS positions, so a
/// projection that inserted or removed leaves can be reduced back to the baseline shape.
#[derive(Debug, Eq, PartialEq)]
enum TreeShape<'a> {
    Leaf(&'a NativePanel),
    Split {
        first: Box<TreeShape<'a>>,
        second: Box<TreeShape<'a>>,
        first_slot_id: &'a WorkbenchPanelSlotId,
        second_slot_id: &'a WorkbenchPanelSlotId,
        orientation: PanelOrientation,
        split_id: &'a WorkbenchSplitId,
    },
}

fn tree_shape<'a>(tree: &'a NativePanelTree, panels: &'a [NativePanel]) -> Option<TreeShape<'a>> {
    match tree {
        NativePanelTree::Panel { .. } => leaf_panel(tree, panels).map(TreeShape::Leaf),
        NativePanelTree::Split {
            first,
            second,
            first_slot_id,
            second_slot_id,
            orientation,
            split_id,
        } => Some(TreeShape::Split {
            first: Box::new(tree_shape(first, panels)?),
            second: Box::new(tree_shape(second, panels)?),
            first_slot_id,
            second_slot_id,
            orientation: *orientation,
            split_id,
        }),
    }
}

fn find_split<'a>(
    tree: &'a NativePanelTree,
    split_id: &WorkbenchSplitId,
) -> Option<&'a NativePanelTree> {
    let NativePanelTree::Split {
        first,
        second,
        split_id: id,
        ..
    } = tree
    else {
        return None;
    };
    if id == split_id {
        return Some(tree);
    }
    find_split(first, split_id).or_else(|| find_split(second, split_id))
}

fn split_contains_id(tree: &NativePanelTree, split_id: &WorkbenchSplitId) -> bool {
    find_split(tree, split_id).is_some()
}

fn leaf_panel<'a>(tree: &'a NativePanelTree, panels: &'a [NativePanel]) -> Option<&'a NativePanel> {
    let NativePanelTree::Panel { panel_index } = tree else {
        return None;
    };
    panels.get(*panel_index)
}

fn leaf_panel_id<'a>(
    tree: &'a NativePanelTree,
    panels: &'a [NativePanel],
) -> Option<&'a WorkbenchPanelId> {
    leaf_panel(tree, panels).map(|panel| &panel.panel_id)
}

/// Replaces the committed Split with one of its children and reduces the tree to a shape; the
/// substitute must itself belong to the same tree so the graft is the only difference.
fn ungrafted_shape<'a>(
    tree: &'a NativePanelTree,
    graft_id: &WorkbenchSplitId,
    replacement: &'a NativePanelTree,
    panels: &'a [NativePanel],
) -> Option<TreeShape<'a>> {
    let NativePanelTree::Split {
        first,
        second,
        first_slot_id,
        second_slot_id,
        orientation,
        split_id,
    } = tree
    else {
        return tree_shape(tree, panels);
    };
    if split_id == graft_id {
        return tree_shape(replacement, panels);
    }
    Some(TreeShape::Split {
        first: Box::new(ungrafted_shape(first, graft_id, replacement, panels)?),
        second: Box::new(ungrafted_shape(second, graft_id, replacement, panels)?),
        first_slot_id,
        second_slot_id,
        orientation: *orientation,
        split_id,
    })
}

/// Removes the leaf carrying `panel_id` and collapses its parent Split into the surviving
/// sibling subtree, yielding the reduced shape. Absent leaves leave the shape unchanged.
fn removed_leaf_shape<'a>(
    tree: &'a NativePanelTree,
    panels: &'a [NativePanel],
    panel_id: &WorkbenchPanelId,
) -> Option<TreeShape<'a>> {
    match tree {
        NativePanelTree::Panel { .. } => {
            if leaf_panel_id(tree, panels) == Some(panel_id) {
                None
            } else {
                tree_shape(tree, panels)
            }
        }
        NativePanelTree::Split {
            first,
            second,
            first_slot_id,
            second_slot_id,
            orientation,
            split_id,
        } => {
            match (
                removed_leaf_shape(first, panels, panel_id),
                removed_leaf_shape(second, panels, panel_id),
            ) {
                (None, None) => None,
                (Some(surviving), None) | (None, Some(surviving)) => Some(surviving),
                (Some(first), Some(second)) => Some(TreeShape::Split {
                    first: Box::new(first),
                    second: Box::new(second),
                    first_slot_id,
                    second_slot_id,
                    orientation: *orientation,
                    split_id,
                }),
            }
        }
    }
}

const fn direction_orientation(direction: WorkbenchPanelSplitDirection) -> PanelOrientation {
    match direction {
        WorkbenchPanelSplitDirection::Left | WorkbenchPanelSplitDirection::Right => {
            PanelOrientation::Horizontal
        }
        WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Below => {
            PanelOrientation::Vertical
        }
    }
}

fn split_graft_matches(
    baseline: &NativeSimpleTab,
    candidate: &NativeSimpleTab,
    expected: &SplitPanelExpectation,
) -> bool {
    let Some(NativePanelTree::Split {
        first,
        second,
        first_slot_id,
        second_slot_id,
        orientation,
        ..
    }) = find_split(&candidate.tree, &expected.split_id)
    else {
        return false;
    };
    if *first_slot_id != expected.first_slot_id
        || *second_slot_id != expected.second_slot_id
        || *orientation != direction_orientation(expected.direction)
        || split_contains_id(&baseline.tree, &expected.split_id)
    {
        return false;
    }
    let (source, added) = match expected.direction {
        WorkbenchPanelSplitDirection::Below | WorkbenchPanelSplitDirection::Right => {
            (first.as_ref(), second.as_ref())
        }
        WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Left => {
            (second.as_ref(), first.as_ref())
        }
    };
    let Some(added_panel) = leaf_panel_id(added, &candidate.panels).and_then(|panel_id| {
        let NativePanelTree::Panel { panel_index } = added else {
            return None;
        };
        candidate
            .panels
            .get(*panel_index)
            .map(|panel| (panel_id, panel))
    }) else {
        return false;
    };
    if added_panel.0 != &expected.new_panel_id
        || added_panel.1.presentation_id != expected.presentation_id
        || added_panel.1.presentation_label != expected.requested_label
    {
        return false;
    }
    if leaf_panel_id(source, &candidate.panels) != Some(&expected.panel_id) {
        return false;
    }
    ungrafted_shape(
        &candidate.tree,
        &expected.split_id,
        source,
        &candidate.panels,
    ) == tree_shape(&baseline.tree, &baseline.panels)
}

fn move_graft_matches(
    baseline: &NativeSimpleTab,
    candidate: &NativeSimpleTab,
    expected: &MovedPanelExpectation,
) -> bool {
    let orientation = direction_orientation(expected.direction);
    let (first_id, second_id) = match expected.direction {
        WorkbenchPanelSplitDirection::Below | WorkbenchPanelSplitDirection::Right => {
            (&expected.target_panel_id, &expected.panel_id)
        }
        WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Left => {
            (&expected.panel_id, &expected.target_panel_id)
        }
    };
    let mut graft = None;
    find_move_graft(
        &candidate.tree,
        &candidate.panels,
        first_id,
        second_id,
        orientation,
        &expected.target_panel_id,
        &mut graft,
    );
    let Some((graft, target_child)) = graft else {
        return false;
    };
    let NativePanelTree::Split {
        first,
        second,
        split_id,
        ..
    } = graft
    else {
        return false;
    };
    if split_contains_id(&baseline.tree, split_id) {
        return false;
    }
    let moved_child = if std::ptr::eq(target_child, first.as_ref()) {
        second.as_ref()
    } else {
        first.as_ref()
    };
    if baseline
        .panel_index(&expected.panel_id)
        .and_then(|index| baseline.panel(index))
        != leaf_panel(moved_child, &candidate.panels)
    {
        return false;
    }
    let Some(candidate_reduced) =
        ungrafted_shape(&candidate.tree, split_id, target_child, &candidate.panels)
    else {
        return false;
    };
    let Some(baseline_reduced) =
        removed_leaf_shape(&baseline.tree, &baseline.panels, &expected.panel_id)
    else {
        return false;
    };
    candidate_reduced == baseline_reduced
}

/// Locates the fresh graft Split whose two children are exactly the moved and target leaves in
/// the direction-implied order, returning the Split and the child that restores the target leaf.
fn find_move_graft<'a>(
    tree: &'a NativePanelTree,
    panels: &'a [NativePanel],
    first_id: &WorkbenchPanelId,
    second_id: &WorkbenchPanelId,
    orientation: PanelOrientation,
    target_id: &WorkbenchPanelId,
    graft: &mut Option<(&'a NativePanelTree, &'a NativePanelTree)>,
) {
    let NativePanelTree::Split {
        first,
        second,
        orientation: node_orientation,
        ..
    } = tree
    else {
        return;
    };
    if graft.is_none()
        && *node_orientation == orientation
        && leaf_panel_id(first, panels) == Some(first_id)
        && leaf_panel_id(second, panels) == Some(second_id)
    {
        let target_child = if leaf_panel_id(first, panels) == Some(target_id) {
            first.as_ref()
        } else {
            second.as_ref()
        };
        *graft = Some((tree, target_child));
        return;
    }
    find_move_graft(
        first,
        panels,
        first_id,
        second_id,
        orientation,
        target_id,
        graft,
    );
    find_move_graft(
        second,
        panels,
        first_id,
        second_id,
        orientation,
        target_id,
        graft,
    );
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
        || !retained_member_tuples(tabs, &candidate.tabs)
        || !retained_structure_tuples(tabs, &candidate.tabs)
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

/// One immutable member record: a leaf Panel member or the tab-level primary Presentation
/// projection. Overlap on any Panel, Presentation or terminal identity requires the complete
/// tuple to match; the Tab identity is a compared field so a changed Tab ID cannot disguise
/// reassignment as removal plus addition, and a Worker ID alone is not a member identity.
type MemberTuple<'a> = (
    &'a WorkbenchTabId,
    &'a WorkbenchPanelId,
    &'a TerminalPresentationId,
    &'a WorkbenchLabel,
    &'a NativeTerminalBinding,
);

fn member_tuples(tab: &NativeSimpleTab) -> impl Iterator<Item = MemberTuple<'_>> {
    std::iter::once((
        &tab.tab_id,
        &tab.panel_id,
        &tab.presentation_id,
        &tab.presentation_label,
        &tab.binding,
    ))
    .chain(tab.panels.iter().map(|panel| {
        (
            &tab.tab_id,
            &panel.panel_id,
            &panel.presentation_id,
            &panel.presentation_label,
            &panel.binding,
        )
    }))
}

/// Every current member identity still present in the candidate must carry the complete original
/// tuple, and a Tab whose identity survives must retain at least one member: genuinely removed
/// members and entirely fresh identities need no comparison.
fn retained_member_tuples(current: &[NativeSimpleTab], candidate: &[NativeSimpleTab]) -> bool {
    let mut panels: HashMap<&WorkbenchPanelId, Vec<MemberTuple>> = HashMap::new();
    let mut presentations: HashMap<&TerminalPresentationId, Vec<MemberTuple>> = HashMap::new();
    let mut terminals: HashMap<&TerminalId, Vec<MemberTuple>> = HashMap::new();
    for tab in candidate {
        for member in member_tuples(tab) {
            panels.entry(member.1).or_default().push(member);
            presentations.entry(member.2).or_default().push(member);
            terminals
                .entry(&member.4.terminal_id)
                .or_default()
                .push(member);
        }
    }
    current.iter().all(|tab| {
        let mut retains_member = false;
        for member in member_tuples(tab) {
            for hit in panels
                .get(member.1)
                .into_iter()
                .flatten()
                .chain(presentations.get(member.2).into_iter().flatten())
                .chain(terminals.get(&member.4.terminal_id).into_iter().flatten())
            {
                if *hit != member {
                    return false;
                }
                retains_member = true;
            }
        }
        retains_member || candidate.iter().all(|entry| entry.tab_id != tab.tab_id)
    })
}

type SplitTuple<'a> = (
    &'a WorkbenchTabId,
    &'a WorkbenchPanelSlotId,
    &'a WorkbenchPanelSlotId,
    PanelOrientation,
);

fn collect_structure_tuples<'a>(
    node: &'a NativePanelTree,
    tab_id: &'a WorkbenchTabId,
    splits: &mut HashMap<&'a WorkbenchSplitId, SplitTuple<'a>>,
    slots: &mut HashMap<&'a WorkbenchPanelSlotId, &'a WorkbenchSplitId>,
) {
    let NativePanelTree::Split {
        first,
        second,
        first_slot_id,
        second_slot_id,
        orientation,
        split_id,
    } = node
    else {
        return;
    };
    splits.insert(
        split_id,
        (tab_id, first_slot_id, second_slot_id, *orientation),
    );
    slots.insert(first_slot_id, split_id);
    slots.insert(second_slot_id, split_id);
    collect_structure_tuples(first, tab_id, splits, slots);
    collect_structure_tuples(second, tab_id, splits, slots);
}

fn structure_tuples_retained(
    node: &NativePanelTree,
    tab_id: &WorkbenchTabId,
    splits: &HashMap<&WorkbenchSplitId, SplitTuple<'_>>,
    slots: &HashMap<&WorkbenchPanelSlotId, &WorkbenchSplitId>,
) -> bool {
    let NativePanelTree::Split {
        first,
        second,
        first_slot_id,
        second_slot_id,
        orientation,
        split_id,
    } = node
    else {
        return true;
    };
    if splits
        .get(split_id)
        .is_some_and(|tuple| *tuple != (tab_id, first_slot_id, second_slot_id, *orientation))
        || [first_slot_id, second_slot_id]
            .iter()
            .any(|slot| slots.get(*slot).is_some_and(|parent| *parent != split_id))
    {
        return false;
    }
    structure_tuples_retained(first, tab_id, splits, slots)
        && structure_tuples_retained(second, tab_id, splits, slots)
}

/// A Split identity that survives keeps its owning Tab, both Slot identities and orientation; a
/// Slot identity that survives keeps its parent Split. Removed or entirely fresh structural
/// identities need no comparison.
fn retained_structure_tuples(current: &[NativeSimpleTab], candidate: &[NativeSimpleTab]) -> bool {
    let mut splits = HashMap::new();
    let mut slots = HashMap::new();
    for tab in candidate {
        collect_structure_tuples(&tab.tree, &tab.tab_id, &mut splits, &mut slots);
    }
    current
        .iter()
        .all(|tab| structure_tuples_retained(&tab.tree, &tab.tab_id, &splits, &slots))
}

#[cfg(test)]
mod tests;
