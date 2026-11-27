//! When the Server's `orchbee-files` process runs, as a state machine over the instants its owner
//! passes in. A call routes over a ready link, waits for a start, or is refused; a demand starts
//! the process, and serving nothing for ten minutes drains it. A crash restarts it on the next
//! demand after 0, 1, 5 or 30 seconds, and the fifth crash within ten minutes leaves it down until
//! the Worker is replaced. A release build never starts it.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// How long a ready process may serve no call and hold no work before it is drained.
const IDLE_EXIT: Duration = Duration::from_secs(600);
/// How far back crashes count toward giving up.
const CRASH_WINDOW: Duration = Duration::from_secs(600);
/// The delay before the start that follows the first, second, third and fourth crash in the
/// window; the fifth gives up.
const BACKOFF: [Duration; 4] = [
    Duration::ZERO,
    Duration::from_secs(1),
    Duration::from_secs(5),
    Duration::from_secs(30),
];
/// How long a drained process may take to end its link before the Server closes it.
const DRAIN_GRACE: Duration = Duration::from_secs(10);

/// Why a call is not served.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    /// A release build never starts the process.
    Withheld,
    /// The process failed, and the next demand may start it again.
    Restarting,
    /// The process crashed five times within ten minutes.
    CrashedRepeatedly,
    /// The cohort's process comes from another build.
    BuildMismatch,
    /// The Session is stopping.
    Stopping,
}

/// What happens to one call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Demand {
    /// The link is ready: the call goes out now and counts as open until it ends.
    Route,
    /// The call waits for the process to start.
    Queue,
    /// The call is answered at once.
    Refuse(Refusal),
}

/// What the supervisor does next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Step {
    /// Start the process.
    Start,
    /// Drain this link, which served nothing for ten minutes.
    Drain(u64),
    /// Close this link, which its drain did not end in time.
    Close(u64),
    /// Nothing is due before this instant, or before the next change when `None`.
    Wait(Option<Instant>),
}

/// How a start failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StartFailure {
    /// No Worker served, so nothing was started.
    NoWorker,
    /// The process comes from another build.
    BuildMismatch,
    /// Anything else, which counts as a crash.
    Crashed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Withheld,
    Idle {
        not_before: Option<Instant>,
    },
    Starting,
    Ready {
        link: u64,
        last_use: Instant,
        calls: usize,
        holds: bool,
    },
    Draining {
        link: u64,
        deadline: Instant,
    },
    CrashedRepeatedly,
    BuildMismatch,
    Stopped,
}

/// The process's phase and the crashes that count toward giving up.
#[derive(Debug)]
pub(super) struct Lifecycle {
    phase: Phase,
    crashes: VecDeque<Instant>,
}

impl Lifecycle {
    /// A lifecycle with no process; `withheld` for a build that never starts one.
    pub(super) fn new(withheld: bool) -> Self {
        Self {
            phase: if withheld {
                Phase::Withheld
            } else {
                Phase::Idle { not_before: None }
            },
            crashes: VecDeque::new(),
        }
    }

    /// Decides one call at `now`. A routed call counts as open until [`Self::call_ended`].
    pub(super) fn demand(&mut self, now: Instant) -> Demand {
        match &mut self.phase {
            Phase::Withheld => Demand::Refuse(Refusal::Withheld),
            Phase::Idle { .. } | Phase::Starting | Phase::Draining { .. } => Demand::Queue,
            Phase::Ready {
                last_use, calls, ..
            } => {
                *last_use = now;
                *calls += 1;
                Demand::Route
            }
            Phase::CrashedRepeatedly => Demand::Refuse(Refusal::CrashedRepeatedly),
            Phase::BuildMismatch => Demand::Refuse(Refusal::BuildMismatch),
            Phase::Stopped => Demand::Refuse(Refusal::Stopping),
        }
    }

    /// The step due at `now`, while calls wait for a start when `wanted`.
    pub(super) fn step(&mut self, now: Instant, wanted: bool) -> Step {
        match self.phase {
            Phase::Idle { not_before } if wanted => match not_before {
                Some(at) if now < at => Step::Wait(Some(at)),
                _ => {
                    self.phase = Phase::Starting;
                    Step::Start
                }
            },
            Phase::Ready {
                link,
                last_use,
                calls: 0,
                holds: false,
            } => {
                let deadline = last_use + IDLE_EXIT;
                if now < deadline {
                    Step::Wait(Some(deadline))
                } else {
                    self.phase = Phase::Draining {
                        link,
                        deadline: now + DRAIN_GRACE,
                    };
                    Step::Drain(link)
                }
            }
            Phase::Draining { link, deadline } if now >= deadline => Step::Close(link),
            Phase::Draining { deadline, .. } => Step::Wait(Some(deadline)),
            _ => Step::Wait(None),
        }
    }

