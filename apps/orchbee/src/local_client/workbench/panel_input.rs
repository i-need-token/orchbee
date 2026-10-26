//! Submits a typed Panel input-policy change and validates its correlated revision and
//! compatible result. It does not send terminal input, grant a controller locally, or retry an
//! uncertain mutation.

use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchWireError, read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    pub(crate) fn submit_workbench_panel_input(
        &mut self,
        request: WorkbenchPanelInputRequest,
        deadline: Instant,
    ) -> WorkbenchPanelInputSubmission {
        if self.connection.is_none() {
            return WorkbenchPanelInputSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelInputNotSubmitted::ConnectionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchPanelInputSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelInputNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchPanelInputSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelInputNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchPanelInputSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelInputNotSubmitted::ConnectionUnavailable,
            );
        };
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            None,
            &frame,
            |io| read_workbench_response(io),
        );
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return WorkbenchPanelInputSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelInputNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_panel_input_unknown(
                    request_id,
                    WorkbenchPanelInputOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_panel_input_unknown(
                    request_id,
                    WorkbenchPanelInputOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_panel_input_unknown(
                    request_id,
                    WorkbenchPanelInputOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_panel_input_unknown(
                request_id,
                WorkbenchPanelInputOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_panel_input_unknown(
                request_id.clone(),
                WorkbenchPanelInputOutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received: result.request_id,
                },
            );
        }
        match classify_outcome(expected_revision, result.outcome) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_panel_input_unknown(request_id, reason),
        }
    }

    fn retire_with_panel_input_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelInputOutcomeUnknownReason,
    ) -> WorkbenchPanelInputSubmission {
        self.connection.take();
        WorkbenchPanelInputSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchPanelInputRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchPanelInputRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::PanelInputSet { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPanelInputPrecommit {
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Denied,
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelInputSubmission {
    DefinitelyNotSubmitted(WorkbenchPanelInputNotSubmitted),
    DefinitePrecommit(WorkbenchPanelInputPrecommit),
    Unchanged {
        revision: WorkbenchRevision,
    },
    Committed {
        revision: WorkbenchRevision,
    },
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelInputOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelInputNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchPanelInputNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP panel-input request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP panel-input request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchPanelInputNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelInputOutcomeUnknownReason {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
    ActionIncompatible,
    ExpectedRevisionOverflow {
        expected: WorkbenchRevision,
    },
    RevisionMismatch {
        expected: WorkbenchRevision,
        received: WorkbenchRevision,
    },
}

impl fmt::Display for WorkbenchPanelInputOutcomeUnknownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(f, "request write failed: {error}"),
            Self::Response(error) => write!(f, "invalid command response: {error}"),
            Self::ResponseDeadline(error) => write!(f, "command response failed: {error}"),
            Self::UnexpectedResponse => f.write_str("expected workbench.command.result response"),
            Self::CorrelationMismatch { expected, received } => write!(
                f,
                "expected Workbench request identity {expected} but received {received}"
            ),
            Self::ActionIncompatible => {
                f.write_str("received an action-incompatible panel-input outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(
                    f,
                    "panel-input expected revision {expected} has no successor"
                )
            }
            Self::RevisionMismatch { expected, received } => write!(
                f,
                "expected Workbench revision {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchPanelInputOutcomeUnknownReason {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse
            | Self::CorrelationMismatch { .. }
            | Self::ActionIncompatible
            | Self::ExpectedRevisionOverflow { .. }
            | Self::RevisionMismatch { .. } => None,
        }
    }
}

fn classify_outcome(
    expected_revision: WorkbenchRevision,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchPanelInputSubmission, WorkbenchPanelInputOutcomeUnknownReason> {
    match outcome {
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if matches!(
                reason,
                WorkbenchRejectionReason::SessionMismatch
                    | WorkbenchRejectionReason::RevisionMismatch
                    | WorkbenchRejectionReason::TerminalContextUnavailable
                    | WorkbenchRejectionReason::TabUnavailable
                    | WorkbenchRejectionReason::ResourceLimit
                    | WorkbenchRejectionReason::RevisionExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) || reason == WorkbenchRejectionReason::PanelUnavailable =>
        {
            Ok(WorkbenchPanelInputSubmission::DefinitePrecommit(
                WorkbenchPanelInputPrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => Ok(WorkbenchPanelInputSubmission::DefinitePrecommit(
            WorkbenchPanelInputPrecommit::Denied,
        )),
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchPanelInputSubmission::DefinitePrecommit(
                WorkbenchPanelInputPrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::Unchanged { revision } => {
            validate_revision(expected_revision, revision)?;
            Ok(WorkbenchPanelInputSubmission::Unchanged { revision })
        }
        WorkbenchCommandOutcome::PanelInputConfigured { revision } => {
            let successor = expected_revision.value().checked_add(1).ok_or(
                WorkbenchPanelInputOutcomeUnknownReason::ExpectedRevisionOverflow {
                    expected: expected_revision,
                },
            )?;
            validate_revision(WorkbenchRevision::new(successor), revision)?;
            Ok(WorkbenchPanelInputSubmission::Committed { revision })
        }
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => Err(WorkbenchPanelInputOutcomeUnknownReason::ActionIncompatible),
    }
}

fn validate_revision(
    expected: WorkbenchRevision,
    received: WorkbenchRevision,
) -> Result<(), WorkbenchPanelInputOutcomeUnknownReason> {
    if expected == received {
        Ok(())
    } else {
        Err(WorkbenchPanelInputOutcomeUnknownReason::RevisionMismatch { expected, received })
    }
}
