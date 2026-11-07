//! Earlier layouts: what part of a kept checkpoint counts as its layout, how it compares with the
//! current Workbench, and grafting back only the Workspaces and Tabs whose identity is not open
//! now. It neither stores history nor launches a terminal; reopened Presentations stay unbound
//! with their retained directory as restart intent, exactly as a restored one does.

use std::collections::{BTreeMap, BTreeSet};

use orchbee_workbench_protocol::{
    ContractError, Directory, MAXIMUM_DIRECTORIES, MAXIMUM_PANEL_NODES_PER_CONTEXT,
    MAXIMUM_TABS_PER_CONTEXT, MAXIMUM_TERMINAL_CONTEXTS, PanelCustomLabel, PanelNode, PanelSlot,
    Presentation, Snapshot, Tab, TabGroup, TerminalContext, TerminalPresentationId,
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchProtocolDocument, WorkbenchRevision, WorkbenchSplitId, WorkbenchTabGroupId,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId, Workspace,
    serialize_document,
};

use super::super::{PlanFailure, WorkbenchAuthority, allocate, identity, workspace_organization};
use super::WorkbenchCheckpoint;

/// The layout of a checkpoint: Workspaces in display order, their Tabs in strip order and each
/// Tab's Panels in tree order, by identity and label. Focus, scroll and geometry are not part of
/// it, so two checkpoints that differ only there have equal layouts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedLayout {
    workspaces: Vec<LayoutWorkspace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayoutWorkspace {
    workspace_id: WorkbenchWorkspaceId,
    label: String,
    automatic_label: bool,
    tabs: Vec<LayoutTab>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayoutTab {
    tab_id: WorkbenchTabId,
    label: String,
    automatic_label: bool,
    panels: Vec<(WorkbenchPanelId, Vec<LayoutPresentation>)>,
}

type LayoutPresentation = (TerminalPresentationId, String, Option<PanelCustomLabel>);

/// How a kept layout compares with the current Workbench.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutHistorySummary {
    pub workspaces: u32,
    pub tabs: u32,
    pub workspaces_not_open: u32,
    pub tabs_not_open: u32,
}

/// What a reopen committed. `revision` is `None` when nothing was missing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenedLayout {
    pub revision: Option<WorkbenchRevision>,
    pub workspaces: u32,
    pub tabs: u32,
    /// The first reopened Tab, which an interactive Client may select.
    pub first_tab: Option<(WorkbenchTerminalContextId, WorkbenchTabId)>,
    /// Every reopened Presentation, unbound, in Workspace, Tab and tree order.
    pub presentations: Vec<TerminalPresentationId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutReopenError {
    Unavailable,
    /// A Close holds a Workspace the reopen would add to.
    Busy,
    ResourceLimit,
    IdentityExhausted,
    RevisionExhausted,
    InvalidProjection(ContractError),
}

