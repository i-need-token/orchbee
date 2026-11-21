//! Single-identity shutdown admission, the `drain` and `exit` control course, observable attempts,
//! the hold of a Server restart that keeps terminals, and the one `handover` step a Client delivers
//! to the lifecycle owner at a time, independent of process adapters.
use orchbee_connection_handshake::ProcessState;
#[cfg(any(windows, test))]
use orchbee_connection_handshake::{
    HandoverOutcome, HandoverRefusal, HandoverStep, HandoverTicket,
};
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
    /// The admitted records and connections that may change the Session, a subset of `records`.
    intents: usize,
    /// Whether a Server restart holds the Session for its successor: intents other than terminal
    /// records are refused like under a `drain`, while reads, terminal records and new Client
    /// connections are still admitted.
    held: bool,
    /// The `handover` step a Client waits on, if any.
    #[cfg(any(windows, test))]
    exchange: Option<Exchange>,
    /// `exit` requesters that have not yet written their reply.
    exit_replies: usize,
    /// Whether a stop signal arrived; it stays set until the process ends.
    #[cfg(any(unix, test))]
    signalled: bool,
    /// Whether the lifecycle owner should look at its Worker before its next tick.
    woken: bool,
}

/// The lifecycle owner's next work: one Session stop attempt, or the requested `exit`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Work {
    Stop(Attempt),
    Exit,
}

/// One `handover` step between the Client that delivered it and the lifecycle owner.
#[cfg(any(windows, test))]
struct Exchange {
    step: HandoverStep,
    ticket: HandoverTicket,
    /// Whether the lifecycle owner took the step; an untaken one is withdrawn at the deadline.
    taken: bool,
    /// The owner's answer, once given.
    answer: Option<HandoverOutcome>,
    /// Whether the Client stopped waiting; the owner's answer then goes nowhere.
    abandoned: bool,
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
            if request.action == StopAction::Retry {
                self.requeue(entry);
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

    /// Whether a Session stop is recorded; it stays recorded until the process ends.
    pub(crate) fn stopping(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry
            .is_some()
    }

    /// Whether a Session stop is recorded or an `exit` asked this Server to end. A Worker
    /// replacement in progress ends at its next step boundary once either holds.
    pub(crate) fn ending(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.entry.is_some() || state.course >= Course::Exiting
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

    /// Admits new work that a restart's hold lets through: a Client connection, or a terminal
    /// record, which then finds no Worker. A record lease is acquired after decoding and remains
    /// held through its result write. There is none once a Session stop is recorded or a `drain` or
    /// `exit` arrived.
    pub(crate) fn enter(&self) -> Option<RecordLease<'_>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() || state.course != Course::Serving {
            return None;
        }
        Some(self.lease(&mut state, true))
    }

