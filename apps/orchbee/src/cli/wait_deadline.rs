//! A wait can omit its total ceiling without unbounding any individual I/O.

use std::time::{Duration, Instant};

use super::{Failure, FailureKind};

#[derive(Clone, Copy)]
pub(super) struct WaitDeadline(Option<Instant>);

impl WaitDeadline {
    pub(super) fn new(start: Instant, timeout_ms: Option<u32>) -> Result<Self, Failure> {
        timeout_ms
            .map(|ms| {
                start
                    .checked_add(Duration::from_millis(u64::from(ms)))
                    .ok_or_else(|| Failure::new(FailureKind::InternalFailure))
            })
            .transpose()
            .map(Self)
    }

    pub(super) fn expired(self, now: Instant) -> bool {
        self.0.is_some_and(|total| now >= total)
    }

    pub(super) fn step(self, now: Instant, budget: Duration) -> Result<Instant, Failure> {
        let step = now
            .checked_add(budget)
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        Ok(self.0.map_or(step, |total| step.min(total)))
    }

    pub(super) fn backoff(self, now: Instant, budget: Duration) -> Duration {
        self.0.map_or(budget, |total| {
            total.saturating_duration_since(now).min(budget)
        })
    }
}

#[cfg(test)]
mod tests;