impl std::fmt::Display for LayoutReopenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("Workbench is unavailable"),
            Self::Busy => formatter.write_str("a Close holds the target Workspace"),
            Self::ResourceLimit => formatter.write_str("the reopened layout does not fit"),
            Self::IdentityExhausted => formatter.write_str("Workbench identities are exhausted"),
            Self::RevisionExhausted => formatter.write_str("Workbench revision is exhausted"),
            Self::InvalidProjection(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LayoutReopenError {}

impl From<PlanFailure> for LayoutReopenError {
    fn from(failure: PlanFailure) -> Self {
        match failure {
            PlanFailure::Rejected(
                orchbee_workbench_protocol::WorkbenchRejectionReason::IdentityExhausted,
            ) => Self::IdentityExhausted,
            PlanFailure::Rejected(
                orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionExhausted,
            ) => Self::RevisionExhausted,
            PlanFailure::Rejected(_) => Self::ResourceLimit,
            PlanFailure::Internal => Self::InvalidProjection(ContractError::InvalidDocument),
        }
    }
}

impl WorkbenchCheckpoint {
    /// This checkpoint's layout, for comparing saves and summarizing history.
    pub fn layout(&self) -> Result<RetainedLayout, super::WorkbenchCheckpointError> {
        let worker = WorkerId::parse("00000000000000000000000000000001")
            .expect("canonical nonzero validation identity");
        let (snapshot, _) = self.retained_graph(&worker)?;
        let workspaces = display_order(&snapshot)
            .map(|workspace| LayoutWorkspace {
                workspace_id: workspace.workspace_id.clone(),
                label: workspace.label.clone(),
                automatic_label: workspace.automatic_label,
                tabs: context(&snapshot, &workspace.terminal_context_id)
                    .map(|context| context.tabs.iter().map(layout_tab).collect())
                    .unwrap_or_default(),
            })
            .collect();
        Ok(RetainedLayout { workspaces })
    }
}

impl RetainedLayout {
    /// Counts this layout's Workspaces and Tabs, and those whose identity `current` lacks.
    #[must_use]
    pub fn summary(&self, current: &Snapshot) -> LayoutHistorySummary {
        let (open_workspaces, open_tabs) = open_identities(current);
        let mut summary = LayoutHistorySummary::default();
        for workspace in &self.workspaces {
            summary.workspaces += 1;
            if !open_workspaces.contains(&workspace.workspace_id) {
                summary.workspaces_not_open += 1;
            }
            for tab in &workspace.tabs {
                summary.tabs += 1;
                if !open_tabs.contains(&tab.tab_id) {
                    summary.tabs_not_open += 1;
                }
            }
        }
        summary
    }
}

impl WorkbenchAuthority {
    /// Reopens from `entry` only the Workspaces and Tabs whose identity is not open now, with
    /// fresh identities and their split layout and labels, in one commit. A missing Workspace
    /// comes back whole; a missing Tab of an open Workspace returns at its saved index, clamped
    /// to the current strip and kept out of its pinned prefix and groups. Nothing open now
    /// changes. Each reopened Presentation keeps its retained directory as restart intent.
    pub fn reopen_layout(
        &mut self,
        entry: &WorkbenchCheckpoint,
        local_worker: &WorkerId,
    ) -> Result<ReopenedLayout, LayoutReopenError> {
        if self.retired {
            return Err(LayoutReopenError::Unavailable);
        }
        let (saved, directories) = entry
            .retained_graph(local_worker)
            .map_err(|_| LayoutReopenError::InvalidProjection(ContractError::InvalidDocument))?;
        let current = self.snapshot();
        let (open_workspaces, open_tabs) = open_identities(current);
        let mut candidate = current.clone();
        let mut graft = Graft {
            cursor: self.last_identity,
            directories: &directories,
            restart: Vec::new(),
            presentations: Vec::new(),
        };
        let mut reopened = ReopenedLayout {
            revision: None,
            workspaces: 0,
            tabs: 0,
            first_tab: None,
            presentations: Vec::new(),
        };
        for workspace in display_order(&saved) {
            let Some(saved_context) = context(&saved, &workspace.terminal_context_id) else {
                continue;
            };
            let missing: Vec<(usize, &Tab)> = saved_context
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, tab)| !open_tabs.contains(&tab.tab_id))
                .collect();
            if open_workspaces.contains(&workspace.workspace_id) {
                if missing.is_empty() {
                    continue;
                }
                if self.workspace_organization_busy(&workspace.workspace_id) {
                    return Err(LayoutReopenError::Busy);
                }
                let context_id = current
                    .navigation
                    .workspace(&workspace.workspace_id)
                    .map(|open| open.terminal_context_id.clone())
                    .ok_or(LayoutReopenError::Unavailable)?;
                let context = candidate
                    .terminal_contexts
                    .iter_mut()
                    .find(|context| context.terminal_context_id == context_id)
                    .ok_or(LayoutReopenError::Unavailable)?;
                for (saved_index, tab) in missing {
                    let mut tab = graft.tab(tab, &BTreeMap::new())?;
                    tab.pinned = false;
                    let index = insertion_index(&context.tabs, saved_index);
                    reopened
                        .first_tab
                        .get_or_insert_with(|| (context_id.clone(), tab.tab_id.clone()));
                    context.tabs.insert(index, tab);
                    reopened.tabs += 1;
                }
                fits(context)?;
            } else {
                if candidate.terminal_contexts.len() >= MAXIMUM_TERMINAL_CONTEXTS {
                    return Err(LayoutReopenError::ResourceLimit);
                }
                let workspace_id = WorkbenchWorkspaceId::parse(&graft.take()?)
                    .map_err(|_| LayoutReopenError::from(PlanFailure::Internal))?;
                let context_id = WorkbenchTerminalContextId::parse(&graft.take()?)
                    .map_err(|_| LayoutReopenError::from(PlanFailure::Internal))?;
                let primary_directory_id = match workspace
                    .primary_directory_id
                    .as_ref()
                    .and_then(|id| saved.navigation.directory(id))
                {
                    Some(directory) => Some(graft.directory(
                        &mut candidate.navigation.directories,
                        &directory.path,
                        local_worker,
                    )?),
                    None => None,
                };
                // Only Groups a reopened Tab uses come back, under fresh identities.
                let mut groups = BTreeMap::new();
                let mut reopened_groups = Vec::new();
                for group in &saved_context.groups {
                    if missing
                        .iter()
                        .any(|(_, tab)| tab.group_id.as_ref() == Some(&group.group_id))
                    {
                        let group_id = WorkbenchTabGroupId::parse(&graft.take()?)
                            .map_err(|_| LayoutReopenError::from(PlanFailure::Internal))?;
                        groups.insert(group.group_id.clone(), group_id.clone());
                        reopened_groups.push(TabGroup {
                            group_id,
                            ..group.clone()
                        });
                    }
                }
                let mut context = TerminalContext {
                    terminal_context_id: context_id.clone(),
                    groups: reopened_groups,
                    tabs: Vec::with_capacity(missing.len()),
                };
                for (_, tab) in &missing {
                    let tab = graft.tab(tab, &groups)?;
                    reopened
                        .first_tab
                        .get_or_insert_with(|| (context_id.clone(), tab.tab_id.clone()));
                    context.tabs.push(tab);
                }
                fits(&context)?;
                // A Project that no longer exists, or is full, leaves the Workspace ungrouped.
                let project_id = workspace.project_id.clone().filter(|project| {
                    workspace_organization::admit_project_member(
                        &candidate.navigation,
                        Some(project),
                    )
                    .is_ok()
                });
                candidate.navigation.workspaces.push(Workspace {
                    automatic_label: workspace.automatic_label,
                    label: workspace.label.clone(),
                    project_id,
                    primary_directory_id,
                    terminal_context_id: context_id,
                    terminal_directory: None,
                    workspace_id: workspace_id.clone(),
                });
                workspace_organization::append_workspace_order(
                    &mut candidate.navigation,
                    &workspace_id,
                );
                reopened.tabs += u32::try_from(context.tabs.len()).unwrap_or(u32::MAX);
                candidate.terminal_contexts.push(context);
                reopened.workspaces += 1;
            }
        }
        if reopened.workspaces == 0 && reopened.tabs == 0 {
            return Ok(reopened);
        }
        let revision = self.next_revision()?;
        candidate.revision = revision;
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).map_err(
            |error| match error {
                ContractError::EncodedValueTooLarge { .. } => LayoutReopenError::ResourceLimit,
                error => LayoutReopenError::InvalidProjection(error),
            },
        )?;
        let advance = self
            .projection
            .prepare_advance(&encoded)
            .map_err(|_| LayoutReopenError::InvalidProjection(ContractError::InvalidDocument))?;
        self.projection.commit_prepared(advance);
        self.last_identity = graft.cursor;
        self.restart_directories.extend(graft.restart);
        reopened.revision = Some(revision);
        reopened.presentations = graft.presentations;
        Ok(reopened)
    }
}

