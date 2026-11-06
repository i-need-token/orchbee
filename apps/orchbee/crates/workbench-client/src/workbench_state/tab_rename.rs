//! Owns captured rename operation identity, known receipt and worker evidence, and canonical
//! membership validation for settlement. It does not edit the draft or change a canonical
//! label without a matching Server outcome.

use super::recovery_fence::RecoveryFence;
use super::rename_draft::{DraftToken, RenameDraftIntent, RenameUpdate};
use super::{AttachmentKey, CarrierFailure, ClientContextTabs};
use crate::exchange_values::workbench::label_rename::{
    WorkbenchLabelRenamePrecommit, WorkbenchLabelRenameRequest,
};
use crate::workbench_projection::renamed::{
    RenamedTabExpectation, ValidatedRenamedProjection, validate_renamed_panel, validate_renamed_tab,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchSnapshotOutcome,
};
use std::time::Instant;

impl super::WorkbenchClientState {
    #[cfg(test)]
    pub(crate) fn rename_receipts_for_test(
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
    pub(crate) fn retain_rename_worker(&mut self, evidence: RenameWorkerEvidence) -> bool {
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
    pub(crate) fn pending_move(&self) -> Option<&super::MoveOperation> {
        match &self.pending_mutation {
            Some(super::PendingMutation::Move(operation)) => Some(operation),
            _ => None,
        }
    }

    pub(crate) fn pending_rename(&self) -> Option<&RenameOperation> {
        match &self.pending_mutation {
            Some(super::PendingMutation::Rename(operation)) => Some(operation),
            _ => None,
        }
    }

    pub(crate) fn mutation_blocks_structure(&self) -> bool {
        self.setup_blocks_structure()
            || self.workspace_close_blocks_structure()
            || self.mutation_in_progress()
    }

    /// Drafts are cancellable UI state; submitted and retained operations are not.
    pub(crate) fn mutation_in_progress(&self) -> bool {
        self.setup_operation_in_progress()
            || self.takeover_pending()
            || self.pending_mutation.is_some()
            || self.recovery_fences(RecoveryFence::Mutation)
    }

    pub(crate) fn plan_rename(
        &self,
        token: DraftToken,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Option<RenameOperation> {
        if self.mutation_blocks_structure() || self.close_status_only() {
            return None;
        }
        RenameOperation::from_draft(self.plan_rename_draft(token)?, request_id, deadline).ok()
    }

    pub(crate) fn begin_rename(&mut self, operation: RenameOperation) -> bool {
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

    pub(crate) fn rename_is_current(&self, operation: &RenameOperation) -> bool {
        self.pending_rename() == Some(operation)
            && &self.cache == operation.baseline()
            && self.desired.as_ref().map(super::PaneKey::anchor) == Some(operation.target())
    }

    pub(crate) fn retain_pending_rename(&mut self, known: PendingRenameCommit) -> bool {
        if self.settled_rename.as_ref() == Some(&known) {
            return true;
        }
        if self.pending_rename() != Some(known.operation()) {
            return false;
        }
        self.settled_rename = Some(known);
        true
    }

    pub(crate) fn rename_membership_is_current(&self, request_id: &WorkbenchRequestId) -> bool {
        self.pending_rename().is_some_and(|operation| {
            operation.request_id() == request_id
                && self.rename_is_current(operation)
                && self
                    .settled_rename
                    .as_ref()
                    .is_some_and(|known| known.operation() == operation)
        })
    }

    pub(crate) fn settle_rename_without_membership(
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
pub(crate) struct RenameOperation {
    request: Box<WorkbenchLabelRenameRequest>,
    intent: RenameDraftIntent,
    deadline: Instant,
}

impl RenameOperation {
    pub(crate) fn from_draft(
        intent: RenameDraftIntent,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<Self, CarrierFailure> {
        let request = WorkbenchLabelRenameRequest::try_from(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: intent.baseline().session_id().clone(),
            expected_revision: intent.baseline().revision(),
            action: {
                let terminal_context_id = intent
                    .baseline()
                    .terminal_context_id()
                    .ok_or(CarrierFailure::Protocol)?
                    .clone();
                let tab_id = intent.target().tab_id().clone();
                match intent.update() {
                    RenameUpdate::Tab(label) => WorkbenchAction::TabRename {
                        terminal_context_id,
                        tab_id,
                        label: label.clone(),
                    },
                    RenameUpdate::Panel { panel, label } => WorkbenchAction::PanelRename {
                        terminal_context_id,
                        tab_id,
                        panel_id: panel.clone(),
                        label: label.clone(),
                    },
                }
            },
        })
        .map_err(|_| CarrierFailure::Protocol)?;
        Ok(Self {
            request: Box::new(request),
            intent,
            deadline,
        })
    }

    pub(crate) const fn request(&self) -> &WorkbenchLabelRenameRequest {
        &self.request
    }
    pub(crate) const fn request_id(&self) -> &WorkbenchRequestId {
        self.request.request_id()
    }
    pub(crate) const fn draft_token(&self) -> DraftToken {
        self.intent.token()
    }
    pub(crate) const fn baseline(&self) -> &ClientContextTabs {
        self.intent.baseline()
    }
    pub(crate) const fn target(&self) -> &AttachmentKey {
        self.intent.target()
    }
    pub(crate) const fn update(&self) -> &RenameUpdate {
        self.intent.update()
    }
    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn known_commit(&self, revision: WorkbenchRevision) -> Option<PendingRenameCommit> {
        (self.baseline().revision().value().checked_add(1) == Some(revision.value())
            && !self.unchanged())
        .then(|| PendingRenameCommit {
            operation: self.clone(),
            revision,
        })
    }

    fn unchanged(&self) -> bool {
        let Some(tab) = self
            .baseline()
            .tab_index(self.target().tab_id())
            .and_then(|index| self.baseline().tabs().get(index))
        else {
            return false;
        };
        match self.update() {
            RenameUpdate::Tab(label) => !tab.automatic_label() && tab.tab_label() == label,
            RenameUpdate::Panel { panel, label } => tab
                .panel_index(panel)
                .and_then(|index| tab.panel(index))
                .is_some_and(|panel| {
                    panel.primary().custom_label()
                        == label.custom_label().map(|label| label.as_str())
                }),
        }
    }

    pub(crate) fn prepare(&self, result: RenameResult) -> PreparedRename {
        let result = match result {
            RenameResult::NotSubmitted => RenamePreparation::NotSubmitted,
            RenameResult::Precommit(outcome) => RenamePreparation::Precommit(outcome),
            RenameResult::Unchanged(revision) => {
                if revision == self.baseline().revision() && self.unchanged() {
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
pub(crate) struct PendingRenameCommit {
    operation: RenameOperation,
    revision: WorkbenchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RenameWorkerEvidence {
    Known(PendingRenameCommit),
    Unknown(RenameOperation),
}

impl PendingRenameCommit {
    pub(crate) const fn operation(&self) -> &RenameOperation {
        &self.operation
    }
    pub(crate) const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    pub(super) fn validate_projection(
        &self,
        candidate: ClientContextTabs,
    ) -> Result<ValidatedRenamedProjection, crate::workbench_projection::ClientProjectionDivergence>
    {
        match self.operation.update() {
            RenameUpdate::Tab(label) => validate_renamed_tab(
                self.operation.baseline(),
                &RenamedTabExpectation::new(
                    self.revision(),
                    self.operation
                        .baseline()
                        .terminal_context_id()
                        .ok_or(crate::workbench_projection::ClientProjectionDivergence)?
                        .clone(),
                    self.operation.target().tab_id().clone(),
                    label.clone(),
                ),
                candidate,
            ),
            RenameUpdate::Panel { panel, label } => validate_renamed_panel(
                self.operation.baseline(),
                self.revision(),
                self.operation.target().tab_id(),
                panel,
                label,
                candidate,
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RenameResult {
    NotSubmitted,
    Precommit(WorkbenchLabelRenamePrecommit),
    Unchanged(WorkbenchRevision),
    Committed {
        revision: WorkbenchRevision,
        outcome: WorkbenchSnapshotOutcome,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RenamePreparation {
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

pub(crate) struct PreparedRename {
    operation: RenameOperation,
    result: RenamePreparation,
}

impl PreparedRename {
    #[cfg(test)]
    pub(crate) const fn operation(&self) -> &RenameOperation {
        &self.operation
    }
    pub(crate) const fn result(&self) -> &RenamePreparation {
        &self.result
    }
    pub(crate) fn into_parts(self) -> (RenameOperation, RenamePreparation) {
        (self.operation, self.result)
    }
}
