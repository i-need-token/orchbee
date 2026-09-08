use super::rename_draft::{RenameDraftIntent, RenameDraftToken};
use super::{AttachmentKey, CarrierFailure, NativeStandaloneTabs};
use crate::local_client::workbench::tab_rename::{
    WorkbenchTabRenamePrecommit, WorkbenchTabRenameRequest,
};
use crate::terminal_session::workbench_projection::{
    project_native_standalone_tabs,
    renamed::{RenamedTabExpectation, ValidatedRenamedProjection, validate_renamed_tab},
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchLabel, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchSnapshotOutcome,
};
use std::time::Instant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenameOperation {
    request: WorkbenchTabRenameRequest,
    intent: RenameDraftIntent,
    deadline: Instant,
}

impl RenameOperation {
    pub(in crate::terminal_session) fn from_draft(
        intent: RenameDraftIntent,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<Self, CarrierFailure> {
        let request = WorkbenchTabRenameRequest::try_from(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            site_id: intent.baseline().site_id().clone(),
            expected_revision: intent.baseline().revision(),
            action: WorkbenchAction::TabRename {
                terminal_context_id: intent.baseline().terminal_context_id().clone(),
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

    pub(in crate::terminal_session) const fn request(&self) -> &WorkbenchTabRenameRequest {
        &self.request
    }
    pub(in crate::terminal_session) const fn request_id(&self) -> &WorkbenchRequestId {
        self.request.request_id()
    }
    pub(in crate::terminal_session) const fn draft_token(&self) -> RenameDraftToken {
        self.intent.token()
    }
    pub(in crate::terminal_session) const fn baseline(&self) -> &NativeStandaloneTabs {
        self.intent.baseline()
    }
    pub(in crate::terminal_session) const fn target(&self) -> &AttachmentKey {
        self.intent.target()
    }
    pub(in crate::terminal_session) const fn label(&self) -> &WorkbenchLabel {
        self.intent.label()
    }
    pub(in crate::terminal_session) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(in crate::terminal_session) fn known_commit(
        &self,
        revision: WorkbenchRevision,
    ) -> Option<PendingRenameCommit> {
        let index = self.baseline().tab_index(self.target().tab_id())?;
        (self.baseline().revision().value().checked_add(1) == Some(revision.value())
            && self.baseline().tabs()[index].tab_label() != self.label())
        .then(|| PendingRenameCommit {
            operation: self.clone(),
            revision,
        })
    }

    pub(in crate::terminal_session) fn prepare(&self, result: RenameResult) -> PreparedRename {
        let result = match result {
            RenameResult::NotSubmitted => RenamePreparation::NotSubmitted,
            RenameResult::Precommit(outcome) => RenamePreparation::Precommit(outcome),
            RenameResult::Unchanged(revision) => {
                if revision == self.baseline().revision()
                    && self
                        .baseline()
                        .tab_index(self.target().tab_id())
                        .is_some_and(|index| {
                            self.baseline().tabs()[index].tab_label() == self.label()
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
                        let projection = project_native_standalone_tabs(&snapshot)
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
pub(in crate::terminal_session) struct PendingRenameCommit {
    operation: RenameOperation,
    revision: WorkbenchRevision,
}

impl PendingRenameCommit {
    pub(in crate::terminal_session) const fn operation(&self) -> &RenameOperation {
        &self.operation
    }
    pub(in crate::terminal_session) const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    fn validate_projection(
        &self,
        candidate: NativeStandaloneTabs,
    ) -> Result<
        ValidatedRenamedProjection,
        super::super::workbench_projection::NativeProjectionDivergence,
    > {
        validate_renamed_tab(
            self.operation.baseline(),
            &RenamedTabExpectation::new(
                self.revision,
                self.operation.baseline().terminal_context_id().clone(),
                self.operation.target().tab_id().clone(),
                self.operation.label().clone(),
            ),
            candidate,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum RenameResult {
    NotSubmitted,
    Precommit(WorkbenchTabRenamePrecommit),
    Unchanged(WorkbenchRevision),
    Committed {
        revision: WorkbenchRevision,
        outcome: WorkbenchSnapshotOutcome,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum RenamePreparation {
    NotSubmitted,
    Precommit(WorkbenchTabRenamePrecommit),
    Unchanged,
    Committed {
        known: PendingRenameCommit,
        projection: Option<ValidatedRenamedProjection>,
    },
    Divergent {
        known: Option<PendingRenameCommit>,
    },
}

pub(in crate::terminal_session) struct PreparedRename {
    operation: RenameOperation,
    result: RenamePreparation,
}

impl PreparedRename {
    pub(in crate::terminal_session) const fn operation(&self) -> &RenameOperation {
        &self.operation
    }
    pub(in crate::terminal_session) const fn result(&self) -> &RenamePreparation {
        &self.result
    }
    pub(in crate::terminal_session) fn into_parts(self) -> (RenameOperation, RenamePreparation) {
        (self.operation, self.result)
    }
}
