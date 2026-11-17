//! One provider command's phase deadlines inside the invocation budget.
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

/// One command slot: eight seconds to prepare/run and 600 milliseconds to
/// terminate and confirm collection of the owned process tree. Eight seconds
/// fits a slow but healthy Agent CLI (Hermes measured 6.4 seconds) and still
/// bounds a hung one; the slot stays within the 40-second invocation budget.
pub(crate) const SLOT: Duration = Duration::from_millis(8_600);
const RUN: Duration = Duration::from_millis(8_000);
const TEARDOWN: Duration = Duration::from_millis(600);

/// The one clock every admission gate and command span reads. Under test-support on
/// unix it is the fault-injection clock, so a test's skew moves both together.
#[cfg(all(any(test, feature = "test-support"), unix))]
#[inline]
pub(crate) fn now() -> Instant {
    super::test_support::fault_injection::now()
}

#[cfg(not(all(any(test, feature = "test-support"), unix)))]
#[inline]
pub(crate) fn now() -> Instant {
    Instant::now()
}

/// The reserve one later re-inspection read needs. Until a provider command has
/// finished in this invocation it is a full slot; afterwards it is the slowest
/// finished command times 1.5 plus teardown, never more than a full slot.
pub(crate) fn reserve_estimate(slowest: Option<Duration>) -> Duration {
    match slowest {
        None => SLOT,
        Some(longest) => SLOT.min(longest * 3 / 2 + TEARDOWN),
    }
}

/// One invocation's command budget: the single monotonic deadline every provider
/// command clips to, and the slowest provider command this invocation has finished.
#[derive(Clone, Copy)]
pub(crate) struct CommandBudget<'a> {
    pub(crate) deadline: Instant,
    pub(crate) slowest: &'a Cell<Option<Duration>>,
}

impl CommandBudget<'_> {
    /// Record a finished command's span, timed-out commands included; only the longest is kept.
    pub(crate) fn record(&self, span: Duration) {
        let longest = self.slowest.get().map_or(span, |slowest| slowest.max(span));
        self.slowest.set(Some(longest));
    }
}

#[derive(Clone, Copy)]
pub(super) struct CommandDeadlines {
    pub(super) run: Instant,
    pub(super) slot: Instant,
}

impl CommandDeadlines {
    /// Allocate the whole slot before any fallible preparation. Preparation
    /// consumes the run phase; teardown always retains its complete phase.
    /// A caller that cannot fund teardown must not start a process.
    pub(super) fn new(start: Instant, invocation_deadline: Instant) -> Option<Self> {
        let local_slot = start.checked_add(SLOT).unwrap_or(invocation_deadline);
        let slot = invocation_deadline.min(local_slot);
        let teardown_start = slot.checked_sub(TEARDOWN)?;
        if teardown_start <= start {
            return None;
        }
        let local_run = start.checked_add(RUN).unwrap_or(teardown_start);
        Some(Self {
            run: local_run.min(teardown_start),
            slot,
        })
    }
}

#[cfg(test)]
mod tests;
