use super::{
    CommitPlan, PlanFailure, PreparedCommandCommit, WorkbenchAuthority, WorkbenchGrant,
    WorkbenchTerminalMembership, allocate, identity, result, unavailable,
};
use orchbee_workbench_protocol::{
    ContractError, PanelNode, PanelOrientation, PanelSlot, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
    WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchPanelSplitDirection,
    WorkbenchProtocolDocument, WorkbenchRejectionReason, WorkbenchSplitId, WorkbenchTabId,
    WorkbenchTerminalContextId, serialize_document,
};

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelMoveBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchPanelMoveAdmission<'a>>),
}

pub struct WorkbenchPanelMoveAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    terminal_context_id: WorkbenchTerminalContextId,
    membership: Option<WorkbenchTerminalMembership>,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    target_panel_id: WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelMovePreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchPanelMove<'a>>),
}

pub struct PreparedWorkbenchPanelMove<'a>(PreparedCommandCommit<'a>);

impl PreparedWorkbenchPanelMove<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchCommandResult {
        &self.0.response
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        self.0.commit()
    }
}

impl<'a> WorkbenchPanelMoveAdmission<'a> {
    #[must_use]
    pub fn standalone_membership(&self) -> Option<&WorkbenchTerminalMembership> {
        self.membership.as_ref()
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// Site validates standalone bootstrap composition before resolving its target or no-op.
    #[must_use]
    pub fn finish(self) -> WorkbenchPanelMovePreparation<'a> {
        let plan = match self.plan() {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => CommitPlan {
                projection: None,
                last_identity: None,
                catalog_next: None,
                outcome: self.authority.rejected(reason),
            },
            Err(PlanFailure::Internal) => {
                return WorkbenchPanelMovePreparation::Ready(self.authority.retire(&self.command));
            }
        };
        // An open aggregate Close fences a committed same-Tab move: publication re-proves
        // the complete captured ordered leaf membership. Rejection and no-op `Unchanged`
        // outcomes keep their precedence, and the transient busy retains nothing.
        if matches!(plan.outcome, WorkbenchCommandOutcome::PanelMoved { .. })
            && self
                .authority
                .aggregate_close_fences_tab(&self.terminal_context_id, &self.tab_id)
        {
            return WorkbenchPanelMovePreparation::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        match self.authority.prepare_command_commit(self.command, plan) {
            Ok(prepared) => WorkbenchPanelMovePreparation::Prepared(Box::new(
                PreparedWorkbenchPanelMove(prepared),
            )),
            Err(result) => WorkbenchPanelMovePreparation::Ready(*result),
        }
    }

    fn plan(&self) -> Result<CommitPlan, PlanFailure> {
        let context = self
            .authority
            .projection
            .snapshot()
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == self.terminal_context_id)
            .ok_or(PlanFailure::Internal)?;
        let tab = context
            .tabs
            .iter()
            .find(|tab| tab.tab_id == self.tab_id)
            .ok_or(PlanFailure::Rejected(
                WorkbenchRejectionReason::TabUnavailable,
            ))?;
        if !contains_leaf_panel(&tab.root, &self.panel_id)
            || !contains_leaf_panel(&tab.root, &self.target_panel_id)
        {
            return Err(PlanFailure::Rejected(
                WorkbenchRejectionReason::PanelUnavailable,
            ));
        }
        if self.panel_id == self.target_panel_id
            || panel_at_move_target(
                &tab.root,
                &self.panel_id,
                &self.target_panel_id,
                self.direction,
            )
        {
            return Ok(self.authority.unchanged());
        }
        let last_identity = allocate(self.authority.last_identity, 3)?;
        let split_id = WorkbenchSplitId::parse(&identity(last_identity - 2))
            .map_err(|_error| PlanFailure::Internal)?;
        let first_slot_id = WorkbenchPanelSlotId::parse(&identity(last_identity - 1))
            .map_err(|_error| PlanFailure::Internal)?;
        let second_slot_id = WorkbenchPanelSlotId::parse(&identity(last_identity))
            .map_err(|_error| PlanFailure::Internal)?;
        let mut candidate = self.authority.projection.snapshot().clone();
        let revision = self.authority.next_revision()?;
        let tab = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == self.terminal_context_id)
            .and_then(|context| {
                context
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.tab_id == self.tab_id)
            })
            .ok_or(PlanFailure::Internal)?;
        // The source leaf is retained whole: identity, Presentations, and bindings move with it.
        let moved = take_leaf_panel(&mut tab.root, &self.panel_id).ok_or(PlanFailure::Internal)?;
        let insertion = PanelMoveInsertion {
            target_panel_id: self.target_panel_id.clone(),
            direction: self.direction,
            moved,
            split_id,
            first_slot_id,
            second_slot_id,
        };
        if !insert_leaf_panel(&mut tab.root, &insertion) {
            return Err(PlanFailure::Internal);
        }
        candidate.revision = revision;
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
            .map_err(|_error| PlanFailure::Internal)?;
        Ok(CommitPlan {
            projection: Some(projection),
            last_identity: Some(last_identity),
            catalog_next: None,
            outcome: WorkbenchCommandOutcome::PanelMoved {
                revision,
                terminal_context_id: self.terminal_context_id.clone(),
                tab_id: self.tab_id.clone(),
                panel_id: self.panel_id.clone(),
                target_panel_id: self.target_panel_id.clone(),
            },
        })
    }
}

