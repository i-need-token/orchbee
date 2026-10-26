use super::{
    BarrierFence, BarrierKind, COMMAND_CAPACITY, CommandMailbox, EventDeadline, EventError,
    MailboxClock, MailboxClosed, MailboxState, MembershipResultKey, MoveAdmissionError,
    MutationResult, PaneKey, QueuedCommand, QueuedRecord, RefreshFlight, StructuralMutationFlight,
    UiCorrelation, UiWaitKey, WorkerEvent, workbench_identity_candidate,
};
use crate::local_client::LocalOperationClock;
use crate::workbench_client::state::{OperationDeadlines, PendingRefreshStage};
use crate::workbench_client::state::{
    WorkbenchClientState,
    workspace_setup::{
        NativeSetupIntent, NativeSetupToken, SetupOperation, SetupResult, SetupWorkerEvidence,
    },
};

pub(super) struct SetupFlight {
    serial: u64,
    desired: Option<PaneKey>,
    operation: SetupOperation,
    stage: PendingRefreshStage,
    not_applied: bool,
    confirmation_required: bool,
    known: Option<crate::workbench_client::state::workspace_setup::SetupKnownCommit>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct WorkspaceSetupMarker(
    pub(in crate::workbench_client) SetupOperation,
);
impl WorkspaceSetupMarker {
    pub(super) fn barrier(&self) -> BarrierFence {
        BarrierFence::from_deadlines(BarrierKind::WorkspaceSetup, self.0.deadlines)
    }
}
impl CommandMailbox {
    pub(in crate::workbench_client) fn try_enqueue_quick_preparation_for(
        &self,
        canonical: &mut WorkbenchClientState,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
        request: Option<crate::workbench_client::ProjectQuickWorkspaceRequest>,
    ) -> bool {
        self.try_enqueue_quick_preparation_with_review(canonical, deadlines, clock, request, false)
    }

