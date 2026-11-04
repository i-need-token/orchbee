//! Atomic retained-Panel relocation admission and lifecycle fencing. Candidate
//! topology planning is independent of Session composition and Client view effects.

use super::{
    CommitPlan, PlanFailure, PreparedCommandCommit, RetainedRequest, WorkbenchAuthority,
    WorkbenchGrant, allocate, identity, result, unavailable,
};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, PanelSlot, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
    WorkbenchPanelId, WorkbenchPanelRelocateDestination, WorkbenchPanelSlotId,
    WorkbenchPanelSplitDirection, WorkbenchRejectionReason, WorkbenchSplitId, WorkbenchTabId,
    WorkbenchTerminalContextId,
};

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelRelocateBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchPanelRelocateAdmission<'a>>),
}

pub struct WorkbenchPanelRelocateAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    destination: WorkbenchPanelRelocateDestination,
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelRelocatePreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchPanelRelocate<'a>>),
}

pub struct PreparedWorkbenchPanelRelocate<'a> {
    commit: PreparedCommandCommit<'a>,
}

struct RelocationPlan {
    commit: CommitPlan,
}

impl PreparedWorkbenchPanelRelocate<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchCommandResult {
        &self.commit.response
    }

    /// Retire a composition failure before publishing any topology or retained success.
    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.commit.authority.retire(&self.commit.retained.command)
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        self.commit.commit()
    }
}

impl<'a> WorkbenchPanelRelocateAdmission<'a> {
    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// Prepare the complete Workspace topology without publishing it.
    #[must_use]
    pub fn finish(self) -> WorkbenchPanelRelocatePreparation<'a> {
        self.finish_inner(None)
    }

    /// The local service supplies the Worker only after its cwd probe and generation recheck.
    #[must_use]
    pub fn finish_with_directory(
        self,
        worker: orchbee_workbench_protocol::WorkerId,
    ) -> WorkbenchPanelRelocatePreparation<'a> {
        self.finish_inner(Some(worker))
    }

    fn finish_inner(
        self,
        worker: Option<orchbee_workbench_protocol::WorkerId>,
    ) -> WorkbenchPanelRelocatePreparation<'a> {
        if matches!(
            &self.destination,
            WorkbenchPanelRelocateDestination::NewWorkspace {
                primary_directory_path: Some(_),
                ..
            }
        ) && worker.is_none()
        {
            return WorkbenchPanelRelocatePreparation::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::DirectoryUnavailable),
            ));
        }
        let plan = match self.plan(worker.as_ref()) {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => RelocationPlan {
                commit: CommitPlan {
                    projection: None,
                    last_identity: None,
                    catalog_next: None,
                    outcome: self.authority.rejected(reason),
                },
            },
            Err(PlanFailure::Internal) => {
                return WorkbenchPanelRelocatePreparation::Ready(
                    self.authority.retire(&self.command),
                );
            }
        };
        // Lifecycle fences are transient and keep rejection precedence: they
        // only fire on a committed relocation, never on a typed refusal.
        if matches!(
            plan.commit.outcome,
            WorkbenchCommandOutcome::PanelRelocated { .. }
                | WorkbenchCommandOutcome::PanelRelocatedToNewTab { .. }
        ) && self.lifecycle_fenced()
        {
            return WorkbenchPanelRelocatePreparation::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        match self
            .authority
            .prepare_command_commit(self.command, plan.commit)
        {
            Ok(commit) => WorkbenchPanelRelocatePreparation::Prepared(Box::new(
                PreparedWorkbenchPanelRelocate { commit },
            )),
            Err(result) => WorkbenchPanelRelocatePreparation::Ready(*result),
        }
    }

    /// Open lifecycle work touching either endpoint or the moved leaf:
    /// an open aggregate Close on either Tab re-proves its captured leaf
    /// membership at publication, an open leaf Close on the moved member
    /// re-proves that exact leaf tuple under the source Tab, and a pending
    /// create/split whose minted Panel is the moved leaf still owes its
    /// terminal settlement — all transient `terminal-busy` refusals that
    /// retain nothing.
    fn lifecycle_fenced(&self) -> bool {
        let (target_context, target_tab) = match &self.destination {
            WorkbenchPanelRelocateDestination::Tab {
                terminal_context_id,
                tab_id,
                ..
            } => (
                Some(
                    terminal_context_id
                        .as_ref()
                        .unwrap_or(&self.terminal_context_id),
                ),
                Some(tab_id),
            ),
            WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id,
                ..
            } => (
                Some(
                    terminal_context_id
                        .as_ref()
                        .unwrap_or(&self.terminal_context_id),
                ),
                None,
            ),
            WorkbenchPanelRelocateDestination::NewWorkspace { .. } => (None, None),
        };
        self.authority
            .aggregate_close_fences_tab(&self.terminal_context_id, &self.tab_id)
            || target_context.is_some_and(|context| {
                self.authority.open_close_fences_context(context)
                    || target_tab.is_some_and(|tab| {
                        self.authority.aggregate_close_fences_tab(context, tab)
                            || self.authority.pending_terminal_covers_tab(context, tab)
                    })
            })
            || self.authority.open_close_covers_panel(
                &self.terminal_context_id,
                &self.tab_id,
                &self.panel_id,
            )
            || self
                .authority
                .pending_terminal_covers_panel(&self.terminal_context_id, &self.panel_id)
    }
}

