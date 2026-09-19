use super::{
    AttachTarget, AttachmentKey, CarrierFailure, MembershipCleanup, NativeSessionState, PaneKey,
    PreparedMembershipCandidate, SessionNotice, SessionPhase,
};
use crate::terminal_session::workbench_projection::{
    MovedTabExpectation, NativeProjectionDivergence, validate_canonical_refresh, validate_moved_tab,
};
use crate::{
    local_client::workbench::{WorkbenchTabMoveCommit, WorkbenchTabMovePrecommit},
    terminal_session::workbench_projection::NativeContextTabs,
};
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTabMoveDirection,
};
use std::time::Instant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct MoveOperation {
    request_id: WorkbenchRequestId,
    target: AttachmentKey,
    baseline: NativeContextTabs,
    direction: WorkbenchTabMoveDirection,
    deadline: Instant,
}

impl MoveOperation {
    pub(in crate::terminal_session) const fn new(
        request_id: WorkbenchRequestId,
        target: AttachmentKey,
        baseline: NativeContextTabs,
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

    pub(in crate::terminal_session) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }
    pub(in crate::terminal_session) const fn target(&self) -> &AttachmentKey {
        &self.target
    }
    pub(in crate::terminal_session) const fn baseline(&self) -> &NativeContextTabs {
        &self.baseline
    }
    pub(in crate::terminal_session) const fn direction(&self) -> WorkbenchTabMoveDirection {
        self.direction
    }
    pub(in crate::terminal_session) const fn deadline(&self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct PendingMoveCommit {
    operation: MoveOperation,
    commit: WorkbenchTabMoveCommit,
}

impl PendingMoveCommit {
    pub(in crate::terminal_session) const fn new(
        operation: MoveOperation,
        commit: WorkbenchTabMoveCommit,
    ) -> Self {
        Self { operation, commit }
    }
    pub(in crate::terminal_session) const fn operation(&self) -> &MoveOperation {
        &self.operation
    }
    pub(in crate::terminal_session) const fn commit(&self) -> &WorkbenchTabMoveCommit {
        &self.commit
    }

    fn valid(&self) -> bool {
        let commit = self.commit();
        let Some(index) = self.operation.baseline.tab_index(&commit.tab_id) else {
            return false;
        };
        let destination = match self.operation.direction {
            WorkbenchTabMoveDirection::Previous => index.saturating_sub(1),
            WorkbenchTabMoveDirection::Next => {
                (index + 1).min(self.operation.baseline.tabs().len() - 1)
            }
            WorkbenchTabMoveDirection::First => 0,
            WorkbenchTabMoveDirection::Last => self.operation.baseline.tabs().len() - 1,
        };
        index != destination
            && self.operation.baseline.revision().value().checked_add(1)
                == Some(self.commit.revision.value())
            && self.operation.baseline.terminal_context_id() == &self.commit.terminal_context_id
            && self.operation.target.tab_id() == &self.commit.tab_id
            && self
                .operation
                .baseline
                .tab_index(&self.commit.tab_id)
                .is_some()
    }

    pub(super) fn validate_projection(
        &self,
        candidate: NativeContextTabs,
    ) -> Result<NativeContextTabs, NativeProjectionDivergence> {
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
pub(in crate::terminal_session) enum MoveResult {
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
pub(in crate::terminal_session) enum MoveDecision {
    Continue,
    Cleanup(MembershipCleanup),
    AttachRecovered(AttachTarget),
    SettledWithoutAttachment,
    Retire,
}

pub(in crate::terminal_session) struct PreparedMove {
    operation: MoveOperation,
    result: PreparedMoveResult,
}

impl PreparedMove {
    pub(in crate::terminal_session) fn membership_candidate(
        &self,
    ) -> Option<&PreparedMembershipCandidate> {
        match &self.result {
            PreparedMoveResult::Membership { prepared, .. } => Some(prepared),
            _ => None,
        }
    }
}

enum PreparedMoveResult {
    NotMoved,
    Unchanged,
    Committed {
        pending: PendingMoveCommit,
        candidate: Option<NativeContextTabs>,
    },
    RefusedProjection(NativeContextTabs),
    Membership {
        pending: Option<PendingMoveCommit>,
        prepared: Box<PreparedMembershipCandidate>,
    },
    Divergent {
        pending: Option<PendingMoveCommit>,
    },
}

impl NativeSessionState {
    #[cfg(test)]
    pub(in crate::terminal_session) fn retained_move_for_test(&self) -> Option<&PendingMoveCommit> {
        self.settled_move.as_ref()
    }

    pub(in crate::terminal_session) fn move_blocks_structure(&self) -> bool {
        self.pending_move().is_some() || self.move_recovery.is_some()
    }

    pub(in crate::terminal_session) fn plan_move(
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
                SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
        {
            return None;
        }
        let index = self.active()?.index();
        let direction = match direction {
            WorkbenchTabMoveDirection::Previous if index == 0 => WorkbenchTabMoveDirection::Last,
            WorkbenchTabMoveDirection::Next if index + 1 == self.cache.tabs().len() => {
                WorkbenchTabMoveDirection::First
            }
            other => other,
        };
        Some(MoveOperation::new(
            request_id,
            self.desired.as_ref()?.anchor().clone(),
            self.cache.clone(),
            direction,
            deadline,
        ))
    }

    pub(in crate::terminal_session) fn begin_move(&mut self, operation: MoveOperation) -> bool {
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

    pub(in crate::terminal_session) fn prepare_move(
        &self,
        operation: &MoveOperation,
        result: MoveResult,
    ) -> PreparedMove {
        let result = match result {
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
        candidate: NativeContextTabs,
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

    pub(in crate::terminal_session) fn apply_prepared_move(
        &mut self,
        prepared: PreparedMove,
    ) -> MoveDecision {
        if !self.move_result_is_current(&prepared.operation) {
            self.retire(CarrierFailure::Protocol);
            return MoveDecision::Retire;
        }
        match prepared.result {
            PreparedMoveResult::NotMoved => self.notice = Some(SessionNotice::MoveNotMoved),
            PreparedMoveResult::Unchanged => self.notice = None,
            PreparedMoveResult::RefusedProjection(candidate) => {
                if !self.merge_projection(candidate) {
                    self.retire(CarrierFailure::AuthorityDivergence);
                    return MoveDecision::Retire;
                }
                self.notice = Some(SessionNotice::MoveNotMoved);
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
                SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
    }

    /// The exact known command outcome survives every later cleanup/install preparation failure.
    pub(in crate::terminal_session) fn retain_prepared_move_membership(
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
            Some(SessionNotice::MoveNotMoved)
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
                    || self.notice == Some(SessionNotice::MoveNotMoved))
        })
    }

    pub(in crate::terminal_session) fn retain_pending_move(
        &mut self,
        pending: PendingMoveCommit,
    ) -> bool {
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
        if matches!(&self.phase, SessionPhase::Retired(_)) {
            return true;
        }
        if self.move_blocks_structure()
            && !matches!(
                &self.phase,
                SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
            && !(matches!(self.phase, SessionPhase::ReconcilingMembership(_))
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
