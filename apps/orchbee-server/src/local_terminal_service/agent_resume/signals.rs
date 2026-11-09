//! What Clients did to restored terminals while resumes are pending: the first Client view of this
//! Server lifetime, which terminals a Client viewed, and which received Client input; and the
//! retried candidates waiting for the running driver. Notes are taken while the noting path holds
//! the coordinator lock, and the driver's final check reads them under the same lock, so an input
//! either precedes a delivery's admission or follows its send. A run ends in the same critical
//! section that finds no retried candidate waiting, so each one either joins the running driver or
//! starts the next run. The state lock is a leaf: nothing is acquired while it is held. It does not
//! decide delivery.
use super::Admitted;
use crate::{
    local_terminal_service::controller_connection::ControlTarget,
    terminal_coordinator::BindingViewerTarget,
};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

#[derive(Default)]
struct State {
    /// Notes are kept only between the driver's start and its end.
    active: bool,
    stop: bool,
    first_view: Option<Instant>,
    viewed: BTreeSet<ControlTarget>,
    touched: BTreeSet<ControlTarget>,
    /// Retried candidates for the running driver, in Retry order. Each Retry releases its hold, so
    /// the held Presentations bound it.
    retried: VecDeque<Admitted>,
}

/// Where a retried candidate goes.
pub(super) enum Offer {
    /// The running driver takes it after its current work.
    Queued,
    /// No driver runs: notes are kept from now, and the caller starts a run with it.
    Start(Admitted),
    /// The Server is stopping; the reference stays pending for the final checkpoint.
    Refused,
}

#[derive(Default)]
pub(super) struct Signals {
    state: Mutex<State>,
    changed: Condvar,
}

impl Signals {
    /// Every update is one assignment or insertion, so a poisoned state is still consistent.
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts keeping notes, unless a stop already arrived.
    pub(super) fn activate(&self) -> bool {
        let mut state = self.state();
        state.active = !state.stop;
        state.active
    }

    /// Stops keeping notes and drops what was noted. Returns the retried candidates no run will
    /// attempt; their references stay pending.
    pub(super) fn finish(&self) -> usize {
        let mut state = self.state();
        Self::end(&mut state);
        std::mem::take(&mut state.retried).len()
    }

    fn end(state: &mut State) {
        state.active = false;
        state.viewed.clear();
        state.touched.clear();
    }

    /// Hands a retried candidate to the running driver, or starts keeping notes for a new run.
    pub(super) fn offer(&self, candidate: Admitted) -> Offer {
        let mut state = self.state();
        if state.stop {
            Offer::Refused
        } else if state.active {
            state.retried.push_back(candidate);
            Offer::Queued
        } else {
            state.active = true;
            Offer::Start(candidate)
        }
    }

    /// The next retried candidate, or none, which ends the run: notes stop, and the next Retry
    /// starts a new run.
    pub(super) fn next_retried(&self) -> Option<Admitted> {
        let mut state = self.state();
        let next = state.retried.pop_front();
        if next.is_none() {
            Self::end(&mut state);
        }
        next
    }

    pub(super) fn stop(&self) {
        self.state().stop = true;
        self.changed.notify_all();
    }

    pub(super) fn stopping(&self) -> bool {
        self.state().stop
    }

    pub(super) fn note_view(&self, target: BindingViewerTarget) {
        let mut state = self.state();
        if state.active {
            state.first_view.get_or_insert_with(Instant::now);
            state.viewed.insert(ControlTarget::from(target));
            self.changed.notify_all();
        }
    }

    pub(super) fn note_input(&self, target: BindingViewerTarget) {
        let mut state = self.state();
        if state.active {
            state.touched.insert(ControlTarget::from(target));
        }
    }

    pub(super) fn viewed(&self, target: BindingViewerTarget) -> bool {
        self.state().viewed.contains(&ControlTarget::from(target))
    }

    pub(super) fn touched(&self, target: BindingViewerTarget) -> bool {
        self.state().touched.contains(&ControlTarget::from(target))
    }

    /// Waits for the first Client view, then until `settle` has passed since it. False when a stop
    /// arrives first.
    pub(super) fn wait_for_view(&self, settle: Duration) -> bool {
        let mut state = self.state();
        loop {
            if state.stop {
                return false;
            }
            let wait = match state.first_view {
                None => Duration::from_secs(3600),
                Some(viewed) => match (viewed + settle).checked_duration_since(Instant::now()) {
                    Some(wait) if !wait.is_zero() => wait,
                    _ => return true,
                },
            };
            state = self
                .changed
                .wait_timeout(state, wait)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Sleeps until `until` unless a stop arrives first, which returns false.
    pub(super) fn sleep_until(&self, until: Instant) -> bool {
        let mut state = self.state();
        loop {
            if state.stop {
                return false;
            }
            let Some(wait) = until
                .checked_duration_since(Instant::now())
                .filter(|wait| !wait.is_zero())
            else {
                return true;
            };
            state = self
                .changed
                .wait_timeout(state, wait)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}