impl WorkbenchAuthority {
    /// Read-only admission before the local service releases its gates to probe a new directory.
    #[must_use]
    pub fn preflight_relocation_directory(
        &self,
        grant: Option<&WorkbenchGrant>,
        command: &WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        if !grant.is_some_and(|grant| {
            grant.session_id == *self.session_id()
                && grant.mutate_navigation
                && grant.associate_directory
        }) {
            return Some(result(command, WorkbenchCommandOutcome::Denied {}));
        }
        if let Some(response) = self.command_preflight(command) {
            return Some(response);
        }
        let reason = if command.session_id != *self.session_id() {
            Some(WorkbenchRejectionReason::SessionMismatch)
        } else if command.expected_revision != self.projection.snapshot().revision {
            Some(WorkbenchRejectionReason::RevisionMismatch)
        } else {
            None
        };
        reason.map(|reason| result(command, self.rejected(reason)))
    }

    pub fn begin_terminal_panel_relocate(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchPanelRelocateBegin<'_> {
        let authorized = matches!(&command.action, WorkbenchAction::PanelRelocate { .. })
            && grant.is_some_and(|grant| {
                grant.session_id == *self.session_id()
                    && grant.mutate_navigation
                    && (!matches!(
                        &command.action,
                        WorkbenchAction::PanelRelocate {
                            destination: WorkbenchPanelRelocateDestination::NewWorkspace {
                                primary_directory_path: Some(_),
                                ..
                            },
                            ..
                        }
                    ) || grant.associate_directory)
            });
        if !authorized {
            return WorkbenchPanelRelocateBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(result) = self.command_preflight(&command) {
            return WorkbenchPanelRelocateBegin::Ready(result);
        }
        let WorkbenchAction::PanelRelocate {
            terminal_context_id,
            tab_id,
            panel_id,
            destination,
        } = command.action.clone()
        else {
            return WorkbenchPanelRelocateBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            ));
        };
        let snapshot = self.projection.snapshot();
        let rejection = if command.session_id != snapshot.session_id {
            Some(WorkbenchRejectionReason::SessionMismatch)
        } else if command.expected_revision != snapshot.revision {
            Some(WorkbenchRejectionReason::RevisionMismatch)
        } else if !snapshot
            .navigation
            .workspaces
            .iter()
            .any(|workspace| workspace.terminal_context_id == terminal_context_id)
        {
            Some(WorkbenchRejectionReason::TerminalContextUnavailable)
        } else {
            None
        };
        if let Some(reason) = rejection {
            return WorkbenchPanelRelocateBegin::Ready(self.retain_rejection(command, reason));
        }
        // The client-visible source resolution precedes the membership
        // invariant: a named-but-absent Tab or Panel, and a named stacked
        // Panel, each earn their typed refusal while a foreign shape
        // elsewhere in the context still retires the authority.
        let source = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == terminal_context_id)
            .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == tab_id));
        let Some(source_tab) = source else {
            return WorkbenchPanelRelocateBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::TabUnavailable),
            );
        };
        let source_leaf = match find_panel(&source_tab.root, &panel_id) {
            None => Some(WorkbenchRejectionReason::PanelUnavailable),
            Some(PanelNode::Panel { .. }) => None,
            Some(PanelNode::Split { .. }) => {
                return WorkbenchPanelRelocateBegin::Ready(self.retire(&command));
            }
        };
        if let Some(reason) = source_leaf {
            return WorkbenchPanelRelocateBegin::Ready(self.retain_rejection(command, reason));
        }
        // Tree membership includes every child Presentation; group/pin ordering
        // retains its separate structural admission policy.
        let _membership = match super::terminal_membership::tree_terminal_membership(
            snapshot,
            &terminal_context_id,
        ) {
            Ok(membership) => membership,
            Err(_invariant) => {
                return WorkbenchPanelRelocateBegin::Ready(self.retire(&command));
            }
        };
        WorkbenchPanelRelocateBegin::Admitted(Box::new(WorkbenchPanelRelocateAdmission {
            authority: self,
            command,
            terminal_context_id,
            tab_id,
            panel_id,
            destination,
        }))
    }

    /// Whether an unsettled terminal create or split still owes this Tab a member.
    pub(super) fn pending_terminal_covers_tab(
        &self,
        context: &WorkbenchTerminalContextId,
        tab: &WorkbenchTabId,
    ) -> bool {
        self.ledger.values().any(|entry| match entry {
            RetainedRequest::PendingTabCreate(entry) => {
                entry.metadata().terminal_context_id() == context
                    && entry.metadata().tab_id() == tab
            }
            RetainedRequest::PendingPanelCreate(entry) => {
                entry.metadata().terminal_context_id() == context
                    && entry.metadata().tab_id() == tab
            }
            _ => false,
        })
    }

    /// Whether an unsettled terminal creation belongs to this Panel. Tab creation
    /// and split mint a Panel; child creation and binding retain their existing Panel.
    /// Relocating that Panel before settlement would invalidate the pending proof.
    pub(super) fn pending_terminal_covers_panel(
        &self,
        terminal_context_id: &WorkbenchTerminalContextId,
        panel_id: &WorkbenchPanelId,
    ) -> bool {
        self.ledger.values().any(|entry| match entry {
            RetainedRequest::PendingTabCreate(entry) => {
                entry.metadata().terminal_context_id() == terminal_context_id
                    && entry.metadata().panel_id() == panel_id
            }
            RetainedRequest::PendingPanelCreate(entry) => {
                entry.metadata().terminal_context_id() == terminal_context_id
                    && entry.metadata().terminal_panel_id() == panel_id
            }
            _ => false,
        })
    }
}

