//! Request identity: a mutation names a `requestId`, and a repeated one answers the outcome its
//! first run recorded instead of running again. Each Workspace keeps its last 256 outcomes for ten
//! minutes, in memory only; a crash loses them, which the Client learns from the mutation stream's
//! OutcomeUnknown close.

use orchbee_files_protocol::RequestId;
use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};

const KEPT: usize = 256;
const KEPT_FOR: Duration = Duration::from_secs(10 * 60);

/// What a mutation that names an identity does.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Begin<T> {
    /// The identity is new: the mutation runs, and its outcome is recorded with [`Requests::complete`].
    Run,
    /// A run with this identity has not ended; its outcome answers this request too.
    Running,
    /// A run with this identity ended with this outcome, which answers this request.
    Completed(T),
}

/// What `request.outcome` answers.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Outcome<T> {
    Completed(T),
    Running,
    /// Never seen, forgotten after ten minutes or 256 later outcomes, or lost with a crash.
    Unknown,
}

/// One Workspace's identities.
pub(crate) struct Requests<T> {
    running: HashSet<RequestId>,
    completed: VecDeque<(RequestId, Instant, T)>,
}

impl<T: Clone> Requests<T> {
    pub(crate) fn new() -> Self {
        Self {
            running: HashSet::new(),
            completed: VecDeque::new(),
        }
    }

    pub(crate) fn begin(&mut self, now: Instant, id: RequestId) -> Begin<T> {
        match self.outcome(now, id) {
            Outcome::Completed(outcome) => Begin::Completed(outcome),
            Outcome::Running => Begin::Running,
            Outcome::Unknown => {
                self.running.insert(id);
                Begin::Run
            }
        }
    }

    /// Records how the run of `id` ended, whether or not its Client still waits.
    pub(crate) fn complete(&mut self, now: Instant, id: RequestId, outcome: T) {
        if self.running.remove(&id) {
            self.completed.push_back((id, now, outcome));
            let excess = self.completed.len().saturating_sub(KEPT);
            self.completed.drain(..excess);
        }
    }

    pub(crate) fn outcome(&mut self, now: Instant, id: RequestId) -> Outcome<T> {
        self.forget(now);
        if self.running.contains(&id) {
            return Outcome::Running;
        }
        self.completed
            .iter()
            .find(|(kept, _, _)| *kept == id)
            .map_or(Outcome::Unknown, |(_, _, outcome)| {
                Outcome::Completed(outcome.clone())
            })
    }

    /// Whether no run is open and every outcome was forgotten, so nothing is left to answer.
    pub(crate) fn idle(&mut self, now: Instant) -> bool {
        self.forget(now);
        self.running.is_empty() && self.completed.is_empty()
    }

    /// Forgets the outcomes kept for ten minutes.
    fn forget(&mut self, now: Instant) {
        let expired = self
            .completed
            .iter()
            .take_while(|(_, at, _)| now.saturating_duration_since(*at) >= KEPT_FOR)
            .count();
        self.completed.drain(..expired);
    }
}

#[cfg(test)]
mod tests;
