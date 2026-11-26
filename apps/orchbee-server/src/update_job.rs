//! The automatic-update job as admitted Clients see it: whether this Server runs one, whether a run
//! is queued or running, and how the last run ended. The automatic-update schedule runs every run,
//! requested or scheduled, on its own thread, so a Client's `workbench.update-check` only queues a
//! run and wakes that thread; it never runs the helper, waits for it, or touches the store. Each
//! answer is read under one lock and returns at once, so a Client connection's serial requests never
//! wait on a run.
use orchbee_workbench_protocol::{UpdateCheckAction, UpdateCheckFinish, UpdateCheckOutcome};
use std::sync::{Condvar, Mutex, PoisonError};

/// The shared job state, created before the Session admits Clients. A poisoned lock reads as
/// stopped, so a broken owner never starts a run and every Client sees the job as unavailable.
#[derive(Default)]
pub(crate) struct UpdateJob {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    /// The Server schedules nothing, or its schedule stopped or ended. Until the Server decides, a
    /// request waits for the schedule to start.
    ended: bool,
    /// A Client asked for a run that has not started yet.
    requested: bool,
    running: bool,
    last: Option<UpdateCheckFinish>,
}

/// What woke the schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Wake {
    /// The scheduled moment came.
    Due,
    /// A Client asked for a run now.
    Requested,
}

impl UpdateJob {
    /// No run happens from now on: the Server schedules nothing, or its schedule stopped or ended.
    /// A queued request is dropped, and Clients read the job as unavailable.
    pub(crate) fn end(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.ended = true;
        state.requested = false;
        state.running = false;
        self.changed.notify_all();
    }

    /// Answers one admitted Client's `start` or `read`.
    pub(crate) fn answer(&self, action: UpdateCheckAction) -> UpdateCheckOutcome {
        let Ok(mut state) = self.state.lock() else {
            return UpdateCheckOutcome::Unavailable {};
        };
        if state.ended {
            return UpdateCheckOutcome::Unavailable {};
        }
        if action == UpdateCheckAction::Start && !state.requested && !state.running {
            state.requested = true;
            self.changed.notify_all();
        }
        if state.requested || state.running {
            return UpdateCheckOutcome::Checking {};
        }
        state.last.map_or(UpdateCheckOutcome::Idle {}, |result| {
            UpdateCheckOutcome::Finished { result }
        })
    }
}

/// The schedule's side.
impl UpdateJob {
    /// Whether the schedule was told to stop; a running helper is ended at its next check.
    pub(crate) fn stopped(&self) -> bool {
        self.state.lock().map_or(true, |state| state.ended)
    }

    /// A run starts; it serves a request queued before it, so a request never adds a second run.
    pub(crate) fn begin(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.requested = false;
            state.running = true;
        }
    }

    /// The run that [`Self::begin`] started ended this way.
    pub(crate) fn finish(&self, result: UpdateCheckFinish) {
        if let Ok(mut state) = self.state.lock() {
            state.running = false;
            state.last = Some(result);
        }
    }

    /// Waits until `due` or a Client's request, whichever comes first, and answers which; `None`
    /// once the schedule stopped. A request that arrived earlier answers at once.
    pub(crate) fn wait_until(&self, due: std::time::Instant) -> Option<Wake> {
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        loop {
            if state.ended {
                return None;
            }
            if state.requested {
                return Some(Wake::Requested);
            }
            let now = std::time::Instant::now();
            if now >= due {
                return Some(Wake::Due);
            }
            state = match self.changed.wait_timeout(state, due - now) {
                Ok((state, _)) => state,
                Err(_) => return None,
            };
        }
    }
}

#[cfg(test)]
mod tests;