enum Minted {
    Split {
        split_id: WorkbenchSplitId,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
    },
    Tab {
        tab_id: WorkbenchTabId,
    },
}

/// Locates the Panel node carrying `panel_id`, whether leaf or stacked.
fn find_panel<'a>(node: &'a PanelNode, panel_id: &WorkbenchPanelId) -> Option<&'a PanelNode> {
    match node {
        PanelNode::Panel {
            panel_id: candidate,
            ..
        } if candidate == panel_id => Some(node),
        PanelNode::Panel { .. } => None,
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| find_panel(&slot.node, panel_id)),
    }
}

fn tree_depth(node: &PanelNode) -> usize {
    match node {
        PanelNode::Panel { .. } => 1,
        PanelNode::Split { children, .. } => {
            1 + children
                .iter()
                .map(|child| tree_depth(&child.node))
                .max()
                .unwrap_or(0)
        }
    }
}

fn move_orientation(direction: WorkbenchPanelSplitDirection) -> PanelOrientation {
    match direction {
        WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Below => {
            PanelOrientation::Vertical
        }
        WorkbenchPanelSplitDirection::Left | WorkbenchPanelSplitDirection::Right => {
            PanelOrientation::Horizontal
        }
    }
}

const fn move_inserts_before(direction: WorkbenchPanelSplitDirection) -> bool {
    matches!(
        direction,
        WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Left
    )
}

/// Detaches the leaf Panel `panel_id`, replacing its parent Split with the
/// sibling subtree, and returns the retained node. The caller guarantees the
/// leaf is not the Tab root; a root move removes its source Tab instead.
fn take_leaf_panel(node: &mut PanelNode, panel_id: &WorkbenchPanelId) -> Option<PanelNode> {
    let PanelNode::Split { children, .. } = node else {
        return None;
    };
    for index in 0..2 {
        if matches!(
            &children[index].node,
            PanelNode::Panel {
                panel_id: candidate,
                ..
            } if candidate == panel_id
        ) {
            let moved = children[index].node.clone();
            *node = children[1 - index].node.clone();
            return Some(moved);
        }
        if let moved @ Some(_) = take_leaf_panel(&mut children[index].node, panel_id) {
            return moved;
        }
    }
    None
}

/// Replaces the destination Tab's root with a fresh Split holding the moved
/// leaf and the previous root in the direction-implied order and
/// orientation.
fn insert_at_root(
    root: &mut PanelNode,
    moved: PanelNode,
    direction: WorkbenchPanelSplitDirection,
    minted: &Minted,
) {
    let Minted::Split {
        split_id,
        first_slot_id,
        second_slot_id,
    } = minted
    else {
        return;
    };
    let previous = root.clone();
    let (first_node, second_node) = if move_inserts_before(direction) {
        (moved, previous)
    } else {
        (previous, moved)
    };
    *root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: first_node,
                slot_id: first_slot_id.clone(),
            },
            PanelSlot {
                node: second_node,
                slot_id: second_slot_id.clone(),
            },
        ]),
        orientation: move_orientation(direction),
        split_id: split_id.clone(),
    };
}

mod plan;

#[cfg(test)]
mod tests;
