//! Owns the node's one Agent integration job: Worker-generation fencing, one running writer
//! thread at a time, cooperative cancellation, and bounded retention of encoded reports. The
//! writer runs with this Worker's own home directory and `PATH`; this module neither renders
//! reports nor authorizes callers.
use orchbee_agent_integration::{
    Direction, IntegrationOperation, IntegrationReport, IntegrationRequest,
    MAXIMUM_INTEGRATION_BUDGET_MILLISECONDS,
};
use orchbee_agent_integration_writer::TerminationSource;
use orchbee_capability_node_protocol::{
    AgentIntegrationJobAction as Action, AgentIntegrationJobCancelOutcome as Cancel,
    AgentIntegrationJobOutcome as Outcome, AgentIntegrationJobPollOutcome as Poll,
    AgentIntegrationJobRequest, AgentIntegrationJobResult,
    AgentIntegrationJobStartOutcome as Start, OperationId,
};
use orchbee_local_worker_protocol::WorkerId;
use std::{
    collections::VecDeque,
    io,
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Settled jobs retained for polling; admission evicts the oldest settled record first.
const CAPACITY: usize = 8;

/// One encoded contract report, or `None` when the job produced no report the contract can
/// carry.
type Report = Option<Vec<u8>>;
type Execute = fn(IntegrationRequest, Arc<CancelFlag>) -> Report;

/// The cooperative cancellation the writer observes between bounded steps.
#[derive(Default)]
pub(crate) struct CancelFlag(AtomicBool);

impl TerminationSource for CancelFlag {
    fn requested(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

enum State {
    Running {
        cancel: Arc<CancelFlag>,
        join: JoinHandle<Report>,
    },
    Completed(Vec<u8>),
    Abandoned,
}

struct Record {
    id: OperationId,
    request: Vec<u8>,
    state: State,
}

struct Inner {
    records: VecDeque<Record>,
    stopping: bool,
}

pub(crate) struct AgentIntegrationJobs {
    inner: Mutex<Inner>,
    execute: Execute,
}

impl AgentIntegrationJobs {
    pub(crate) fn new() -> Self {
        Self::with_executor(run)
    }

    pub(crate) fn with_executor(execute: Execute) -> Self {
        Self {
            inner: Mutex::new(Inner {
                records: VecDeque::with_capacity(CAPACITY),
                stopping: false,
            }),
            execute,
        }
    }

    /// Fences the request to this Worker generation before any owner access. A Start's job ID is
    /// its own operation ID.
    pub(crate) fn execute(
        &self,
        request: AgentIntegrationJobRequest,
        worker: WorkerId,
        generation: NonZeroU64,
    ) -> AgentIntegrationJobResult {
        let fenced = request.worker_id.as_bytes() != worker.as_bytes()
            || request.worker_generation.get() != generation.get();
        let (job_id, outcome) = match request.action {
            Action::Start { request: document } => (
                request.operation_id,
                Outcome::Start(if fenced {
                    Start::RejectedFencing
                } else {
                    self.start(request.operation_id, document)
                }),
            ),
            Action::Poll { job_id } => (
                job_id,
                Outcome::Poll(if fenced {
                    Poll::RejectedFencing
                } else {
                    self.poll(job_id)
                }),
            ),
            Action::Cancel { job_id } => (
                job_id,
                Outcome::Cancel(if fenced {
                    Cancel::RejectedFencing
                } else {
                    self.cancel(job_id)
                }),
            ),
        };
        AgentIntegrationJobResult {
            operation_id: request.operation_id,
            job_id,
            outcome,
        }
    }

    fn start(&self, id: OperationId, document: Vec<u8>) -> Start {
        let Ok(request) = orchbee_agent_integration::decode_request(&document) else {
            return Start::InvalidRequest;
        };
        let Ok(mut inner) = self.inner.lock() else {
            return Start::Dead;
        };
        settle(&mut inner);
        if let Some(record) = inner.records.iter().find(|record| record.id == id) {
            // A replayed Start names the same job; a different document under it never runs.
            return if record.request == document {
                Start::Accepted
            } else {
                Start::InvalidRequest
            };
        }
        if inner.stopping {
            return Start::Dead;
        }
        if inner
            .records
            .iter()
            .any(|record| matches!(record.state, State::Running { .. }))
        {
            return Start::Busy;
        }
        if inner.records.len() >= CAPACITY {
            inner.records.pop_front();
        }
        let cancel = Arc::new(CancelFlag::default());
        let thread_cancel = Arc::clone(&cancel);
        let execute = self.execute;
        let Ok(join) = thread::Builder::new()
            .name("orchbee-agent-integration".into())
            .spawn(move || execute(request, thread_cancel))
        else {
            return Start::SpawnThreadFailed;
        };
        inner.records.push_back(Record {
            id,
            request: document,
            state: State::Running { cancel, join },
        });
        Start::Accepted
    }

    fn poll(&self, id: OperationId) -> Poll {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        settle(&mut inner);
        match inner.records.iter().find(|record| record.id == id) {
            None => Poll::Unknown,
            Some(Record {
                state: State::Running { .. },
                ..
            }) => Poll::Pending,
            Some(Record {
                state: State::Completed(report),
                ..
            }) => Poll::completed(report.clone()).unwrap_or(Poll::Abandoned),
            Some(Record {
                state: State::Abandoned,
                ..
            }) => Poll::Abandoned,
        }
    }

    fn cancel(&self, id: OperationId) -> Cancel {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        settle(&mut inner);
        match inner.records.iter().find(|record| record.id == id) {
            None => Cancel::Unknown,
            Some(Record {
                state: State::Running { cancel, .. },
                ..
            }) => {
                cancel.0.store(true, Ordering::Relaxed);
                Cancel::Registered
            }
            Some(_) => Cancel::AlreadyTerminal,
        }
    }

    /// Closes admission and signals the running job without waiting for it.
    pub(crate) fn request_stop(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        request_stop(&mut inner);
    }

    /// Requests stop, then observes the running job's completion before the deadline. `Ok` means
    /// the job has ended and been joined; `TimedOut` leaves the stop requested and pending.
    pub(crate) fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        loop {
            {
                let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
                settle(&mut inner);
                if !request_stop(&mut inner) {
                    return Ok(());
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }
}

/// Closes admission and signals the running job; reports whether it is still running.
fn request_stop(inner: &mut Inner) -> bool {
    inner.stopping = true;
    let mut running = false;
    for record in &inner.records {
        if let State::Running { cancel, .. } = &record.state {
            cancel.0.store(true, Ordering::Relaxed);
            running = true;
        }
    }
    running
}

fn settle(inner: &mut Inner) {
    for record in &mut inner.records {
        if !matches!(&record.state, State::Running { join, .. } if join.is_finished()) {
            continue;
        }
        let State::Running { join, .. } = std::mem::replace(&mut record.state, State::Abandoned)
        else {
            unreachable!("finished job checked above")
        };
        if let Ok(Some(report)) = join.join() {
            record.state = State::Completed(report);
        }
    }
}

impl Drop for AgentIntegrationJobs {
    fn drop(&mut self) {
        // The bounded `stop_before` owns completion. Drop only signals the running job and
        // joins one that has already finished; an unfinished job is never waited for here.
        let inner = self
            .inner
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        settle(inner);
        request_stop(inner);
    }
}

/// Runs one operation with this Worker's environment under the request's budget.
fn run(request: IntegrationRequest, cancel: Arc<CancelFlag>) -> Report {
    let budget = request
        .budget_milliseconds
        .unwrap_or(MAXIMUM_INTEGRATION_BUDGET_MILLISECONDS);
    let ceiling = Instant::now() + Duration::from_millis(u64::from(budget));
    let source: Arc<dyn TerminationSource> = cancel;
    let report = match orchbee_agent_integration_writer::context_before(
        request.adapter_source.as_deref(),
        Some(source),
        ceiling,
    ) {
        Err(_) => IntegrationReport::HomeUnavailable,
        Ok(context) => match request.operation {
            IntegrationOperation::List { provider } => {
                let (items, capsule) = orchbee_agent_integration_writer::list(&context, provider);
                IntegrationReport::List { items, capsule }
            }
            IntegrationOperation::Explain { provider } => {
                let (item, detail, capsule, plan) =
                    orchbee_agent_integration_writer::explain(&context, provider);
                IntegrationReport::Explain {
                    item,
                    detail,
                    capsule,
                    plan,
                }
            }
            IntegrationOperation::Enable { provider } => IntegrationReport::Mutation {
                report: orchbee_agent_integration_writer::mutate(
                    &context,
                    provider,
                    Direction::Enable,
                ),
            },
            IntegrationOperation::Disable { provider } => IntegrationReport::Mutation {
                report: orchbee_agent_integration_writer::mutate(
                    &context,
                    provider,
                    Direction::Disable,
                ),
            },
        },
    };
    orchbee_agent_integration::encode_report(&report).ok()
}

#[cfg(test)]
mod tests;
