use super::{
    CloseExchangeError, WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted,
    validate_correlation, validate_target,
};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchTabCloseResume,
    WorkbenchTabCloseResumeOutcome,
};
use std::time::Instant;

#[derive(Debug)]
pub(crate) enum WorkbenchTabCloseResumeSubmission {
    DefinitelyNotSubmitted(WorkbenchTabCloseNotSubmitted),
    Returned(WorkbenchTabCloseResumeOutcome),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchTabCloseFailure,
    },
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
            Err(CloseExchangeError::Failed(reason)) => {
                return WorkbenchTabCloseResumeSubmission::OutcomeUnknown {
                    request_id: request.request_id,
                    reason,
                };
            }
        };
        match validate_resume_result(&request, response) {
            Ok(outcome) => WorkbenchTabCloseResumeSubmission::Returned(outcome),
            Err(reason) => {
                self.connection.take();
                WorkbenchTabCloseResumeSubmission::OutcomeUnknown {
                    request_id: request.request_id,
                    reason,
                }
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
    if request.site_id != result.site_id {
        return Err(WorkbenchTabCloseFailure::SiteMismatch {
            expected: request.site_id.clone(),
            received: result.site_id,
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
        _ => {}
    }
    // A fixed Resumed receipt describes its submitted attempt, not the operation's current generation.
    Ok(result.outcome)
}
