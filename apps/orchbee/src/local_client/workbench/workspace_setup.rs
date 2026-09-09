use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchDirectoryId, WorkbenchProjectId, WorkbenchRejectionReason, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchTerminalContextId,
    WorkbenchWireError, WorkbenchWorkspaceId, WorkerId, read_workbench_response,
    write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    pub(crate) fn submit_workbench_setup(
        &mut self,
        request: WorkbenchSetupRequest,
        deadline: Instant,
    ) -> WorkbenchSetupSubmission {
        if self.connection.is_none() {
            return WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                WorkbenchSetupNotSubmitted::SessionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let action = command.action.clone();
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                WorkbenchSetupNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                    WorkbenchSetupNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                WorkbenchSetupNotSubmitted::SessionUnavailable,
            );
        };
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            &frame,
            |io| read_workbench_response(io),
        );
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                    WorkbenchSetupNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_setup_unknown(
                    request_id,
                    WorkbenchSetupOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_setup_unknown(
                    request_id,
                    WorkbenchSetupOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_setup_unknown(
                    request_id,
                    WorkbenchSetupOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_setup_unknown(
                request_id,
                WorkbenchSetupOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_setup_unknown(
                request_id.clone(),
                WorkbenchSetupOutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received: result.request_id,
                },
            );
        }
        match classify_outcome(&action, expected_revision, result.outcome) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_setup_unknown(request_id, reason),
        }
    }

    fn retire_with_setup_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchSetupOutcomeUnknownReason,
    ) -> WorkbenchSetupSubmission {
        self.connection.take();
        WorkbenchSetupSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchSetupRequest(WorkbenchCommand);

impl WorkbenchSetupRequest {
    pub(crate) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.0.request_id
    }
}

impl TryFrom<WorkbenchCommand> for WorkbenchSetupRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(
            command.action,
            WorkbenchAction::ProjectCreate { .. }
                | WorkbenchAction::WorkspaceCreate { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchSetupPrecommit {
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
pub(crate) enum WorkbenchSetupSubmission {
    DefinitelyNotSubmitted(WorkbenchSetupNotSubmitted),
    DefinitePrecommit(WorkbenchSetupPrecommit),
    Committed(WorkbenchSetupCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchSetupOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchSetupNotSubmitted {
    SessionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchSetupNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionUnavailable => f.write_str("local Client Session is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP workspace setup request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP workspace setup request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchSetupNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SessionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchSetupOutcomeUnknownReason {
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

impl fmt::Display for WorkbenchSetupOutcomeUnknownReason {
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
                f.write_str("received an action-incompatible workspace setup outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(
                    f,
                    "workspace setup expected revision {expected} has no successor"
                )
            }
            Self::RevisionMismatch { expected, received } => write!(
                f,
                "expected Workbench revision {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchSetupOutcomeUnknownReason {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchSetupCommit {
    ProjectCreated {
        revision: WorkbenchRevision,
        project_id: WorkbenchProjectId,
    },
    WorkspaceCreated {
        revision: WorkbenchRevision,
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
    WorkspacePrimaryDirectoryAccepted {
        revision: WorkbenchRevision,
        directory_id: WorkbenchDirectoryId,
        worker_id: WorkerId,
    },
}

impl WorkbenchSetupCommit {
    pub(crate) const fn revision(&self) -> WorkbenchRevision {
        match self {
            Self::ProjectCreated { revision, .. }
            | Self::WorkspaceCreated { revision, .. }
            | Self::WorkspacePrimaryDirectoryAccepted { revision, .. } => *revision,
        }
    }
}

fn classify_outcome(
    action: &WorkbenchAction,
    expected_revision: WorkbenchRevision,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchSetupSubmission, WorkbenchSetupOutcomeUnknownReason> {
    let commit = match (action, outcome) {
        (_, WorkbenchCommandOutcome::Rejected { revision, reason })
            if compatible_rejection(action, reason) =>
        {
            return Ok(WorkbenchSetupSubmission::DefinitePrecommit(
                WorkbenchSetupPrecommit::Rejected { revision, reason },
            ));
        }
        (_, WorkbenchCommandOutcome::Denied {}) => {
            return Ok(WorkbenchSetupSubmission::DefinitePrecommit(
                WorkbenchSetupPrecommit::Denied,
            ));
        }
        (_, WorkbenchCommandOutcome::Unavailable { reason })
            if compatible_unavailable(action, reason) =>
        {
            return Ok(WorkbenchSetupSubmission::DefinitePrecommit(
                WorkbenchSetupPrecommit::Unavailable { reason },
            ));
        }
        (
            WorkbenchAction::ProjectCreate { .. },
            WorkbenchCommandOutcome::ProjectCreated {
                revision,
                project_id,
            },
        ) => WorkbenchSetupCommit::ProjectCreated {
            revision,
            project_id,
        },
        (
            WorkbenchAction::WorkspaceCreate { .. },
            WorkbenchCommandOutcome::WorkspaceCreated {
                revision,
                workspace_id,
                terminal_context_id,
            },
        ) => WorkbenchSetupCommit::WorkspaceCreated {
            revision,
            workspace_id,
            terminal_context_id,
        },
        (
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. },
            WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
                revision,
                directory_id,
                worker_id,
            },
        ) => WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
            revision,
            directory_id,
            worker_id,
        },
        _ => return Err(WorkbenchSetupOutcomeUnknownReason::ActionIncompatible),
    };
    let successor = expected_revision.value().checked_add(1).ok_or(
        WorkbenchSetupOutcomeUnknownReason::ExpectedRevisionOverflow {
            expected: expected_revision,
        },
    )?;
    let expected = WorkbenchRevision::new(successor);
    let received = commit.revision();
    if expected != received {
        return Err(WorkbenchSetupOutcomeUnknownReason::RevisionMismatch { expected, received });
    }
    Ok(WorkbenchSetupSubmission::Committed(commit))
}

fn compatible_rejection(action: &WorkbenchAction, reason: WorkbenchRejectionReason) -> bool {
    match reason {
        WorkbenchRejectionReason::SiteMismatch
        | WorkbenchRejectionReason::RevisionMismatch
        | WorkbenchRejectionReason::ResourceLimit
        | WorkbenchRejectionReason::IdentityExhausted
        | WorkbenchRejectionReason::RevisionExhausted
        | WorkbenchRejectionReason::RequestIdentityConflict => true,
        WorkbenchRejectionReason::ProjectUnavailable => matches!(
            action,
            WorkbenchAction::WorkspaceCreate { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceUnavailable
        | WorkbenchRejectionReason::WorkspaceDirectoryConflict => matches!(
            action,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceDirectoryUnavailable
        | WorkbenchRejectionReason::TerminalContextUnavailable
        | WorkbenchRejectionReason::TabUnavailable => false,
    }
}

fn compatible_unavailable(
    action: &WorkbenchAction,
    reason: WorkbenchCommandUnavailableReason,
) -> bool {
    match reason {
        WorkbenchCommandUnavailableReason::RequestLedgerFull
        | WorkbenchCommandUnavailableReason::ServiceUnavailable => true,
        WorkbenchCommandUnavailableReason::NoCompatibleWorker
        | WorkbenchCommandUnavailableReason::ServerBusy
        | WorkbenchCommandUnavailableReason::DirectoryUnavailable => matches!(
            action,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchCommandUnavailableReason::TerminalBusy
        | WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable => false,
    }
}
