//! Deterministic relocation candidates conserve retained terminal bindings while
//! atomically changing Tab/Workspace topology. Runtime and Client effects stay outside.

use super::{
    CommitPlan, Minted, PlanFailure, RelocationPlan, WorkbenchPanelRelocateAdmission, allocate,
    find_panel, identity, insert_at_root, take_leaf_panel, tree_depth,
};
use crate::workbench_authority::terminal_membership::tree_terminal_membership;
use orchbee_workbench_protocol::{
    ContractError, MAXIMUM_PANEL_NODE_DEPTH, MAXIMUM_PANEL_NODES_PER_CONTEXT,
    MAXIMUM_TABS_PER_CONTEXT, MAXIMUM_TERMINAL_CONTEXTS, MAXIMUM_WORKSPACES_PER_PROJECT, PanelNode,
    Snapshot, Tab, TerminalContext, WorkbenchCommandOutcome, WorkbenchPanelRelocateDestination,
    WorkbenchPanelSlotId, WorkbenchPanelTransfer, WorkbenchProtocolDocument,
    WorkbenchRejectionReason, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, Workspace, serialize_document,
};

impl WorkbenchPanelRelocateAdmission<'_> {
    pub(super) fn plan(
        &self,
        directory_worker: Option<&orchbee_workbench_protocol::WorkerId>,
    ) -> Result<RelocationPlan, PlanFailure> {
        let snapshot = self.authority.projection.snapshot();
        let source = context(snapshot, &self.terminal_context_id)?;
        let source_tab = source
            .tabs
            .iter()
            .find(|tab| tab.tab_id == self.tab_id)
            .ok_or(PlanFailure::Rejected(
                WorkbenchRejectionReason::TabUnavailable,
            ))?;
        let closed_source_tab = matches!(&source_tab.root, PanelNode::Panel { .. });
        let count = match self.destination {
            WorkbenchPanelRelocateDestination::Tab { .. } => 3,
            WorkbenchPanelRelocateDestination::NewWorkspace {
                ref primary_directory_path,
                ..
            } => {
                if primary_directory_path.is_some() {
                    4
                } else {
                    3
                }
            }
            WorkbenchPanelRelocateDestination::NewTab { .. } => 1,
        };

        // Resolve all caller-owned destinations before allocation or candidate mutation.
        match &self.destination {
            WorkbenchPanelRelocateDestination::Tab {
                terminal_context_id,
                tab_id,
                target_panel_id,
                ..
            } => {
                let target_context = terminal_context_id
                    .as_ref()
                    .unwrap_or(&self.terminal_context_id);
                let target = context(snapshot, target_context)?;
                let tab = target.tabs.iter().find(|tab| &tab.tab_id == tab_id).ok_or(
                    PlanFailure::Rejected(WorkbenchRejectionReason::TabUnavailable),
                )?;
                if target_context == &self.terminal_context_id && tab_id == &self.tab_id {
                    return Err(PlanFailure::Rejected(WorkbenchRejectionReason::SameTab));
                }
                if let Some(panel) = target_panel_id
                    && find_panel(&tab.root, panel).is_none()
                {
                    return Err(PlanFailure::Rejected(
                        WorkbenchRejectionReason::PanelUnavailable,
                    ));
                }
                tree_terminal_membership(snapshot, target_context)
                    .map_err(|_| PlanFailure::Internal)?;
            }
            WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id,
                ..
            } => {
                let target = terminal_context_id
                    .as_ref()
                    .unwrap_or(&self.terminal_context_id);
                context(snapshot, target)?;
                tree_terminal_membership(snapshot, target).map_err(|_| PlanFailure::Internal)?;
            }
            WorkbenchPanelRelocateDestination::NewWorkspace {
                project_id,
                primary_directory_id,
                primary_directory_path,
                ..
            } => {
                if project_id
                    .as_ref()
                    .is_some_and(|id| snapshot.navigation.project(id).is_none())
                {
                    return Err(PlanFailure::Rejected(
                        WorkbenchRejectionReason::ProjectUnavailable,
                    ));
                }
                if primary_directory_id.as_ref().is_some_and(|directory| {
                    !snapshot
                        .navigation
                        .directories
                        .iter()
                        .any(|member| &member.directory_id == directory)
                }) {
                    return Err(PlanFailure::Rejected(
                        WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                    ));
                }
                if primary_directory_path.is_some() {
                    if primary_directory_id.is_some() {
                        return Err(PlanFailure::Rejected(
                            WorkbenchRejectionReason::WorkspaceDirectoryConflict,
                        ));
                    }
                    if snapshot.navigation.directories.len()
                        >= orchbee_workbench_protocol::MAXIMUM_DIRECTORIES
                    {
                        return Err(PlanFailure::Rejected(
                            WorkbenchRejectionReason::ResourceLimit,
                        ));
                    }
                    let Some(PanelNode::Panel {
                        primary_presentation,
                        ..
                    }) = find_panel(&source_tab.root, &self.panel_id)
                    else {
                        return Err(PlanFailure::Internal);
                    };
                    if primary_presentation
                        .binding
                        .as_ref()
                        .is_none_or(|binding| Some(&binding.worker_id) != directory_worker)
                    {
                        return Err(PlanFailure::Rejected(
                            WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                        ));
                    }
                }
            }
        }
        let last_identity = allocate(self.authority.last_identity, count)?;
        let minted = match &self.destination {
            WorkbenchPanelRelocateDestination::Tab { .. } => Minted::Split {
                split_id: WorkbenchSplitId::parse(&identity(last_identity - 2))
                    .map_err(|_| PlanFailure::Internal)?,
                first_slot_id: WorkbenchPanelSlotId::parse(&identity(last_identity - 1))
                    .map_err(|_| PlanFailure::Internal)?,
                second_slot_id: WorkbenchPanelSlotId::parse(&identity(last_identity))
                    .map_err(|_| PlanFailure::Internal)?,
            },
            _ => Minted::Tab {
                tab_id: WorkbenchTabId::parse(&identity(last_identity))
                    .map_err(|_| PlanFailure::Internal)?,
            },
        };
        let target_context_id = match &self.destination {
            WorkbenchPanelRelocateDestination::Tab {
                terminal_context_id,
                ..
            }
            | WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id,
                ..
            } => terminal_context_id
                .clone()
                .unwrap_or_else(|| self.terminal_context_id.clone()),
            WorkbenchPanelRelocateDestination::NewWorkspace { .. } => {
                WorkbenchTerminalContextId::parse(&identity(last_identity - 1))
                    .map_err(|_| PlanFailure::Internal)?
            }
        };
        let revision = self.authority.next_revision()?;
        let mut candidate = snapshot.clone();
        let source = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == self.terminal_context_id)
            .ok_or(PlanFailure::Internal)?;
        let source_index = source
            .tabs
            .iter()
            .position(|tab| tab.tab_id == self.tab_id)
            .ok_or(PlanFailure::Internal)?;
        let moved = if closed_source_tab {
            source.tabs.remove(source_index).root
        } else {
            take_leaf_panel(&mut source.tabs[source_index].root, &self.panel_id)
                .ok_or(PlanFailure::Internal)?
        };
        let mut closed_source_workspace_id = None;
        if source.tabs.is_empty() && self.terminal_context_id != target_context_id {
            let index = candidate
                .navigation
                .workspaces
                .iter()
                .position(|workspace| workspace.terminal_context_id == self.terminal_context_id)
                .ok_or(PlanFailure::Internal)?;
            let removed = candidate.navigation.workspaces.remove(index);
            candidate
                .navigation
                .workspace_order
                .retain(|id| id != &removed.workspace_id);
            closed_source_workspace_id = Some(removed.workspace_id);
            candidate
                .terminal_contexts
                .retain(|context| context.terminal_context_id != self.terminal_context_id);
        }
        let mut created_workspace_id = None;
        let mut created_directory_id = None;
        if let WorkbenchPanelRelocateDestination::NewWorkspace {
            project_id,
            primary_directory_id,
            primary_directory_path,
            label,
            ..
        } = &self.destination
        {
            let workspace_id = WorkbenchWorkspaceId::parse(&identity(last_identity - 2))
                .map_err(|_| PlanFailure::Internal)?;
            if let Some(path) = primary_directory_path {
                let directory_id = orchbee_workbench_protocol::WorkbenchDirectoryId::parse(
                    &identity(last_identity - 3),
                )
                .map_err(|_| PlanFailure::Internal)?;
                candidate
                    .navigation
                    .directories
                    .push(orchbee_workbench_protocol::Directory {
                        directory_id: directory_id.clone(),
                        worker_id: directory_worker.ok_or(PlanFailure::Internal)?.clone(),
                        path: path.clone(),
                    });
                created_directory_id = Some(directory_id);
            }
            candidate.navigation.workspaces.push(Workspace {
                project_id: project_id.clone(),
                label: label.as_str().to_owned(),
                primary_directory_id: created_directory_id
                    .clone()
                    .or_else(|| primary_directory_id.clone()),
                terminal_context_id: target_context_id.clone(),
                workspace_id: workspace_id.clone(),
            });
            super::super::workspace_organization::append_workspace_order(
                &mut candidate.navigation,
                &workspace_id,
            );
            candidate.terminal_contexts.push(TerminalContext {
                groups: Vec::new(),
                tabs: Vec::new(),
                terminal_context_id: target_context_id.clone(),
            });
            created_workspace_id = Some(workspace_id);
        }
        let transfer =
            (closed_source_tab || target_context_id != self.terminal_context_id).then(|| {
                WorkbenchPanelTransfer {
                    target_terminal_context_id: target_context_id.clone(),
                    closed_source_tab,
                    closed_source_workspace_id,
                    created_workspace_id,
                    created_directory_id,
                }
            });
        let target = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == target_context_id)
            .ok_or(PlanFailure::Internal)?;
        let outcome = match (&self.destination, &minted) {
            (
                WorkbenchPanelRelocateDestination::Tab {
                    tab_id,
                    target_panel_id,
                    direction,
                    ..
                },
                Minted::Split {
                    split_id,
                    first_slot_id,
                    second_slot_id,
                },
            ) => {
                let tab = target
                    .tabs
                    .iter_mut()
                    .find(|tab| &tab.tab_id == tab_id)
                    .ok_or(PlanFailure::Internal)?;
                let graft = match target_panel_id {
                    Some(panel_id) => super::super::panel_mut(&mut tab.root, panel_id)
                        .ok_or(PlanFailure::Internal)?,
                    None => &mut tab.root,
                };
                insert_at_root(graft, moved, *direction, &minted);
                WorkbenchCommandOutcome::PanelRelocated {
                    revision,
                    terminal_context_id: self.terminal_context_id.clone(),
                    tab_id: self.tab_id.clone(),
                    panel_id: self.panel_id.clone(),
                    target_tab_id: tab_id.clone(),
                    split_id: split_id.clone(),
                    first_slot_id: first_slot_id.clone(),
                    second_slot_id: second_slot_id.clone(),
                    transfer,
                }
            }
            (WorkbenchPanelRelocateDestination::NewTab { label, .. }, Minted::Tab { tab_id })
            | (
                WorkbenchPanelRelocateDestination::NewWorkspace {
                    tab_label: label, ..
                },
                Minted::Tab { tab_id },
            ) => {
                target.tabs.push(Tab {
                    automatic_label: label.is_none(),
                    label: label.as_ref().map_or_else(
                        || (target.tabs.len() + 1).to_string(),
                        |label| label.as_str().to_owned(),
                    ),
                    tab_id: tab_id.clone(),
                    group_id: None,
                    pinned: false,
                    root: moved,
                });
                WorkbenchCommandOutcome::PanelRelocatedToNewTab {
                    revision,
                    terminal_context_id: self.terminal_context_id.clone(),
                    tab_id: self.tab_id.clone(),
                    panel_id: self.panel_id.clone(),
                    new_tab_id: tab_id.clone(),
                    transfer,
                }
            }
            _ => return Err(PlanFailure::Internal),
        };
        // Check the resulting shape, allowing a final-leaf exchange at capacity.
        if candidate.terminal_contexts.len() > MAXIMUM_TERMINAL_CONTEXTS
            || candidate.navigation.projects.iter().any(|project| {
                candidate
                    .navigation
                    .workspaces_in_project(Some(&project.project_id))
                    .count()
                    > MAXIMUM_WORKSPACES_PER_PROJECT
            })
            || candidate.terminal_contexts.iter().any(|context| {
                context.tabs.len() > MAXIMUM_TABS_PER_CONTEXT
                    || context
                        .tabs
                        .iter()
                        .any(|tab| tree_depth(&tab.root) > MAXIMUM_PANEL_NODE_DEPTH)
                    || context
                        .tabs
                        .iter()
                        .map(|tab| node_count(&tab.root))
                        .sum::<usize>()
                        > MAXIMUM_PANEL_NODES_PER_CONTEXT
            })
        {
            return Err(PlanFailure::Rejected(
                WorkbenchRejectionReason::ResourceLimit,
            ));
        }
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).map_err(
            |error| match error {
                ContractError::EncodedValueTooLarge { .. } => {
                    PlanFailure::Rejected(WorkbenchRejectionReason::ResourceLimit)
                }
                _ => PlanFailure::Internal,
            },
        )?;
        let projection = self
            .authority
            .projection
            .prepare_advance(&encoded)
            .map_err(|_| PlanFailure::Internal)?;
        Ok(RelocationPlan {
            commit: CommitPlan {
                projection: Some(projection),
                last_identity: Some(last_identity),
                catalog_next: None,
                outcome,
            },
        })
    }
}

fn context<'a>(
    snapshot: &'a Snapshot,
    id: &WorkbenchTerminalContextId,
) -> Result<&'a TerminalContext, PlanFailure> {
    snapshot
        .terminal_contexts
        .iter()
        .find(|context| &context.terminal_context_id == id)
        .ok_or(PlanFailure::Rejected(
            WorkbenchRejectionReason::TerminalContextUnavailable,
        ))
}

fn node_count(node: &PanelNode) -> usize {
    match node {
        PanelNode::Panel { .. } => 1,
        PanelNode::Split { children, .. } => {
            1 + children
                .iter()
                .map(|slot| node_count(&slot.node))
                .sum::<usize>()
        }
    }
}
