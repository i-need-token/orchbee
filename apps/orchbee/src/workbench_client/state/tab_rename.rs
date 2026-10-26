use super::recovery_fence::RecoveryFence;
use super::rename_draft::{RenameDraftIntent, RenameDraftToken};
use super::{AttachmentKey, CarrierFailure, ClientContextTabs};
use crate::local_client::workbench::label_rename::{
    WorkbenchLabelRenamePrecommit, WorkbenchLabelRenameRequest,
};
use crate::workbench_client::workbench_projection::renamed::{
    RenamedTabExpectation, ValidatedRenamedProjection, validate_renamed_tab,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchLabel, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchSnapshotOutcome,
};
use std::time::Instant;

impl super::WorkbenchClientState {
    #[cfg(test)]
    pub(in crate::workbench_client) fn rename_receipts_for_test(
        &self,
    ) -> (
        Option<&RenameOperation>,
        Option<&PendingRenameCommit>,
        Option<&RenameOperation>,
    ) {
        (
            self.pending_rename(),
            self.settled_rename.as_ref(),
            self.rename_uncertain.as_ref(),
        )
    }
    pub(super) fn rename_invariant_holds(&self) -> bool {
        if self.rename_recovery.as_ref().is_some_and(|known| {
            self.setup_blocks_structure()
                || self.pending_mutation.is_some()
                || self.settled_rename.as_ref() != Some(known)
        }) {
            return false;
        }
        if matches!(self.phase, super::ClientPhase::Retired(_)) {
            return true;
        }
        let operation = self.pending_rename().or_else(|| {
            self.rename_recovery
                .as_ref()
                .map(PendingRenameCommit::operation)
        });
        operation.is_none_or(|operation| {
            self.cache == *operation.baseline()
                && self.desired.as_ref().map(super::PaneKey::anchor) == Some(operation.target())
                && (matches!(
                    self.phase,
                    super::ClientPhase::Ready(_)
                        | super::ClientPhase::SelectedWithoutAttachment(_)
                        | super::ClientPhase::RetryableAttachUnavailable(_)
                ) || matches!(self.phase, super::ClientPhase::ReconcilingMembership(_))
                    && self.pending_membership.is_some())
        })
    }
    pub(in crate::workbench_client) fn retain_rename_worker(
        &mut self,
        evidence: RenameWorkerEvidence,
    ) -> bool {
        match evidence {
            RenameWorkerEvidence::Known(known) => self.retain_pending_rename(known),
            RenameWorkerEvidence::Unknown(operation) => {
                if self.pending_rename() != Some(&operation) {
                    return false;
                }
                self.rename_uncertain = Some(operation);
                true
            }
        }
    }
    pub(in crate::workbench_client) fn pending_move(&self) -> Option<&super::MoveOperation> {
        match &self.pending_mutation {
            Some(super::PendingMutation::Move(operation)) => Some(operation),
            _ => None,
        }
    }

    pub(in crate::workbench_client) fn pending_rename(&self) -> Option<&RenameOperation> {
        match &self.pending_mutation {
            Some(super::PendingMutation::Rename(operation)) => Some(operation),
            _ => None,
        }
    }

    pub(in crate::workbench_client) fn mutation_blocks_structure(&self) -> bool {
        self.setup_blocks_structure()
            || self.workspace_close_blocks_structure()
            || self.mutation_in_progress()
    }

    /// Drafts are cancellable UI state; submitted and retained operations are not.
    pub(in crate::workbench_client) fn mutation_in_progress(&self) -> bool {
        self.setup_operation_in_progress()
            || self.takeover_pending()
            || self.pending_mutation.is_some()
            || self.recovery_fences(RecoveryFence::Mutation)
    }

