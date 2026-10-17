use orchbee_workbench_protocol::{
    MAXIMUM_TABS_PER_CONTEXT, MAXIMUM_TERMINAL_CONTEXTS, PanelNode, PanelOrientation, SiteId,
    Snapshot, Tab, TerminalId, TerminalPresentationId, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchPanelSplitDirection, WorkbenchProjectId, WorkbenchRevision,
    WorkbenchSplitId, WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, WorkerId,
};
use std::collections::HashMap;

mod closed;
mod created;
pub(super) use created::validate_created_tab;
#[cfg(windows)]
mod menu_source;
#[cfg(windows)]
pub(super) use menu_source::NativeMenuContextSource;
mod presentations;
pub(super) mod renamed;
mod tab_metadata;
pub(super) use closed::{ClosedTabExpectation, validate_closed_tab};
pub(crate) use presentations::{NativePanel, NativePresentation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeContextTabs {
    // Retain the bounded input once for exact receipts, including non-selected contexts.
    canonical: std::sync::Arc<Snapshot>,
    canonical_context: usize,
    site_id: SiteId,
    revision: WorkbenchRevision,
    tabs: Vec<NativeSimpleTab>,
    location: NativeContextLocation,
    navigation: std::sync::Arc<[NativeContext]>,
    projects: std::sync::Arc<[orchbee_workbench_protocol::Project]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(windows, derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    windows,
    serde(tag = "kind", content = "identity", deny_unknown_fields)
)]
pub(crate) enum NativeContextLocation {
    // Historical wire/checkpoint identity. Only a nonempty context is a visible compatibility
    // Workspace. An empty context remains valid for explicit targets and no-Workspace landing.
    Standalone(WorkbenchTerminalContextId),
    Workspace {
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
}

/// An explicit destination is bound to a Site as well as its navigation identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeContextTarget {
    pub(crate) site_id: SiteId,
    pub(crate) location: NativeContextLocation,
}

#[derive(Clone, Debug)]
pub(crate) enum NativeStartupTarget {
    Context(NativeContextTarget),
    #[cfg(windows)]
    Agent(Box<crate::machine_agents::Target>),
    #[cfg(windows)]
    LocalDirectory {
        position: Option<super::startup_preference::Position>,
        fallback: std::sync::Arc<std::sync::atomic::AtomicBool>,
        directory: Box<super::startup_directory::StartupDirectory>,
    },
}
impl PartialEq for NativeStartupTarget {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Context(left), Self::Context(right)) => left == right,
            #[cfg(windows)]
            (Self::Agent(left), Self::Agent(right)) => left == right,
            #[cfg(windows)]
            (
                Self::LocalDirectory {
                    position: left,
                    fallback: left_notice,
                    directory: left_directory,
                },
                Self::LocalDirectory {
                    position: right,
                    fallback: right_notice,
                    directory: right_directory,
                },
            ) => {
                left == right
                    && left_directory == right_directory
                    && std::sync::Arc::ptr_eq(left_notice, right_notice)
            }
            #[cfg(windows)]
            _ => false,
        }
    }
}
impl Eq for NativeStartupTarget {}
impl From<NativeContextTarget> for NativeStartupTarget {
    fn from(value: NativeContextTarget) -> Self {
        Self::Context(value)
    }
}
impl NativeStartupTarget {
    pub(super) fn context(&self) -> Option<&NativeContextTarget> {
        match self {
            Self::Context(context) => Some(context),
            #[cfg(windows)]
            Self::Agent(target) => Some(&target.context),
            #[cfg(windows)]
            Self::LocalDirectory { .. } => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(windows, derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(windows, serde(deny_unknown_fields))]
pub(crate) struct NativeMemberTarget {
    pub(crate) tab: WorkbenchTabId,
    pub(crate) panel: WorkbenchPanelId,
    pub(crate) presentation: TerminalPresentationId,
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
    pub(super) project_id: Option<WorkbenchProjectId>,
    pub(super) project_label: Option<WorkbenchLabel>,
    pub(super) workspace_label: Option<WorkbenchLabel>,
}

impl NativeContextTabs {
    pub(super) fn canonical_context(&self) -> &orchbee_workbench_protocol::TerminalContext {
        &self.canonical.terminal_contexts[self.canonical_context]
    }
    #[cfg(windows)]
    pub(super) fn shared_canonical_snapshot(&self) -> std::sync::Arc<Snapshot> {
        std::sync::Arc::clone(&self.canonical)
    }
    pub(super) fn canonical_snapshot(&self) -> &Snapshot {
        &self.canonical
    }
    pub(super) fn groups(&self) -> &[orchbee_workbench_protocol::TabGroup] {
        &self.canonical_context().groups
    }

    pub(super) fn projects(&self) -> &[orchbee_workbench_protocol::Project] {
        &self.projects
    }

    pub(super) fn directory(&self) -> Option<&orchbee_workbench_protocol::Directory> {
        let NativeContextLocation::Workspace { workspace_id, .. } = &self.location else {
            return None;
        };
        self.canonical.navigation.workspace_directory(workspace_id)
    }

    pub(super) fn project_id(&self) -> Option<&WorkbenchProjectId> {
        let NativeContextLocation::Workspace { workspace_id, .. } = &self.location else {
            return None;
        };
        self.canonical
            .navigation
            .workspace(workspace_id)?
            .project_id
            .as_ref()
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

    /// Allocation-free membership proof for render-only pixels retained from
    /// another context or Client lane. The canonical Snapshot has already
    /// passed the protocol's global identity and tree validation; this lookup
    /// still requires the complete placement and physical binding tuple.
    pub(in crate::terminal_session) fn contains_terminal_presentation(
        &self,
        site: &SiteId,
        context: &WorkbenchTerminalContextId,
        tab: &WorkbenchTabId,
        panel: &WorkbenchPanelId,
        presentation: &TerminalPresentationId,
        binding: &NativeTerminalBinding,
    ) -> bool {
        if &self.canonical.site_id != site {
            return false;
        }
        let Some(context) = self
            .canonical
            .terminal_contexts
            .iter()
            .find(|candidate| &candidate.terminal_context_id == context)
        else {
            return false;
        };
        let Some(tab) = context
            .tabs
            .iter()
            .find(|candidate| &candidate.tab_id == tab)
        else {
            return false;
        };
        canonical_panel(&tab.root, panel).is_some_and(|(primary, children)| {
            std::iter::once(primary)
                .chain(children.iter())
                .any(|candidate| {
                    &candidate.presentation_id == presentation
                        && candidate.binding.as_ref().is_some_and(|candidate_binding| {
                            &candidate_binding.worker_id == binding.worker_id()
                                && &candidate_binding.terminal_id == binding.terminal_id()
                        })
                })
        })
    }
}

fn canonical_panel<'a>(
    node: &'a PanelNode,
    expected: &WorkbenchPanelId,
) -> Option<(
    &'a orchbee_workbench_protocol::Presentation,
    &'a [orchbee_workbench_protocol::Presentation],
)> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } => (panel_id == expected).then_some((primary_presentation, child_presentations)),
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|child| canonical_panel(&child.node, expected)),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeSimpleTab {
    tab_id: WorkbenchTabId,
    tab_label: WorkbenchLabel,
    automatic_label: bool,
    pinned: bool,
    group: Option<orchbee_workbench_protocol::TabGroup>,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    presentation_label: WorkbenchLabel,
    binding: Option<NativeTerminalBinding>,
    panels: Box<[NativePanel]>,
    tree: NativePanelTree,
}

impl NativeSimpleTab {
    pub(super) fn group(&self) -> Option<&orchbee_workbench_protocol::TabGroup> {
        self.group.as_ref()
    }

