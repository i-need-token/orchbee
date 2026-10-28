//! One provider command's phase deadlines inside the invocation budget.
use std::time::{Duration, Instant};

/// One command slot: three seconds to prepare/run and 600 milliseconds to
/// terminate and confirm collection of the owned process tree.
pub(crate) const SLOT: Duration = Duration::from_millis(3_600);
const RUN: Duration = Duration::from_millis(3_000);
const TEARDOWN: Duration = Duration::from_millis(600);

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
