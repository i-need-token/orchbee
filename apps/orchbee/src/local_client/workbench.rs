#[cfg(windows)]
pub(crate) mod agent_manifests;
mod agent_view;
#[cfg(windows)]
pub(crate) mod client_view;
mod close_operation;
pub(crate) mod configuration;
mod configured_command;
#[cfg(windows)]
pub(crate) mod creation;
#[cfg(windows)]
mod events;
pub(crate) mod metadata_exchange;
pub(crate) mod notification;
#[cfg(windows)]
mod pane_graphics;
pub(crate) mod panel_agent;
pub(crate) mod panel_metadata;
mod plugins;
mod settings;
mod terminal_editor;
mod window_title;
#[cfg(windows)]
mod workspace_git;
pub(crate) mod workspace_metadata;
use orchbee_local_ipc::LocalIo as BorrowedFd;
use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    LaunchProfileId, PublicationReadIntent, SnapshotReadIntent, TerminalPresentationId,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchPanelId, WorkbenchPublicationResult, WorkbenchRejectionReason, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotResult,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome, WorkbenchWireError, WorkbenchWorktreeListRead,
    WorkbenchWorktreeListResult, read_workbench_response, write_workbench_request,
};

use super::{ExchangeAttempt, LocalClient, exchange_once};

pub(crate) mod label_rename;
pub(crate) mod launch_profile;
pub(crate) mod panel_close;
pub(crate) mod panel_input;
pub(crate) mod panel_placement;
pub(crate) mod panel_relocate;
pub(crate) mod panel_split;
#[cfg(windows)]
pub(crate) mod split_ratio;
mod tab_close;
mod tab_move;
mod workspace_close;
mod workspace_group_close;
pub(crate) mod workspace_launch;
pub(crate) mod workspace_setup;
mod worktree_operation;
#[cfg(windows)]
pub(crate) mod worktree_query;
#[cfg(any(target_os = "linux", windows))]
pub(crate) use launch_profile::{LaunchProfileCatalogReadError, LaunchProfileCatalogReadFailure};
pub(crate) use tab_close::{
    WorkbenchTabCloseAccepted, WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted,
    WorkbenchTabClosePrecommit, WorkbenchTabCloseReadError, WorkbenchTabCloseRequest,
    WorkbenchTabCloseResumeSubmission, WorkbenchTabCloseSubmission,
};
pub(crate) use tab_move::{
    WorkbenchTabMoveCommit, WorkbenchTabMoveNotSubmitted, WorkbenchTabMoveOutcomeUnknownReason,
    WorkbenchTabMovePrecommit, WorkbenchTabMoveRequest, WorkbenchTabMoveSubmission,
};
pub(crate) use workspace_close::{
    WorkbenchWorkspaceCloseAccepted, WorkbenchWorkspaceCloseRequest,
    WorkbenchWorkspaceCloseSubmission,
};
#[cfg(any(target_os = "linux", windows))]
pub(crate) use workspace_close::{
    WorkbenchWorkspaceCloseFailure, WorkbenchWorkspaceCloseNotSubmitted,
    WorkbenchWorkspaceClosePrecommit, WorkbenchWorkspaceCloseReadError,
    WorkbenchWorkspaceCloseResumeSubmission,
};

