//! Submits one explicit generation-fenced Workspace close resume and validates the returned
//! scope and outcome. It does not choose a new close scope or replay a request whose delivery
//! is uncertain.

use super::{
    WorkbenchWorkspaceCloseFailure, WorkbenchWorkspaceCloseNotSubmitted,
    WorkspaceCloseExchangeError, validate_correlation, validate_scope,
};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome,
};
use std::time::Instant;

#[derive(Debug)]
pub(crate) enum WorkbenchWorkspaceCloseResumeSubmission {
    DefinitelyNotSubmitted(WorkbenchWorkspaceCloseNotSubmitted),
    Returned(WorkbenchWorkspaceCloseResumeOutcome),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchWorkspaceCloseFailure,
    },
}

impl LocalClient {
    pub(crate) fn submit_workbench_workspace_close_resume(
        &mut self,
        request: WorkbenchWorkspaceCloseResume,
        deadline: Instant,
    ) -> WorkbenchWorkspaceCloseResumeSubmission {
        let response = match self.exchange_workspace_close(
            WorkbenchRequest::WorkspaceCloseResume(request.clone()),
            deadline,
        ) {
            Ok(response) => response,
            Err(WorkspaceCloseExchangeError::NotSubmitted(reason)) => {
                return WorkbenchWorkspaceCloseResumeSubmission::DefinitelyNotSubmitted(reason);
            }
            Err(WorkspaceCloseExchangeError::Failed(reason)) => {
                return WorkbenchWorkspaceCloseResumeSubmission::OutcomeUnknown {
                    request_id: request.request_id,
                    reason,
                };
            }
        };
        match validate_resume_result(&request, response) {
            Ok(outcome) => WorkbenchWorkspaceCloseResumeSubmission::Returned(outcome),
            Err(reason) => {
                self.connection.take();
                WorkbenchWorkspaceCloseResumeSubmission::OutcomeUnknown {
                    request_id: request.request_id,
                    reason,
                }
            }
        }
    }
}

fn validate_resume_result(
    request: &WorkbenchWorkspaceCloseResume,
    response: WorkbenchResponse,
) -> Result<WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceCloseFailure> {
    let WorkbenchResponse::WorkspaceCloseResume(result) = response else {
        return Err(WorkbenchWorkspaceCloseFailure::UnexpectedResponse);
    };
    validate_correlation(&request.request_id, &result.request_id)?;
    if request.session_id != result.session_id {
        return Err(WorkbenchWorkspaceCloseFailure::SessionMismatch {
            expected: request.session_id.clone(),
            received: result.session_id,
        });
    }
    validate_scope(&request.workspace_id, &result.workspace_id)?;
    if request.close_request_id != result.close_request_id {
        return Err(WorkbenchWorkspaceCloseFailure::OperationMismatch {
            expected: request.close_request_id.clone(),
            received: result.close_request_id,
        });
    }
    let expected = request.expected_attempt_generation;
    match &result.outcome {
        WorkbenchWorkspaceCloseResumeOutcome::Resumed { attempt_generation }
            if expected.checked_next() != Some(*attempt_generation) =>
        {
            return Err(WorkbenchWorkspaceCloseFailure::AttemptGenerationMismatch {
                expected,
                received: *attempt_generation,
            });
        }
        WorkbenchWorkspaceCloseResumeOutcome::Stale { attempt_generation }
            if *attempt_generation <= expected =>
        {
            return Err(WorkbenchWorkspaceCloseFailure::AttemptGenerationMismatch {
                expected,
                received: *attempt_generation,
            });
        }
        WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
        | WorkbenchWorkspaceCloseResumeOutcome::Stale { .. }
        | WorkbenchWorkspaceCloseResumeOutcome::Denied { .. }
        | WorkbenchWorkspaceCloseResumeOutcome::Unavailable { .. }
        | WorkbenchWorkspaceCloseResumeOutcome::Rejected { .. } => {}
    }
    // A fixed Resumed receipt describes its submitted attempt, not the operation's current generation.
    Ok(result.outcome)
}