/// Fresh identities for grafted structure, and the restart intent its Presentations carry.
struct Graft<'a> {
    cursor: u128,
    directories: &'a BTreeMap<TerminalPresentationId, WorkbenchDirectoryPath>,
    restart: Vec<(TerminalPresentationId, WorkbenchDirectoryPath)>,
    presentations: Vec<TerminalPresentationId>,
}

impl Graft<'_> {
    fn take(&mut self) -> Result<String, LayoutReopenError> {
        self.cursor = allocate(self.cursor, 1)?;
        Ok(identity(self.cursor))
    }

    /// The Directory registered at `path`, or a new one for the local Worker.
    fn directory(
        &mut self,
        directories: &mut Vec<Directory>,
        path: &WorkbenchDirectoryPath,
        local_worker: &WorkerId,
    ) -> Result<WorkbenchDirectoryId, LayoutReopenError> {
        if let Some(existing) = directories
            .iter()
            .find(|directory| &directory.path == path && &directory.worker_id == local_worker)
        {
            return Ok(existing.directory_id.clone());
        }
        if directories.len() >= MAXIMUM_DIRECTORIES {
            return Err(LayoutReopenError::ResourceLimit);
        }
        let directory_id = WorkbenchDirectoryId::parse(&self.take()?)
            .map_err(|_| LayoutReopenError::from(PlanFailure::Internal))?;
        directories.push(Directory {
            directory_id: directory_id.clone(),
            worker_id: local_worker.clone(),
            path: path.clone(),
        });
        Ok(directory_id)
    }

    fn tab(
        &mut self,
        tab: &Tab,
        groups: &BTreeMap<WorkbenchTabGroupId, WorkbenchTabGroupId>,
    ) -> Result<Tab, LayoutReopenError> {
        let tab_id = WorkbenchTabId::parse(&self.take()?)
            .map_err(|_| LayoutReopenError::from(PlanFailure::Internal))?;
        Ok(Tab {
            automatic_label: tab.automatic_label,
            group_id: tab.group_id.as_ref().and_then(|id| groups.get(id).cloned()),
            label: tab.label.clone(),
            pinned: tab.pinned,
            root: self.node(&tab.root)?,
            tab_id,
        })
    }

    fn node(&mut self, node: &PanelNode) -> Result<PanelNode, LayoutReopenError> {
        let internal = |_| LayoutReopenError::from(PlanFailure::Internal);
        Ok(match node {
            PanelNode::Panel {
                right_click,
                child_presentations,
                primary_presentation,
                ..
            } => PanelNode::Panel {
                right_click: *right_click,
                panel_id: WorkbenchPanelId::parse(&self.take()?).map_err(internal)?,
                primary_presentation: self.presentation(primary_presentation)?,
                child_presentations: child_presentations
                    .iter()
                    .map(|child| self.presentation(child))
                    .collect::<Result<_, _>>()?,
            },
            PanelNode::Split {
                children,
                orientation,
                ratio,
                ..
            } => {
                let split_id = WorkbenchSplitId::parse(&self.take()?).map_err(internal)?;
                let first = PanelSlot {
                    slot_id: WorkbenchPanelSlotId::parse(&self.take()?).map_err(internal)?,
                    node: self.node(&children[0].node)?,
                };
                let second = PanelSlot {
                    slot_id: WorkbenchPanelSlotId::parse(&self.take()?).map_err(internal)?,
                    node: self.node(&children[1].node)?,
                };
                PanelNode::Split {
                    children: Box::new([first, second]),
                    orientation: *orientation,
                    split_id,
                    ratio: *ratio,
                }
            }
        })
    }

    fn presentation(&mut self, saved: &Presentation) -> Result<Presentation, LayoutReopenError> {
        let presentation_id = TerminalPresentationId::parse(&self.take()?)
            .map_err(|_| LayoutReopenError::from(PlanFailure::Internal))?;
        if let Some(directory) = self.directories.get(&saved.presentation_id) {
            self.restart
                .push((presentation_id.clone(), directory.clone()));
        }
        self.presentations.push(presentation_id.clone());
        Ok(Presentation {
            binding: None,
            custom_label: saved.custom_label.clone(),
            label: saved.label.clone(),
            launch_failure: None,
            launch_program: None,
            presentation_id,
        })
    }
}

