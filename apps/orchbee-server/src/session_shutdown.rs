//! Single-identity shutdown admission, the `drain` and `exit` control course, and observable
//! attempts, independent of process adapters.
use orchbee_connection_handshake::ProcessState;
use orchbee_local_client_protocol::{
    SessionStopRequest, SessionStopResult, SessionStopStatus, StopAction, StopAttempt, StopFailure,
    StopId, StopPhase,
};
use std::{
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};

/// Exact stop identity and attempt fence required by every progress update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Attempt {
    pub(crate) id: StopId,
    pub(crate) number: StopAttempt,
}

enum Progress {
    Queued,
    Running(StopPhase),
    Failed {
        phase: StopPhase,
        failure: StopFailure,
        disposed_terminals: u32,
    },
}
struct Entry {
    attempt: Attempt,
    progress: Progress,
}
/// What the `drain` and `exit` control requests asked of this Server. The course only advances.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
enum Course {
    #[default]
    Serving,
    /// Admits no new Client or intent record while reads continue and admitted records finish.
    Draining,
    /// The lifecycle owner is ending the Server and its Worker, leaving every terminal running.
    Exiting,
    /// The process ends next; waiting `exit` requesters reply now.
    Ended,
}

#[derive(Default)]
struct State {
    entry: Option<Entry>,
    course: Course,
    records: usize,
    /// `exit` requesters that have not yet written their reply.
    exit_replies: usize,
}

/// The lifecycle owner's next work: one Session stop attempt, or the requested `exit`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Work {
    Stop(Attempt),
    Exit,
}

/// Owns stop admission; effects are performed only by the managed process owner.
#[derive(Default)]
pub(crate) struct SessionShutdown {
    state: Mutex<State>,
    changed: Condvar,
}

impl SessionShutdown {
    /// Atomically admits one identity, observes duplicates, or explicitly retries a failed attempt.
    pub(crate) fn request(&self, request: &SessionStopRequest) -> SessionStopResult {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.entry.as_mut() {
            if entry.attempt.id != request.id {
                return SessionStopResult {
                    id: request.id,
                    status: if request.action == StopAction::Status {
                        SessionStopStatus::NotRecorded
                    } else {
                        SessionStopStatus::Busy {
                            active_id: entry.attempt.id,
                        }
                    },
                };
            }
            if request.action == StopAction::Retry
                && matches!(entry.progress, Progress::Failed { .. })
                && let Ok(number) = StopAttempt::new(entry.attempt.number.get() + 1)
            {
                entry.attempt.number = number;
                entry.progress = Progress::Queued;
                self.changed.notify_all();
            }
        } else if request.action == StopAction::Start {
            state.entry = Some(Entry {
                attempt: Attempt {
                    id: request.id,
                    number: StopAttempt::new(1).expect("first attempt"),
                },
                progress: Progress::Queued,
            });
            self.changed.notify_all();
        }
        SessionStopResult {
            id: request.id,
            status: state
                .entry
                .as_ref()
                .map_or(SessionStopStatus::NotRecorded, status),
        }
    }

    pub(crate) fn stopping(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry
            .is_some()
    }

    /// The state a `query` or `drain` reports: `stopping` once a Session stop or an `exit` is
    /// recorded, `draining` after a `drain`, `ready` otherwise.
    pub(crate) fn process_state(&self) -> ProcessState {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match state.course {
            _ if state.entry.is_some() => ProcessState::Stopping,
            Course::Serving => ProcessState::Ready,
            Course::Draining => ProcessState::Draining,
            Course::Exiting | Course::Ended => ProcessState::Stopping,
        }
    }

    /// The state the handshake weighs an offer against; anything but `ready` refuses it with
    /// `stopping`. A recorded Session stop still answers offers, so its status and retry exchanges
    /// reach this owner; the Client phase refuses them through [`Self::enter`].
    pub(crate) fn offer_state(&self) -> ProcessState {
        match self.state.lock().unwrap_or_else(|e| e.into_inner()).course {
            Course::Serving => ProcessState::Ready,
            Course::Draining => ProcessState::Draining,
            Course::Exiting | Course::Ended => ProcessState::Stopping,
        }
    }

