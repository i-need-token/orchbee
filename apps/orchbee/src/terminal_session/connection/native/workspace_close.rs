use super::{NativeExecutor, Result, closed, event};
use crate::terminal_session::{
    mailbox::{CommandLease, EventDeadline, MutationResult, RefreshMarker, WorkerEvent},
    state::{
        CarrierFailure,
        workspace_close::{
            WorkspaceCloseEvidence, WorkspaceCloseOperation, WorkspaceCloseResult,
            retains_group_observation,
        },
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchSnapshotOutcome, WorkbenchWorkspaceGroupCloseRequest,
    WorkspaceGroupCloseAction, WorkspaceGroupCloseOutcome,
};
use std::{thread, time::Duration};

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
        let source_current = matches!(&outcome, WorkbenchSnapshotOutcome::Available { snapshot } if operation.baseline.project_snapshot(snapshot).ok().as_ref() == Some(&operation.baseline));
        if !source_current {
            self.retain_workspace_close(&operation, WorkspaceCloseResult::Refused)?;
        } else {
            self.ensure_before(cutoff)?;
            // Once this handoff can occur, a missing result is ambiguous. A known receipt below
            // replaces this evidence synchronously, before the next allocation/read/UI wait.
            self.retain_workspace_close(&operation, WorkspaceCloseResult::Unknown)?;
            let first = match self.client.submit_workspace_group_close(
                operation.request.clone(),
                cutoff,
                None,
            ) {
                Ok(reply) => reply,
                Err(error) => {
                    if !error.submitted {
                        self.retain_workspace_close(
                            &operation,
                            WorkspaceCloseResult::NotSubmitted,
                        )?;
                    }
                    return Err(if error.submitted {
                        CarrierFailure::OutcomeUnknown
                    } else {
                        CarrierFailure::Transport
                    });
                }
            };
            if matches!(first.outcome, WorkspaceGroupCloseOutcome::Available { .. }) {
                self.retain_workspace_close(
                    &operation,
                    WorkspaceCloseResult::Observed(Box::new(first)),
                )?;
                while self
                    .pending_workspace_close
                    .as_ref()
                    .is_some_and(|evidence| evidence.result.is_pending())
                {
                    self.ensure_before(cutoff)?;
                    if let Some(failure) = self.mailbox.is_closed() {
                        return Err(failure);
                    }
                    thread::sleep(
                        Duration::from_millis(50)
                            .min(cutoff.saturating_duration_since(self.clock.now())),
                    );
                    self.ensure_before(cutoff)?;
                    let request = WorkbenchWorkspaceGroupCloseRequest {
                        protocol_version: ProtocolVersion,
                        request_id: self
                            .mailbox
                            .allocate_workbench_request_id()
                            .map_err(closed)?,
                        site_id: operation.request.site_id.clone(),
                        action: WorkspaceGroupCloseAction::Read {
                            operation_id: operation.request.action.operation_id().clone(),
                        },
                    };
                    let next = self
                        .client
                        .submit_workspace_group_close(request, cutoff, None)
                        .map_err(|_| CarrierFailure::Transport)?;
                    let Some(WorkspaceCloseEvidence {
                        result: WorkspaceCloseResult::Observed(before),
                        ..
                    }) = &self.pending_workspace_close
                    else {
                        return Err(CarrierFailure::Protocol);
                    };
                    if !retains_group_observation(before, &next) {
                        return Err(CarrierFailure::Protocol);
                    }
                    self.retain_workspace_close(
                        &operation,
                        WorkspaceCloseResult::Observed(Box::new(next)),
                    )?;
                }
            } else {
                self.retain_workspace_close(&operation, WorkspaceCloseResult::Refused)?;
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
    fn retain_workspace_close(
        &mut self,
        operation: &WorkspaceCloseOperation,
        result: WorkspaceCloseResult,
    ) -> Result<()> {
        if !operation.admits(&result) {
            return Err(CarrierFailure::Protocol);
        }
        if let Some(WorkspaceCloseEvidence {
            result: WorkspaceCloseResult::Observed(before),
            ..
        }) = &self.pending_workspace_close
        {
            let WorkspaceCloseResult::Observed(after) = &result else {
                return Err(CarrierFailure::Protocol);
            };
            if !retains_group_observation(before, after) {
                return Err(CarrierFailure::Protocol);
            }
        }
        let evidence = WorkspaceCloseEvidence {
            operation: operation.clone(),
            result: result.clone(),
        };
        // Keep the receipt even if cancellation invalidated the mailbox lease.
        self.pending_workspace_close = Some(evidence);
        if !self
            .mailbox
            .retain_workspace_close_result(operation, result)
        {
            return Err(CarrierFailure::Protocol);
        }
        Ok(())
    }
}
