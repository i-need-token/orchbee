//! Owns a Client Tab-move operation's captured identity, known commit, canonical projection
//! validation, and membership reconciliation. It does not reorder Server state locally or
//! replay an operation whose delivery is unknown.

use super::{
    AttachTarget, AttachmentKey, CarrierFailure, ClientNotice, ClientPhase, MembershipCleanup,
    PaneKey, PreparedMembershipCandidate, WorkbenchClientState,
};
use crate::exchange_values::workbench::{WorkbenchTabMoveCommit, WorkbenchTabMovePrecommit};
use crate::workbench_projection::{ClientContextTabs, TabMovePlacement};
use crate::workbench_projection::{
    ClientProjectionDivergence, MovedTabExpectation, validate_canonical_refresh, validate_moved_tab,
};
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTabMoveDirection,
};
use std::time::Instant;

/// Previous from the first of `count` Tabs wraps to the last place, and Next from the last wraps
/// to the first, as the GUI's move backward and forward do. The keyboard move and the menu's Move
/// Tab rows both apply it over the Tabs pinned as the moving one.
pub(crate) const fn wrapped_tab_move(
    direction: WorkbenchTabMoveDirection,
    index: usize,
    count: usize,
) -> WorkbenchTabMoveDirection {
    match direction {
        WorkbenchTabMoveDirection::Previous if index == 0 => WorkbenchTabMoveDirection::Last,
        WorkbenchTabMoveDirection::Next if index + 1 >= count => WorkbenchTabMoveDirection::First,
        other => other,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MoveOperation {
    request_id: WorkbenchRequestId,
    target: AttachmentKey,
    baseline: ClientContextTabs,
    direction: WorkbenchTabMoveDirection,
    deadline: Instant,
}

impl MoveOperation {
    pub(crate) const fn new(
        request_id: WorkbenchRequestId,
        target: AttachmentKey,
        baseline: ClientContextTabs,
        direction: WorkbenchTabMoveDirection,
        deadline: Instant,
    ) -> Self {
        Self {
            request_id,
            target,
            baseline,
            direction,
            deadline,
        }
    }

    pub(crate) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }
    pub(crate) const fn target(&self) -> &AttachmentKey {
        &self.target
    }
    pub(crate) const fn baseline(&self) -> &ClientContextTabs {
        &self.baseline
    }
    #[cfg(test)]
    pub(crate) const fn direction(&self) -> WorkbenchTabMoveDirection {
        self.direction
    }
    /// Where the move puts its Tab within the Tabs pinned as it is, or `None` once the baseline
    /// no longer holds it.
    pub(crate) fn placement(&self) -> Option<TabMovePlacement> {
        let index = self.baseline.tab_index(self.target.tab_id())?;
        TabMovePlacement::within_pinned(self.baseline.tabs(), index, self.direction)
    }
    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingMoveCommit {
    operation: MoveOperation,
    commit: WorkbenchTabMoveCommit,
}

impl PendingMoveCommit {
    pub(crate) const fn new(operation: MoveOperation, commit: WorkbenchTabMoveCommit) -> Self {
        Self { operation, commit }
    }
    pub(crate) const fn operation(&self) -> &MoveOperation {
        &self.operation
    }
    pub(crate) const fn commit(&self) -> &WorkbenchTabMoveCommit {
        &self.commit
    }

    fn valid(&self) -> bool {
        let commit = self.commit();
        let Some(index) = self.operation.baseline.tab_index(&commit.tab_id) else {
            return false;
        };
        let tabs = self.operation.baseline.tabs();
        let destination = TabMovePlacement::within_pinned(tabs, index, self.operation.direction)
            .and_then(|placement| placement.destination(index, tabs.len()));
        destination.is_some_and(|destination| destination != index)
            && self.operation.baseline.revision().value().checked_add(1)
                == Some(self.commit.revision.value())
            && self.operation.baseline.terminal_context_id()
                == Some(&self.commit.terminal_context_id)
            && self.operation.target.tab_id() == &self.commit.tab_id
            && self
                .operation
                .baseline
                .tab_index(&self.commit.tab_id)
                .is_some()
    }

    pub(super) fn validate_projection(
        &self,
        candidate: ClientContextTabs,
    ) -> Result<ClientContextTabs, ClientProjectionDivergence> {
        validate_moved_tab(
            &self.operation.baseline,
            &MovedTabExpectation::new(
                self.commit.revision,
                self.commit.terminal_context_id.clone(),
                self.commit.tab_id.clone(),
                self.operation.direction,
            ),
            candidate,
        )
        .map(|validated| validated.candidate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MoveResult {
    NotSubmitted,
    Precommit(WorkbenchTabMovePrecommit),
    TabUnavailable {
        revision: WorkbenchRevision,
        outcome: WorkbenchSnapshotOutcome,
    },
    Unchanged(WorkbenchRevision),
    Committed {
        commit: WorkbenchTabMoveCommit,
        outcome: WorkbenchSnapshotOutcome,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MoveDecision {
    Continue,
    Cleanup(MembershipCleanup),
    AttachRecovered(AttachTarget),
    SettledWithoutAttachment,
    Retire,
}

pub(crate) struct PreparedMove {
    operation: MoveOperation,
    result: PreparedMoveResult,
}

impl PreparedMove {
    pub(crate) fn membership_candidate(&self) -> Option<&PreparedMembershipCandidate> {
        match &self.result {
            PreparedMoveResult::Membership { prepared, .. } => Some(prepared),
            _ => None,
        }
    }
}

enum PreparedMoveResult {
    NotMoved,
    OrderConflict,
    Unchanged,
    Committed {
        pending: PendingMoveCommit,
        candidate: Option<ClientContextTabs>,
    },
    RefusedProjection(ClientContextTabs),
    Membership {
        pending: Option<PendingMoveCommit>,
        prepared: Box<PreparedMembershipCandidate>,
    },
    Divergent {
        pending: Option<PendingMoveCommit>,
    },
}

impl WorkbenchClientState {
    #[cfg(test)]
    pub(crate) fn retained_move_for_test(&self) -> Option<&PendingMoveCommit> {
        self.settled_move.as_ref()
    }

    pub(crate) fn move_blocks_structure(&self) -> bool {
        self.pending_move().is_some() || self.move_recovery.is_some()
    }

    pub(crate) fn plan_move(
        &self,
        request_id: WorkbenchRequestId,
        direction: WorkbenchTabMoveDirection,
        deadline: Instant,
    ) -> Option<MoveOperation> {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.pending_refresh.is_some()
            || !self.invariant_holds()
            || !matches!(
                &self.phase,
                ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
        {
            return None;
        }
        // Pinned Tabs precede unpinned ones; the move wraps among those pinned as the active one.
        let index = self.active()?.index();
        let tabs = self.cache.tabs();
        let boundary = tabs.iter().take_while(|tab| tab.pinned()).count();
        let (position, count) = if tabs.get(index)?.pinned() {
            (index, boundary)
        } else {
            (index.checked_sub(boundary)?, tabs.len() - boundary)
        };
        let direction = wrapped_tab_move(direction, position, count);
        Some(MoveOperation::new(
            request_id,
            self.desired.as_ref()?.anchor().clone(),
            self.cache.clone(),
            direction,
            deadline,
        ))
    }

    pub(crate) fn begin_move(&mut self, operation: MoveOperation) -> bool {
        let Some(expected) = self.plan_move(
            operation.request_id.clone(),
            operation.direction,
            operation.deadline,
        ) else {
            return false;
        };
        if expected != operation {
            return false;
        }
        self.pending_mutation = Some(super::PendingMutation::Move(operation));
        self.notice = None;
        true
    }

    pub(crate) fn prepare_move(
        &self,
        operation: &MoveOperation,
        result: MoveResult,
    ) -> PreparedMove {
        let result = match result {
            MoveResult::Precommit(WorkbenchTabMovePrecommit::Rejected {
                reason: orchbee_workbench_protocol::WorkbenchRejectionReason::TabMoveOrderConflict,
                ..
            }) => PreparedMoveResult::OrderConflict,
            MoveResult::NotSubmitted | MoveResult::Precommit(_) => PreparedMoveResult::NotMoved,
            MoveResult::TabUnavailable { revision, outcome } => {
                if revision.value() < operation.baseline.revision().value() {
                    PreparedMoveResult::Divergent { pending: None }
                } else if let WorkbenchSnapshotOutcome::Available { snapshot } = outcome {
                    match operation
                        .baseline
                        .project_snapshot(&snapshot)
                        .map_err(|_| ())
                        .and_then(|candidate| {
                            if candidate.revision().value() < revision.value()
                                || (candidate.revision() == revision
                                    && candidate.tab_index(operation.target.tab_id()).is_some())
                            {
                                return Err(());
                            }
                            validate_canonical_refresh(&operation.baseline, candidate)
                                .map_err(|_| ())
                        }) {
                        Ok(candidate) => self.prepare_move_candidate(operation, None, candidate),
                        Err(()) => PreparedMoveResult::Divergent { pending: None },
                    }
                } else {
                    PreparedMoveResult::NotMoved
                }
            }
            MoveResult::Unchanged(revision) if revision == operation.baseline.revision() => {
                PreparedMoveResult::Unchanged
            }
            MoveResult::Unchanged(_) => PreparedMoveResult::Divergent { pending: None },
            MoveResult::Committed { commit, outcome } => {
                let pending = PendingMoveCommit::new(operation.clone(), commit);
                if !pending.valid() {
                    PreparedMoveResult::Divergent { pending: None }
                } else if let WorkbenchSnapshotOutcome::Available { snapshot } = outcome {
                    match operation
                        .baseline
                        .project_snapshot(&snapshot)
                        .map_err(|_| ())
                        .and_then(|candidate| {
                            pending.validate_projection(candidate).map_err(|_| ())
                        }) {
                        Ok(candidate) => {
                            self.prepare_move_candidate(operation, Some(pending), candidate)
                        }
                        Err(()) => PreparedMoveResult::Divergent {
                            pending: Some(pending),
                        },
                    }
                } else {
                    PreparedMoveResult::Committed {
                        pending,
                        candidate: None,
                    }
                }
            }
        };
        PreparedMove {
            operation: operation.clone(),
            result,
        }
    }

    fn prepare_move_candidate(
        &self,
        operation: &MoveOperation,
        pending: Option<PendingMoveCommit>,
        candidate: ClientContextTabs,
    ) -> PreparedMoveResult {
        let mut leaf_repair = None;
        if candidate.tab_index(operation.target.tab_id()).is_some() {
            // A superseding candidate may have folded in a peer panel.close that removed the
            // selected leaf while the Tab survives: repair the selection through the paired
            // membership path instead of a direct merge that cannot find it.
            leaf_repair = self
                .active()
                .and_then(|active| self.selected_leaf_repair_index(&candidate, active));
            if leaf_repair.is_none() {
                return match pending {
                    Some(pending) => PreparedMoveResult::Committed {
                        pending,
                        candidate: Some(candidate),
                    },
                    None => PreparedMoveResult::RefusedProjection(candidate),
                };
            }
        }
        match self.prepare_membership_candidate(
            candidate,
            Some(operation.target.tab_id()),
            operation.baseline.tab_index(operation.target.tab_id()),
            None,
            None,
        ) {
            Ok(mut prepared) => {
                if let Some(panel_index) = leaf_repair {
                    prepared.retarget_selected_panel(panel_index);
                }
                PreparedMoveResult::Membership {
                    pending,
                    prepared: Box::new(prepared),
                }
            }
            Err(_) => PreparedMoveResult::Divergent { pending },
        }
    }

    pub(crate) fn apply_prepared_move(&mut self, prepared: PreparedMove) -> MoveDecision {
        if !self.move_result_is_current(&prepared.operation) {
            self.retire(CarrierFailure::Protocol);
            return MoveDecision::Retire;
        }
        match prepared.result {
            PreparedMoveResult::NotMoved => self.notice = Some(ClientNotice::MoveNotMoved),
            PreparedMoveResult::OrderConflict => {
                self.notice = Some(ClientNotice::MoveOrderConflict)
            }
            PreparedMoveResult::Unchanged => self.notice = None,
            PreparedMoveResult::RefusedProjection(candidate) => {
                if !self.merge_projection(candidate) {
                    self.retire(CarrierFailure::AuthorityDivergence);
                    return MoveDecision::Retire;
                }
                self.notice = Some(ClientNotice::MoveNotMoved);
            }
            PreparedMoveResult::Membership { pending, .. } => {
                // Membership must use the common paired installation/cleanup consumer.
                if let Some(pending) = pending {
                    self.settled_move = Some(pending);
                }
                self.retire(CarrierFailure::Protocol);
                return MoveDecision::Retire;
            }
            PreparedMoveResult::Committed { pending, candidate } => {
                self.settled_move = Some(pending.clone());
                if let Some(candidate) = candidate {
                    if !self.merge_projection(candidate) {
                        self.retire(CarrierFailure::AuthorityDivergence);
                        return MoveDecision::Retire;
                    }
                } else {
                    self.move_recovery = Some(pending);
                }
                self.notice = None;
            }
            PreparedMoveResult::Divergent { pending } => {
                if let Some(pending) = pending {
                    self.settled_move = Some(pending);
                }
                self.retire(CarrierFailure::AuthorityDivergence);
                return MoveDecision::Retire;
            }
        }
        self.pending_mutation = None;
        MoveDecision::Continue
    }

    fn move_result_is_current(&self, operation: &MoveOperation) -> bool {
        self.pending_move() == Some(operation)
            && self.cache == operation.baseline
            && self.desired.as_ref().map(PaneKey::anchor) == Some(&operation.target)
            && self.invariant_holds()
            && matches!(
                self.phase,
                ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
    }

    /// The exact known command outcome survives every later cleanup/install preparation failure.
    pub(crate) fn retain_prepared_move_membership(
        &mut self,
        prepared: PreparedMove,
    ) -> Result<(MoveOperation, PreparedMembershipCandidate), CarrierFailure> {
        if !self.move_result_is_current(&prepared.operation) {
            return Err(CarrierFailure::Protocol);
        }
        let PreparedMoveResult::Membership {
            pending,
            prepared: candidate,
        } = prepared.result
        else {
            return Err(CarrierFailure::Protocol);
        };
        self.notice = if let Some(pending) = pending {
            self.settled_move = Some(pending);
            None
        } else {
            Some(ClientNotice::MoveNotMoved)
        };
        Ok((prepared.operation, *candidate))
    }

    pub(super) fn move_membership_is_current(&self, request_id: &WorkbenchRequestId) -> bool {
        self.pending_move().is_some_and(|operation| {
            operation.request_id() == request_id
                && operation.baseline() == &self.cache
                && self.desired.as_ref().map(PaneKey::anchor) == Some(operation.target())
                && (self
                    .settled_move
                    .as_ref()
                    .is_some_and(|pending| pending.operation() == operation)
                    || self.notice == Some(ClientNotice::MoveNotMoved))
        })
    }

    pub(crate) fn retain_pending_move(&mut self, pending: PendingMoveCommit) -> bool {
        if !pending.valid() {
            return false;
        }
        if self.settled_move.as_ref() == Some(&pending) {
            return true;
        }
        if self.pending_move() != Some(&pending.operation) {
            return false;
        }
        self.settled_move = Some(pending);
        true
    }

    pub(super) fn move_invariant_holds(&self) -> bool {
        if let Some(recovery) = &self.move_recovery
            && (self.pending_move().is_some()
                || !recovery.valid()
                || self.settled_move.as_ref() != Some(recovery))
        {
            return false;
        }
        if matches!(&self.phase, ClientPhase::Retired(_)) {
            return true;
        }
        if self.move_blocks_structure()
            && !matches!(
                &self.phase,
                ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
            && !(matches!(self.phase, ClientPhase::ReconcilingMembership(_))
                && ((self.move_recovery.is_some() && self.pending_refresh.is_some())
                    || self.pending_move().is_some())
                && self.pending_membership.is_some())
        {
            return false;
        }
        self.pending_move().is_none_or(|operation| {
            Some(&operation.target) == self.desired.as_ref().map(PaneKey::anchor)
                && operation.baseline == self.cache
        }) && self.move_recovery.as_ref().is_none_or(|pending| {
            Some(&pending.operation.target) == self.desired.as_ref().map(PaneKey::anchor)
                && pending.operation.baseline == self.cache
        })
    }
}
