use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    LaunchProfileCatalogRevision, LaunchProfileId, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason, WorkbenchLaunchProfileCatalogRead,
    WorkbenchLaunchProfileCatalogResult, WorkbenchRejectionReason, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchWireError,
    read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    /// The `launch-profile list`/`get` read: one typed request on the
    /// admitted session under the caller's absolute deadline. The selector is
    /// resolved Server-side under the same coordinator lock that owns the
    /// catalog, so a `get` answer and its ambiguity candidates are one
    /// point-in-time observation. It is a pure read — a failed exchange is
    /// classified like the worktree-list read's, never replayed.
    pub(crate) fn submit_workbench_launch_profile_catalog_read_before(
        &mut self,
        request: WorkbenchLaunchProfileCatalogRead,
        deadline: Instant,
    ) -> Result<WorkbenchLaunchProfileCatalogResult, LaunchProfileCatalogReadError> {
        if self.connection.is_none() {
            return Err(LaunchProfileCatalogReadError::ConnectionUnavailable);
        }

        let request_id = request.request_id.clone();
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::LaunchProfileCatalogRead(request),
        )
        .map_err(LaunchProfileCatalogReadError::RequestEncoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(LaunchProfileCatalogReadError::RequestNotSubmitted)?;
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(LaunchProfileCatalogReadError::ConnectionUnavailable);
            };
            exchange_once(
                connection,
                &self.clock,
                deadline,
                self.io_chunk_limits,
                None,
                &frame,
                |io| read_workbench_response(io),
            )
        };
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return Err(LaunchProfileCatalogReadError::RequestNotSubmitted(error));
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_launch_profile_catalog_failure(
                    request_id,
                    LaunchProfileCatalogReadFailure::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_launch_profile_catalog_failure(
                    request_id,
                    LaunchProfileCatalogReadFailure::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_launch_profile_catalog_failure(
                    request_id,
                    LaunchProfileCatalogReadFailure::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::LaunchProfileCatalog(result) = response else {
            return self.retire_with_launch_profile_catalog_failure(
                request_id,
                LaunchProfileCatalogReadFailure::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            let received = result.request_id.clone();
            return self.retire_with_launch_profile_catalog_failure(
                request_id.clone(),
                LaunchProfileCatalogReadFailure::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    fn retire_with_launch_profile_catalog_failure<Value>(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: LaunchProfileCatalogReadFailure,
    ) -> Result<Value, LaunchProfileCatalogReadError> {
        self.connection.take();
        Err(LaunchProfileCatalogReadError::Failed { request_id, reason })
    }

    /// The `launch-profile create` mutation: an ordinary journaled Workbench
    /// command. The same request identity replays the Server's retained
    /// result; a transport failure is `OutcomeUnknown`, never retried.
    pub(crate) fn submit_workbench_launch_profile_create(
        &mut self,
        request: WorkbenchLaunchProfileCreateRequest,
        deadline: Instant,
    ) -> WorkbenchLaunchProfileMutationSubmission {
        self.submit_workbench_launch_profile_mutation(request.0, deadline)
    }

    /// The `launch-profile delete` mutation: the selector resolves once,
    /// Server-side; not-found and ambiguous settle as journaled typed
    /// outcomes that replay identically for the same request identity.
    pub(crate) fn submit_workbench_launch_profile_delete(
        &mut self,
        request: WorkbenchLaunchProfileDeleteRequest,
        deadline: Instant,
    ) -> WorkbenchLaunchProfileMutationSubmission {
        self.submit_workbench_launch_profile_mutation(request.0, deadline)
    }

    fn submit_workbench_launch_profile_mutation(
        &mut self,
        command: WorkbenchCommand,
        deadline: Instant,
    ) -> WorkbenchLaunchProfileMutationSubmission {
        if self.connection.is_none() {
            return WorkbenchLaunchProfileMutationSubmission::DefinitelyNotSubmitted(
                LaunchProfileMutationNotSubmitted::ConnectionUnavailable,
            );
        }
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchLaunchProfileMutationSubmission::DefinitelyNotSubmitted(
                LaunchProfileMutationNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchLaunchProfileMutationSubmission::DefinitelyNotSubmitted(
                    LaunchProfileMutationNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchLaunchProfileMutationSubmission::DefinitelyNotSubmitted(
                LaunchProfileMutationNotSubmitted::ConnectionUnavailable,
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
                return WorkbenchLaunchProfileMutationSubmission::DefinitelyNotSubmitted(
                    LaunchProfileMutationNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_launch_profile_unknown(
                    request_id,
                    LaunchProfileOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_launch_profile_unknown(
                    request_id,
                    LaunchProfileOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_launch_profile_unknown(
                    request_id,
                    LaunchProfileOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_launch_profile_unknown(
                request_id,
                LaunchProfileOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_launch_profile_unknown(
                request_id.clone(),
                LaunchProfileOutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received: result.request_id,
                },
            );
        }
        match classify_mutation_outcome(expected_revision, result.outcome) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_launch_profile_unknown(request_id, reason),
        }
    }

    fn retire_with_launch_profile_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: LaunchProfileOutcomeUnknownReason,
    ) -> WorkbenchLaunchProfileMutationSubmission {
        self.connection.take();
        WorkbenchLaunchProfileMutationSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchLaunchProfileCreateRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchLaunchProfileCreateRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::LaunchProfileCreate { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchLaunchProfileDeleteRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchLaunchProfileDeleteRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::LaunchProfileDelete { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

/// The definite outcomes one profile mutation can settle into: precommit
/// rejections, the committed create/delete row, and the typed selector
/// resolutions the Server journaled at the same resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchLaunchProfileMutationPrecommit {
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
pub(crate) enum WorkbenchLaunchProfileMutationSubmission {
    DefinitelyNotSubmitted(LaunchProfileMutationNotSubmitted),
    DefinitePrecommit(WorkbenchLaunchProfileMutationPrecommit),
    Created {
        revision: WorkbenchRevision,
        catalog_revision: LaunchProfileCatalogRevision,
        profile_id: LaunchProfileId,
    },
    Deleted {
        revision: WorkbenchRevision,
        catalog_revision: LaunchProfileCatalogRevision,
        profile_id: LaunchProfileId,
    },
    /// The selector matched nothing at the Server's own resolution.
    ProfileUnavailable,
    /// The selector's name cardinality exceeded one; candidates are the
    /// bounded id list captured at that same resolution.
    ProfileAmbiguous {
        candidate_ids: Vec<LaunchProfileId>,
    },
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: LaunchProfileOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum LaunchProfileMutationNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for LaunchProfileMutationNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP launch-profile request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP launch-profile request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for LaunchProfileMutationNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum LaunchProfileOutcomeUnknownReason {
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

impl fmt::Display for LaunchProfileOutcomeUnknownReason {
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
                f.write_str("received an action-incompatible launch-profile outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(f, "expected revision {expected} has no successor")
            }
            Self::RevisionMismatch { expected, received } => write!(
                f,
                "expected Workbench revision {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for LaunchProfileOutcomeUnknownReason {
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

#[derive(Debug)]
pub(crate) enum LaunchProfileCatalogReadError {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
    Failed {
        request_id: WorkbenchRequestId,
        reason: LaunchProfileCatalogReadFailure,
    },
}

impl fmt::Display for LaunchProfileCatalogReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(
                    f,
                    "OBWP launch-profile-catalog read encoding failed: {error}"
                )
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP launch-profile-catalog read failed before any transport handoff: {error}"
            ),
            Self::Failed { request_id, reason } => {
                write!(
                    f,
                    "launch-profile-catalog read {request_id} failed: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for LaunchProfileCatalogReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable | Self::RequestNotSubmitted(_) | Self::Failed { .. } => {
                None
            }
            Self::RequestEncoding(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum LaunchProfileCatalogReadFailure {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
}

impl fmt::Display for LaunchProfileCatalogReadFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(f, "request write failed: {error}"),
            Self::Response(error) => write!(f, "invalid catalog response: {error}"),
            Self::ResponseDeadline(error) => write!(f, "catalog response failed: {error}"),
            Self::UnexpectedResponse => {
                f.write_str("expected workbench.launch-profile-catalog.result response")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                f,
                "expected Workbench request identity {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for LaunchProfileCatalogReadFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse | Self::CorrelationMismatch { .. } => None,
        }
    }
}

fn classify_mutation_outcome(
    expected_revision: WorkbenchRevision,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchLaunchProfileMutationSubmission, LaunchProfileOutcomeUnknownReason> {
    match outcome {
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if matches!(
                reason,
                WorkbenchRejectionReason::SiteMismatch
                    | WorkbenchRejectionReason::RevisionMismatch
                    | WorkbenchRejectionReason::ResourceLimit
                    | WorkbenchRejectionReason::RevisionExhausted
                    | WorkbenchRejectionReason::IdentityExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) =>
        {
            Ok(WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(
                WorkbenchLaunchProfileMutationPrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => {
            Ok(WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(
                WorkbenchLaunchProfileMutationPrecommit::Denied,
            ))
        }
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(
                WorkbenchLaunchProfileMutationPrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::LaunchProfileCreated {
            revision,
            catalog_revision,
            profile_id,
        } => {
            validate_successor(expected_revision, revision)?;
            Ok(WorkbenchLaunchProfileMutationSubmission::Created {
                revision,
                catalog_revision,
                profile_id,
            })
        }
        WorkbenchCommandOutcome::LaunchProfileDeleted {
            revision,
            catalog_revision,
            profile_id,
        } => {
            validate_successor(expected_revision, revision)?;
            Ok(WorkbenchLaunchProfileMutationSubmission::Deleted {
                revision,
                catalog_revision,
                profile_id,
            })
        }
        WorkbenchCommandOutcome::LaunchProfileUnavailable {} => {
            Ok(WorkbenchLaunchProfileMutationSubmission::ProfileUnavailable)
        }
        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids } => {
            Ok(WorkbenchLaunchProfileMutationSubmission::ProfileAmbiguous { candidate_ids })
        }
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => Err(LaunchProfileOutcomeUnknownReason::ActionIncompatible),
    }
}

fn validate_successor(
    expected: WorkbenchRevision,
    received: WorkbenchRevision,
) -> Result<(), LaunchProfileOutcomeUnknownReason> {
    let successor = expected
        .value()
        .checked_add(1)
        .ok_or(LaunchProfileOutcomeUnknownReason::ExpectedRevisionOverflow { expected })?;
    if received == WorkbenchRevision::new(successor) {
        Ok(())
    } else {
        Err(LaunchProfileOutcomeUnknownReason::RevisionMismatch {
            expected: WorkbenchRevision::new(successor),
            received,
        })
    }
}