    pub(super) const fn pinned(&self) -> bool {
        self.pinned
    }

    pub(super) fn group_label(&self) -> Option<&str> {
        self.group.as_ref().map(|group| group.label.as_str())
    }

    pub(super) fn has_replaced_terminals_from(&self, prior: &Self) -> bool {
        self.panels.iter().any(|panel| {
            prior
                .panels
                .iter()
                .find(|old| old.panel_id == panel.panel_id)
                .is_some_and(|old| {
                    old.presentations()
                        .map(|p| (p.presentation_id(), p.binding()))
                        .collect::<Vec<_>>()
                        != panel
                            .presentations()
                            .map(|p| (p.presentation_id(), p.binding()))
                            .collect::<Vec<_>>()
                })
        })
    }
    pub(super) fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }

    pub(super) fn tab_label(&self) -> &WorkbenchLabel {
        &self.tab_label
    }

    pub(super) const fn automatic_label(&self) -> bool {
        self.automatic_label
    }

    pub(super) fn display_label(&self, position: usize) -> std::borrow::Cow<'_, str> {
        orchbee_workbench_protocol::tab_display_label(
            self.tab_label.as_str(),
            self.automatic_label,
            position,
        )
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
    pub(super) fn binding(&self) -> Option<&NativeTerminalBinding> {
        self.binding.as_ref()
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
        ratio: orchbee_workbench_protocol::WorkbenchSplitRatio,
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
    pub(super) fn split_ratio(
        &self,
        id: &WorkbenchSplitId,
        first_id: &WorkbenchPanelSlotId,
        second_id: &WorkbenchPanelSlotId,
    ) -> Option<orchbee_workbench_protocol::WorkbenchSplitRatio> {
        let Self::Split {
            split_id,
            first_slot_id,
            second_slot_id,
            first,
            second,
            ratio,
            ..
        } = self
        else {
            return None;
        };
        if split_id == id {
            return (first_slot_id == first_id && second_slot_id == second_id).then_some(*ratio);
        }
        first
            .split_ratio(id, first_id, second_id)
            .or_else(|| second.split_ratio(id, first_id, second_id))
    }
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
                ratio,
                ..
            } => match orientation {
                PanelOrientation::Horizontal => {
                    let first_width = width * ratio.value();
                    first.collect_bounds(left, top, first_width, height, output);
                    second.collect_bounds(
                        left + first_width,
                        top,
                        width - first_width,
                        height,
                        output,
                    );
                }
                PanelOrientation::Vertical => {
                    let first_height = height * ratio.value();
                    first.collect_bounds(left, top, width, first_height, output);
                    second.collect_bounds(
                        left,
                        top + first_height,
                        width,
                        height - first_height,
                        output,
                    );
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
    placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    requested_label: WorkbenchLabel,
    automatic_label: bool,
}

impl CreatedTabExpectation {
    pub(super) fn new(
        committed_revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        requested_label: WorkbenchLabel,
        automatic_label: bool,
    ) -> Self {
        Self {
            placement: None,
            committed_revision,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            requested_label,
            automatic_label,
        }
    }

    pub(super) fn with_placement(
        mut self,
        placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
    ) -> Self {
        self.placement = placement;
        self
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

    pub(super) fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }

    #[cfg(test)]
    pub(super) fn requested_label(&self) -> &WorkbenchLabel {
        &self.requested_label
    }
}

/// A canonical context or its visible Workspace projection failed validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeProjectionError;

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

/// Resolves the legacy Workspace only when it owns content. Its identity remains a Context,
/// never a fabricated canonical Workspace ID. The required compatibility slot is always checked.
pub(crate) fn compatibility_workspace(
    snapshot: &Snapshot,
) -> Result<Option<NativeContextLocation>, NativeProjectionError> {
    let id = &snapshot.navigation.standalone_terminal_context_id;
    let mut contexts = snapshot
        .terminal_contexts
        .iter()
        .filter(|context| &context.terminal_context_id == id);
    let context = contexts.next().ok_or(NativeProjectionError)?;
    if contexts.next().is_some() {
        return Err(NativeProjectionError);
    }
    Ok((!context.tabs.is_empty()).then(|| NativeContextLocation::Standalone(id.clone())))
}

/// Ordinary entry and removal prefer canonical Workspaces, including empty ones. The
/// compatibility slot is the last landing and may represent the explicit no-Workspace state.
pub(super) fn preferred_workspace_location(
    snapshot: &Snapshot,
) -> Result<NativeContextLocation, NativeProjectionError> {
    compatibility_workspace(snapshot)?;
    Ok(snapshot
        .navigation
        .ordered_workspaces()
        .map_err(|_| NativeProjectionError)?
        .first()
        .map_or_else(
            || {
                NativeContextLocation::Standalone(
                    snapshot.navigation.standalone_terminal_context_id.clone(),
                )
            },
            |(_, workspace)| NativeContextLocation::Workspace {
                workspace_id: workspace.workspace_id.clone(),
                terminal_context_id: workspace.terminal_context_id.clone(),
            },
        ))
}

pub(super) fn project_native_context_tabs(
    snapshot: &Snapshot,
    location: &NativeContextLocation,
) -> Result<NativeContextTabs, NativeProjectionError> {
    // Visibility does not relax the mandatory disjoint compatibility slot or its capacity.
    if snapshot.navigation.workspaces.len() >= MAXIMUM_TERMINAL_CONTEXTS {
        return Err(NativeProjectionError);
    }
    let mut navigation = Vec::new();
    if let Some(location) = compatibility_workspace(snapshot)? {
        navigation.push(NativeContext {
            location,
            project_id: None,
            project_label: None,
            workspace_label: None,
        });
    }
    for (project, workspace) in snapshot
        .navigation
        .ordered_workspaces()
        .map_err(|_| NativeProjectionError)?
    {
        if workspace.terminal_context_id == snapshot.navigation.standalone_terminal_context_id
            || workspace.primary_directory_id.as_ref().is_some_and(|id| {
                snapshot
                    .navigation
                    .directories
                    .iter()
                    .filter(|directory| &directory.directory_id == id)
                    .count()
                    != 1
            })
        {
            return Err(NativeProjectionError);
        }
        if navigation.len() == MAXIMUM_TERMINAL_CONTEXTS {
            return Err(NativeProjectionError);
        }
        navigation.push(NativeContext {
            location: NativeContextLocation::Workspace {
                workspace_id: workspace.workspace_id.clone(),
                terminal_context_id: workspace.terminal_context_id.clone(),
            },
            project_id: workspace.project_id.clone(),
            project_label: project
                .map(|project| WorkbenchLabel::parse(&project.label))
                .transpose()
                .map_err(|_| NativeProjectionError)?,
            workspace_label: Some(
                WorkbenchLabel::parse(&workspace.label).map_err(|_| NativeProjectionError)?,
            ),
        });
    }
    let admitted_location = match location {
        NativeContextLocation::Standalone(id) => {
            id == &snapshot.navigation.standalone_terminal_context_id
        }
        NativeContextLocation::Workspace { .. } => {
            navigation.iter().any(|entry| &entry.location == location)
        }
    };
    if !admitted_location
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
        .enumerate()
        .filter(|(_, context)| &context.terminal_context_id == location.terminal_context_id());
    let (canonical_context, context) = contexts.next().ok_or(NativeProjectionError)?;
    if contexts.next().is_some() || context.tabs.len() > MAXIMUM_TABS_PER_CONTEXT {
        return Err(NativeProjectionError);
    }
    tab_metadata::validate(context)?;

    let tabs = context
        .tabs
        .iter()
        .map(|tab| project_tab(tab, &context.groups))
        .collect::<Result<Vec<_>, _>>()?;
    if !unique_structural_ids(&tabs) {
        return Err(NativeProjectionError);
    }
    Ok(NativeContextTabs {
        canonical: std::sync::Arc::new(snapshot.clone()),
        canonical_context,
        site_id: snapshot.site_id.clone(),
        revision: snapshot.revision,
        tabs,
        location: location.clone(),
        navigation: navigation.into(),
        projects: snapshot
            .navigation
            .ordered_projects()
            .map_err(|_| NativeProjectionError)?
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
            .into(),
    })
}

pub(super) fn validate_canonical_refresh(
    current: &NativeContextTabs,
    candidate: NativeContextTabs,
) -> Result<NativeContextTabs, NativeProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    Ok(candidate)
}

pub(super) fn validate_moved_tab(
    baseline: &NativeContextTabs,
    expected: &MovedTabExpectation,
    candidate: NativeContextTabs,
) -> Result<ValidatedMovedProjection, NativeProjectionDivergence> {
    validate_canonical_candidate(baseline, &candidate)?;
    tab_metadata::validate_receipt(baseline, &candidate, expected.committed_revision)?;
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
    tab_metadata::validate_receipt(baseline, &candidate, expected.committed_revision)?;
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
            || candidate_tab.automatic_label != baseline_tab.automatic_label
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
    tab_metadata::validate_receipt(baseline, &candidate, expected.committed_revision)?;
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
            || candidate_tab.automatic_label != baseline_tab.automatic_label
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
            ..
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
        ..
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
            ..
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
        || added_panel.1.presentation_count() != 1
        || added_panel.1.primary.presentation_id != expected.presentation_id
        || added_panel.1.primary.presentation_label != expected.requested_label
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

fn project_tab(
    tab: &Tab,
    groups: &[orchbee_workbench_protocol::TabGroup],
) -> Result<NativeSimpleTab, NativeProjectionError> {
    let mut panels = Vec::new();
    let tree = project_panel_tree(&tab.root, &mut panels)?;
    let primary = panels.first().ok_or(NativeProjectionError)?;
    Ok(NativeSimpleTab {
        tab_id: tab.tab_id.clone(),
        tab_label: WorkbenchLabel::parse(&tab.label).map_err(|_error| NativeProjectionError)?,
        automatic_label: tab.automatic_label,
        pinned: tab.pinned,
        group: tab
            .group_id
            .as_ref()
            .and_then(|id| groups.iter().find(|group| &group.group_id == id))
            .cloned(),
        panel_id: primary.panel_id.clone(),
        presentation_id: primary.primary.presentation_id.clone(),
        presentation_label: primary.primary.presentation_label.clone(),
        binding: primary.primary.binding.clone(),
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
            right_click,
            child_presentations,
            panel_id,
            primary_presentation,
        } => {
            if child_presentations.len()
                > orchbee_workbench_protocol::MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL
            {
                return Err(NativeProjectionError);
            }
            let panel_index = panels.len();
            panels.push(NativePanel {
                right_click: *right_click,
                panel_id: panel_id.clone(),
                primary: NativePresentation::project(primary_presentation)?,
                children: child_presentations
                    .iter()
                    .map(NativePresentation::project)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            });
            Ok(NativePanelTree::Panel { panel_index })
        }
        PanelNode::Split {
            children,
            orientation,
            split_id,
            ratio,
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
                ratio: *ratio,
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
        current.groups(),
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
    groups: &[orchbee_workbench_protocol::TabGroup],
    candidate: &NativeContextTabs,
) -> Result<(), NativeProjectionDivergence> {
    if &candidate.site_id != site_id
        || &candidate.location != location
        || candidate.revision < revision
    {
        return Err(NativeProjectionDivergence);
    }
    if candidate.revision == revision {
        return (candidate.tabs.as_slice() == tabs && candidate.groups() == groups)
            .then_some(())
            .ok_or(NativeProjectionDivergence);
    }
    if candidate.tabs.len() > MAXIMUM_TABS_PER_CONTEXT
        || !unique_structural_ids(&candidate.tabs)
        || !retained_tab_labels(tabs, &candidate.tabs)
        || !retained_member_tuples(
            tabs,
            &candidate.tabs,
            matches!(location, NativeContextLocation::Workspace { .. }),
        )
        || !retained_structure_tuples(tabs, &candidate.tabs)
    {
        return Err(NativeProjectionDivergence);
    }
    Ok(())
}

fn unique_structural_ids(tabs: &[NativeSimpleTab]) -> bool {
    let mut tab_ids = std::collections::HashSet::new();
    let mut panels = std::collections::HashSet::new();
    let mut presentations = std::collections::HashSet::new();
    let mut terminals = std::collections::HashSet::new();
    let mut splits = std::collections::HashSet::new();
    let mut slots = std::collections::HashSet::new();
    tabs.iter().all(|tab| {
        let mut split_ids = Vec::new();
        let mut slot_ids = Vec::new();
        tab.tree.split_ids(&mut split_ids);
        tab.tree.slot_ids(&mut slot_ids);
        tab.panels.first().is_some_and(|panel| {
            tab.panel_id == panel.panel_id
                && tab.presentation_id == panel.primary.presentation_id
                && tab.presentation_label == panel.primary.presentation_label
                && tab.binding == panel.primary.binding
        }) && tab_ids.insert(&tab.tab_id)
            && split_ids.into_iter().all(|id| splits.insert(id))
            && slot_ids.into_iter().all(|id| slots.insert(id))
            && tab.panels.iter().all(|panel| {
                panels.insert(&panel.panel_id)
                    && panel.presentations().all(|member| {
                        presentations.insert(member.presentation_id())
                            && member
                                .binding()
                                .is_none_or(|binding| terminals.insert(binding.terminal_id()))
                    })
            })
    })
}

fn retained_tab_labels(before: &[NativeSimpleTab], after: &[NativeSimpleTab]) -> bool {
    after.iter().all(|next| {
        !next.automatic_label
            || before
                .iter()
                .find(|prior| prior.tab_id == next.tab_id)
                .is_none_or(|prior| prior.automatic_label && prior.tab_label == next.tab_label)
    })
}

/// One member record: a leaf Panel member or the tab-level primary Presentation
/// projection. Overlap on any Panel, Presentation or terminal identity requires every
/// identity-bearing field to match — Panel, Presentation, label, and the complete binding
/// — while the owning Tab field is the one authorized delta: `panel.relocate` moves a
/// member between Tabs inside this same TerminalContext, so a changed Tab identity is the
/// legitimate signature of a cross-Tab relocation, never of identity replacement. A Worker
/// ID alone is still not a member identity.
type MemberTuple<'a> = (
    &'a WorkbenchTabId,
    &'a WorkbenchPanelId,
    &'a TerminalPresentationId,
    &'a WorkbenchLabel,
    &'a Option<NativeTerminalBinding>,
);

fn member_tuples(tab: &NativeSimpleTab) -> impl Iterator<Item = MemberTuple<'_>> {
    std::iter::once((
        &tab.tab_id,
        &tab.panel_id,
        &tab.presentation_id,
        &tab.presentation_label,
        &tab.binding,
    ))
    .chain(tab.panels.iter().flat_map(move |panel| {
        panel.presentations().map(move |member| {
            (
                &tab.tab_id,
                &panel.panel_id,
                &member.presentation_id,
                &member.presentation_label,
                &member.binding,
            )
        })
    }))
}

/// Surviving Presentation and Terminal identities retain their Panel, label and binding.
/// A Workspace Panel may instead acquire a fresh Presentation and Terminal on the same Worker;
/// standalone Panels retain the original complete tuple. The owning Tab field may differ
/// — exactly the authorized same-context relocation delta, expressed per member identity
/// rather than per Tab roster. A Tab whose identity survives must still own at least one of
/// its own original members inside that same Tab: genuinely removed members and entirely
/// fresh identities need no comparison. This surviving-Tab floor is a conservative consumer
/// boundary, not a complete characterization of every reachable producer history — a folded
/// history that relocates all of a Tab's members elsewhere (for example a two-Tab full member
/// exchange, which legal command steps can produce) is still rejected as divergence.
fn retained_member_tuples(
    current: &[NativeSimpleTab],
    candidate: &[NativeSimpleTab],
    allow_replacement: bool,
) -> bool {
    let current_presentations: std::collections::HashSet<_> = current
        .iter()
        .flat_map(member_tuples)
        .map(|member| member.2)
        .collect();
    let current_terminals: std::collections::HashSet<_> = current
        .iter()
        .flat_map(member_tuples)
        .filter_map(|member| member.4.as_ref().map(|binding| &binding.terminal_id))
        .collect();
    let mut primary_panels: HashMap<&WorkbenchPanelId, MemberTuple> = HashMap::new();
    let mut presentations: HashMap<&TerminalPresentationId, Vec<MemberTuple>> = HashMap::new();
    let mut terminals: HashMap<&TerminalId, Vec<MemberTuple>> = HashMap::new();
    for tab in candidate {
        for panel in &tab.panels {
            primary_panels.insert(
                &panel.panel_id,
                (
                    &tab.tab_id,
                    &panel.panel_id,
                    &panel.primary.presentation_id,
                    &panel.primary.presentation_label,
                    &panel.primary.binding,
                ),
            );
        }
        for member in member_tuples(tab) {
            presentations.entry(member.2).or_default().push(member);
            if let Some(binding) = member.4 {
                terminals
                    .entry(&binding.terminal_id)
                    .or_default()
                    .push(member);
            }
        }
    }
    current.iter().all(|tab| {
        // A surviving Tab identity must still own at least one of its own
        // current members under the same Tab identity — a conservative
        // consumer-side floor, deliberately stricter than the producer's
        // `last-panel-in-tab` move rejection it is often mistaken for.
        let mut retains_member = false;
        for member in member_tuples(tab) {
            let primary = tab
                .panels
                .iter()
                .any(|panel| panel.primary.presentation_id == *member.2);
            for hit in primary_panels
                .get(member.1)
                .into_iter()
                .filter(|_| primary)
                .chain(presentations.get(member.2).into_iter().flatten())
                .chain(
                    member
                        .4
                        .as_ref()
                        .and_then(|binding| terminals.get(&binding.terminal_id))
                        .into_iter()
                        .flatten(),
                )
            {
                let exact = hit.1 == member.1
                    && hit.2 == member.2
                    && hit.3 == member.3
                    && hit.4 == member.4;
                let newly_bound = member.4.is_none()
                    && hit.1 == member.1
                    && hit.2 == member.2
                    && hit.3 == member.3
                    && hit
                        .4
                        .as_ref()
                        .is_some_and(|binding| !current_terminals.contains(&binding.terminal_id));
                let replacement = allow_replacement
                    && hit.1 == member.1
                    && hit.3 == member.3
                    && hit
                        .4
                        .as_ref()
                        .zip(member.4.as_ref())
                        .is_some_and(|(next, previous)| {
                            next.worker_id == previous.worker_id
                                && !current_terminals.contains(&next.terminal_id)
                        })
                    && !current_presentations.contains(hit.2);
                if !exact && !newly_bound && !replacement {
                    return false;
                }
                retains_member = retains_member || hit.0 == member.0;
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
        ..
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
        ..
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