    pub(in crate::workbench_client) fn plan_rename(
        &self,
        token: RenameDraftToken,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Option<RenameOperation> {
        if self.mutation_blocks_structure() || self.close_status_only() {
            return None;
        }
        RenameOperation::from_draft(self.plan_rename_draft(token)?, request_id, deadline).ok()
    }

    pub(in crate::workbench_client) fn begin_rename(&mut self, operation: RenameOperation) -> bool {
        if self
            .plan_rename(
                operation.draft_token(),
                operation.request_id().clone(),
                operation.deadline(),
            )
            .as_ref()
            != Some(&operation)
        {
            return false;
        }
        self.pending_mutation = Some(super::PendingMutation::Rename(operation));
        self.cancel_rename_draft();
        self.notice = None;
        true
    }

    pub(in crate::workbench_client) fn rename_is_current(
        &self,
        operation: &RenameOperation,
    ) -> bool {
        self.pending_rename() == Some(operation)
            && &self.cache == operation.baseline()
            && self.desired.as_ref().map(super::PaneKey::anchor) == Some(operation.target())
    }

    pub(in crate::workbench_client) fn retain_pending_rename(
        &mut self,
        known: PendingRenameCommit,
    ) -> bool {
        if self.settled_rename.as_ref() == Some(&known) {
            return true;
        }
        if self.pending_rename() != Some(known.operation()) {
            return false;
        }
        self.settled_rename = Some(known);
        true
    }

    pub(in crate::workbench_client) fn rename_membership_is_current(
        &self,
        request_id: &WorkbenchRequestId,
    ) -> bool {
        self.pending_rename().is_some_and(|operation| {
            operation.request_id() == request_id
                && self.rename_is_current(operation)
                && self
                    .settled_rename
                    .as_ref()
                    .is_some_and(|known| known.operation() == operation)
        })
    }

    pub(in crate::workbench_client) fn settle_rename_without_membership(
        &mut self,
        operation: &RenameOperation,
        result: RenamePreparation,
    ) -> Result<(), CarrierFailure> {
        if !self.rename_is_current(operation) {
            return Err(CarrierFailure::Protocol);
        }
        match result {
            RenamePreparation::NotSubmitted | RenamePreparation::Precommit(_) => {
                self.notice = Some(super::ClientNotice::RenameNotRenamed)
            }
            RenamePreparation::Unchanged => self.notice = None,
            RenamePreparation::Committed {
                known,
                projection: None,
            } => {
                if !self.retain_pending_rename(known.clone()) {
                    return Err(CarrierFailure::Protocol);
                }
                self.rename_recovery = Some(known);
                self.notice = None;
            }
            RenamePreparation::Divergent { known } => {
                if let Some(known) = known {
                    self.retain_pending_rename(known);
                }
                return Err(CarrierFailure::AuthorityDivergence);
            }
            RenamePreparation::Committed {
                projection: Some(_),
                ..
            } => return Err(CarrierFailure::Protocol),
        }
        self.pending_mutation = None;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct RenameOperation {
    request: WorkbenchLabelRenameRequest,
    intent: RenameDraftIntent,
    deadline: Instant,
}

impl RenameOperation {
    pub(in crate::workbench_client) fn from_draft(
        intent: RenameDraftIntent,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<Self, CarrierFailure> {
        let request = WorkbenchLabelRenameRequest::try_from(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: intent.baseline().session_id().clone(),
            expected_revision: intent.baseline().revision(),
            action: WorkbenchAction::TabRename {
                terminal_context_id: intent
                    .baseline()
                    .terminal_context_id()
                    .ok_or(CarrierFailure::Protocol)?
                    .clone(),
                tab_id: intent.target().tab_id().clone(),
                label: intent.label().clone(),
            },
        })
        .map_err(|_| CarrierFailure::Protocol)?;
        Ok(Self {
            request,
            intent,
            deadline,
        })
    }

    pub(in crate::workbench_client) const fn request(&self) -> &WorkbenchLabelRenameRequest {
        &self.request
    }
    pub(in crate::workbench_client) const fn request_id(&self) -> &WorkbenchRequestId {
        self.request.request_id()
    }
    pub(in crate::workbench_client) const fn draft_token(&self) -> RenameDraftToken {
        self.intent.token()
    }
    pub(in crate::workbench_client) const fn baseline(&self) -> &ClientContextTabs {
        self.intent.baseline()
    }
    pub(in crate::workbench_client) const fn target(&self) -> &AttachmentKey {
        self.intent.target()
    }
    pub(in crate::workbench_client) const fn label(&self) -> &WorkbenchLabel {
        self.intent.label()
    }
    pub(in crate::workbench_client) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(in crate::workbench_client) fn known_commit(
        &self,
        revision: WorkbenchRevision,
    ) -> Option<PendingRenameCommit> {
        let index = self.baseline().tab_index(self.target().tab_id())?;
        (self.baseline().revision().value().checked_add(1) == Some(revision.value())
            && (self.baseline().tabs()[index].automatic_label()
                || self.baseline().tabs()[index].tab_label() != self.label()))
        .then(|| PendingRenameCommit {
            operation: self.clone(),
            revision,
        })
    }

    pub(in crate::workbench_client) fn prepare(&self, result: RenameResult) -> PreparedRename {
        let result = match result {
            RenameResult::NotSubmitted => RenamePreparation::NotSubmitted,
            RenameResult::Precommit(outcome) => RenamePreparation::Precommit(outcome),
            RenameResult::Unchanged(revision) => {
                if revision == self.baseline().revision()
                    && self
                        .baseline()
                        .tab_index(self.target().tab_id())
                        .is_some_and(|index| {
                            !self.baseline().tabs()[index].automatic_label()
                                && self.baseline().tabs()[index].tab_label() == self.label()
                        })
                {
                    RenamePreparation::Unchanged
                } else {
                    RenamePreparation::Divergent { known: None }
                }
            }
            RenameResult::Committed { revision, outcome } => match self.known_commit(revision) {
                None => RenamePreparation::Divergent { known: None },
                Some(known) => match outcome {
                    WorkbenchSnapshotOutcome::Available { snapshot } => {
                        let projection = self
                            .baseline()
                            .project_snapshot(&snapshot)
                            .map_err(|_| ())
                            .and_then(|candidate| {
                                known.validate_projection(candidate).map_err(|_| ())
                            });
                        match projection {
                            Ok(projection) => RenamePreparation::Committed {
                                known,
                                projection: Some(projection),
                            },
                            Err(()) => RenamePreparation::Divergent { known: Some(known) },
                        }
                    }
                    WorkbenchSnapshotOutcome::Denied {}
                    | WorkbenchSnapshotOutcome::Unavailable { .. } => {
                        RenamePreparation::Committed {
                            known,
                            projection: None,
                        }
                    }
                },
            },
        };
        PreparedRename {
            operation: self.clone(),
            result,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct PendingRenameCommit {
    operation: RenameOperation,
    revision: WorkbenchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum RenameWorkerEvidence {
    Known(PendingRenameCommit),
    Unknown(RenameOperation),
}

impl PendingRenameCommit {
    pub(in crate::workbench_client) const fn operation(&self) -> &RenameOperation {
        &self.operation
    }
    pub(in crate::workbench_client) const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    pub(super) fn validate_projection(
        &self,
        candidate: ClientContextTabs,
    ) -> Result<
        ValidatedRenamedProjection,
        super::super::workbench_projection::ClientProjectionDivergence,
    > {
        validate_renamed_tab(
            self.operation.baseline(),
            &RenamedTabExpectation::new(
                self.revision(),
                self.operation
                    .baseline()
                    .terminal_context_id()
                    .ok_or(super::super::workbench_projection::ClientProjectionDivergence)?
                    .clone(),
                self.operation.target().tab_id().clone(),
                self.operation.label().clone(),
            ),
            candidate,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum RenameResult {
    NotSubmitted,
    Precommit(WorkbenchLabelRenamePrecommit),
    Unchanged(WorkbenchRevision),
    Committed {
        revision: WorkbenchRevision,
        outcome: WorkbenchSnapshotOutcome,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum RenamePreparation {
    NotSubmitted,
    Precommit(WorkbenchLabelRenamePrecommit),
    Unchanged,
    Committed {
        known: PendingRenameCommit,
        projection: Option<ValidatedRenamedProjection>,
    },
    Divergent {
        known: Option<PendingRenameCommit>,
    },
}

pub(in crate::workbench_client) struct PreparedRename {
    operation: RenameOperation,
    result: RenamePreparation,
}

impl PreparedRename {
    #[cfg(test)]
    pub(in crate::workbench_client) const fn operation(&self) -> &RenameOperation {
        &self.operation
    }
    pub(in crate::workbench_client) const fn result(&self) -> &RenamePreparation {
        &self.result
    }
    pub(in crate::workbench_client) fn into_parts(self) -> (RenameOperation, RenamePreparation) {
        (self.operation, self.result)
    }
}
