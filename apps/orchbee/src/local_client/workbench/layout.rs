//! Send-once transport for layout structure and exact unbound-member creation.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome as Outcome,
    WorkbenchCommandUnavailableReason as Unavailable, WorkbenchPanelTerminalTarget,
    WorkbenchRejectionReason as Rejection, WorkbenchRequest, WorkbenchResponse,
    WorkbenchTerminalOutcome, read_workbench_response, write_workbench_request,
};
use std::{io, time::Instant};

pub(crate) struct LayoutRequest(WorkbenchCommand);
impl TryFrom<WorkbenchCommand> for LayoutRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(
            command.action,
            WorkbenchAction::TabCreateLayout { .. }
                | WorkbenchAction::PanelCreateTerminal {
                    target: WorkbenchPanelTerminalTarget::Unbound { .. },
                    profile: None,
                    ..
                }
        ) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}
#[derive(Debug)]
pub(crate) enum LayoutSubmission {
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
    pub(crate) fn submit_workbench_layout(
        &mut self,
        request: LayoutRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> LayoutSubmission {
        fn not_submitted(kind: io::ErrorKind, reason: &'static str) -> LayoutSubmission {
            LayoutSubmission::NotSubmitted { kind, reason }
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
                    if compatible(&command, &result.outcome) {
                        return LayoutSubmission::Outcome(Box::new(result.outcome));
                    }
                    "outcome-mismatch"
                }
                _ => "response-mismatch",
            },
        };
        self.connection.take();
        LayoutSubmission::Unknown { reason }
    }
}
fn compatible(command: &WorkbenchCommand, outcome: &Outcome) -> bool {
    let structure = matches!(command.action, WorkbenchAction::TabCreateLayout { .. });
    match outcome {
        Outcome::Denied {} => return true,
        Outcome::Rejected { reason, .. } => {
            return match reason {
                Rejection::SiteMismatch
                | Rejection::RevisionMismatch
                | Rejection::TerminalContextUnavailable
                | Rejection::ResourceLimit
                | Rejection::RevisionExhausted
                | Rejection::RequestIdentityConflict => true,
                Rejection::IdentityExhausted => structure,
                Rejection::TabUnavailable
                | Rejection::PanelUnavailable
                | Rejection::WorkspaceDirectoryUnavailable => !structure,
                _ => false,
            };
        }
        Outcome::Unavailable { reason } => {
            return match reason {
                Unavailable::RequestLedgerFull
                | Unavailable::ServiceUnavailable
                | Unavailable::TerminalBusy => true,
                Unavailable::NoCompatibleWorker
                | Unavailable::ServerBusy
                | Unavailable::TerminalRuntimeUnavailable
                | Unavailable::DirectoryUnavailable => !structure,
            };
        }
        _ => {}
    }
    let Some(next) = command.expected_revision.value().checked_add(1) else {
        return false;
    };
    match (&command.action, outcome) {
        (
            WorkbenchAction::TabCreateLayout {
                terminal_context_id: expected,
                ..
            },
            Outcome::TabLayoutCreated {
                revision,
                terminal_context_id,
                ..
            },
        ) => revision.value() == next && terminal_context_id == expected,
        (
            WorkbenchAction::PanelCreateTerminal {
                terminal_context_id: context,
                tab_id: tab,
                panel_id: panel,
                target:
                    WorkbenchPanelTerminalTarget::Unbound {
                        presentation_id: presentation,
                    },
                geometry,
                ..
            },
            Outcome::PanelTerminalPending {
                revision,
                terminal_context_id,
                tab_id,
                panel_id,
                presentation_id,
            }
            | Outcome::PanelTerminalCreated {
                revision,
                terminal_context_id,
                tab_id,
                panel_id,
                presentation_id,
                ..
            },
        ) => {
            revision.value() == next
                && terminal_context_id == context
                && tab_id == tab
                && panel_id == panel
                && presentation_id == presentation
                && !matches!(outcome,
                    Outcome::PanelTerminalCreated { terminal_outcome: WorkbenchTerminalOutcome::Started { geometry: actual }, .. } if actual != geometry)
        }
        _ => false,
    }
}
