//! Starts the Server-authorized Agent integration job on the selected Worker and polls it to its
//! report under one deadline, registering cooperative cancellation when the caller is interrupted.
//! It neither inspects provider configuration nor renders reports.
use super::metadata_exchange::{MetadataExchangeError, MetadataExchangeReason};
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_agent_integration::{IntegrationReport, IntegrationRequest};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    AgentIntegrationAction as Action, AgentIntegrationFailure as Failure,
    AgentIntegrationOutcome as Outcome, AgentIntegrationRequest, AgentIntegrationResult,
    ProtocolVersion, SessionId, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    read_workbench_response, write_workbench_request,
};
use std::{
    thread,
    time::{Duration, Instant},
};

const FIRST_POLL: Duration = Duration::from_millis(20);
const POLL_CEILING: Duration = Duration::from_millis(250);

/// How one Agent integration job ended for this Client.
#[derive(Debug)]
pub enum AgentIntegrationJobEnd {
    /// The job's report. `cancel_registered` records that an interruption asked the job to stop.
    Completed {
        report: Box<IntegrationReport>,
        cancel_registered: bool,
    },
    /// The authority answered a closed failure; `started` records that a job had been admitted.
    Failed { reason: Failure, started: bool },
    /// An exchange failed. `submitted` records that the failed request may have reached the
    /// Server; `started` records that a job had been admitted.
    Exchange { submitted: bool, started: bool },
    /// The deadline passed while the admitted job was pending; cancellation was requested.
    Deadline,
}

impl LocalClient {
    /// One correlated `workbench.agent-integration` exchange.
    pub fn agent_integration(
        &mut self,
        request: AgentIntegrationRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<AgentIntegrationResult, MetadataExchangeError> {
        let before = |reason| MetadataExchangeError {
            submitted: false,
            reason,
        };
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::AgentIntegration(request.clone()),
        )
        .map_err(|_| before(MetadataExchangeReason::Encoding))?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| before(MetadataExchangeReason::Deadline))?;
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| before(MetadataExchangeReason::ConnectionUnavailable))?;
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        let reason = match attempt {
            ExchangeAttempt::Result(WorkbenchResponse::AgentIntegration(result))
                if result.answers(&request) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_) => MetadataExchangeReason::Correlation,
            ExchangeAttempt::NotSubmitted(_) => {
                return Err(before(MetadataExchangeReason::NotSubmitted));
            }
            ExchangeAttempt::WriteFailed(_) => MetadataExchangeReason::Write,
            ExchangeAttempt::ReadFailed(
                orchbee_workbench_protocol::WorkbenchWireError::Contract(_)
                | orchbee_workbench_protocol::WorkbenchWireError::InvalidHeader,
            ) => MetadataExchangeReason::Protocol,
            ExchangeAttempt::ReadFailed(orchbee_workbench_protocol::WorkbenchWireError::Io(
                error,
            )) if error.kind() == std::io::ErrorKind::TimedOut => {
                MetadataExchangeReason::ResponseDeadline
            }
            ExchangeAttempt::ReadFailed(_) => MetadataExchangeReason::Read,
            ExchangeAttempt::ReadDeadline(_) => MetadataExchangeReason::ResponseDeadline,
        };
        self.connection = None;
        Err(MetadataExchangeError {
            submitted: true,
            reason,
        })
    }

    /// Starts `request`, then polls the job until its report, a failure, or `deadline`. `wait`
    /// pauses up to the given duration and answers whether the caller was interrupted; the first
    /// interruption registers cancellation, and polling continues to the job's final report.
    /// `request_id` mints one fresh ID per exchange.
    pub fn agent_integration_job(
        &mut self,
        session_id: &SessionId,
        request: IntegrationRequest,
        deadline: Instant,
        mut request_id: impl FnMut() -> Option<WorkbenchRequestId>,
        mut wait: impl FnMut(Duration) -> bool,
    ) -> AgentIntegrationJobEnd {
        let mut exchange = |client: &mut Self, action: Action| {
            let request_id = request_id().ok_or(false)?;
            client
                .agent_integration(
                    AgentIntegrationRequest {
                        protocol_version: ProtocolVersion,
                        request_id,
                        session_id: session_id.clone(),
                        action,
                    },
                    deadline,
                    None,
                )
                .map(|result| result.outcome)
                .map_err(|error| error.submitted)
        };
        let job = match exchange(self, Action::Start { request }) {
            Ok(Outcome::Accepted { job }) => job,
            Ok(Outcome::Failed { reason }) => {
                return AgentIntegrationJobEnd::Failed {
                    reason,
                    started: false,
                };
            }
            // A correlated Start result is either accepted or failed.
            Ok(_) => {
                return AgentIntegrationJobEnd::Failed {
                    reason: Failure::Malformed,
                    started: false,
                };
            }
            Err(submitted) => {
                return AgentIntegrationJobEnd::Exchange {
                    submitted,
                    started: false,
                };
            }
        };
        let mut cancel_registered = false;
        let mut pause = FIRST_POLL;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                if !cancel_registered {
                    let _settled_by_deadline = exchange(self, Action::Cancel { job });
                }
                return AgentIntegrationJobEnd::Deadline;
            }
            let step = pause.min(remaining);
            pause = (pause * 2).min(POLL_CEILING);
            if cancel_registered {
                thread::sleep(step);
            } else if wait(step) {
                cancel_registered = true;
                match exchange(self, Action::Cancel { job: job.clone() }) {
                    Ok(Outcome::CancelRegistered { .. } | Outcome::AlreadyTerminal { .. }) => {}
                    Ok(Outcome::Failed { reason }) => {
                        return AgentIntegrationJobEnd::Failed {
                            reason,
                            started: true,
                        };
                    }
                    Ok(_) => {
                        return AgentIntegrationJobEnd::Failed {
                            reason: Failure::Malformed,
                            started: true,
                        };
                    }
                    Err(submitted) => {
                        return AgentIntegrationJobEnd::Exchange {
                            submitted,
                            started: true,
                        };
                    }
                }
            }
            match exchange(self, Action::Poll { job: job.clone() }) {
                Ok(Outcome::Pending { .. }) => {}
                Ok(Outcome::Completed { report, .. }) => {
                    return AgentIntegrationJobEnd::Completed {
                        report,
                        cancel_registered,
                    };
                }
                Ok(Outcome::Failed { reason }) => {
                    return AgentIntegrationJobEnd::Failed {
                        reason,
                        started: true,
                    };
                }
                Ok(_) => {
                    return AgentIntegrationJobEnd::Failed {
                        reason: Failure::Malformed,
                        started: true,
                    };
                }
                Err(submitted) => {
                    return AgentIntegrationJobEnd::Exchange {
                        submitted,
                        started: true,
                    };
                }
            }
        }
    }
}
