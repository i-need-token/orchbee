//! One-shot directory Workspace launch. Unknown outcomes retire the connection;
//! a committed pending or failed terminal retains its structural identities.
use super::{
    WorkbenchTabCreateNotSubmitted as NotSubmitted,
    WorkbenchTabCreateOutcomeUnknownReason as Unknown, WorkbenchTabCreatePrecommit as Precommit,
};
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    TerminalPresentationId, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandUnavailableReason, WorkbenchDirectoryId, WorkbenchPanelId, WorkbenchProjectId,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalOutcome,
    WorkbenchWorkspaceId, read_workbench_response, write_workbench_request,
};
use std::time::Instant;

pub(crate) struct WorkspaceLaunchRequest(WorkbenchCommand);
impl TryFrom<WorkbenchCommand> for WorkspaceLaunchRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::WorkspaceLaunch { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkspaceLaunchSubmission {
    DefinitelyNotSubmitted(NotSubmitted),
    DefinitePrecommit(Precommit),
    Committed(WorkspaceLaunchCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: Unknown,
    },
}

#[derive(Debug)]
pub(crate) struct WorkspaceLaunchCommit {
    pub(crate) revision: WorkbenchRevision,
    pub(crate) project_id: WorkbenchProjectId,
    pub(crate) workspace_id: WorkbenchWorkspaceId,
    pub(crate) directory_id: WorkbenchDirectoryId,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) tab_id: WorkbenchTabId,
    pub(crate) panel_id: WorkbenchPanelId,
    pub(crate) presentation_id: TerminalPresentationId,
    /// None is a known committed creation awaiting terminal settlement.
    pub(crate) terminal_outcome: Option<WorkbenchTerminalOutcome>,
}