    /// Admits any other intent record: as [`Self::enter`], and none while a restart holds the
    /// Session, since its successor starts from the state captured when the hold began.
    pub(crate) fn enter_intent(&self) -> Option<RecordLease<'_>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() || state.course != Course::Serving || state.held {
            return None;
        }
        Some(self.lease(&mut state, true))
    }

    /// Admits a read record: a `drain` keeps serving reads, while a Session stop or an `exit`
    /// admits none.
    pub(crate) fn enter_read(&self) -> Option<RecordLease<'_>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() || state.course > Course::Draining {
            return None;
        }
        Some(self.lease(&mut state, false))
    }

    fn lease(&self, state: &mut State, intent: bool) -> RecordLease<'_> {
        state.records += 1;
        state.intents += usize::from(intent);
        RecordLease {
            owner: self,
            intent,
        }
    }

    #[cfg(any(windows, test))]
    /// Holds the Session for a restart's successor, or lifts that hold.
    pub(crate) fn hold(&self, held: bool) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).held = held;
    }

    #[cfg(any(windows, test))]
    /// Waits until no admitted intent record or connection is left, without renewing the deadline;
    /// reads go on.
    pub(crate) fn settle_intents(&self, deadline: Instant) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.intents > 0 {
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

    #[cfg(any(windows, test))]
    /// Delivers one `handover` step to the lifecycle owner and waits for its answer until
    /// `deadline`. A step that arrives while another is waiting is refused `busy`. `None` means
    /// the owner did not answer in time: a step it never took is withdrawn, and the answer to one it
    /// took goes nowhere.
    pub(crate) fn handover(
        &self,
        step: HandoverStep,
        ticket: HandoverTicket,
        deadline: Instant,
    ) -> Option<HandoverOutcome> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.exchange.is_some() {
            return Some(HandoverOutcome::Refused(HandoverRefusal::Busy));
        }
        state.exchange = Some(Exchange {
            step,
            ticket,
            taken: false,
            answer: None,
            abandoned: false,
        });
        // The owner's wait ends early, so it takes the step at once.
        state.woken = true;
        self.changed.notify_all();
        loop {
            if let Some(answer) = state.exchange.as_ref().and_then(|exchange| exchange.answer) {
                state.exchange = None;
                self.changed.notify_all();
                return Some(answer);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                match state.exchange.as_mut() {
                    Some(exchange) if exchange.taken => exchange.abandoned = true,
                    _ => state.exchange = None,
                }
                return None;
            }
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    #[cfg(any(windows, test))]
    /// The waiting `handover` step, granting the lifecycle owner its answer exactly once.
    pub(crate) fn take_handover(&self) -> Option<(HandoverStep, HandoverTicket)> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let exchange = state.exchange.as_mut().filter(|exchange| !exchange.taken)?;
        exchange.taken = true;
        Some((exchange.step, exchange.ticket))
    }

    #[cfg(any(windows, test))]
    /// The lifecycle owner's answer to the step it took.
    pub(crate) fn answer_handover(&self, answer: HandoverOutcome) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match state.exchange.as_mut() {
            Some(exchange) if exchange.abandoned => state.exchange = None,
            Some(exchange) if exchange.taken => exchange.answer = Some(answer),
            _ => {}
        }
        self.changed.notify_all();
    }

    /// Admits the lifecycle owner's own stop under `id` when no stop exists yet.
    pub(crate) fn begin(&self, id: StopId) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() {
            return false;
        }
        self.record_own(&mut state, id);
        true
    }

    /// Admits a stop signal: it begins the lifecycle owner's own stop under `own` when no stop is
    /// recorded, and otherwise queues the next attempt of the recorded stop once its attempt
    /// failed, whoever recorded it, since a signal outranks the Client that began it. The signal
    /// stays recorded, so the owner can tell that the process must end.
    #[cfg(any(unix, test))]
    pub(crate) fn signal(&self, own: StopId) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.signalled = true;
        match state.entry.as_mut() {
            None => self.record_own(&mut state, own),
            Some(entry) => self.requeue(entry),
        }
    }

    /// Queues the next attempt of a failed stop; a queued or running attempt stays as it is.
    fn requeue(&self, entry: &mut Entry) {
        if matches!(entry.progress, Progress::Failed { .. })
            && let Ok(number) = StopAttempt::new(entry.attempt.number.get() + 1)
        {
            entry.attempt.number = number;
            entry.progress = Progress::Queued;
            self.changed.notify_all();
        }
    }

    /// Whether a stop signal arrived.
    #[cfg(any(unix, test))]
    pub(crate) fn signalled(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .signalled
    }

    /// Admits the idle move's stop under `id` only while nothing is admitted at this moment: no
    /// stop, `drain` or `exit` recorded, no record lease held, and `idle` holds. `idle` runs under
    /// this owner's lock, which every Client admission passes through first, so a Client is either
    /// seen by `idle` or refused afterwards. It must not call back into this owner.
    #[cfg(any(windows, target_os = "macos", test))]
    pub(crate) fn begin_idle(&self, id: StopId, idle: impl FnOnce() -> bool) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some()
            || state.course != Course::Serving
            || state.records > 0
            || state.held
            || !idle()
        {
            return false;
        }
        self.record_own(&mut state, id);
        true
    }

    fn record_own(&self, state: &mut State, id: StopId) {
        state.entry = Some(Entry {
            attempt: Attempt {
                id,
                number: StopAttempt::new(1).expect("first attempt"),
            },
            progress: Progress::Queued,
        });
        self.changed.notify_all();
    }

    /// Ends the lifecycle owner's current wait early, so it looks at its Worker now: the Worker
    /// connection ended.
    pub(crate) fn wake(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).woken = true;
        self.changed.notify_all();
    }

    /// Waits up to `timeout` for one queued attempt, granting its execution ownership exactly once,
    /// or for an `exit`. A queued attempt goes first, since it ends the process too. A
    /// [`wake`](Self::wake) ends the wait early with no work.
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
            if std::mem::take(&mut state.woken) {
                return None;
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
pub(crate) struct RecordLease<'a> {
    owner: &'a SessionShutdown,
    intent: bool,
}
impl Drop for RecordLease<'_> {
    fn drop(&mut self) {
        let mut state = self.owner.state.lock().unwrap_or_else(|e| e.into_inner());
        state.records = state.records.checked_sub(1).expect("owned record lease");
        if self.intent {
            state.intents = state.intents.checked_sub(1).expect("owned intent lease");
        }
        self.owner.changed.notify_all();
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
