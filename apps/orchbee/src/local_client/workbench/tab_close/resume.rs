//! Submits one explicit generation-fenced Tab close resume and validates its correlated scope
//! and result. It does not choose whether resumption is appropriate or retry a possibly
//! delivered resume.

use super::{
    CloseExchangeError, WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted,
    validate_correlation, validate_target,
};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
};
use std::time::Instant;

#[derive(Debug)]
pub(crate) enum WorkbenchTabCloseResumeSubmission {
    DefinitelyNotSubmitted(WorkbenchTabCloseNotSubmitted),
    Returned(WorkbenchTabCloseResumeOutcome),
    OutcomeUnknown,
}

impl LocalClient {
    pub(crate) fn submit_workbench_tab_close_resume(
        &mut self,
        request: WorkbenchTabCloseResume,
        deadline: Instant,
    ) -> WorkbenchTabCloseResumeSubmission {
        let response = match self
            .exchange_tab_close(WorkbenchRequest::TabCloseResume(request.clone()), deadline)
        {
            Ok(response) => response,
            Err(CloseExchangeError::NotSubmitted(reason)) => {
                return WorkbenchTabCloseResumeSubmission::DefinitelyNotSubmitted(reason);
            }
            Err(CloseExchangeError::Failed(_)) => {
                return WorkbenchTabCloseResumeSubmission::OutcomeUnknown;
            }
        };
        match validate_resume_result(&request, response) {
            Ok(outcome) => WorkbenchTabCloseResumeSubmission::Returned(outcome),
            Err(_) => {
                self.connection.take();
                WorkbenchTabCloseResumeSubmission::OutcomeUnknown
            }
        }
    }
}

fn validate_resume_result(
    request: &WorkbenchTabCloseResume,
    response: WorkbenchResponse,
) -> Result<WorkbenchTabCloseResumeOutcome, WorkbenchTabCloseFailure> {
    let WorkbenchResponse::TabCloseResume(result) = response else {
        return Err(WorkbenchTabCloseFailure::UnexpectedResponse);
    };
    validate_correlation(&request.request_id, &result.request_id)?;
    if request.session_id != result.session_id {
        return Err(WorkbenchTabCloseFailure::SessionMismatch {
            expected: request.session_id.clone(),
            received: result.session_id,
        });
    }
    validate_target(
        &request.terminal_context_id,
        &request.tab_id,
        &result.terminal_context_id,
        &result.tab_id,
    )?;
    if request.close_request_id != result.close_request_id {
        return Err(WorkbenchTabCloseFailure::OperationMismatch {
            expected: request.close_request_id.clone(),
            received: result.close_request_id,
        });
    }
    let expected = request.expected_attempt_generation;
    match &result.outcome {
        WorkbenchTabCloseResumeOutcome::Resumed { attempt_generation }
            if expected.checked_next() != Some(*attempt_generation) =>
        {
            return Err(WorkbenchTabCloseFailure::AttemptGenerationMismatch {
                expected,
                received: *attempt_generation,
            });
        }
        WorkbenchTabCloseResumeOutcome::Stale { attempt_generation }
            if *attempt_generation <= expected =>
        {
            return Err(WorkbenchTabCloseFailure::AttemptGenerationMismatch {
                expected,
                received: *attempt_generation,
            });
        }
        WorkbenchTabCloseResumeOutcome::Resumed { .. }
        | WorkbenchTabCloseResumeOutcome::Stale { .. }
        | WorkbenchTabCloseResumeOutcome::Denied { .. }
        | WorkbenchTabCloseResumeOutcome::Unavailable { .. }
        | WorkbenchTabCloseResumeOutcome::Rejected { .. } => {}
    }
    // A fixed Resumed receipt describes its submitted attempt, not the operation's current generation.
    Ok(result.outcome)
}
