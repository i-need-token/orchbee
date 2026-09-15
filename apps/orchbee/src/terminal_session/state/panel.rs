use super::AttachmentKey;
use super::{
    AttachTarget, CarrierFailure, MembershipCleanup, NativeSessionState,
    PreparedMembershipCandidate, SessionNotice, SessionPhase,
};
use crate::terminal_session::workbench_projection::{
    MovedPanelExpectation, NativeContextTabs, NativeProjectionDivergence, SplitPanelExpectation,
    validate_canonical_refresh, validate_moved_panel, validate_split_panel,
};
use crate::{
    client_view::PanelDirection,
    local_client::workbench::{
        panel_close::{WorkbenchPanelCloseAccepted, WorkbenchPanelClosePrecommit},
        panel_move::{WorkbenchPanelMoveCommit, WorkbenchPanelMovePrecommit},
        panel_split::{WorkbenchPanelSplitCommit, WorkbenchPanelSplitPrecommit},
    },
};
use orchbee_workbench_protocol::{
    WorkbenchCommandUnavailableReason, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchPanelSplitDirection, WorkbenchRejectionReason, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTerminalGeometry,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum PanelRequestKind {
    Split { direction: PanelDirection },
    Close,
    Move { direction: PanelDirection },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum PanelIntent {
    Split {
        direction: WorkbenchPanelSplitDirection,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
    },
    Close,
    Move {
        target_panel_id: WorkbenchPanelId,
        direction: WorkbenchPanelSplitDirection,
    },
}

impl PanelIntent {
    fn matches_commit(&self, commit: &PanelCommit) -> bool {
        match (self, commit) {
            (
                Self::Move {
                    target_panel_id, ..
                },
                PanelCommit::Move(commit),
            ) => *target_panel_id == commit.target_panel_id,
            (Self::Split { .. }, PanelCommit::Split(_)) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct PanelOperation {
    request_id: WorkbenchRequestId,
    target: AttachmentKey,
    panel_id: WorkbenchPanelId,
    baseline: NativeContextTabs,
    kind: PanelRequestKind,
    intent: PanelIntent,
    deadline: Instant,
}

impl PanelOperation {
    pub(in crate::terminal_session) const fn new(
        request_id: WorkbenchRequestId,
        target: AttachmentKey,
        panel_id: WorkbenchPanelId,
        baseline: NativeContextTabs,
        kind: PanelRequestKind,
        intent: PanelIntent,
        deadline: Instant,
    ) -> Self {
        Self {
            request_id,
            target,
            panel_id,
            baseline,
            kind,
            intent,
            deadline,
        }
    }

    pub(in crate::terminal_session) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }
    pub(in crate::terminal_session) const fn target(&self) -> &AttachmentKey {
        &self.target
    }
    pub(in crate::terminal_session) const fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }
    pub(in crate::terminal_session) const fn baseline(&self) -> &NativeContextTabs {
        &self.baseline
    }
    pub(in crate::terminal_session) const fn intent(&self) -> &PanelIntent {
        &self.intent
    }
    pub(in crate::terminal_session) const fn deadline(&self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum PanelCommit {
    Split(WorkbenchPanelSplitCommit),
    Move(WorkbenchPanelMoveCommit),
}

impl PanelCommit {
    fn revision(&self) -> WorkbenchRevision {
        match self {
            Self::Split(WorkbenchPanelSplitCommit::Pending { revision, .. })
            | Self::Split(WorkbenchPanelSplitCommit::Created { revision, .. }) => *revision,
            Self::Move(commit) => commit.revision,
        }
    }

    fn terminal_context_id(&self) -> &orchbee_workbench_protocol::WorkbenchTerminalContextId {
        match self {
            Self::Split(WorkbenchPanelSplitCommit::Pending {
                terminal_context_id,
                ..
            })
            | Self::Split(WorkbenchPanelSplitCommit::Created {
                terminal_context_id,
                ..
            }) => terminal_context_id,
            Self::Move(commit) => &commit.terminal_context_id,
        }
    }

    fn tab_id(&self) -> &orchbee_workbench_protocol::WorkbenchTabId {
        match self {
            Self::Split(WorkbenchPanelSplitCommit::Pending { tab_id, .. })
            | Self::Split(WorkbenchPanelSplitCommit::Created { tab_id, .. }) => tab_id,
            Self::Move(commit) => &commit.tab_id,
        }
    }

    fn panel_id(&self) -> &WorkbenchPanelId {
        match self {
            Self::Split(WorkbenchPanelSplitCommit::Pending { panel_id, .. })
            | Self::Split(WorkbenchPanelSplitCommit::Created { panel_id, .. }) => panel_id,
            Self::Move(commit) => &commit.panel_id,
        }
    }

    fn new_panel_id(&self) -> Option<&WorkbenchPanelId> {
        match self {
            Self::Split(WorkbenchPanelSplitCommit::Pending { new_panel_id, .. })
            | Self::Split(WorkbenchPanelSplitCommit::Created { new_panel_id, .. }) => {
                Some(new_panel_id)
            }
            Self::Move(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct PendingPanelCommit {
    operation: PanelOperation,
    commit: PanelCommit,
}

impl PendingPanelCommit {
    pub(in crate::terminal_session) const fn new(
        operation: PanelOperation,
        commit: PanelCommit,
    ) -> Self {
        Self { operation, commit }
    }

    pub(in crate::terminal_session) const fn operation(&self) -> &PanelOperation {
        &self.operation
    }

    fn valid(&self) -> bool {
        let Some(index) = self.operation.baseline.tab_index(self.commit.tab_id()) else {
            return false;
        };
        let tab = &self.operation.baseline.tabs()[index];
        self.operation.baseline.revision().value().checked_add(1)
            == Some(self.commit.revision().value())
            && self.operation.baseline.terminal_context_id() == self.commit.terminal_context_id()
            && self.operation.target.tab_id() == self.commit.tab_id()
            && &self.operation.panel_id == self.commit.panel_id()
            && tab.panel_index(&self.operation.panel_id).is_some()
            && self.operation.intent.matches_commit(&self.commit)
            && match &self.operation.intent {
                PanelIntent::Move {
                    target_panel_id, ..
                } => tab.panel_index(target_panel_id).is_some(),
                PanelIntent::Split { .. } => self
                    .commit
                    .new_panel_id()
                    .is_some_and(|panel_id| tab.panel_index(panel_id).is_none()),
                PanelIntent::Close => false,
            }
    }

    pub(super) fn validate_projection(
        &self,
        candidate: NativeContextTabs,
    ) -> Result<NativeContextTabs, NativeProjectionDivergence> {
        match (&self.operation.intent, &self.commit) {
            (
                PanelIntent::Split {
                    direction, label, ..
                },
                PanelCommit::Split(commit),
            ) => {
                let (
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                ) = match commit {
                    WorkbenchPanelSplitCommit::Pending {
                        revision,
                        terminal_context_id,
                        tab_id,
                        panel_id,
                        new_panel_id,
                        split_id,
                        first_slot_id,
                        second_slot_id,
                        presentation_id,
                    }
                    | WorkbenchPanelSplitCommit::Created {
                        revision,
                        terminal_context_id,
                        tab_id,
                        panel_id,
                        new_panel_id,
                        split_id,
                        first_slot_id,
                        second_slot_id,
                        presentation_id,
                        ..
                    } => (
                        revision,
                        terminal_context_id,
                        tab_id,
                        panel_id,
                        new_panel_id,
                        split_id,
                        first_slot_id,
                        second_slot_id,
                        presentation_id,
                    ),
                };
                validate_split_panel(
                    &self.operation.baseline,
                    &SplitPanelExpectation {
                        committed_revision: *revision,
                        terminal_context_id: terminal_context_id.clone(),
                        tab_id: tab_id.clone(),
                        panel_id: panel_id.clone(),
                        new_panel_id: new_panel_id.clone(),
                        split_id: split_id.clone(),
                        first_slot_id: first_slot_id.clone(),
                        second_slot_id: second_slot_id.clone(),
                        presentation_id: presentation_id.clone(),
                        direction: *direction,
                        requested_label: label.clone(),
                    },
                    candidate,
                )
            }
            (
                PanelIntent::Move {
                    target_panel_id,
                    direction,
                },
                PanelCommit::Move(commit),
            ) => validate_moved_panel(
                &self.operation.baseline,
                &MovedPanelExpectation::new(
                    commit.revision,
                    commit.terminal_context_id.clone(),
                    commit.tab_id.clone(),
                    commit.panel_id.clone(),
                    target_panel_id.clone(),
                    *direction,
                ),
                candidate,
            ),
            _ => Err(NativeProjectionDivergence),
        }
    }
}

/// One product-facing refusal class shared by the three panel submissions; the
/// operation already knows which adapter produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum PanelPrecommit {
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Denied,
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
}

impl From<WorkbenchPanelSplitPrecommit> for PanelPrecommit {
    fn from(precommit: WorkbenchPanelSplitPrecommit) -> Self {
        match precommit {
            WorkbenchPanelSplitPrecommit::Rejected { revision, reason } => {
                Self::Rejected { revision, reason }
            }
            WorkbenchPanelSplitPrecommit::Denied => Self::Denied,
            WorkbenchPanelSplitPrecommit::Unavailable { reason } => Self::Unavailable { reason },
        }
    }
}

impl From<WorkbenchPanelMovePrecommit> for PanelPrecommit {
    fn from(precommit: WorkbenchPanelMovePrecommit) -> Self {
        match precommit {
            WorkbenchPanelMovePrecommit::Rejected { revision, reason } => {
                Self::Rejected { revision, reason }
            }
            WorkbenchPanelMovePrecommit::Denied => Self::Denied,
            WorkbenchPanelMovePrecommit::Unavailable { reason } => Self::Unavailable { reason },
        }
    }
}

impl From<WorkbenchPanelClosePrecommit> for PanelPrecommit {
    fn from(precommit: WorkbenchPanelClosePrecommit) -> Self {
        match precommit {
            WorkbenchPanelClosePrecommit::Rejected { revision, reason } => {
                Self::Rejected { revision, reason }
            }
            WorkbenchPanelClosePrecommit::Denied => Self::Denied,
            WorkbenchPanelClosePrecommit::Unavailable { reason } => Self::Unavailable { reason },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum PanelResult {
    NotSubmitted,
    Precommit(PanelPrecommit),
    TabUnavailable {
        revision: WorkbenchRevision,
        outcome: WorkbenchSnapshotOutcome,
    },
    Unchanged(WorkbenchRevision),
    CloseAccepted(WorkbenchPanelCloseAccepted),
    Committed {
        commit: Box<PanelCommit>,
        outcome: WorkbenchSnapshotOutcome,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum PanelDecision {
    Continue,
    Cleanup(MembershipCleanup),
    AttachRecovered(AttachTarget),
    SettledWithoutAttachment,
    Retire,
}

pub(in crate::terminal_session) struct PreparedPanel {
    operation: PanelOperation,
    result: PreparedPanelResult,
}

impl PreparedPanel {
    pub(in crate::terminal_session) fn membership_candidate(
        &self,
    ) -> Option<&PreparedMembershipCandidate> {
        match &self.result {
            PreparedPanelResult::Membership { prepared, .. } => Some(prepared),
            _ => None,
        }
    }
}

enum PreparedPanelResult {
    NotApplied,
    Unchanged,
    CloseAccepted,
    Committed {
        pending: PendingPanelCommit,
        candidate: Option<NativeContextTabs>,
    },
    RefusedProjection(NativeContextTabs),
    Membership {
        pending: Option<PendingPanelCommit>,
        prepared: Box<PreparedMembershipCandidate>,
    },
    Divergent {
        pending: Option<PendingPanelCommit>,
    },
}

const fn wire_direction(direction: PanelDirection) -> WorkbenchPanelSplitDirection {
    match direction {
        PanelDirection::Left => WorkbenchPanelSplitDirection::Left,
        PanelDirection::Right => WorkbenchPanelSplitDirection::Right,
        PanelDirection::Up => WorkbenchPanelSplitDirection::Above,
        PanelDirection::Down => WorkbenchPanelSplitDirection::Below,
    }
}

impl NativeSessionState {
    #[cfg(test)]
    pub(in crate::terminal_session) fn retained_panel_for_test(
        &self,
    ) -> Option<&PendingPanelCommit> {
        self.settled_panel.as_ref()
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn panel_recovery_for_test(
        &self,
    ) -> Option<&PendingPanelCommit> {
        self.panel_recovery.as_ref()
    }

    pub(in crate::terminal_session) fn panel_blocks_structure(&self) -> bool {
        self.pending_panel().is_some() || self.panel_recovery.is_some()
    }

    pub(in crate::terminal_session) fn pending_panel(&self) -> Option<&PanelOperation> {
        match &self.pending_mutation {
            Some(super::PendingMutation::Panel(operation)) => Some(operation),
            _ => None,
        }
    }

    pub(in crate::terminal_session) fn plan_panel(
        &self,
        request_id: WorkbenchRequestId,
        kind: PanelRequestKind,
        deadline: Instant,
    ) -> Option<PanelOperation> {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.close_status_only()
            || self.pending_refresh_generation.is_some()
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
        let active = self.active()?;
        let tab = self.cache.tabs().get(active.index())?;
        let panel = tab.panel(active.panel_index())?;
        let intent = match kind {
            PanelRequestKind::Split { direction } => PanelIntent::Split {
                direction: wire_direction(direction),
                label: panel.presentation_label().clone(),
                geometry: WorkbenchTerminalGeometry::new(
                    self.latest_geometry.columns(),
                    self.latest_geometry.rows(),
                    self.latest_geometry.cell_pixel_width(),
                    self.latest_geometry.cell_pixel_height(),
                )
                .ok()?,
            },
            PanelRequestKind::Close => PanelIntent::Close,
            PanelRequestKind::Move { direction } => {
                let target = tab
                    .panel_focus_target(active.panel_index(), direction)
                    .and_then(|index| tab.panel(index))?;
                PanelIntent::Move {
                    target_panel_id: target.panel_id().clone(),
                    direction: wire_direction(direction),
                }
            }
        };
        Some(PanelOperation::new(
            request_id,
            self.desired.clone()?,
            panel.panel_id().clone(),
            self.cache.clone(),
            kind,
            intent,
            deadline,
        ))
    }

    pub(in crate::terminal_session) fn begin_panel(&mut self, operation: PanelOperation) -> bool {
        let expected = self.plan_panel(
            operation.request_id.clone(),
            operation.kind,
            operation.deadline,
        );
        if expected.as_ref() != Some(&operation) {
            return false;
        }
        self.pending_mutation = Some(super::PendingMutation::Panel(operation));
        self.notice = None;
        true
    }

    pub(in crate::terminal_session) fn prepare_panel(
        &self,
        operation: &PanelOperation,
        result: PanelResult,
    ) -> PreparedPanel {
        let result = match result {
            PanelResult::NotSubmitted | PanelResult::Precommit(_) => {
                PreparedPanelResult::NotApplied
            }
            PanelResult::TabUnavailable { revision, outcome } => {
                if revision.value() < operation.baseline.revision().value() {
                    PreparedPanelResult::Divergent { pending: None }
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
                        Ok(candidate) => self.prepare_panel_candidate(operation, None, candidate),
                        Err(()) => PreparedPanelResult::Divergent { pending: None },
                    }
                } else {
                    PreparedPanelResult::NotApplied
                }
            }
            PanelResult::Unchanged(revision) if revision == operation.baseline.revision() => {
                PreparedPanelResult::Unchanged
            }
            PanelResult::Unchanged(_) => PreparedPanelResult::Divergent { pending: None },
            PanelResult::CloseAccepted(accepted) => {
                if &accepted.terminal_context_id == operation.baseline.terminal_context_id()
                    && &accepted.tab_id == operation.target.tab_id()
                    && accepted.panel_id == operation.panel_id
                    && accepted.accepted_revision <= operation.baseline.revision()
                {
                    PreparedPanelResult::CloseAccepted
                } else {
                    PreparedPanelResult::Divergent { pending: None }
                }
            }
            PanelResult::Committed { commit, outcome } => {
                let pending = PendingPanelCommit::new(operation.clone(), *commit);
                if !pending.valid() {
                    PreparedPanelResult::Divergent { pending: None }
                } else if let WorkbenchSnapshotOutcome::Available { snapshot } = outcome {
                    match operation
                        .baseline
                        .project_snapshot(&snapshot)
                        .map_err(|_| ())
                        .and_then(|candidate| {
                            pending.validate_projection(candidate).map_err(|_| ())
                        }) {
                        Ok(candidate) => {
                            self.prepare_panel_candidate(operation, Some(pending), candidate)
                        }
                        Err(()) => PreparedPanelResult::Divergent {
                            pending: Some(pending),
                        },
                    }
                } else {
                    PreparedPanelResult::Committed {
                        pending,
                        candidate: None,
                    }
                }
            }
        };
        PreparedPanel {
            operation: operation.clone(),
            result,
        }
    }

    fn prepare_panel_candidate(
        &self,
        operation: &PanelOperation,
        pending: Option<PendingPanelCommit>,
        candidate: NativeContextTabs,
    ) -> PreparedPanelResult {
        if candidate.tab_index(operation.target.tab_id()).is_some() {
            return match pending {
                Some(pending) => PreparedPanelResult::Committed {
                    pending,
                    candidate: Some(candidate),
                },
                None => PreparedPanelResult::RefusedProjection(candidate),
            };
        }
        match self.prepare_membership_candidate(
            candidate,
            Some(operation.target.tab_id()),
            operation.baseline.tab_index(operation.target.tab_id()),
            None,
        ) {
            Ok(prepared) => PreparedPanelResult::Membership {
                pending,
                prepared: Box::new(prepared),
            },
            Err(_) => PreparedPanelResult::Divergent { pending },
        }
    }

    pub(in crate::terminal_session) fn apply_prepared_panel(
        &mut self,
        prepared: PreparedPanel,
    ) -> PanelDecision {
        if !self.panel_result_is_current(&prepared.operation) {
            self.retire(CarrierFailure::Protocol);
            return PanelDecision::Retire;
        }
        match prepared.result {
            PreparedPanelResult::NotApplied => self.notice = Some(SessionNotice::PanelNotApplied),
            PreparedPanelResult::Unchanged => self.notice = None,
            PreparedPanelResult::CloseAccepted => {
                self.notice = None;
                self.note_publication_hint();
            }
            PreparedPanelResult::RefusedProjection(candidate) => {
                if !self.merge_projection(candidate) {
                    self.retire(CarrierFailure::AuthorityDivergence);
                    return PanelDecision::Retire;
                }
                self.notice = Some(SessionNotice::PanelNotApplied);
            }
            PreparedPanelResult::Membership { pending, .. } => {
                // Membership must use the common paired installation/cleanup consumer.
                if let Some(pending) = pending {
                    self.settled_panel = Some(pending);
                }
                self.retire(CarrierFailure::Protocol);
                return PanelDecision::Retire;
            }
            PreparedPanelResult::Committed { pending, candidate } => {
                self.settled_panel = Some(pending.clone());
                if let Some(candidate) = candidate {
                    if !self.merge_projection(candidate) {
                        self.retire(CarrierFailure::AuthorityDivergence);
                        return PanelDecision::Retire;
                    }
                } else {
                    self.panel_recovery = Some(pending);
                }
                self.notice = None;
            }
            PreparedPanelResult::Divergent { pending } => {
                if let Some(pending) = pending {
                    self.settled_panel = Some(pending);
                }
                self.retire(CarrierFailure::AuthorityDivergence);
                return PanelDecision::Retire;
            }
        }
        self.pending_mutation = None;
        PanelDecision::Continue
    }

    fn panel_result_is_current(&self, operation: &PanelOperation) -> bool {
        self.pending_panel() == Some(operation)
            && self.cache == operation.baseline
            && self.desired.as_ref() == Some(&operation.target)
            && self.invariant_holds()
            && matches!(
                self.phase,
                SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
    }

    /// The exact known command outcome survives every later cleanup/install preparation failure.
    pub(in crate::terminal_session) fn retain_prepared_panel_membership(
        &mut self,
        prepared: PreparedPanel,
    ) -> Result<(PanelOperation, PreparedMembershipCandidate), CarrierFailure> {
        if !self.panel_result_is_current(&prepared.operation) {
            return Err(CarrierFailure::Protocol);
        }
        let PreparedPanelResult::Membership {
            pending,
            prepared: candidate,
        } = prepared.result
        else {
            return Err(CarrierFailure::Protocol);
        };
        self.notice = if let Some(pending) = pending {
            self.settled_panel = Some(pending);
            None
        } else {
            Some(SessionNotice::PanelNotApplied)
        };
        Ok((prepared.operation, *candidate))
    }

    pub(super) fn panel_membership_is_current(&self, request_id: &WorkbenchRequestId) -> bool {
        self.pending_panel().is_some_and(|operation| {
            operation.request_id() == request_id
                && operation.baseline() == &self.cache
                && self.desired.as_ref() == Some(operation.target())
                && (self
                    .settled_panel
                    .as_ref()
                    .is_some_and(|pending| pending.operation() == operation)
                    || self.notice == Some(SessionNotice::PanelNotApplied))
        })
    }

    pub(in crate::terminal_session) fn retain_pending_panel(
        &mut self,
        pending: PendingPanelCommit,
    ) -> bool {
        if !pending.valid() {
            return false;
        }
        if self.settled_panel.as_ref() == Some(&pending) {
            return true;
        }
        if self.pending_panel() != Some(&pending.operation) {
            return false;
        }
        self.settled_panel = Some(pending);
        true
    }

    pub(super) fn panel_invariant_holds(&self) -> bool {
        if let Some(recovery) = &self.panel_recovery
            && (self.pending_panel().is_some()
                || !recovery.valid()
                || self.settled_panel.as_ref() != Some(recovery))
        {
            return false;
        }
        if matches!(&self.phase, SessionPhase::Retired(_)) {
            return true;
        }
        if self.panel_blocks_structure()
            && !matches!(
                &self.phase,
                SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
            && !(matches!(self.phase, SessionPhase::ReconcilingMembership(_))
                && ((self.panel_recovery.is_some() && self.pending_refresh_generation.is_some())
                    || self.pending_panel().is_some())
                && self.pending_membership.is_some())
        {
            return false;
        }
        self.pending_panel().is_none_or(|operation| {
            Some(&operation.target) == self.desired.as_ref() && operation.baseline == self.cache
        }) && self.panel_recovery.as_ref().is_none_or(|pending| {
            Some(&pending.operation.target) == self.desired.as_ref()
                && pending.operation.baseline == self.cache
        })
    }
}
