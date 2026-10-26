use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchWireError, read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    pub(crate) fn submit_workbench_label_rename(
        &mut self,
        request: WorkbenchLabelRenameRequest,
        deadline: Instant,
    ) -> WorkbenchLabelRenameSubmission {
        if self.connection.is_none() {
            return WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(
                WorkbenchLabelRenameNotSubmitted::ConnectionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let panel_target = matches!(command.action, WorkbenchAction::PanelRename { .. });
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(
                WorkbenchLabelRenameNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(
                    WorkbenchLabelRenameNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(
                WorkbenchLabelRenameNotSubmitted::ConnectionUnavailable,
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
                return WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(
                    WorkbenchLabelRenameNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_label_rename_unknown(
                    request_id,
                    WorkbenchLabelRenameOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_label_rename_unknown(
                    request_id,
                    WorkbenchLabelRenameOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_label_rename_unknown(
                    request_id,
                    WorkbenchLabelRenameOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_label_rename_unknown(
                request_id,
                WorkbenchLabelRenameOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_label_rename_unknown(
                request_id.clone(),
                WorkbenchLabelRenameOutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received: result.request_id,
                },
            );
        }
        match classify_outcome(expected_revision, panel_target, result.outcome) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_label_rename_unknown(request_id, reason),
        }
    }

    fn retire_with_label_rename_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchLabelRenameOutcomeUnknownReason,
    ) -> WorkbenchLabelRenameSubmission {
        self.connection.take();
        WorkbenchLabelRenameSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchLabelRenameRequest(WorkbenchCommand);

impl WorkbenchLabelRenameRequest {
    pub(crate) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.0.request_id
    }
}

impl TryFrom<WorkbenchCommand> for WorkbenchLabelRenameRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(
            command.action,
            WorkbenchAction::TabRename { .. } | WorkbenchAction::PanelRename { .. }
        ) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchLabelRenamePrecommit {
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
pub(crate) enum WorkbenchLabelRenameSubmission {
    DefinitelyNotSubmitted(WorkbenchLabelRenameNotSubmitted),
    DefinitePrecommit(WorkbenchLabelRenamePrecommit),
    Unchanged {
        revision: WorkbenchRevision,
    },
    Committed {
        revision: WorkbenchRevision,
    },
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchLabelRenameOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchLabelRenameNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchLabelRenameNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP label-rename request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP label-rename request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchLabelRenameNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchLabelRenameOutcomeUnknownReason {
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

impl fmt::Display for WorkbenchLabelRenameOutcomeUnknownReason {
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
                f.write_str("received an action-incompatible label-rename outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(
                    f,
                    "label-rename expected revision {expected} has no successor"
                )
            }
            Self::RevisionMismatch { expected, received } => write!(
                f,
                "expected Workbench revision {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchLabelRenameOutcomeUnknownReason {
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
    panel_target: bool,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchLabelRenameSubmission, WorkbenchLabelRenameOutcomeUnknownReason> {
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
            ) || (panel_target && reason == WorkbenchRejectionReason::PanelUnavailable) =>
        {
            Ok(WorkbenchLabelRenameSubmission::DefinitePrecommit(
                WorkbenchLabelRenamePrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => {
            Ok(WorkbenchLabelRenameSubmission::DefinitePrecommit(
                WorkbenchLabelRenamePrecommit::Denied,
            ))
        }
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchLabelRenameSubmission::DefinitePrecommit(
                WorkbenchLabelRenamePrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::Unchanged { revision } => {
            validate_revision(expected_revision, revision)?;
            Ok(WorkbenchLabelRenameSubmission::Unchanged { revision })
        }
        WorkbenchCommandOutcome::Renamed { revision } => {
            let successor = expected_revision.value().checked_add(1).ok_or(
                WorkbenchLabelRenameOutcomeUnknownReason::ExpectedRevisionOverflow {
                    expected: expected_revision,
                },
            )?;
            validate_revision(WorkbenchRevision::new(successor), revision)?;
            Ok(WorkbenchLabelRenameSubmission::Committed { revision })
        }
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => Err(WorkbenchLabelRenameOutcomeUnknownReason::ActionIncompatible),
    }
}

fn validate_revision(
    expected: WorkbenchRevision,
    received: WorkbenchRevision,
) -> Result<(), WorkbenchLabelRenameOutcomeUnknownReason> {
    if expected == received {
        Ok(())
    } else {
        Err(WorkbenchLabelRenameOutcomeUnknownReason::RevisionMismatch { expected, received })
    }
}
