//! Reads retained Workspace close progress and validates its exact correlated scope and
//! member-status result. It does not resume closure or replace the Server's physical cleanup
//! evidence with membership observations.

use super::{
    WorkbenchWorkspaceCloseFailure, WorkbenchWorkspaceCloseNotSubmitted,
    WorkspaceCloseExchangeError, validate_correlation, validate_scope,
};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchWorkspaceCloseRead,
    WorkbenchWorkspaceCloseReadOutcome,
};
use std::{fmt, time::Instant};

impl LocalClient {
    pub(crate) fn submit_workbench_workspace_close_read(
        &mut self,
        request: WorkbenchWorkspaceCloseRead,
        expected_close_request_id: Option<&WorkbenchRequestId>,
        deadline: Instant,
    ) -> Result<WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseReadError> {
        let response = match self.exchange_workspace_close(
            WorkbenchRequest::WorkspaceCloseRead(request.clone()),
            deadline,
        ) {
            Ok(response) => response,
            Err(WorkspaceCloseExchangeError::NotSubmitted(reason)) => {
                return Err(WorkbenchWorkspaceCloseReadError::NotSubmitted(reason));
            }
            Err(WorkspaceCloseExchangeError::Failed(reason)) => {
                return Err(WorkbenchWorkspaceCloseReadError::Failed {
                    request_id: request.request_id,
                    reason,
                });
            }
        };
        match validate_read_result(&request, expected_close_request_id, response) {
            Ok(outcome) => Ok(outcome),
            Err(reason) => {
                self.connection.take();
                Err(WorkbenchWorkspaceCloseReadError::Failed {
                    request_id: request.request_id,
                    reason,
                })
            }
        }
    }
}

fn validate_read_result(
    request: &WorkbenchWorkspaceCloseRead,
    expected_close_request_id: Option<&WorkbenchRequestId>,
    response: WorkbenchResponse,
) -> Result<WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseFailure> {
    let WorkbenchResponse::WorkspaceCloseRead(result) = response else {
        return Err(WorkbenchWorkspaceCloseFailure::UnexpectedResponse);
    };
    validate_correlation(&request.request_id, &result.request_id)?;
    if let WorkbenchWorkspaceCloseReadOutcome::Available {
        close_request_id,
        workspace_id,
        ..
    } = &result.outcome
    {
        validate_scope(&request.workspace_id, workspace_id)?;
        if let Some(expected) = expected_close_request_id
            && expected != close_request_id
        {
            return Err(WorkbenchWorkspaceCloseFailure::OperationMismatch {
                expected: expected.clone(),
                received: close_request_id.clone(),
            });
        }
    }
    // NotClosing and unavailable observations do not erase caller-owned known Close facts.
    Ok(result.outcome)
}

#[derive(Debug)]
pub(crate) enum WorkbenchWorkspaceCloseReadError {
    NotSubmitted(WorkbenchWorkspaceCloseNotSubmitted),
    Failed {
        request_id: WorkbenchRequestId,
        reason: WorkbenchWorkspaceCloseFailure,
    },
}
impl fmt::Display for WorkbenchWorkspaceCloseReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSubmitted(reason) => {
                write!(f, "Workspace Close status read was not submitted: {reason}")
            }
            Self::Failed { request_id, reason } => {
                write!(
                    f,
                    "Workspace Close status read {request_id} failed: {reason}"
                )
            }
        }
    }
}
impl std::error::Error for WorkbenchWorkspaceCloseReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotSubmitted(reason) => Some(reason),
            Self::Failed { reason, .. } => Some(reason),
        }
    }
}
