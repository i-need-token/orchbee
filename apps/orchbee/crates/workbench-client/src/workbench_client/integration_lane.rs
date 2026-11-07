//! Agent integration jobs on the Session's shared carrier, as the GUI runs them: start one job,
//! poll it from 20 ms up to every 250 ms, and after 50 seconds ask the Worker to stop it. Polling
//! never blocks the UI tick, and nothing here retries a job or decides its effects.
use crate::{
    local_client::{LocalClientOrigin, LocalOperationClock, subscription::Request},
    native_settings::{IntegrationJob, JobEnd},
};
use orchbee_agent_integration::IntegrationRequest;
use orchbee_workbench_protocol::{
    AgentIntegrationAction as Action, AgentIntegrationFailure as Failure,
    AgentIntegrationJob as Job, AgentIntegrationOutcome as Outcome, AgentIntegrationRequest,
    ProtocolVersion, SessionId, WorkbenchRequest, WorkbenchResponse,
};
use std::time::{Duration, Instant};

const EXCHANGE: Duration = Duration::from_secs(5);
/// The Worker ends an operation within 40 s; like the CLI and GUI, a job gets 50 s.
const JOB: Duration = Duration::from_secs(50);
const FIRST_POLL: Duration = Duration::from_millis(20);
const POLL_CEILING: Duration = Duration::from_millis(250);

/// One correlated request; only its own answer settles it.
struct Exchange {
    request: AgentIntegrationRequest,
    pending: Request,
}

enum Exchanged {
    Answered(Outcome),
    Lost { submitted: bool },
}

impl Exchange {
    fn poll(&self) -> Option<Exchanged> {
        Some(match self.pending.poll()? {
            Ok(WorkbenchResponse::AgentIntegration(result))
                if result.answers(&self.request) && result.validate().is_ok() =>
            {
                Exchanged::Answered(result.outcome)
            }
            // A result this Client cannot match leaves the request's outcome unknown.
            Ok(_) => Exchanged::Lost { submitted: true },
            Err(submitted) => Exchanged::Lost { submitted },
        })
    }
}

enum Phase {
    Starting(Exchange),
    Waiting {
        job: Job,
        at: Instant,
        pause: Duration,
    },
    Polling {
        job: Job,
        pause: Duration,
        exchange: Exchange,
    },
}

pub(super) struct Lane {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    session: SessionId,
    running: Option<(Instant, Phase)>,
    /// Cancellations sent past a job's deadline stay alive until answered or expired.
    cancels: Vec<Request>,
}

impl Lane {
    pub(super) fn new(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session: SessionId,
    ) -> Self {
        Self {
            origin,
            clock,
            session,
            running: None,
            cancels: Vec::new(),
        }
    }

    pub(super) const fn session(&self) -> &SessionId {
        &self.session
    }

    /// Starts one job; a request that cannot be sent settles at once as never submitted.
    pub(super) fn start(&mut self, job: IntegrationJob) -> Option<JobEnd> {
        if self.running.is_some() {
            return Some(JobEnd::Failed {
                reason: Failure::Busy,
                started: false,
            });
        }
        let deadline = self.clock.now() + JOB;
        let action = Action::Start {
            request: IntegrationRequest {
                operation: job.operation(),
                adapter_source: None,
                budget_milliseconds: None,
            },
        };
        match self.send(action, deadline) {
            Some(exchange) => {
                self.running = Some((deadline, Phase::Starting(exchange)));
                None
            }
            None => Some(JobEnd::Exchange {
                started: false,
                submitted: false,
            }),
        }
    }

    pub(super) fn poll(&mut self) -> Option<JobEnd> {
        self.cancels.retain(|cancel| cancel.poll().is_none());
        let (deadline, phase) = self.running.take()?;
        match self.advance(deadline, phase) {
            Ok(phase) => {
                self.running = Some((deadline, phase));
                None
            }
            Err(end) => Some(end),
        }
    }

    /// The Client stops following its job, which keeps running on the Worker.
    pub(super) fn abandon(&mut self) -> Option<JobEnd> {
        let (_, phase) = self.running.take()?;
        Some(JobEnd::Exchange {
            started: !matches!(phase, Phase::Starting(_)),
            submitted: true,
        })
    }

    fn advance(&mut self, deadline: Instant, phase: Phase) -> Result<Phase, JobEnd> {
        match phase {
            Phase::Starting(exchange) => match exchange.poll() {
                None => Ok(Phase::Starting(exchange)),
                Some(Exchanged::Lost { submitted }) => Err(JobEnd::Exchange {
                    started: false,
                    submitted,
                }),
                Some(Exchanged::Answered(Outcome::Accepted { job })) => Ok(Phase::Waiting {
                    job,
                    at: (self.clock.now() + FIRST_POLL).min(deadline),
                    pause: FIRST_POLL,
                }),
                Some(Exchanged::Answered(Outcome::Failed { reason })) => Err(JobEnd::Failed {
                    reason,
                    started: false,
                }),
                Some(Exchanged::Answered(_)) => Err(JobEnd::Failed {
                    reason: Failure::Malformed,
                    started: false,
                }),
            },
            Phase::Waiting { job, at, pause } => {
                let now = self.clock.now();
                if now >= deadline {
                    self.cancel(job);
                    return Err(JobEnd::Deadline);
                }
                if now < at {
                    return Ok(Phase::Waiting { job, at, pause });
                }
                match self.send(Action::Poll { job: job.clone() }, deadline) {
                    Some(exchange) => Ok(Phase::Polling {
                        job,
                        pause,
                        exchange,
                    }),
                    None => Err(JobEnd::Exchange {
                        started: true,
                        submitted: false,
                    }),
                }
            }
            Phase::Polling {
                job,
                pause,
                exchange,
            } => match exchange.poll() {
                None => Ok(Phase::Polling {
                    job,
                    pause,
                    exchange,
                }),
                Some(Exchanged::Lost { submitted }) => Err(JobEnd::Exchange {
                    started: true,
                    submitted,
                }),
                Some(Exchanged::Answered(Outcome::Pending { .. })) => {
                    let pause = (pause * 2).min(POLL_CEILING);
                    Ok(Phase::Waiting {
                        job,
                        at: (self.clock.now() + pause).min(deadline),
                        pause,
                    })
                }
                Some(Exchanged::Answered(Outcome::Completed { report, .. })) => {
                    Err(JobEnd::Completed(report))
                }
                Some(Exchanged::Answered(Outcome::Failed { reason })) => Err(JobEnd::Failed {
                    reason,
                    started: true,
                }),
                Some(Exchanged::Answered(_)) => Err(JobEnd::Failed {
                    reason: Failure::Malformed,
                    started: true,
                }),
            },
        }
    }

    /// Asks the Worker to stop a job past its deadline; the answer changes nothing here.
    fn cancel(&mut self, job: Job) {
        let deadline = self.clock.now() + EXCHANGE;
        if let Some(exchange) = self.send(Action::Cancel { job }, deadline) {
            self.cancels.push(exchange.pending);
        }
    }

    fn send(&self, action: Action, deadline: Instant) -> Option<Exchange> {
        let request = AgentIntegrationRequest {
            protocol_version: ProtocolVersion,
            request_id: crate::machine_observation::request_id().ok()?,
            session_id: self.session.clone(),
            action,
        };
        let pending = self
            .origin
            .request(
                &WorkbenchRequest::AgentIntegration(request.clone()),
                (self.clock.now() + EXCHANGE).min(deadline),
                self.clock.clone(),
            )
            .ok()?;
        Some(Exchange { request, pending })
    }
}