impl LocalClient {
    pub(crate) fn submit_workbench_snapshot_read_before(
        &mut self,
        request: SnapshotReadIntent,
        deadline: Instant,
    ) -> Result<WorkbenchSnapshotResult, SnapshotReadError> {
        self.submit_workbench_snapshot_read_with_deadline(request, Some(deadline), None)
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) fn submit_workbench_snapshot_read_before_cancellable(
        &mut self,
        request: SnapshotReadIntent,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<WorkbenchSnapshotResult, SnapshotReadError> {
        self.submit_workbench_snapshot_read_with_deadline(request, Some(deadline), wake)
    }

    fn submit_workbench_snapshot_read_with_deadline(
        &mut self,
        request: SnapshotReadIntent,
        caller_deadline: Option<Instant>,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<WorkbenchSnapshotResult, SnapshotReadError> {
        if self.connection.is_none() {
            return Err(SnapshotReadError::ConnectionUnavailable);
        }

        let request_id = request.request_id.clone();
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::SnapshotRead(request))
            .map_err(SnapshotReadError::RequestEncoding)?;
        let deadline = match caller_deadline {
            Some(deadline) => self
                .submission_deadline_before(deadline)
                .map_err(SnapshotReadError::RequestNotSubmitted)?,
            None => {
                let mut deadline =
                    super::SubmissionDeadline::start(&self.clock, self.request_timeout)
                        .map_err(SnapshotReadError::RequestNotSubmitted)?;
                if let Some(startup) = self.submission_deadline {
                    deadline.expires_at = deadline.expires_at.min(startup);
                }
                deadline
            }
        };
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(SnapshotReadError::ConnectionUnavailable);
            };
            exchange_once(
                connection,
                &self.clock,
                deadline,
                self.io_chunk_limits,
                wake,
                &frame,
                |io| read_workbench_response(io),
            )
        };
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return Err(SnapshotReadError::RequestNotSubmitted(error));
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self
                    .retire_with_snapshot_failure(request_id, SnapshotReadFailure::Write(error));
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_snapshot_failure(
                    request_id,
                    SnapshotReadFailure::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_snapshot_failure(
                    request_id,
                    SnapshotReadFailure::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Snapshot(result) = response else {
            return self
                .retire_with_snapshot_failure(request_id, SnapshotReadFailure::UnexpectedResponse);
        };
        if result.request_id != request_id {
            let received = result.request_id.clone();
            return self.retire_with_snapshot_failure(
                request_id.clone(),
                SnapshotReadFailure::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_workbench_publication_read_before(
        &mut self,
        request: PublicationReadIntent,
        deadline: Instant,
    ) -> Result<WorkbenchPublicationResult, PublicationReadError> {
        self.submit_workbench_publication_read_with_deadline(request, Some(deadline))
    }

    fn submit_workbench_publication_read_with_deadline(
        &mut self,
        request: PublicationReadIntent,
        caller_deadline: Option<Instant>,
    ) -> Result<WorkbenchPublicationResult, PublicationReadError> {
        if self.connection.is_none() {
            return Err(PublicationReadError::ConnectionUnavailable);
        }

        let request_id = request.request_id.clone();
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::PublicationRead(request))
            .map_err(PublicationReadError::RequestEncoding)?;
        let deadline = match caller_deadline {
            Some(deadline) => self
                .submission_deadline_before(deadline)
                .map_err(PublicationReadError::RequestNotSubmitted)?,
            None => {
                let mut deadline =
                    super::SubmissionDeadline::start(&self.clock, self.request_timeout)
                        .map_err(PublicationReadError::RequestNotSubmitted)?;
                if let Some(startup) = self.submission_deadline {
                    deadline.expires_at = deadline.expires_at.min(startup);
                }
                deadline
            }
        };
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(PublicationReadError::ConnectionUnavailable);
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
                return Err(PublicationReadError::RequestNotSubmitted(error));
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_publication_failure(
                    request_id,
                    PublicationReadFailure::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_publication_failure(
                    request_id,
                    PublicationReadFailure::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_publication_failure(
                    request_id,
                    PublicationReadFailure::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Publication(result) = response else {
            return self.retire_with_publication_failure(
                request_id,
                PublicationReadFailure::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            let received = result.request_id.clone();
            return self.retire_with_publication_failure(
                request_id.clone(),
                PublicationReadFailure::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    /// The `worktree list` read: one typed request on the admitted session
    /// under the caller's absolute deadline. It is a pure read — a failed
    /// exchange is classified like the snapshot read's, never replayed.
    pub(crate) fn submit_workbench_worktree_list_read_before(
        &mut self,
        request: WorkbenchWorktreeListRead,
        deadline: Instant,
    ) -> Result<WorkbenchWorktreeListResult, WorktreeListReadError> {
        if self.connection.is_none() {
            return Err(WorktreeListReadError::ConnectionUnavailable);
        }

        let request_id = request.request_id.clone();
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::WorktreeListRead(request))
            .map_err(WorktreeListReadError::RequestEncoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(WorktreeListReadError::RequestNotSubmitted)?;
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(WorktreeListReadError::ConnectionUnavailable);
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
                return Err(WorktreeListReadError::RequestNotSubmitted(error));
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_worktree_list_failure(
                    request_id,
                    WorktreeListReadFailure::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_worktree_list_failure(
                    request_id,
                    WorktreeListReadFailure::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_worktree_list_failure(
                    request_id,
                    WorktreeListReadFailure::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::WorktreeList(result) = response else {
            return self.retire_with_worktree_list_failure(
                request_id,
                WorktreeListReadFailure::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            let received = result.request_id.clone();
            return self.retire_with_worktree_list_failure(
                request_id.clone(),
                WorktreeListReadFailure::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_workbench_tab_create(
        &mut self,
        request: WorkbenchTabCreateRequest,
        deadline: Instant,
    ) -> WorkbenchTabCreateSubmission {
        self.submit_workbench_tab_create_with_wake(request, deadline, None)
    }

    pub(crate) fn submit_workbench_tab_create_with_wake(
        &mut self,
        request: WorkbenchTabCreateRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkbenchTabCreateSubmission {
        if self.connection.is_none() {
            return WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
                WorkbenchTabCreateNotSubmitted::ConnectionUnavailable,
            );
        }

        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let WorkbenchAction::TabCreate {
            terminal_context_id,
            geometry,
            placement,
            ..
        } = &command.action
        else {
            unreachable!("opaque tab-create request only admits tab.create")
        };
        let expected_terminal_context_id = terminal_context_id.clone();
        let expected_geometry = *geometry;
        let expected_placement = placement.clone();
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
                WorkbenchTabCreateNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
                    WorkbenchTabCreateNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
                    WorkbenchTabCreateNotSubmitted::ConnectionUnavailable,
                );
            };
            exchange_once(
                connection,
                &self.clock,
                deadline,
                self.io_chunk_limits,
                wake,
                &frame,
                |io| read_workbench_response(io),
            )
        };
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
                    WorkbenchTabCreateNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_tab_create_unknown(
                    request_id,
                    WorkbenchTabCreateOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_tab_create_unknown(
                    request_id,
                    WorkbenchTabCreateOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_tab_create_unknown(
                    request_id,
                    WorkbenchTabCreateOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_tab_create_unknown(
                request_id,
                WorkbenchTabCreateOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            let received = result.request_id;
            return self.retire_with_tab_create_unknown(
                request_id.clone(),
                WorkbenchTabCreateOutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        match classify_tab_create_outcome(
            expected_revision,
            &expected_terminal_context_id,
            expected_geometry,
            expected_placement.as_ref(),
            result.outcome,
        ) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_tab_create_unknown(request_id, reason),
        }
    }

    fn retire_with_snapshot_failure<Value>(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: SnapshotReadFailure,
    ) -> Result<Value, SnapshotReadError> {
        self.connection.take();
        Err(SnapshotReadError::Failed { request_id, reason })
    }

    fn retire_with_publication_failure<Value>(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: PublicationReadFailure,
    ) -> Result<Value, PublicationReadError> {
        self.connection.take();
        Err(PublicationReadError::Failed { request_id, reason })
    }

    fn retire_with_worktree_list_failure<Value>(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorktreeListReadFailure,
    ) -> Result<Value, WorktreeListReadError> {
        self.connection.take();
        Err(WorktreeListReadError::Failed { request_id, reason })
    }

    fn retire_with_tab_create_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchTabCreateOutcomeUnknownReason,
    ) -> WorkbenchTabCreateSubmission {
        self.connection.take();
        WorkbenchTabCreateSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Debug)]
pub(crate) struct WorkbenchTabCreateRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchTabCreateRequest {
    type Error = WorkbenchCommand;

    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::TabCreate { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchTabCreatePrecommit {
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Denied,
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
    ProfileUnavailable,
    ProfileAmbiguous {
        candidate_ids: Vec<LaunchProfileId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchTabCreateCommit {
    Pending {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
    },
    Created {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        terminal_outcome: WorkbenchTerminalOutcome,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabCreateSubmission {
    DefinitelyNotSubmitted(WorkbenchTabCreateNotSubmitted),
    DefinitePrecommit(WorkbenchTabCreatePrecommit),
    Committed(WorkbenchTabCreateCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchTabCreateOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabCreateNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchTabCreateNotSubmitted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => {
                formatter.write_str("local Client Connection is unavailable")
            }
            Self::RequestEncoding(error) => {
                write!(
                    formatter,
                    "OBWP tab.create request encoding failed: {error}"
                )
            }
            Self::RequestNotSubmitted(error) => write!(
                formatter,
                "OBWP tab.create request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchTabCreateNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
            Self::ConnectionUnavailable => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabCreateOutcomeUnknownReason {
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
    CommittedRevisionMismatch {
        expected: WorkbenchRevision,
        received: WorkbenchRevision,
    },
    TerminalContextMismatch {
        expected: WorkbenchTerminalContextId,
        received: WorkbenchTerminalContextId,
    },
    StartedGeometryMismatch {
        expected: WorkbenchTerminalGeometry,
        received: WorkbenchTerminalGeometry,
    },
}

impl fmt::Display for WorkbenchTabCreateOutcomeUnknownReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Response(error) => write!(formatter, "invalid command response: {error}"),
            Self::ResponseDeadline(error) => write!(formatter, "command response failed: {error}"),
            Self::UnexpectedResponse => {
                formatter.write_str("expected workbench.command.result response")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                formatter,
                "expected Workbench request identity {expected} but received {received}"
            ),
            Self::ActionIncompatible => {
                formatter.write_str("received an action-incompatible tab.create outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => write!(
                formatter,
                "tab.create expected revision {expected} has no successor"
            ),
            Self::CommittedRevisionMismatch { expected, received } => write!(
                formatter,
                "expected committed Workbench revision {expected} but received {received}"
            ),
            Self::TerminalContextMismatch { expected, received } => write!(
                formatter,
                "expected terminal context {expected} but received {received}"
            ),
            Self::StartedGeometryMismatch { expected, received } => write!(
                formatter,
                "expected started geometry {expected:?} but received {received:?}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchTabCreateOutcomeUnknownReason {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse
            | Self::CorrelationMismatch { .. }
            | Self::ActionIncompatible
            | Self::ExpectedRevisionOverflow { .. }
            | Self::CommittedRevisionMismatch { .. }
            | Self::TerminalContextMismatch { .. }
            | Self::StartedGeometryMismatch { .. } => None,
        }
    }
}

fn classify_tab_create_outcome(
    expected_revision: WorkbenchRevision,
    expected_terminal_context_id: &WorkbenchTerminalContextId,
    expected_geometry: WorkbenchTerminalGeometry,
    placement: Option<&orchbee_workbench_protocol::TabCreationPlacement>,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchTabCreateSubmission, WorkbenchTabCreateOutcomeUnknownReason> {
    match outcome {
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if tab_create_rejection_is_compatible(reason, placement) =>
        {
            Ok(WorkbenchTabCreateSubmission::DefinitePrecommit(
                WorkbenchTabCreatePrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => Ok(WorkbenchTabCreateSubmission::DefinitePrecommit(
            WorkbenchTabCreatePrecommit::Denied,
        )),
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::NoCompatibleWorker
                    | WorkbenchCommandUnavailableReason::ServerBusy
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchTabCreateSubmission::DefinitePrecommit(
                WorkbenchTabCreatePrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::LaunchProfileUnavailable {} => {
            Ok(WorkbenchTabCreateSubmission::DefinitePrecommit(
                WorkbenchTabCreatePrecommit::ProfileUnavailable,
            ))
        }
        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids } => {
            Ok(WorkbenchTabCreateSubmission::DefinitePrecommit(
                WorkbenchTabCreatePrecommit::ProfileAmbiguous { candidate_ids },
            ))
        }
        WorkbenchCommandOutcome::TabCreatePending {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
        } => {
            validate_committed_tab_fields(
                expected_revision,
                expected_terminal_context_id,
                revision,
                &terminal_context_id,
            )?;
            Ok(WorkbenchTabCreateSubmission::Committed(
                WorkbenchTabCreateCommit::Pending {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id,
                },
            ))
        }
        WorkbenchCommandOutcome::TabCreated {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            terminal_outcome,
        } => {
            validate_committed_tab_fields(
                expected_revision,
                expected_terminal_context_id,
                revision,
                &terminal_context_id,
            )?;
            if let WorkbenchTerminalOutcome::Started { geometry } = terminal_outcome
                && geometry != expected_geometry
            {
                return Err(
                    WorkbenchTabCreateOutcomeUnknownReason::StartedGeometryMismatch {
                        expected: expected_geometry,
                        received: geometry,
                    },
                );
            }
            Ok(WorkbenchTabCreateSubmission::Committed(
                WorkbenchTabCreateCommit::Created {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id,
                    terminal_outcome,
                },
            ))
        }
        _ => Err(WorkbenchTabCreateOutcomeUnknownReason::ActionIncompatible),
    }
}

fn tab_create_rejection_is_compatible(
    reason: WorkbenchRejectionReason,
    placement: Option<&orchbee_workbench_protocol::TabCreationPlacement>,
) -> bool {
    use orchbee_workbench_protocol::TabCreationPlacement;
    if matches!(
        (reason, placement),
        (
            WorkbenchRejectionReason::TabGroupUnavailable,
            Some(TabCreationPlacement::InGroup { .. })
        ) | (
            WorkbenchRejectionReason::TabUnavailable,
            Some(TabCreationPlacement::Duplicate { .. } | TabCreationPlacement::AfterTab { .. })
        ) | (
            WorkbenchRejectionReason::TabMoveOrderConflict,
            Some(TabCreationPlacement::AfterTab { .. })
        )
    ) {
        return true;
    }
    matches!(
        reason,
        WorkbenchRejectionReason::SiteMismatch
            | WorkbenchRejectionReason::RevisionMismatch
            | WorkbenchRejectionReason::TerminalContextUnavailable
            | WorkbenchRejectionReason::WorkspaceDirectoryUnavailable
            | WorkbenchRejectionReason::ResourceLimit
            | WorkbenchRejectionReason::IdentityExhausted
            | WorkbenchRejectionReason::RevisionExhausted
            | WorkbenchRejectionReason::RequestIdentityConflict
    )
}

fn validate_committed_tab_fields(
    expected_revision: WorkbenchRevision,
    expected_terminal_context_id: &WorkbenchTerminalContextId,
    received_revision: WorkbenchRevision,
    received_terminal_context_id: &WorkbenchTerminalContextId,
) -> Result<(), WorkbenchTabCreateOutcomeUnknownReason> {
    let Some(expected_value) = expected_revision.value().checked_add(1) else {
        return Err(
            WorkbenchTabCreateOutcomeUnknownReason::ExpectedRevisionOverflow {
                expected: expected_revision,
            },
        );
    };
    let expected_next_revision = WorkbenchRevision::new(expected_value);
    if received_revision != expected_next_revision {
        return Err(
            WorkbenchTabCreateOutcomeUnknownReason::CommittedRevisionMismatch {
                expected: expected_next_revision,
                received: received_revision,
            },
        );
    }
    if received_terminal_context_id != expected_terminal_context_id {
        return Err(
            WorkbenchTabCreateOutcomeUnknownReason::TerminalContextMismatch {
                expected: expected_terminal_context_id.clone(),
                received: received_terminal_context_id.clone(),
            },
        );
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum SnapshotReadError {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
    Failed {
        request_id: WorkbenchRequestId,
        reason: SnapshotReadFailure,
    },
}

impl fmt::Display for SnapshotReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => {
                formatter.write_str("local Client Connection is unavailable")
            }
            Self::RequestEncoding(error) => {
                write!(formatter, "OBWP snapshot request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                formatter,
                "OBWP snapshot request failed before any transport handoff: {error}"
            ),
            Self::Failed { request_id, reason } => {
                write!(
                    formatter,
                    "OBWP snapshot request {request_id} failed: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for SnapshotReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
            Self::Failed { reason, .. } => reason.source(),
            Self::ConnectionUnavailable => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum SnapshotReadFailure {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
}

impl fmt::Display for SnapshotReadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Response(error) => write!(formatter, "invalid snapshot response: {error}"),
            Self::ResponseDeadline(error) => write!(formatter, "snapshot response failed: {error}"),
            Self::UnexpectedResponse => {
                formatter.write_str("expected workbench.snapshot.result response")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                formatter,
                "expected Workbench request identity {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for SnapshotReadFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse | Self::CorrelationMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorktreeListReadError {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
    Failed {
        request_id: WorkbenchRequestId,
        reason: WorktreeListReadFailure,
    },
}

impl fmt::Display for WorktreeListReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => {
                formatter.write_str("local Client Connection is unavailable")
            }
            Self::RequestEncoding(error) => {
                write!(
                    formatter,
                    "OBWP worktree-list request encoding failed: {error}"
                )
            }
            Self::RequestNotSubmitted(error) => write!(
                formatter,
                "OBWP worktree-list request failed before any transport handoff: {error}"
            ),
            Self::Failed { request_id, reason } => {
                write!(
                    formatter,
                    "OBWP worktree-list request {request_id} failed: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for WorktreeListReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
            Self::Failed { reason, .. } => reason.source(),
            Self::ConnectionUnavailable => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorktreeListReadFailure {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
}

impl fmt::Display for WorktreeListReadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Response(error) => {
                write!(formatter, "invalid worktree-list response: {error}")
            }
            Self::ResponseDeadline(error) => {
                write!(formatter, "worktree-list response failed: {error}")
            }
            Self::UnexpectedResponse => {
                formatter.write_str("expected workbench.worktree-list.result response")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                formatter,
                "expected Workbench request identity {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for WorktreeListReadFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse | Self::CorrelationMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum PublicationReadError {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
    Failed {
        request_id: WorkbenchRequestId,
        reason: PublicationReadFailure,
    },
}

impl fmt::Display for PublicationReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => {
                formatter.write_str("local Client Connection is unavailable")
            }
            Self::RequestEncoding(error) => {
                write!(
                    formatter,
                    "OBWP publication request encoding failed: {error}"
                )
            }
            Self::RequestNotSubmitted(error) => write!(
                formatter,
                "OBWP publication request failed before any transport handoff: {error}"
            ),
            Self::Failed { request_id, reason } => {
                write!(
                    formatter,
                    "OBWP publication request {request_id} failed: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for PublicationReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
            Self::Failed { reason, .. } => reason.source(),
            Self::ConnectionUnavailable => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum PublicationReadFailure {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
}

impl fmt::Display for PublicationReadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Response(error) => write!(formatter, "invalid publication response: {error}"),
            Self::ResponseDeadline(error) => {
                write!(formatter, "publication response failed: {error}")
            }
            Self::UnexpectedResponse => {
                formatter.write_str("expected workbench.publication.result response")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                formatter,
                "expected Workbench request identity {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for PublicationReadFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse | Self::CorrelationMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
