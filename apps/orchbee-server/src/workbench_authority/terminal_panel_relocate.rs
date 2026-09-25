use super::{
    CommitPlan, PlanFailure, PreparedCommandCommit, RetainedRequest, WorkbenchAuthority,
    WorkbenchGrant, WorkbenchTerminalMembership, allocate, identity, result, unavailable,
};
use orchbee_workbench_protocol::{
    ContractError, MAXIMUM_PANEL_NODE_DEPTH, MAXIMUM_TABS_PER_CONTEXT, PanelNode, PanelOrientation,
    PanelSlot, Tab, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchPanelId,
    WorkbenchPanelRelocateDestination, WorkbenchPanelSlotId, WorkbenchPanelSplitDirection,
    WorkbenchProtocolDocument, WorkbenchRejectionReason, WorkbenchSplitId, WorkbenchTabId,
    WorkbenchTerminalContextId, serialize_document,
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
    membership: WorkbenchTerminalMembership,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    destination: WorkbenchPanelRelocateDestination,
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelRelocatePreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchPanelRelocate<'a>>),
}

pub struct PreparedWorkbenchPanelRelocate<'a>(PreparedCommandCommit<'a>);

impl PreparedWorkbenchPanelRelocate<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchCommandResult {
        &self.0.response
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        self.0.commit()
    }
}

impl<'a> WorkbenchPanelRelocateAdmission<'a> {
    /// The standalone composition input — present only when the source
    /// context is the standalone context.
    #[must_use]
    pub fn standalone_membership(&self) -> Option<&WorkbenchTerminalMembership> {
        self.membership.is_standalone().then_some(&self.membership)
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// Site validates standalone bootstrap composition before the plan is
    /// resolved; relocation keeps the context's binding multiset identical,
    /// so the pre-checked composition stays valid through the commit.
    #[must_use]
    pub fn finish(self) -> WorkbenchPanelRelocatePreparation<'a> {
        let plan = match self.plan() {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => CommitPlan {
                projection: None,
                last_identity: None,
                catalog_next: None,
                outcome: self.authority.rejected(reason),
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
            plan.outcome,
            WorkbenchCommandOutcome::PanelRelocated { .. }
                | WorkbenchCommandOutcome::PanelRelocatedToNewTab { .. }
        ) && self.lifecycle_fenced()
        {
            return WorkbenchPanelRelocatePreparation::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        match self.authority.prepare_command_commit(self.command, plan) {
            Ok(prepared) => WorkbenchPanelRelocatePreparation::Prepared(Box::new(
                PreparedWorkbenchPanelRelocate(prepared),
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
        let target_tab = match &self.destination {
            WorkbenchPanelRelocateDestination::Tab { tab_id, .. } => Some(tab_id.clone()),
            WorkbenchPanelRelocateDestination::NewTab { .. } => None,
        };
        self.authority
            .aggregate_close_fences_tab(&self.terminal_context_id, &self.tab_id)
            || target_tab.is_some_and(|tab_id| {
                self.authority
                    .aggregate_close_fences_tab(&self.terminal_context_id, &tab_id)
            })
            || self
                .membership
                .member_for_panel(&self.tab_id, &self.panel_id)
                .is_some_and(|member| self.authority.open_close_covers_member(member))
            || self
                .authority
                .pending_terminal_covers_panel(&self.terminal_context_id, &self.panel_id)
    }

    fn plan(&self) -> Result<CommitPlan, PlanFailure> {
        let snapshot = self.authority.projection.snapshot();
        let context = snapshot
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
        // Leaf-ness is distinguished from absence: a stacked Panel answers
        // `panel-not-leaf`, never `panel-unavailable`.
        match find_panel(&tab.root, &self.panel_id) {
            None => {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::PanelUnavailable,
                ));
            }
            Some(PanelNode::Panel {
                child_presentations,
                ..
            }) if !child_presentations.is_empty() => {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::PanelNotLeaf,
                ));
            }
            Some(PanelNode::Panel { .. }) => {}
            Some(PanelNode::Split { .. }) => return Err(PlanFailure::Internal),
        }
        // The destination Tab resolves inside the source context only;
        // naming the source Tab is a typed refusal, not a degenerate move.
        let target = match &self.destination {
            WorkbenchPanelRelocateDestination::Tab { tab_id, direction } => {
                if tab_id == &self.tab_id {
                    return Err(PlanFailure::Rejected(WorkbenchRejectionReason::SameTab));
                }
                let tab = context
                    .tabs
                    .iter()
                    .find(|tab| &tab.tab_id == tab_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::TabUnavailable,
                    ))?;
                // Wrapping the destination root in a fresh Split deepens its
                // tree by one; the node count is unchanged overall (the
                // collapsed source Split balances the fresh root Split).
                if tree_depth(&tab.root) + 1 > MAXIMUM_PANEL_NODE_DEPTH {
                    return Err(PlanFailure::Rejected(
                        WorkbenchRejectionReason::ResourceLimit,
                    ));
                }
                RelocateTarget::Existing {
                    direction: *direction,
                }
            }
            WorkbenchPanelRelocateDestination::NewTab { label } => {
                if context.tabs.len() + 1 > MAXIMUM_TABS_PER_CONTEXT {
                    return Err(PlanFailure::Rejected(
                        WorkbenchRejectionReason::ResourceLimit,
                    ));
                }
                RelocateTarget::NewTab {
                    label: label.as_str().to_owned(),
                }
            }
        };
        if leaf_count(&tab.root) <= 1 {
            return Err(PlanFailure::Rejected(
                WorkbenchRejectionReason::LastPanelInTab,
            ));
        }
        // Fresh identities only for structurally new entities: a root Split
        // plus two Slots for an existing-Tab insert, or just the Tab identity
        // for a new Tab — the moved leaf keeps its Panel/Presentation.
        let identity_count = match &target {
            RelocateTarget::Existing { .. } => 3,
            RelocateTarget::NewTab { .. } => 1,
        };
        let last_identity = allocate(self.authority.last_identity, identity_count)?;
        let minted = match &target {
            RelocateTarget::Existing { .. } => Minted::Split {
                split_id: WorkbenchSplitId::parse(&identity(last_identity - 2))
                    .map_err(|_error| PlanFailure::Internal)?,
                first_slot_id: WorkbenchPanelSlotId::parse(&identity(last_identity - 1))
                    .map_err(|_error| PlanFailure::Internal)?,
                second_slot_id: WorkbenchPanelSlotId::parse(&identity(last_identity))
                    .map_err(|_error| PlanFailure::Internal)?,
            },
            RelocateTarget::NewTab { .. } => Minted::Tab {
                tab_id: WorkbenchTabId::parse(&identity(last_identity))
                    .map_err(|_error| PlanFailure::Internal)?,
            },
        };
        let mut candidate = snapshot.clone();
        let revision = self.authority.next_revision()?;
        let context = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == self.terminal_context_id)
            .ok_or(PlanFailure::Internal)?;
        let tab = context
            .tabs
            .iter_mut()
            .find(|tab| tab.tab_id == self.tab_id)
            .ok_or(PlanFailure::Internal)?;
        // The source leaf is retained whole: identity, Presentation, and
        // binding move with it; nothing is launched, stopped, or rebound.
        let moved = take_leaf_panel(&mut tab.root, &self.panel_id).ok_or(PlanFailure::Internal)?;
        let outcome = match (&target, &minted) {
            (
                RelocateTarget::Existing { direction },
                Minted::Split {
                    split_id,
                    first_slot_id,
                    second_slot_id,
                },
            ) => {
                let target_tab_id = match &self.destination {
                    WorkbenchPanelRelocateDestination::Tab { tab_id, .. } => tab_id.clone(),
                    WorkbenchPanelRelocateDestination::NewTab { .. } => {
                        return Err(PlanFailure::Internal);
                    }
                };
                let target_tab = context
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.tab_id == target_tab_id)
                    .ok_or(PlanFailure::Internal)?;
                insert_at_root(&mut target_tab.root, moved, *direction, &minted);
                WorkbenchCommandOutcome::PanelRelocated {
                    revision,
                    terminal_context_id: self.terminal_context_id.clone(),
                    tab_id: self.tab_id.clone(),
                    panel_id: self.panel_id.clone(),
                    target_tab_id,
                    split_id: split_id.clone(),
                    first_slot_id: first_slot_id.clone(),
                    second_slot_id: second_slot_id.clone(),
                }
            }
            (RelocateTarget::NewTab { label }, Minted::Tab { tab_id }) => {
                context.tabs.push(Tab {
                    automatic_label: false,
                    tab_id: tab_id.clone(),
                    label: label.clone(),
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
                }
            }
            _ => return Err(PlanFailure::Internal),
        };
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
            outcome,
        })
    }
}

