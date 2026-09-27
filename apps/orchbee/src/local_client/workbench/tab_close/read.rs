use super::{
    CloseExchangeError, WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted,
    validate_correlation, validate_target,
};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadOutcome,
};
use std::{fmt, time::Instant};

impl LocalClient {
    pub(crate) fn submit_workbench_tab_close_read(
        &mut self,
        request: WorkbenchTabCloseRead,
        expected_close_request_id: Option<&WorkbenchRequestId>,
        deadline: Instant,
    ) -> Result<WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadError> {
        self.submit_workbench_tab_close_read_with_wake(
            request,
            expected_close_request_id,
            deadline,
            None,
        )
    }

    /// Read retained Close state with an optional cancellation wake and exact operation correlation.
    ///
    /// # Errors
    /// Distinguishes a request not submitted from a failed observation; neither erases known Close facts.
    pub(crate) fn submit_workbench_tab_close_read_with_wake(
        &mut self,
        request: WorkbenchTabCloseRead,
        expected_close_request_id: Option<&WorkbenchRequestId>,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> Result<WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadError> {
        let response = match self.exchange_tab_close_with_wake(
            WorkbenchRequest::TabCloseRead(request.clone()),
            deadline,
            wake,
        ) {
            Ok(response) => response,
            Err(CloseExchangeError::NotSubmitted(reason)) => {
                return Err(WorkbenchTabCloseReadError::NotSubmitted(reason));
            }
            Err(CloseExchangeError::Failed(reason)) => {
                return Err(WorkbenchTabCloseReadError::Failed {
                    request_id: request.request_id,
                    reason,
                });
            }
        };
        match validate_read_result(&request, expected_close_request_id, response) {
            Ok(outcome) => Ok(outcome),
            Err(reason) => {
                self.connection.take();
                Err(WorkbenchTabCloseReadError::Failed {
                    request_id: request.request_id,
                    reason,
                })
            }
        }
    }
}

fn validate_read_result(
    request: &WorkbenchTabCloseRead,
    expected_close_request_id: Option<&WorkbenchRequestId>,
    response: WorkbenchResponse,
) -> Result<WorkbenchTabCloseReadOutcome, WorkbenchTabCloseFailure> {
    let WorkbenchResponse::TabCloseRead(result) = response else {
        return Err(WorkbenchTabCloseFailure::UnexpectedResponse);
    };
    validate_correlation(&request.request_id, &result.request_id)?;
    if let WorkbenchTabCloseReadOutcome::Available {
        close_request_id,
        terminal_context_id,
        tab_id,
        ..
    } = &result.outcome
    {
        validate_target(
            &request.terminal_context_id,
            &request.tab_id,
            terminal_context_id,
            tab_id,
        )?;
        if let Some(expected) = expected_close_request_id
            && expected != close_request_id
        {
            return Err(WorkbenchTabCloseFailure::OperationMismatch {
                expected: expected.clone(),
                received: close_request_id.clone(),
            });
        }
    }
    // NotClosing and unavailable observations do not erase caller-owned known Close facts.
    Ok(result.outcome)
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabCloseReadError {
    NotSubmitted(WorkbenchTabCloseNotSubmitted),
    Failed {
        request_id: WorkbenchRequestId,
        reason: WorkbenchTabCloseFailure,
    },
}
impl fmt::Display for WorkbenchTabCloseReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSubmitted(reason) => {
                write!(f, "Close status read was not submitted: {reason}")
            }
            Self::Failed { request_id, reason } => {
                write!(f, "Close status read {request_id} failed: {reason}")
            }
        }
    }
}
impl std::error::Error for WorkbenchTabCloseReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotSubmitted(reason) => Some(reason),
            Self::Failed { reason, .. } => Some(reason),
        }
    }
}
