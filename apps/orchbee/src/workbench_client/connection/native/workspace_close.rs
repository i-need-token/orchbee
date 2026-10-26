//! Submit the confirmed scope once, retain admission, and observe it without replaying effects.
use super::{NativeExecutor, Result, closed, event};
use crate::{
    local_client::workbench::{
        WorkbenchWorkspaceClosePrecommit, WorkbenchWorkspaceCloseRequest,
        WorkbenchWorkspaceCloseSubmission,
    },
    workbench_client::{
        mailbox::{CommandLease, EventDeadline, MutationResult, RefreshMarker, WorkerEvent},
        state::{
            CarrierFailure,
            workspace_close::{
                WorkspaceCloseEvidence, WorkspaceCloseOperation, WorkspaceCloseResult,
                retains_close_result,
            },
        },
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchSnapshotOutcome, WorkbenchWorkspaceCloseRead,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceGroupCloseRequest,
    WorkspaceGroupCloseAction, WorkspaceGroupCloseOutcome,
};
use std::{
    thread,
    time::{Duration, Instant},
};

impl NativeExecutor {
    pub(super) fn run_workspace_close(
        &mut self,
        lease: &CommandLease,
        marker: RefreshMarker,
        operation: WorkspaceCloseOperation,
    ) -> Result<()> {
        if self.pending_workspace_close.is_some()
            || self
                .mailbox
                .mark_refresh_in_flight(operation.generation)
                .map_err(closed)?
                != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        self.retain_workspace_close(&operation, WorkspaceCloseResult::NotSubmitted)?;
        let cutoff = self.effective_lease_cutoff(lease, operation.deadlines.forward_cutoff())?;
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let mut outcome = self.read_workbench_before(request_id, cutoff)?;
        let source_current = matches!(&outcome, WorkbenchSnapshotOutcome::Available { snapshot }
            if operation.baseline.project_snapshot(snapshot).ok().as_ref() == Some(&operation.baseline));
        if !source_current {
            self.retain_workspace_close(&operation, WorkspaceCloseResult::Refused)?;
        } else {
            self.ensure_before(cutoff)?;
            // A missing result is ambiguous as soon as a handoff can occur. Admission replaces
            // this evidence synchronously before the first read, allocation, or UI wait.
            self.retain_workspace_close(&operation, WorkspaceCloseResult::Unknown)?;
            if operation.workspace_command().is_some() {
                self.submit_single_workspace_close(&operation, cutoff)?;
            } else {
                self.submit_project_close_all(&operation, cutoff)?;
            }
            self.ensure_before(cutoff)?;
            let request_id = self
                .mailbox
                .allocate_workbench_request_id()
                .map_err(closed)?;
            outcome = self.read_workbench_before(request_id, cutoff)?;
        }
        let evidence = self
            .pending_workspace_close
            .clone()
            .ok_or(CarrierFailure::Protocol)?;
        let wait = self
            .mailbox
            .publish_event_before(
                WorkerEvent::WorkspaceCloseResult {
                    evidence,
                    outcome: Box::new(outcome),
                },
                EventDeadline::PlacementOnly(cutoff),
                &self.clock,
            )
            .map_err(event)?;
        let result = self.finish_refresh(lease, &marker, &wait);
        if result.is_ok() {
            self.pending_workspace_close = None;
        }
        result
    }

    fn submit_single_workspace_close(
        &mut self,
        operation: &WorkspaceCloseOperation,
        cutoff: Instant,
    ) -> Result<()> {
        let command = operation
            .workspace_command()
            .ok_or(CarrierFailure::Protocol)?;
        let request = WorkbenchWorkspaceCloseRequest::try_from(command)
            .map_err(|_| CarrierFailure::Protocol)?;
        let accepted = match self
            .client
            .submit_workbench_workspace_close(request, cutoff)
        {
            WorkbenchWorkspaceCloseSubmission::DefinitelyNotSubmitted(_) => {
                self.retain_workspace_close(operation, WorkspaceCloseResult::NotSubmitted)?;
                return Err(CarrierFailure::Transport);
            }
            WorkbenchWorkspaceCloseSubmission::DefinitePrecommit(reason) => {
                let review = matches!(
                    operation.workspace_command().map(|command| command.action),
                    Some(orchbee_workbench_protocol::WorkbenchAction::WorkspaceCloseIfIdle { .. })
                ) && matches!(reason, WorkbenchWorkspaceClosePrecommit::Rejected {
                        revision, reason: orchbee_workbench_protocol::WorkbenchRejectionReason::ConfirmationRequired,
                    } if revision == operation.baseline.revision());
                self.retain_workspace_close(
                    operation,
                    if review {
                        WorkspaceCloseResult::ConfirmationRequired
                    } else {
                        WorkspaceCloseResult::Refused
                    },
                )?;
                return Ok(());
            }
            WorkbenchWorkspaceCloseSubmission::OutcomeUnknown { .. } => {
                return Err(CarrierFailure::OutcomeUnknown);
            }
            WorkbenchWorkspaceCloseSubmission::Accepted(accepted) => accepted,
        };
        self.retain_workspace_close(
            operation,
            WorkspaceCloseResult::WorkspaceAccepted(accepted.clone()),
        )?;
        loop {
            self.ensure_before(cutoff)?;
            let request = WorkbenchWorkspaceCloseRead {
                protocol_version: ProtocolVersion,
                request_id: self
                    .mailbox
                    .allocate_workbench_request_id()
                    .map_err(closed)?,
                site_id: operation.baseline.site_id().clone(),
                workspace_id: accepted.workspace_id.clone(),
            };
            let outcome = self
                .client
                .submit_workbench_workspace_close_read(
                    request,
                    Some(&accepted.close_request_id),
                    cutoff,
                )
                .map_err(|_| CarrierFailure::Transport)?;
            // An unavailable read says nothing about the retained admission or earlier observation.
            if !matches!(
                outcome,
                WorkbenchWorkspaceCloseReadOutcome::Available { .. }
            ) {
                break;
            }
            self.retain_workspace_close(
                operation,
                WorkspaceCloseResult::WorkspaceObserved {
                    accepted: accepted.clone(),
                    outcome: Box::new(outcome),
                },
            )?;
            if !self
                .pending_workspace_close
                .as_ref()
                .is_some_and(|evidence| evidence.result.is_pending())
            {
                break;
            }
            self.wait_close_observation(cutoff)?;
        }
        Ok(())
    }

    fn submit_project_close_all(
        &mut self,
        operation: &WorkspaceCloseOperation,
        cutoff: Instant,
    ) -> Result<()> {
        let request = operation
            .project_request()
            .ok_or(CarrierFailure::Protocol)?;
        let first = match self
            .client
            .submit_workspace_group_close(request, cutoff, None)
        {
            Ok(reply) => reply,
            Err(error) => {
                if !error.submitted {
                    self.retain_workspace_close(operation, WorkspaceCloseResult::NotSubmitted)?;
                }
                return Err(if error.submitted {
                    CarrierFailure::OutcomeUnknown
                } else {
                    CarrierFailure::Transport
                });
            }
        };
        if !matches!(first.outcome, WorkspaceGroupCloseOutcome::Available { .. }) {
            return self.retain_workspace_close(operation, WorkspaceCloseResult::Refused);
        }
        self.retain_workspace_close(
            operation,
            WorkspaceCloseResult::ProjectObserved(Box::new(first)),
        )?;
        while self
            .pending_workspace_close
            .as_ref()
            .is_some_and(|evidence| evidence.result.is_pending())
        {
            self.wait_close_observation(cutoff)?;
            let request = WorkbenchWorkspaceGroupCloseRequest {
                protocol_version: ProtocolVersion,
                request_id: self
                    .mailbox
                    .allocate_workbench_request_id()
                    .map_err(closed)?,
                site_id: operation.baseline.site_id().clone(),
                action: WorkspaceGroupCloseAction::Read {
                    operation_id: operation.request_id.clone(),
                },
            };
            let next = self
                .client
                .submit_workspace_group_close(request, cutoff, None)
                .map_err(|_| CarrierFailure::Transport)?;
            if !matches!(next.outcome, WorkspaceGroupCloseOutcome::Available { .. }) {
                break;
            }
            self.retain_workspace_close(
                operation,
                WorkspaceCloseResult::ProjectObserved(Box::new(next)),
            )?;
        }
        Ok(())
    }

    fn wait_close_observation(&self, cutoff: Instant) -> Result<()> {
        self.ensure_before(cutoff)?;
        if let Some(failure) = self.mailbox.is_closed() {
            return Err(failure);
        }
        thread::sleep(
            Duration::from_millis(50).min(cutoff.saturating_duration_since(self.clock.now())),
        );
        self.ensure_before(cutoff)
    }

    fn retain_workspace_close(
        &mut self,
        operation: &WorkspaceCloseOperation,
        result: WorkspaceCloseResult,
    ) -> Result<()> {
        if !operation.admits(&result)
            || self.pending_workspace_close.as_ref().is_some_and(|before| {
                before.operation != *operation || !retains_close_result(&before.result, &result)
            })
        {
            return Err(CarrierFailure::Protocol);
        }
        // Keep admission and observed truth even if cancellation invalidated the mailbox lease.
        self.pending_workspace_close = Some(WorkspaceCloseEvidence {
            operation: operation.clone(),
            result: result.clone(),
        });
        if !self
            .mailbox
            .retain_workspace_close_result(operation, result)
        {
            return Err(CarrierFailure::Protocol);
        }
        Ok(())
    }
}