impl WorkbenchAuthority {
    pub fn begin_terminal_panel_move(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchPanelMoveBegin<'_> {
        let authorized = matches!(&command.action, WorkbenchAction::PanelMove { .. })
            && grant
                .is_some_and(|grant| grant.site_id == *self.site_id() && grant.mutate_navigation);
        if !authorized {
            return WorkbenchPanelMoveBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(result) = self.command_preflight(&command) {
            return WorkbenchPanelMoveBegin::Ready(result);
        }
        let WorkbenchAction::PanelMove {
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
            direction,
        } = command.action.clone()
        else {
            return WorkbenchPanelMoveBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            ));
        };
        let snapshot = self.projection.snapshot();
        let rejection = if command.site_id != snapshot.site_id {
            Some(WorkbenchRejectionReason::SiteMismatch)
        } else if command.expected_revision != snapshot.revision {
            Some(WorkbenchRejectionReason::RevisionMismatch)
        } else if terminal_context_id != snapshot.navigation.standalone_terminal_context_id
            && !snapshot.navigation.projects.iter().any(|project| {
                project
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.terminal_context_id == terminal_context_id)
            })
        {
            Some(WorkbenchRejectionReason::TerminalContextUnavailable)
        } else {
            None
        };
        if let Some(reason) = rejection {
            return WorkbenchPanelMoveBegin::Ready(self.retain_rejection(command, reason));
        }
        // Tree membership admits Split-bearing contexts; the standalone composition check
        // compares it against Site composition only for the standalone context.
        let membership = if terminal_context_id
            == snapshot.navigation.standalone_terminal_context_id
        {
            match self.tree_standalone_membership() {
                Ok(membership) => Some(membership),
                Err(_invariant) => return WorkbenchPanelMoveBegin::Ready(self.retire(&command)),
            }
        } else {
            if super::terminal_membership::tree_terminal_membership(snapshot, &terminal_context_id)
                .is_err()
            {
                return WorkbenchPanelMoveBegin::Ready(self.retire(&command));
            }
            None
        };
        WorkbenchPanelMoveBegin::Admitted(Box::new(WorkbenchPanelMoveAdmission {
            authority: self,
            command,
            terminal_context_id,
            membership,
            tab_id,
            panel_id,
            target_panel_id,
            direction,
        }))
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

fn leaf_panel_id(node: &PanelNode) -> Option<&WorkbenchPanelId> {
    match node {
        PanelNode::Panel {
            child_presentations,
            panel_id,
            ..
        } if child_presentations.is_empty() => Some(panel_id),
        _ => None,
    }
}

fn contains_leaf_panel(node: &PanelNode, panel_id: &WorkbenchPanelId) -> bool {
    match node {
        PanelNode::Panel { .. } => leaf_panel_id(node) == Some(panel_id),
        PanelNode::Split { children, .. } => children
            .iter()
            .any(|child| contains_leaf_panel(&child.node, panel_id)),
    }
}

/// The move is already satisfied when the source and target leaves are the two direct children
/// of a matching-orientation Split in the direction-implied order.
fn panel_at_move_target(
    node: &PanelNode,
    panel_id: &WorkbenchPanelId,
    target_panel_id: &WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
) -> bool {
    let PanelNode::Split {
        children,
        orientation,
        ..
    } = node
    else {
        return false;
    };
    if *orientation == move_orientation(direction) {
        let (leading, trailing) = if move_inserts_before(direction) {
            (panel_id, target_panel_id)
        } else {
            (target_panel_id, panel_id)
        };
        if leaf_panel_id(&children[0].node) == Some(leading)
            && leaf_panel_id(&children[1].node) == Some(trailing)
        {
            return true;
        }
    }
    children
        .iter()
        .any(|child| panel_at_move_target(&child.node, panel_id, target_panel_id, direction))
}

/// Detaches the leaf Panel `panel_id`, replacing its parent Split with the sibling subtree, and
/// returns the retained node. The caller guarantees the leaf is not the Tab root.
fn take_leaf_panel(node: &mut PanelNode, panel_id: &WorkbenchPanelId) -> Option<PanelNode> {
    let PanelNode::Split { children, .. } = node else {
        return None;
    };
    for index in 0..2 {
        if leaf_panel_id(&children[index].node) == Some(panel_id) {
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

struct PanelMoveInsertion {
    target_panel_id: WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
    moved: PanelNode,
    split_id: WorkbenchSplitId,
    first_slot_id: WorkbenchPanelSlotId,
    second_slot_id: WorkbenchPanelSlotId,
}

/// Replaces the target leaf with a fresh Split holding the retained source leaf and the target
/// leaf in the direction-implied order.
fn insert_leaf_panel(node: &mut PanelNode, insertion: &PanelMoveInsertion) -> bool {
    match node {
        PanelNode::Panel { .. } if leaf_panel_id(node) == Some(&insertion.target_panel_id) => {
            let target = node.clone();
            let (first_node, second_node) = if move_inserts_before(insertion.direction) {
                (insertion.moved.clone(), target)
            } else {
                (target, insertion.moved.clone())
            };
            *node = PanelNode::Split {
                children: Box::new([
                    PanelSlot {
                        node: first_node,
                        slot_id: insertion.first_slot_id.clone(),
                    },
                    PanelSlot {
                        node: second_node,
                        slot_id: insertion.second_slot_id.clone(),
                    },
                ]),
                orientation: move_orientation(insertion.direction),
                split_id: insertion.split_id.clone(),
            };
            true
        }
        PanelNode::Panel { .. } => false,
        PanelNode::Split { children, .. } => children
            .iter_mut()
            .any(|child| insert_leaf_panel(&mut child.node, insertion)),
    }
}

#[cfg(test)]
mod tests;
