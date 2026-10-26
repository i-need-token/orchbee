//! Single-identity shutdown admission and observable attempts, independent of process adapters.
use orchbee_local_client_protocol::{
    SessionStopRequest, SessionStopResult, SessionStopStatus, StopAction, StopAttempt, StopFailure,
    StopId, StopPhase,
};
use std::{
    sync::{Condvar, Mutex},
    time::Instant,
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
#[derive(Default)]
struct State {
    entry: Option<Entry>,
    records: usize,
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

    /// A record lease is acquired after decoding and remains held through its result write.
    pub(crate) fn enter(&self) -> Option<RecordLease<'_>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entry.is_some() {
            return None;
        }
        state.records += 1;
        Some(RecordLease(self))
    }

    /// Waits for one queued attempt and grants its execution ownership exactly once.
    pub(crate) fn take(&self) -> Attempt {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(entry) = state.entry.as_mut()
                && matches!(entry.progress, Progress::Queued)
            {
                entry.progress = Progress::Running(StopPhase::Admission);
                return entry.attempt;
            }
            state = self.changed.wait(state).unwrap_or_else(|e| e.into_inner());
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

#[cfg(test)]
mod tests;
