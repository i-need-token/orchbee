//! One cancellable canonical mutation; lost or mismatched replies never trigger replay.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    ClientViewControlReason as Reason, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    read_workbench_response, write_workbench_request,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SplitRatioFailure {
    pub(crate) may_have_committed: bool,
    pub(crate) reason: Reason,
}
impl LocalClient {
    pub(crate) fn set_split_ratio(
        &mut self,
        command: WorkbenchCommand,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<WorkbenchRevision, SplitRatioFailure> {
        let before = |reason| SplitRatioFailure {
            may_have_committed: false,
            reason,
        };
        let WorkbenchAction::SplitRatioSet {
            previous_ratio,
            ratio,
            ..
        } = &command.action
        else {
            return Err(before(Reason::Unsupported));
        };
        let changed = ratio != previous_ratio;
        let expected = if changed {
            command.expected_revision.value().checked_add(1)
        } else {
            Some(command.expected_revision.value())
        };
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::Command(command.clone()))
            .map_err(|_| before(Reason::TargetUnavailable))?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| before(Reason::Expired))?;
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| before(Reason::ClientDisconnected))?;
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        match attempt {
            ExchangeAttempt::Result(WorkbenchResponse::Command(result))
                if result.request_id == command.request_id =>
            {
                match result.outcome {
                    WorkbenchCommandOutcome::SplitRatioConfigured { revision }
                        if changed && Some(revision.value()) == expected =>
                    {
                        return Ok(revision);
                    }
                    WorkbenchCommandOutcome::Unchanged { revision }
                        if !changed && Some(revision.value()) == expected =>
                    {
                        return Ok(revision);
                    }
                    WorkbenchCommandOutcome::Rejected { reason, .. } => {
                        let reason = match reason {
                            WorkbenchRejectionReason::RevisionMismatch
                            | WorkbenchRejectionReason::SessionMismatch => Reason::StaleView,
                            WorkbenchRejectionReason::ResourceLimit
                            | WorkbenchRejectionReason::RevisionExhausted
                            | WorkbenchRejectionReason::IdentityExhausted => Reason::Busy,
                            WorkbenchRejectionReason::ProjectUnavailable
                            | WorkbenchRejectionReason::WorkspaceUnavailable
                            | WorkbenchRejectionReason::WorkspaceMoveIndexOutOfBounds
                            | WorkbenchRejectionReason::WorkspaceMoveTargetInBlock
                            | WorkbenchRejectionReason::InvalidNavigationConfiguration
                            | WorkbenchRejectionReason::WorkspaceGroupCloseRequired
                            | WorkbenchRejectionReason::WorkspaceDirectoryUnavailable
                            | WorkbenchRejectionReason::WorkspaceDirectoryConflict
                            | WorkbenchRejectionReason::TerminalContextUnavailable
                            | WorkbenchRejectionReason::TabUnavailable
                            | WorkbenchRejectionReason::TabGroupUnavailable
                            | WorkbenchRejectionReason::TabMoveIndexOutOfBounds
                            | WorkbenchRejectionReason::TabMoveOrderConflict
                            | WorkbenchRejectionReason::PanelUnavailable
                            | WorkbenchRejectionReason::SplitUnavailable
                            | WorkbenchRejectionReason::RequestIdentityConflict
                            | WorkbenchRejectionReason::ConfirmationRequired
                            | WorkbenchRejectionReason::SameTab => Reason::TargetUnavailable,
                        };
                        return Err(before(reason));
                    }
                    WorkbenchCommandOutcome::Denied {} => return Err(before(Reason::Unsupported)),
                    WorkbenchCommandOutcome::Unavailable { .. } => {
                        return Err(before(Reason::Busy));
                    }
                    // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
                    _ => {}
                }
            }
            ExchangeAttempt::NotSubmitted(_) => return Err(before(Reason::ClientDisconnected)),
            // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
            _ => {}
        }
        self.connection = None;
        Err(SplitRatioFailure {
            may_have_committed: true,
            reason: Reason::ProjectionUnavailable,
        })
    }
}

#[cfg(test)]
mod tests;
