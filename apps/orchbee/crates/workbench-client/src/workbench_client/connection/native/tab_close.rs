//! Submits or explicitly resumes one captured retained close and carries its exact read or
//! receipt outcome back to the UI. It does not decide that a close succeeded from connection
//! loss or repeat a possibly delivered close.

use super::{ClientExecutor, Result, closed, event, io_failure};
use crate::local_client::workbench::{
    WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted, WorkbenchTabCloseReadError,
    WorkbenchTabCloseRequest, WorkbenchTabCloseResumeSubmission, WorkbenchTabCloseSubmission,
};
use crate::workbench_client::mailbox::{CommandLease, MutationResult, UiReply};
use crate::workbench_state::{
    CarrierFailure, CloseOperation, CloseOperationKind, CloseResult, PendingCloseResult,
    close_set_premises_match,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchTabCloseRead,
    WorkbenchTabCloseResume, WorkbenchWireError,
};
use std::time::Instant;

impl ClientExecutor {
    pub(super) fn run_close(
        &mut self,
        lease: &CommandLease,
        operation: CloseOperation,
    ) -> Result<()> {
        if self.pending_close.is_some()
            || self
                .mailbox
                .mark_close_in_flight(&operation)
                .map_err(closed)?
                != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        // Executing is not submission evidence. A cutoff or lease refusal below is still unsent.
        self.pending_close = Some(PendingCloseResult::new(
            operation.clone(),
            CloseResult::NotSubmitted,
        ));
        let cutoff = self.effective_lease_cutoff(lease, operation.deadline())?;
        let (result, failure) = self.exchange_close_once(&operation, cutoff)?;
        // Retain accepted/disposed/Closed facts before timed deposition or any UI wait can fail.
        self.pending_close = Some(PendingCloseResult::new(operation.clone(), result.clone()));
        if let Some(failure) = failure {
            return Err(failure);
        }
        let wait = self
            .mailbox
            .publish_close_result_before(operation.clone(), result, cutoff, &self.clock)
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply(&wait)
            .map_err(closed)?
            .ok_or(CarrierFailure::Protocol)?;
        if !matches!(reply, UiReply::Close(ref key) if key == operation.key())
            || !self
                .pending_close
                .as_ref()
                .is_some_and(|pending| pending.operation() == &operation)
        {
            return Err(CarrierFailure::Protocol);
        }
        // A later flight may already be admitted. Consume only this worker's exact receipt.
        self.pending_close = None;
        Ok(())
    }