    /// Stops accepting new work while admitted records continue; repeating it changes nothing.
    /// Returns the state a `drain-result` reports.
    pub(crate) fn drain(&self) -> ProcessState {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.course = state.course.max(Course::Draining);
        }
        self.process_state()
    }

    /// Records an `exit` for the lifecycle owner and registers one reply that the owner awaits
    /// before the process ends.
    pub(crate) fn exit(&self) -> ExitReply<'_> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.course = state.course.max(Course::Exiting);
        state.exit_replies += 1;
        self.changed.notify_all();
        ExitReply(self)
    }

    /// Marks the end of the process and waits until every `exit` requester has written its reply,
    /// or `deadline`.
    pub(crate) fn end(&self, deadline: Instant) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.course = Course::Ended;
        self.changed.notify_all();
        while state.exit_replies > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        true
    }

    /// Admits new work: a Client connection, or an intent record. A record lease is acquired after
    /// decoding and remains held through its result write. There is none once a Session stop is
    /// recorded or a `drain` or `exit` arrived.
    pub(crate) fn enter(&self) -> Option<RecordLease<'_>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() || state.course != Course::Serving {
            return None;
        }
        state.records += 1;
        Some(RecordLease(self))
    }

    /// Admits a read record: a `drain` keeps serving reads, while a Session stop or an `exit`
    /// admits none.
    pub(crate) fn enter_read(&self) -> Option<RecordLease<'_>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() || state.course > Course::Draining {
            return None;
        }
        state.records += 1;
        Some(RecordLease(self))
    }

    /// Admits the lifecycle owner's own stop under `id` when no stop exists yet.
    #[cfg(windows)]
    pub(crate) fn begin(&self, id: StopId) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() {
            return false;
        }
        state.entry = Some(Entry {
            attempt: Attempt {
                id,
                number: StopAttempt::new(1).expect("first attempt"),
            },
            progress: Progress::Queued,
        });
        self.changed.notify_all();
        true
    }

    /// Waits up to `timeout` for one queued attempt, granting its execution ownership exactly once,
    /// or for an `exit`. A queued attempt goes first, since it ends the process too.
    pub(crate) fn take_within(&self, timeout: Duration) -> Option<Work> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(entry) = state.entry.as_mut()
                && matches!(entry.progress, Progress::Queued)
            {
                entry.progress = Progress::Running(StopPhase::Admission);
                return Some(Work::Stop(entry.attempt));
            }
            if state.course >= Course::Exiting {
                return Some(Work::Exit);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Waits for admitted record leases to release without renewing the deadline.
    pub(crate) fn settle_records(&self, deadline: Instant) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.records > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        true
    }

    /// Publishes progress only while the exact attempt still owns execution.
    pub(crate) fn phase(&self, attempt: Attempt, phase: StopPhase) {
        self.update(attempt, Progress::Running(phase));
    }
    /// Retains known failure facts for explicit recovery, ignoring stale attempts.
    pub(crate) fn fail(
        &self,
        attempt: Attempt,
        phase: StopPhase,
        failure: StopFailure,
        disposed_terminals: u32,
    ) {
        self.update(
            attempt,
            Progress::Failed {
                phase,
                failure,
                disposed_terminals,
            },
        );
    }
    fn update(&self, attempt: Attempt, progress: Progress) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.entry.as_mut()
            && entry.attempt == attempt
            && matches!(entry.progress, Progress::Running(_))
        {
            entry.progress = progress;
        }
    }
}

fn status(entry: &Entry) -> SessionStopStatus {
    let attempt = entry.attempt.number;
    match entry.progress {
        Progress::Queued => SessionStopStatus::Pending {
            attempt,
            phase: StopPhase::Admission,
        },
        Progress::Running(phase) => SessionStopStatus::Pending { attempt, phase },
        Progress::Failed {
            phase,
            failure,
            disposed_terminals,
        } => SessionStopStatus::Failed {
            attempt,
            phase,
            failure,
            disposed_terminals,
        },
    }
}

/// Keeps one admitted operation and its response write ahead of lifecycle cleanup until drop.
pub(crate) struct RecordLease<'a>(&'a SessionShutdown);
impl Drop for RecordLease<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.records = state.records.checked_sub(1).expect("owned record lease");
        self.0.changed.notify_all();
    }
}

/// One `exit` requester's reply slot; the process ends only after it is dropped or the owner's
/// deadline passes.
pub(crate) struct ExitReply<'a>(&'a SessionShutdown);
impl ExitReply<'_> {
    /// Waits until the owner ends the process, or `deadline`; true means the reply may be written.
    pub(crate) fn ended_before(&self, deadline: Instant) -> bool {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.course != Course::Ended {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            state = self
                .0
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        true
    }
}
impl Drop for ExitReply<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.exit_replies = state.exit_replies.checked_sub(1).expect("owned exit reply");
        self.0.changed.notify_all();
    }
}

#[cfg(test)]
mod tests;