fn layout_tab(tab: &Tab) -> LayoutTab {
    fn visit(node: &PanelNode, panels: &mut Vec<(WorkbenchPanelId, Vec<LayoutPresentation>)>) {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } => panels.push((
                panel_id.clone(),
                std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .map(|member| {
                        (
                            member.presentation_id.clone(),
                            member.label.clone(),
                            member.custom_label.clone(),
                        )
                    })
                    .collect(),
            )),
            PanelNode::Split { children, .. } => {
                for child in children.iter() {
                    visit(&child.node, panels);
                }
            }
        }
    }
    let mut panels = Vec::new();
    visit(&tab.root, &mut panels);
    LayoutTab {
        tab_id: tab.tab_id.clone(),
        label: tab.label.clone(),
        automatic_label: tab.automatic_label,
        panels,
    }
}

fn display_order(snapshot: &Snapshot) -> impl Iterator<Item = &Workspace> {
    let navigation = &snapshot.navigation;
    let ordered: Vec<&Workspace> = if navigation.workspace_order.is_empty() {
        navigation.workspaces.iter().collect()
    } else {
        navigation
            .workspace_order
            .iter()
            .filter_map(|id| navigation.workspace(id))
            .collect()
    };
    ordered.into_iter()
}

fn context<'a>(
    snapshot: &'a Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Option<&'a TerminalContext> {
    snapshot
        .terminal_contexts
        .iter()
        .find(|context| &context.terminal_context_id == context_id)
}

fn open_identities(
    current: &Snapshot,
) -> (BTreeSet<WorkbenchWorkspaceId>, BTreeSet<WorkbenchTabId>) {
    (
        current
            .navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect(),
        current
            .terminal_contexts
            .iter()
            .flat_map(|context| context.tabs.iter().map(|tab| tab.tab_id.clone()))
            .collect(),
    )
}

/// `saved` clamped to the strip, after its pinned prefix and never inside a Tab group.
fn insertion_index(tabs: &[Tab], saved: usize) -> usize {
    let pinned = tabs.iter().take_while(|tab| tab.pinned).count();
    let mut index = saved.clamp(pinned, tabs.len());
    while index > 0
        && index < tabs.len()
        && tabs[index - 1].group_id.is_some()
        && tabs[index - 1].group_id == tabs[index].group_id
    {
        index += 1;
    }
    index
}

fn fits(context: &TerminalContext) -> Result<(), LayoutReopenError> {
    fn nodes(node: &PanelNode) -> usize {
        match node {
            PanelNode::Panel { .. } => 1,
            PanelNode::Split { children, .. } => {
                1 + children.iter().map(|slot| nodes(&slot.node)).sum::<usize>()
            }
        }
    }
    if context.tabs.len() > MAXIMUM_TABS_PER_CONTEXT
        || context
            .tabs
            .iter()
            .map(|tab| nodes(&tab.root))
            .sum::<usize>()
            > MAXIMUM_PANEL_NODES_PER_CONTEXT
    {
        return Err(LayoutReopenError::ResourceLimit);
    }
    Ok(())
}
