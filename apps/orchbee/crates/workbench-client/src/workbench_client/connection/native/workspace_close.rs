//! Submit the confirmed scope once and retain admission; the submission lease ends at acceptance.
//! Each settlement read then runs in its own short lease and never replays an effect.
use super::{ClientExecutor, Result, closed, event};
use crate::local_client::workbench::{
    WorkbenchWorkspaceClosePrecommit, WorkbenchWorkspaceCloseRequest,
    WorkbenchWorkspaceCloseSubmission,
};
use crate::workbench_client::mailbox::{
    CommandLease, EventDeadline, MutationResult, RefreshMarker, WorkerEvent,
};
use crate::workbench_state::CarrierFailure;
use crate::workbench_state::workspace_close::{
    WorkspaceCloseEvidence, WorkspaceCloseOperation, WorkspaceCloseResult,
    WorkspaceCloseSettlementRead, retains_close_result,
};
use orchbee_workbench_protocol::{WorkbenchSnapshotOutcome, WorkspaceGroupCloseOutcome};
use std::time::Instant;

impl ClientExecutor {
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
            // A newer projection is not a refusal of the target: the Client installs it and
            // decides whether the unchanged scope is submitted once more.
            let result = if matches!(&outcome, WorkbenchSnapshotOutcome::Available { .. }) {
                WorkspaceCloseResult::Stale
            } else {
                WorkspaceCloseResult::Refused
            };
            self.retain_workspace_close(&operation, result)?;
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
                // The Server committed nothing: another commit advanced the revision first.
                let raced =
                    matches!(reason, WorkbenchWorkspaceClosePrecommit::Rejected {
                    reason: orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
                    ..
                });
                self.retain_workspace_close(
                    operation,
                    if review {
                        WorkspaceCloseResult::ConfirmationRequired
                    } else if raced {
                        WorkspaceCloseResult::Stale
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
        // Admission ends this lease's part: settlement reads follow in their own leases.
        self.retain_workspace_close(operation, WorkspaceCloseResult::WorkspaceAccepted(accepted))
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
        // The group receipt is admission; settlement reads follow in their own leases.
        self.retain_workspace_close(
            operation,
            WorkspaceCloseResult::ProjectObserved(Box::new(first)),
        )
    }

    /// One settlement read of an accepted close, in its own lease. It sends only a status read
    /// for the retained admission, so it can never submit the close again, and publishes what it
    /// saw for the UI to retain. A read the Server no longer answers is published as such; it
    /// decides nothing about the retained admission.
    pub(super) fn run_workspace_close_read(
        &mut self,
        lease: &CommandLease,
        read: &WorkspaceCloseSettlementRead,
    ) -> Result<()> {
        let cutoff = self.effective_lease_cutoff(lease, read.deadline())?;
        self.ensure_before(cutoff)?;
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let observation =
            if let Some((request, close_request_id)) = read.workspace_request(request_id.clone()) {
                let outcome = self
                    .client
                    .submit_workbench_workspace_close_read(request, Some(close_request_id), cutoff)
                    .map_err(|_| CarrierFailure::Transport)?;
                read.workspace_observation(outcome)
            } else {
                let request = read
                    .project_request(request_id)
                    .ok_or(CarrierFailure::Protocol)?;
                let reply = self
                    .client
                    .submit_workspace_group_close(request, cutoff, None)
                    .map_err(|_| CarrierFailure::Transport)?;
                WorkspaceCloseSettlementRead::project_observation(reply)
            };
        self.mailbox
            .publish_notice_before(
                WorkerEvent::WorkspaceCloseObserved {
                    generation: read.generation(),
                    observation,
                },
                cutoff,
                &self.clock,
            )
            .map_err(event)?;
        Ok(())
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