    /// A start made `link` ready at `now`; false when the Session stopped meanwhile, so the link
    /// must be drained at once.
    pub(super) fn started(&mut self, now: Instant, link: u64) -> bool {
        if self.phase != Phase::Starting {
            return false;
        }
        self.phase = Phase::Ready {
            link,
            last_use: now,
            calls: 0,
            holds: false,
        };
        true
    }

    /// A start failed at `now`; the refusal answers the calls that waited for it.
    pub(super) fn start_failed(&mut self, now: Instant, failure: StartFailure) -> Refusal {
        if self.phase != Phase::Starting {
            return Refusal::Stopping;
        }
        match failure {
            StartFailure::NoWorker => {
                self.phase = Phase::Idle { not_before: None };
                Refusal::Restarting
            }
            StartFailure::BuildMismatch => {
                self.phase = Phase::BuildMismatch;
                Refusal::BuildMismatch
            }
            StartFailure::Crashed => self.crash(now),
        }
    }

    /// A routed call ended at `now`.
    pub(super) fn call_ended(&mut self, now: Instant, link: u64) {
        if let Phase::Ready {
            link: ready,
            last_use,
            calls,
            ..
        } = &mut self.phase
            && *ready == link
        {
            *calls = calls.saturating_sub(1);
            if *calls == 0 {
                *last_use = now;
            }
        }
    }

    /// The process reported at `now` whether it holds work no open call covers.
    pub(super) fn holds(&mut self, now: Instant, link: u64, holding: bool) {
        if let Phase::Ready {
            link: ready,
            last_use,
            holds,
            ..
        } = &mut self.phase
            && *ready == link
        {
            if *holds && !holding {
                *last_use = now;
            }
            *holds = holding;
        }
    }

    /// `link` ended at `now`. Ending a ready link counts as a crash; a drained one's end, or a
    /// link already replaced, does not. True when this end was the crash that gives up.
    pub(super) fn link_ended(&mut self, now: Instant, link: u64) -> bool {
        match self.phase {
            Phase::Ready { link: ready, .. } if ready == link => {
                self.crash(now) == Refusal::CrashedRepeatedly
            }
            Phase::Draining { link: draining, .. } if draining == link => {
                self.phase = Phase::Idle { not_before: None };
                false
            }
            _ => false,
        }
    }

    /// The Worker that started the process was replaced: its successor starts a fresh process
    /// on demand, with no crash counted against it. Returns the link the old Worker's process
    /// held, which no longer counts.
    pub(super) fn retire(&mut self) -> Option<u64> {
        let link = match self.phase {
            Phase::Ready { link, .. } | Phase::Draining { link, .. } => Some(link),
            _ => None,
        };
        if matches!(
            self.phase,
            Phase::Idle { .. }
                | Phase::Ready { .. }
                | Phase::Draining { .. }
                | Phase::CrashedRepeatedly
                | Phase::BuildMismatch
        ) {
            self.phase = Phase::Idle { not_before: None };
            self.crashes.clear();
        }
        link
    }

    /// The Session stops: nothing starts again. Returns the link to drain, if one serves.
    pub(super) fn stop(&mut self) -> Option<u64> {
        let link = match self.phase {
            Phase::Ready { link, .. } | Phase::Draining { link, .. } => Some(link),
            _ => None,
        };
        if self.phase != Phase::Withheld {
            self.phase = Phase::Stopped;
        }
        link
    }

    fn crash(&mut self, now: Instant) -> Refusal {
        while self
            .crashes
            .front()
            .is_some_and(|at| now.duration_since(*at) >= CRASH_WINDOW)
        {
            self.crashes.pop_front();
        }
        self.crashes.push_back(now);
        match BACKOFF.get(self.crashes.len() - 1) {
            Some(delay) => {
                self.phase = Phase::Idle {
                    not_before: Some(now + *delay),
                };
                Refusal::Restarting
            }
            None => {
                self.phase = Phase::CrashedRepeatedly;
                Refusal::CrashedRepeatedly
            }
        }
    }
}

#[cfg(test)]
mod tests;