    fn exchange_close_once(
        &mut self,
        operation: &CloseOperation,
        cutoff: Instant,
    ) -> Result<(CloseResult, Option<CarrierFailure>)> {
        let target = operation.key().target();
        let request_id = operation.key().request_id().clone();
        let result = match operation.kind() {
            CloseOperationKind::Close => {
                let mut expected_revision = operation.selection().base_revision();
                if let Some(source) = operation.set_source() {
                    let read_id = self
                        .mailbox
                        .allocate_workbench_request_id()
                        .map_err(closed)?;
                    let outcome = self.read_workbench_before(read_id, cutoff)?;
                    let orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                        snapshot,
                    } = outcome
                    else {
                        return Ok((CloseResult::NotSubmitted, None));
                    };
                    if !close_set_premises_match(&snapshot, source) {
                        return Ok((CloseResult::NotSubmitted, None));
                    }
                    // This read reserves nothing. The existing Tab Close revision CAS remains the
                    // only admission authority, at the revision this read just proved acceptable.
                    expected_revision = snapshot.revision;
                    self.ensure_before(cutoff)?;
                }
                let request = WorkbenchTabCloseRequest::try_from(WorkbenchCommand {
                    protocol_version: ProtocolVersion,
                    request_id,
                    session_id: target.session_id().clone(),
                    expected_revision,
                    action: WorkbenchAction::TabClose {
                        terminal_context_id: target.terminal_context_id().clone(),
                        tab_id: target.tab_id().clone(),
                    },
                })
                .map_err(|_command| CarrierFailure::Protocol)?;
                match self.client.submit_workbench_tab_close(request, cutoff) {
                    WorkbenchTabCloseSubmission::DefinitelyNotSubmitted(reason) => {
                        return Ok(self.close_not_submitted(reason));
                    }
                    WorkbenchTabCloseSubmission::DefinitePrecommit(outcome) => {
                        CloseResult::Precommit(outcome)
                    }
                    WorkbenchTabCloseSubmission::Accepted(accepted) => {
                        CloseResult::Accepted(accepted)
                    }
                    WorkbenchTabCloseSubmission::OutcomeUnknown { .. } => {
                        return Ok((CloseResult::Ambiguous, Some(CarrierFailure::OutcomeUnknown)));
                    }
                }
            }
            CloseOperationKind::Read {
                expected_operation, ..
            } => {
                let request = WorkbenchTabCloseRead {
                    protocol_version: ProtocolVersion,
                    request_id,
                    session_id: target.session_id().clone(),
                    terminal_context_id: target.terminal_context_id().clone(),
                    tab_id: target.tab_id().clone(),
                };
                match self.client.submit_workbench_tab_close_read(
                    request,
                    expected_operation.as_ref(),
                    cutoff,
                ) {
                    Ok(outcome) => CloseResult::Read(outcome),
                    Err(WorkbenchTabCloseReadError::NotSubmitted(reason)) => {
                        return Ok(self.close_not_submitted(reason));
                    }
                    Err(WorkbenchTabCloseReadError::Failed { reason, .. }) => {
                        return Ok((
                            CloseResult::ObservationUnavailable,
                            Some(close_read_failure(reason)),
                        ));
                    }
                }
            }
            CloseOperationKind::Resume {
                operation: close_request_id,
                generation,
            } => {
                let request = WorkbenchTabCloseResume {
                    protocol_version: ProtocolVersion,
                    request_id,
                    session_id: target.session_id().clone(),
                    terminal_context_id: target.terminal_context_id().clone(),
                    tab_id: target.tab_id().clone(),
                    close_request_id: close_request_id.clone(),
                    expected_attempt_generation: *generation,
                };
                match self
                    .client
                    .submit_workbench_tab_close_resume(request, cutoff)
                {
                    WorkbenchTabCloseResumeSubmission::DefinitelyNotSubmitted(reason) => {
                        return Ok(self.close_not_submitted(reason));
                    }
                    WorkbenchTabCloseResumeSubmission::Returned(outcome) => {
                        CloseResult::Resume(outcome)
                    }
                    WorkbenchTabCloseResumeSubmission::OutcomeUnknown => {
                        return Ok((CloseResult::Ambiguous, Some(CarrierFailure::OutcomeUnknown)));
                    }
                }
            }
        };
        Ok((result, None))
    }

    fn close_not_submitted(
        &self,
        reason: WorkbenchTabCloseNotSubmitted,
    ) -> (CloseResult, Option<CarrierFailure>) {
        let unavailable = matches!(reason, WorkbenchTabCloseNotSubmitted::ConnectionUnavailable)
            || self.client.generation().is_none();
        (
            CloseResult::NotSubmitted,
            unavailable.then_some(CarrierFailure::Transport),
        )
    }
}

fn close_read_failure(reason: WorkbenchTabCloseFailure) -> CarrierFailure {
    match reason {
        WorkbenchTabCloseFailure::Write(error)
        | WorkbenchTabCloseFailure::ResponseDeadline(error)
        | WorkbenchTabCloseFailure::Response(WorkbenchWireError::Io(error)) => io_failure(&error),
        WorkbenchTabCloseFailure::Response(_)
        | WorkbenchTabCloseFailure::UnexpectedResponse
        | WorkbenchTabCloseFailure::ActionIncompatible
        | WorkbenchTabCloseFailure::CorrelationMismatch { .. }
        | WorkbenchTabCloseFailure::SessionMismatch { .. }
        | WorkbenchTabCloseFailure::TerminalContextMismatch { .. }
        | WorkbenchTabCloseFailure::TabMismatch { .. }
        | WorkbenchTabCloseFailure::OperationMismatch { .. }
        | WorkbenchTabCloseFailure::AcceptedRevisionMismatch { .. }
        | WorkbenchTabCloseFailure::AttemptGenerationMismatch { .. } => CarrierFailure::Protocol,
    }
}