    pub(in crate::workbench_client) fn try_enqueue_quick_preparation_with_review(
        &self,
        canonical: &mut WorkbenchClientState,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
        request: Option<crate::workbench_client::ProjectQuickWorkspaceRequest>,
        review_directory: bool,
    ) -> bool {
        let admission_cutoff = request
            .as_ref()
            .map_or(deadlines.forward_cutoff(), |request| {
                request.deadline().min(deadlines.forward_cutoff())
            });
        let Some((state, _)) = self.lock_before(admission_cutoff, clock) else {
            return false;
        };
        if clock.now() >= admission_cutoff {
            return false;
        }
        self.try_enqueue_refresh_intent_for(
            state,
            canonical,
            deadlines.total(),
            Some(deadlines),
            |canonical, generation| {
                if clock.now() >= admission_cutoff {
                    return Err(());
                }
                (if review_directory {
                    canonical
                        .begin_quick_preparation_with_review(generation, deadlines, request, true)
                } else {
                    canonical.begin_quick_preparation(generation, deadlines, request)
                })
                .map(|p| super::RefreshPurpose::Quick(Box::new(p)))
                .ok_or(())
            },
            |_canonical, _result| {},
        )
        .is_ok()
    }
    pub(in crate::workbench_client) fn open_setup_draft_for(
        &self,
        canonical: &mut WorkbenchClientState,
        intent: NativeSetupIntent,
    ) -> Option<NativeSetupToken> {
        let state = self.lock();
        if !Self::setup_draft_available(&state, canonical) {
            return None;
        }
        canonical.open_setup_draft(intent)
    }
    pub(in crate::workbench_client) fn open_project_rename_draft_for(
        &self,
        canonical: &mut WorkbenchClientState,
        request: Box<crate::workbench_client::ProjectRenameRequest>,
        clock: &LocalOperationClock,
    ) -> Option<NativeSetupToken> {
        self.open_project_rename_draft_for_using(canonical, request, clock)
    }
    pub(super) fn open_project_rename_draft_for_using(
        &self,
        canonical: &mut WorkbenchClientState,
        request: Box<crate::workbench_client::ProjectRenameRequest>,
        clock: &dyn MailboxClock,
    ) -> Option<NativeSetupToken> {
        let deadline = request.deadline();
        let (state, _) = self.lock_before(deadline, clock)?;
        if !Self::setup_draft_available(&state, canonical) || clock.now() >= deadline {
            return None;
        }
        let token = canonical.open_setup_draft(NativeSetupIntent::RenameProject(request))?;
        // The draft is still private to this admission. Preparation may have
        // crossed the original cutoff; never publish its token in that case.
        if clock.now() >= deadline {
            canonical.cancel_setup_draft(token);
            return None;
        }
        Some(token)
    }
    fn setup_draft_available(state: &MailboxState, canonical: &WorkbenchClientState) -> bool {
        if Self::ensure_open(state).is_err()
            || state.desired.awaiting_bootstrap()
            || state.desired.key() != canonical.desired()
            || state.structural_mutation_pending.is_some()
            || state.refresh_flight.is_some()
            || state.close_flight.is_some()
            || state.close_reconciliation.is_some()
            || state
                .queue
                .iter()
                .any(|entry| entry.command.barrier().is_some())
        {
            return false;
        }
        true
    }
    pub(in crate::workbench_client) fn try_enqueue_setup_for(
        &self,
        canonical: &mut WorkbenchClientState,
        token: NativeSetupToken,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> Result<(), MoveAdmissionError<()>> {
        self.try_enqueue_setup_for_using(canonical, token, deadlines, clock)
    }
    pub(super) fn try_enqueue_setup_for_using(
        &self,
        canonical: &mut WorkbenchClientState,
        token: NativeSetupToken,
        deadlines: OperationDeadlines,
        clock: &dyn MailboxClock,
    ) -> Result<(), MoveAdmissionError<()>> {
        self.enqueue_setup(
            canonical,
            deadlines,
            clock,
            |state, request_id, generation| {
                state.plan_setup(token, request_id, generation, deadlines)
            },
        )
    }
    pub(in crate::workbench_client) fn try_enqueue_tab_configuration_for(
        &self,
        canonical: &mut WorkbenchClientState,
        operation: orchbee_workbench_protocol::TabConfiguration,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> Result<(), MoveAdmissionError<()>> {
        self.enqueue_setup(
            canonical,
            deadlines,
            clock,
            |state, request_id, generation| {
                state.plan_tab_configuration(operation, request_id, generation, deadlines)
            },
        )
    }
    pub(in crate::workbench_client) fn try_enqueue_sidebar_organization_for(
        &self,
        canonical: &mut WorkbenchClientState,
        request: &crate::workbench_client::WorkspaceOrganizationRequest,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> Result<(), MoveAdmissionError<()>> {
        let deadlines = deadlines.bounded_by(request.deadline());
        if clock.now() >= deadlines.forward_cutoff() {
            return Err(MoveAdmissionError::Deadline);
        }
        self.enqueue_setup(
            canonical,
            deadlines,
            clock,
            |state, request_id, generation| {
                state.plan_sidebar_organization(request, request_id, generation, deadlines)
            },
        )
    }
    pub(in crate::workbench_client) fn try_enqueue_navigation_configuration_for(
        &self,
        canonical: &mut WorkbenchClientState,
        operation: orchbee_workbench_protocol::NavigationConfiguration,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> Result<(), MoveAdmissionError<()>> {
        self.enqueue_setup(
            canonical,
            deadlines,
            clock,
            |state, request_id, generation| {
                state.plan_navigation_configuration(operation, request_id, generation, deadlines)
            },
        )
    }
    fn enqueue_setup(
        &self,
        canonical: &mut WorkbenchClientState,
        deadlines: OperationDeadlines,
        clock: &dyn MailboxClock,
        plan: impl FnOnce(
            &WorkbenchClientState,
            orchbee_workbench_protocol::WorkbenchRequestId,
            u64,
        ) -> Option<SetupOperation>,
    ) -> Result<(), MoveAdmissionError<()>> {
        let (mut state, _) = self
            .lock_before(deadlines.forward_cutoff(), clock)
            .ok_or(MoveAdmissionError::Deadline)?;
        Self::ensure_open(&state).map_err(MoveAdmissionError::Closed)?;
        if state.desired.awaiting_bootstrap()
            || state.desired.key() != canonical.desired()
            || state.structural_mutation_pending.is_some()
            || state.refresh_flight.is_some()
            || state.close_flight.is_some()
            || state.close_reconciliation.is_some()
            || state
                .queue
                .iter()
                .any(|entry| entry.command.barrier().is_some())
        {
            return Err(MoveAdmissionError::Busy);
        }
        if state.queue.len() == COMMAND_CAPACITY {
            return Err(MoveAdmissionError::Full);
        }
        let (cursor, request_id) = workbench_identity_candidate(state.identity_cursor)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let serial = state
            .next_command_serial
            .checked_add(1)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let generation = state
            .refresh_generation
            .checked_add(1)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let operation =
            plan(canonical, request_id, generation).ok_or(MoveAdmissionError::Build(()))?;
        let flight = Box::new(SetupFlight {
            serial,
            desired: canonical.desired().cloned(),
            operation: operation.clone(),
            stage: PendingRefreshStage::Queued,
            not_applied: false,
            confirmation_required: false,
            known: None,
        });
        let marker = WorkspaceSetupMarker(operation.clone());
        if clock.now() >= deadlines.forward_cutoff() {
            return Err(MoveAdmissionError::Deadline);
        }
        canonical.begin_setup(operation);
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.refresh_generation = generation;
        state.refresh_flight = Some(RefreshFlight {
            generation,
            stage: PendingRefreshStage::Queued,
        });
        state.structural_mutation_pending = Some(StructuralMutationFlight::Setup(flight));
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::WorkspaceSetup(Box::new(marker)),
            retry_at: None,
        });
        self.changed.notify_all();
        Ok(())
    }
    pub(in crate::workbench_client) fn mark_setup_in_flight(
        &self,
        operation: &SetupOperation,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let serial = state.active_lease.map(|lease| lease.serial);
        let valid = matches!(&state.structural_mutation_pending, Some(StructuralMutationFlight::Setup(flight)) if flight.operation == *operation && flight.stage == PendingRefreshStage::Queued && serial == Some(flight.serial));
        if !valid {
            return Ok(MutationResult::Stale);
        }
        let Some(StructuralMutationFlight::Setup(flight)) =
            state.structural_mutation_pending.as_mut()
        else {
            return Ok(MutationResult::Stale);
        };
        flight.stage = PendingRefreshStage::InFlight;
        state.refresh_flight = Some(RefreshFlight {
            generation: operation.generation,
            stage: PendingRefreshStage::InFlight,
        });
        Ok(MutationResult::Applied)
    }
    pub(in crate::workbench_client) fn publish_setup_result_before(
        &self,
        operation: SetupOperation,
        outcome: SetupResult,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        let deadline = operation.deadlines.forward_cutoff();
        self.publish_event_before(
            WorkerEvent::WorkspaceSetupResult { operation, outcome },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }
    pub(in crate::workbench_client) fn retain_setup_not_applied(
        &self,
        operation: &SetupOperation,
    ) -> bool {
        let mut state = self.lock();
        let Some(StructuralMutationFlight::Setup(flight)) =
            state.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if flight.operation != *operation
            || flight.stage != PendingRefreshStage::InFlight
            || flight.known.is_some()
        {
            return false;
        }
        flight.not_applied = true;
        true
    }
    pub(in crate::workbench_client) fn retain_setup_known(
        &self,
        known: &crate::workbench_client::state::workspace_setup::SetupKnownCommit,
    ) -> bool {
        let prepared = known.clone();
        let mut state = self.lock();
        let Some(StructuralMutationFlight::Setup(flight)) =
            state.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if flight.not_applied
            || flight.operation != known.operation
            || flight.stage != PendingRefreshStage::InFlight
            || flight.known.as_ref().is_some_and(|prior| prior != known)
        {
            return false;
        }
        flight.known = Some(prepared);
        true
    }
    pub(in crate::workbench_client) fn retain_setup_confirmation_required(
        &self,
        operation: &SetupOperation,
        reason: &crate::local_client::workbench::workspace_setup::WorkbenchSetupPrecommit,
    ) -> bool {
        use crate::local_client::workbench::workspace_setup::WorkbenchSetupPrecommit;
        if !matches!(
            operation.command.action,
            orchbee_workbench_protocol::WorkbenchAction::ProjectDissolveIfIdle { .. }
        ) || !matches!(reason, WorkbenchSetupPrecommit::Rejected {
                revision, reason: orchbee_workbench_protocol::WorkbenchRejectionReason::ConfirmationRequired,
            } if revision == &operation.command.expected_revision)
        {
            return false;
        }
        let mut state = self.lock();
        let Some(StructuralMutationFlight::Setup(flight)) =
            state.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if flight.operation != *operation
            || flight.stage != PendingRefreshStage::InFlight
            || !flight.not_applied
            || flight.known.is_some()
        {
            return false;
        }
        flight.confirmation_required = true;
        true
    }
    pub(in crate::workbench_client) fn setup_worker_loss(&self) -> Option<SetupWorkerEvidence> {
        let state = self.lock();
        let Some(StructuralMutationFlight::Setup(flight)) = &state.structural_mutation_pending
        else {
            return None;
        };
        if let Some(known) = &flight.known {
            return Some(SetupWorkerEvidence::Known(known.clone()));
        }
        if flight.stage == PendingRefreshStage::Queued {
            return Some(SetupWorkerEvidence::NotApplied(flight.operation.clone()));
        }
        Some(if flight.not_applied {
            SetupWorkerEvidence::NotApplied(flight.operation.clone())
        } else {
            SetupWorkerEvidence::Unknown(flight.operation.clone())
        })
    }
}
impl MailboxState {
    pub(super) fn setup_reply_current(&self, correlation: &UiCorrelation) -> bool {
        let Some(StructuralMutationFlight::Setup(flight)) = &self.structural_mutation_pending
        else {
            return true;
        };
        let generation = match correlation {
            UiCorrelation::Refresh(generation)
            | UiCorrelation::Membership(MembershipResultKey::Refresh(generation), _) => *generation,
            _ => return true,
        };
        generation == flight.operation.generation
            && flight.stage == PendingRefreshStage::AwaitingUi
            && self.active_lease.map(|lease| lease.serial) == Some(flight.serial)
            && self.event_anchor_serial == Some(flight.serial)
            && self.desired.key() == flight.desired.as_ref()
    }
    pub(super) fn place_setup_result(
        &mut self,
        operation: &SetupOperation,
        outcome: &SetupResult,
        deadline: EventDeadline,
        anchor: Option<u64>,
    ) -> bool {
        let Some(StructuralMutationFlight::Setup(flight)) =
            self.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if flight.operation != *operation
            || flight.stage != PendingRefreshStage::InFlight
            || anchor != Some(flight.serial)
            || deadline != EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff())
        {
            return false;
        }
        let valid_result = match outcome {
            SetupResult::Committed { known, .. } => flight.known.as_ref() == Some(known.as_ref()),
            SetupResult::NotApplied | SetupResult::Stale | SetupResult::ConfirmationRequired => {
                flight.not_applied
                && flight.known.is_none()
                && (!matches!(outcome, SetupResult::ConfirmationRequired)
                    || flight.confirmation_required
                        && matches!(
                            operation.command.action,
                            orchbee_workbench_protocol::WorkbenchAction::ProjectDissolveIfIdle { .. }
                        ))
            }
        };
        if !valid_result {
            return false;
        }
        flight.stage = PendingRefreshStage::AwaitingUi;
        self.refresh_flight = Some(RefreshFlight {
            generation: operation.generation,
            stage: PendingRefreshStage::AwaitingUi,
        });
        true
    }
}