impl WorkbenchAuthority {
    pub fn begin_terminal_panel_relocate(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchPanelRelocateBegin<'_> {
        let authorized = matches!(&command.action, WorkbenchAction::PanelRelocate { .. })
            && grant
                .is_some_and(|grant| grant.site_id == *self.site_id() && grant.mutate_navigation);
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
            Some(PanelNode::Panel {
                child_presentations,
                ..
            }) if !child_presentations.is_empty() => Some(WorkbenchRejectionReason::PanelNotLeaf),
            Some(PanelNode::Panel { .. }) => None,
            Some(PanelNode::Split { .. }) => {
                return WorkbenchPanelRelocateBegin::Ready(self.retire(&command));
            }
        };
        if let Some(reason) = source_leaf {
            return WorkbenchPanelRelocateBegin::Ready(self.retain_rejection(command, reason));
        }
        // Tree membership admits Split-bearing contexts; a context carrying
        // stacked Panels, groups, or pinned Tabs is outside the supported
        // shape and retires the authority exactly like `panel move`.
        let membership = match super::terminal_membership::tree_terminal_membership(
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
            membership,
            tab_id,
            panel_id,
            destination,
        }))
    }

    /// Whether an unsettled terminal-create settlement minted this Panel —
    /// the pending Tab-create mints `panel_id`, the pending Panel-split mints
    /// `new_panel_id`; relocating either while its terminal outcome is
    /// outstanding races the pending record, so the move fences transiently.
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
            RetainedRequest::PendingPanelSplit(entry) => {
                entry.metadata().terminal_context_id() == terminal_context_id
                    && entry.metadata().new_panel_id() == panel_id
            }
            _ => false,
        })
    }
}

enum RelocateTarget {
    Existing {
        direction: WorkbenchPanelSplitDirection,
    },
    NewTab {
        label: String,
    },
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

fn leaf_count(node: &PanelNode) -> usize {
    match node {
        PanelNode::Panel { .. } => 1,
        PanelNode::Split { children, .. } => {
            children.iter().map(|child| leaf_count(&child.node)).sum()
        }
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
/// leaf is not the Tab root (the last-leaf refusal precedes this call).
fn take_leaf_panel(node: &mut PanelNode, panel_id: &WorkbenchPanelId) -> Option<PanelNode> {
    let PanelNode::Split { children, .. } = node else {
        return None;
    };
    for index in 0..2 {
        if matches!(
            &children[index].node,
            PanelNode::Panel {
                child_presentations,
                panel_id: candidate,
                ..
            } if child_presentations.is_empty() && candidate == panel_id
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

#[cfg(test)]
mod tests;