impl LocalClient {
    pub(crate) fn submit_workspace_launch(
        &mut self,
        request: WorkspaceLaunchRequest,
        deadline: Instant,
    ) -> WorkspaceLaunchSubmission {
        self.submit_workspace_launch_with_wake(request, deadline, None)
    }
    pub(crate) fn submit_workspace_launch_with_wake(
        &mut self,
        request: WorkspaceLaunchRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkspaceLaunchSubmission {
        if self.connection.is_none() {
            return WorkspaceLaunchSubmission::DefinitelyNotSubmitted(
                NotSubmitted::SessionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let action = command.action.clone();
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkspaceLaunchSubmission::DefinitelyNotSubmitted(
                NotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkspaceLaunchSubmission::DefinitelyNotSubmitted(
                    NotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkspaceLaunchSubmission::DefinitelyNotSubmitted(
                NotSubmitted::SessionUnavailable,
            );
        };
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return WorkspaceLaunchSubmission::DefinitelyNotSubmitted(
                    NotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self
                    .retire_with_workspace_launch_unknown(request_id, Unknown::Write(error));
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self
                    .retire_with_workspace_launch_unknown(request_id, Unknown::Response(error));
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_workspace_launch_unknown(
                    request_id,
                    Unknown::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self
                .retire_with_workspace_launch_unknown(request_id, Unknown::UnexpectedResponse);
        };
        if result.request_id != request_id {
            return self.retire_with_workspace_launch_unknown(
                request_id.clone(),
                Unknown::CorrelationMismatch {
                    expected: request_id,
                    received: result.request_id,
                },
            );
        }
        match classify_outcome(&action, expected_revision, result.outcome) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_workspace_launch_unknown(request_id, reason),
        }
    }

    fn retire_with_workspace_launch_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: Unknown,
    ) -> WorkspaceLaunchSubmission {
        self.connection.take();
        WorkspaceLaunchSubmission::OutcomeUnknown { request_id, reason }
    }
}

fn classify_outcome(
    action: &WorkbenchAction,
    expected_revision: WorkbenchRevision,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkspaceLaunchSubmission, Unknown> {
    let WorkbenchAction::WorkspaceLaunch {
        project_id: expected_project,
        geometry: expected_geometry,
        ..
    } = action
    else {
        return Err(Unknown::ActionIncompatible);
    };
    let commit = match outcome {
        WorkbenchCommandOutcome::WorkspaceLaunchPending {
            revision,
            project_id,
            workspace_id,
            directory_id,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
        } => WorkspaceLaunchCommit {
            revision,
            project_id,
            workspace_id,
            directory_id,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            terminal_outcome: None,
        },
        WorkbenchCommandOutcome::WorkspaceLaunched {
            revision,
            project_id,
            workspace_id,
            directory_id,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            terminal_outcome,
        } => WorkspaceLaunchCommit {
            revision,
            project_id,
            workspace_id,
            directory_id,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            terminal_outcome: Some(terminal_outcome),
        },
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if matches!(
                reason,
                WorkbenchRejectionReason::SiteMismatch
                    | WorkbenchRejectionReason::RevisionMismatch
                    | WorkbenchRejectionReason::ProjectUnavailable
                    | WorkbenchRejectionReason::ResourceLimit
                    | WorkbenchRejectionReason::IdentityExhausted
                    | WorkbenchRejectionReason::RevisionExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) =>
        {
            return Ok(WorkspaceLaunchSubmission::DefinitePrecommit(
                Precommit::Rejected { revision, reason },
            ));
        }
        WorkbenchCommandOutcome::Denied {} => {
            return Ok(WorkspaceLaunchSubmission::DefinitePrecommit(
                Precommit::Denied,
            ));
        }
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::NoCompatibleWorker
                    | WorkbenchCommandUnavailableReason::ServerBusy
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
                    | WorkbenchCommandUnavailableReason::DirectoryUnavailable
            ) =>
        {
            return Ok(WorkspaceLaunchSubmission::DefinitePrecommit(
                Precommit::Unavailable { reason },
            ));
        }
        WorkbenchCommandOutcome::LaunchProfileUnavailable {} => {
            return Ok(WorkspaceLaunchSubmission::DefinitePrecommit(
                Precommit::ProfileUnavailable,
            ));
        }
        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids } => {
            return Ok(WorkspaceLaunchSubmission::DefinitePrecommit(
                Precommit::ProfileAmbiguous { candidate_ids },
            ));
        }
        _ => return Err(Unknown::ActionIncompatible),
    };
    let identities = [
        commit.project_id.as_str(),
        commit.workspace_id.as_str(),
        commit.directory_id.as_str(),
        commit.terminal_context_id.as_str(),
        commit.tab_id.as_str(),
        commit.panel_id.as_str(),
        commit.presentation_id.as_str(),
    ];
    if identities
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != identities.len()
    {
        return Err(Unknown::ActionIncompatible);
    }
    if &commit.project_id != expected_project {
        return Err(Unknown::ActionIncompatible);
    }
    let expected_value =
        expected_revision
            .value()
            .checked_add(1)
            .ok_or(Unknown::ExpectedRevisionOverflow {
                expected: expected_revision,
            })?;
    if commit.revision.value() != expected_value {
        return Err(Unknown::CommittedRevisionMismatch {
            expected: WorkbenchRevision::new(expected_value),
            received: commit.revision,
        });
    }
    if let Some(WorkbenchTerminalOutcome::Started { geometry }) = &commit.terminal_outcome
        && geometry != expected_geometry
    {
        return Err(Unknown::StartedGeometryMismatch {
            expected: *expected_geometry,
            received: *geometry,
        });
    }
    Ok(WorkspaceLaunchSubmission::Committed(commit))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn action() -> WorkbenchAction {
        WorkbenchAction::WorkspaceLaunch {
            project_id: WorkbenchProjectId::parse(&format!("{:032x}", 1)).unwrap(),
            label: orchbee_workbench_protocol::WorkbenchLabel::parse("test").unwrap(),
            geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(80, 24, 0, 0)
                .unwrap(),
            launch: None,
            profile: None,
            options: Box::new(orchbee_workbench_protocol::LaunchOptions {
                plugin_pane: None,
                plugin_context: None,
                cwd: Some(
                    orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(r"C:\repo").unwrap(),
                ),
                environment: Default::default(),
            }),
        }
    }
    fn outcome() -> WorkbenchCommandOutcome {
        WorkbenchCommandOutcome::WorkspaceLaunchPending {
            revision: WorkbenchRevision::new(2),
            project_id: WorkbenchProjectId::parse(&format!("{:032x}", 1)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 2)).unwrap(),
            directory_id: WorkbenchDirectoryId::parse(&format!("{:032x}", 3)).unwrap(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 4)).unwrap(),
            tab_id: WorkbenchTabId::parse(&format!("{:032x}", 5)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 6)).unwrap(),
            presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 7)).unwrap(),
        }
    }
    #[test]
    fn workspace_launch_only_accepts_correlated_unique_committed_resources() {
        let action = action();
        let revision = WorkbenchRevision::new(1);
        assert!(matches!(
            classify_outcome(&action, revision, outcome()),
            Ok(WorkspaceLaunchSubmission::Committed(_))
        ));
        for field in ["projectId", "workspaceId", "revision"] {
            let mut wire = serde_json::to_value(outcome()).unwrap();
            wire[field] = if field == "revision" {
                serde_json::json!("3")
            } else {
                serde_json::json!(format!("{:032x}", 7))
            };
            assert!(
                classify_outcome(&action, revision, serde_json::from_value(wire).unwrap()).is_err()
            );
        }
        assert!(
            classify_outcome(
                &action,
                revision,
                WorkbenchCommandOutcome::WorkspaceCreated {
                    revision: WorkbenchRevision::new(2),
                    workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 2)).unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 4))
                        .unwrap(),
                }
            )
            .is_err()
        );
        assert!(matches!(
            classify_outcome(
                &action,
                revision,
                WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::DirectoryUnavailable,
                }
            ),
            Ok(WorkspaceLaunchSubmission::DefinitePrecommit(_))
        ));
    }
}
