//! Send-once transport for unbound layout structure and terminal member creation.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome as Outcome, WorkbenchRequest,
    WorkbenchResponse, read_workbench_response, write_workbench_request,
};
use std::{io, time::Instant};

pub(crate) struct CreationRequest(WorkbenchCommand);
impl TryFrom<WorkbenchCommand> for CreationRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(
            command.action,
            WorkbenchAction::TabCreateLayout { .. } | WorkbenchAction::PanelCreateTerminal { .. }
        ) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}
#[derive(Debug)]
pub(crate) enum CreationSubmission {
    Outcome(Box<Outcome>),
    NotSubmitted {
        kind: io::ErrorKind,
        reason: &'static str,
    },
    Unknown {
        reason: &'static str,
    },
}
impl LocalClient {
    pub(crate) fn submit_workbench_creation(
        &mut self,
        request: CreationRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> CreationSubmission {
        fn not_submitted(kind: io::ErrorKind, reason: &'static str) -> CreationSubmission {
            CreationSubmission::NotSubmitted { kind, reason }
        }
        let command = request.0;
        if self.connection.is_none() {
            return not_submitted(io::ErrorKind::NotConnected, "session-unavailable");
        }
        let mut frame = Vec::new();
        if write_workbench_request(&mut frame, &WorkbenchRequest::Command(command.clone())).is_err()
        {
            return not_submitted(io::ErrorKind::InvalidInput, "request-encoding");
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(value) => value,
            Err(error) => return not_submitted(error.kind(), "request-not-submitted"),
        };
        let attempt = exchange_once(
            self.connection.as_mut().expect("checked connection"),
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        let reason = match attempt {
            ExchangeAttempt::NotSubmitted(error) => {
                return not_submitted(error.kind(), "request-not-submitted");
            }
            ExchangeAttempt::WriteFailed(_) => "request-write",
            ExchangeAttempt::ReadFailed(_) => "response-read",
            ExchangeAttempt::ReadDeadline(_) => "response-deadline",
            ExchangeAttempt::Result(response) => match response {
                WorkbenchResponse::Command(result) if result.request_id == command.request_id => {
                    if command.admits_creation_outcome(&result.outcome) {
                        return CreationSubmission::Outcome(Box::new(result.outcome));
                    }
                    "outcome-mismatch"
                }
                // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
                _ => "response-mismatch",
            },
        };
        self.connection.take();
        CreationSubmission::Unknown { reason }
    }
}
