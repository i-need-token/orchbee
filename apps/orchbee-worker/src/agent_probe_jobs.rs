//! Owns the node's one Agent probe job: Worker-generation fencing, one running lookup thread at a
//! time, cooperative cancellation, and bounded retention of results. The lookup answers whether
//! each requested command is found on the search path a newly started shell would use; this
//! module neither chooses the commands nor interprets the answers.
use orchbee_capability_node_protocol::{
    AgentProbeJobAction as Action, AgentProbeJobCancelOutcome as Cancel,
    AgentProbeJobOutcome as Outcome, AgentProbeJobPollOutcome as Poll, AgentProbeJobRequest,
    AgentProbeJobResult, AgentProbeJobStartOutcome as Start, AgentProbeResult, OperationId,
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

mod lookup;

/// Settled jobs retained for polling; admission evicts the oldest settled record first.
const CAPACITY: usize = 8;

/// One answer per requested command, in request order.
type Results = Vec<AgentProbeResult>;
/// Runs one lookup until it ends, its own deadline passes, or the flag is set.
type Execute = fn(&[String], &AtomicBool) -> Results;

enum State {
    Running {
        cancel: Arc<AtomicBool>,
        join: JoinHandle<Results>,
    },
    Completed(Results),
    Abandoned,
}

struct Record {
    id: OperationId,
    commands: Vec<String>,
    state: State,
}

struct Inner {
    records: VecDeque<Record>,
    stopping: bool,
}

pub(crate) struct AgentProbeJobs {
    inner: Mutex<Inner>,
    /// `None` where this platform has no lookup: every Start answers `Unsupported`.
    execute: Option<Execute>,
}

impl AgentProbeJobs {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                records: VecDeque::with_capacity(CAPACITY),
                stopping: false,
            }),
            execute: lookup::executor(),
        }
    }

    /// Fences the request to this Worker generation before any owner access. A Start's job ID is
    /// its own operation ID.
    pub(crate) fn execute(
        &self,
        request: AgentProbeJobRequest,
        worker: WorkerId,
        generation: NonZeroU64,
    ) -> AgentProbeJobResult {
        let fenced = request.worker_id.as_bytes() != worker.as_bytes()
            || request.worker_generation.get() != generation.get();
        let (job_id, outcome) = match request.action {
            Action::Start { commands } => (
                request.operation_id,
                Outcome::Start(if fenced {
                    Start::RejectedFencing
                } else {
                    self.start(request.operation_id, commands)
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
        AgentProbeJobResult {
            operation_id: request.operation_id,
            job_id,
            outcome,
        }
    }

    fn start(&self, id: OperationId, commands: Vec<String>) -> Start {
        let Some(execute) = self.execute else {
            return Start::Unsupported;
        };
        let Ok(mut inner) = self.inner.lock() else {
            return Start::Dead;
        };
        settle(&mut inner);
        if let Some(record) = inner.records.iter().find(|record| record.id == id) {
            // A replayed Start names the same job; different commands under it never run.
            return if record.commands == commands {
                Start::Accepted
            } else {
                Start::Busy
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
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let thread_commands = commands.clone();
        let Ok(join) = thread::Builder::new()
            .name("orchbee-agent-probe".into())
            .spawn(move || execute(&thread_commands, &thread_cancel))
        else {
            return Start::SpawnThreadFailed;
        };
        inner.records.push_back(Record {
            id,
            commands,
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
                state: State::Completed(results),
                ..
            }) => Poll::completed(results.clone()).unwrap_or(Poll::Abandoned),
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
                cancel.store(true, Ordering::Release);
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
            cancel.store(true, Ordering::Release);
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
        // Results of another length cannot be paired with the request, so the job is abandoned.
        if let Ok(results) = join.join()
            && results.len() == record.commands.len()
        {
            record.state = State::Completed(results);
        }
    }
}

impl Drop for AgentProbeJobs {
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
