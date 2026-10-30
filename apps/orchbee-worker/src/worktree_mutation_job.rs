//! One retained physical Git owner with non-replaying mutation receipts.
use orchbee_capability_node_protocol::{
    OperationId, WorktreeMutationCancelOutcome as Cancel, WorktreeMutationCompletion,
    WorktreeMutationIntent, WorktreeMutationPollOutcome as Poll,
    WorktreeMutationStartOutcome as Start,
};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const CAPACITY: usize = 64;
const WORK_BUDGET: Duration = Duration::from_secs(30);
type Report = Result<WorktreeMutationCompletion, ()>;
type Execute = fn(WorktreeMutationIntent, &AtomicBool, Instant) -> Report;

enum State {
    Running {
        cancel: Arc<AtomicBool>,
        join: JoinHandle<Report>,
    },
    Complete(WorktreeMutationCompletion),
    PhysicalUnknown,
}

struct Record {
    id: OperationId,
    intent: WorktreeMutationIntent,
    state: State,
}

struct Inner {
    records: Vec<Record>,
    retired: bool,
    stopping: bool,
}

pub(crate) struct WorktreeMutationSlot {
    inner: Mutex<Inner>,
    execute: Execute,
}

impl WorktreeMutationSlot {
    pub(crate) fn new(execute: Execute) -> Self {
        Self {
            inner: Mutex::new(Inner {
                records: Vec::with_capacity(CAPACITY),
                retired: false,
                stopping: false,
            }),
            execute,
        }
    }

    pub(crate) fn start(&self, id: OperationId, intent: WorktreeMutationIntent) -> Start {
        let Ok(mut inner) = self.inner.lock() else {
            return Start::PhysicalUnknown;
        };
        settle(&mut inner);
        if let Some(record) = inner.records.iter().find(|record| record.id == id) {
            return if record.intent == intent {
                Start::Existing
            } else {
                Start::Conflict
            };
        }
        if inner.retired || inner.stopping {
            return Start::PhysicalUnknown;
        }
        if inner
            .records
            .iter()
            .any(|record| matches!(record.state, State::Running { .. }))
        {
            return Start::Busy;
        }
        if inner.records.len() >= CAPACITY {
            return Start::Full;
        }
        let (admit, admitted) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let thread_intent = intent.clone();
        let execute = self.execute;
        let deadline = Instant::now() + WORK_BUDGET;
        let join = match thread::Builder::new()
            .name("orchbee-worktree-mutation".into())
            .spawn(move || {
                admitted.recv().map_err(|_| ())?;
                execute(thread_intent, &thread_cancel, deadline)
            }) {
            Ok(join) => join,
            Err(_) => return Start::SpawnFailed,
        };
        inner.records.push(Record {
            id,
            intent,
            state: State::Running { cancel, join },
        });
        // The physical owner cannot start until its exact intent and join handle
        // have a retained owner. Failed delivery is observed by settlement.
        if admit.send(()).is_err() {
            inner.retired = true;
        }
        Start::Accepted
    }

    pub(crate) fn poll(&self, id: OperationId) -> Poll {
        let Ok(mut inner) = self.inner.lock() else {
            return Poll::PhysicalUnknown;
        };
        settle(&mut inner);
        match inner
            .records
            .iter()
            .find(|record| record.id == id)
            .map(|record| &record.state)
        {
            None => Poll::Unknown,
            Some(State::Running { .. }) => Poll::Pending,
            Some(State::Complete(value)) => Poll::Completed(value.clone()),
            Some(State::PhysicalUnknown) => Poll::PhysicalUnknown,
        }
    }

    pub(crate) fn cancel(&self, id: OperationId) -> Cancel {
        let Ok(mut inner) = self.inner.lock() else {
            return Cancel::PhysicalUnknown;
        };
        settle(&mut inner);
        match inner
            .records
            .iter()
            .find(|record| record.id == id)
            .map(|record| &record.state)
        {
            None => Cancel::Unknown,
            Some(State::Running { cancel, .. }) => {
                cancel.store(true, Ordering::Relaxed);
                Cancel::Registered
            }
            Some(State::Complete(_)) => Cancel::AlreadyTerminal,
            Some(State::PhysicalUnknown) => Cancel::PhysicalUnknown,
        }
    }

    /// Closes admission and signals the running owner without waiting for it.
    pub(crate) fn request_stop(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        request_stop(&mut inner);
    }

    /// Requests stop, then observes the owner's completion before the deadline. `Ok` means the
    /// owner has ended and been joined; `TimedOut` leaves the stop requested and pending.
    pub(crate) fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        loop {
            {
                let mut inner = self
                    .inner
                    .lock()
                    .map_err(|_| io::Error::other("Worktree owner is poisoned"))?;
                settle(&mut inner);
                let running = request_stop(&mut inner);
                if inner.retired {
                    return Err(io::Error::other("Worktree cleanup is unknown"));
                }
                if !running {
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

/// Closes admission and signals each running owner; reports whether any owner is still running.
fn request_stop(inner: &mut Inner) -> bool {
    inner.stopping = true;
    let mut running = false;
    for record in &inner.records {
        if let State::Running { cancel, .. } = &record.state {
            cancel.store(true, Ordering::Relaxed);
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
        let State::Running { join, .. } =
            std::mem::replace(&mut record.state, State::PhysicalUnknown)
        else {
            unreachable!("running owner checked above")
        };
        match join.join() {
            Ok(Ok(value)) => record.state = State::Complete(value),
            _ => inner.retired = true,
        }
    }
}

impl Drop for WorktreeMutationSlot {
    fn drop(&mut self) {
        // The bounded `stop_before` owns completion. Drop only requests stop and joins an
        // owner that has already finished; an unfinished owner is never waited for here.
        let inner = self
            .inner
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        settle(inner);
        request_stop(inner);
    }
}

#[cfg(test)]
mod tests;
