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
mod tests {
    use super::*;
    #[test]
    fn unbounded_total_keeps_reads_bounded_and_finite_totals_never_renew() {
        let start = Instant::now();
        let infinite = WaitDeadline::new(start, None).unwrap();
        let later = start + Duration::from_secs(365 * 24 * 3600);
        assert!(!infinite.expired(later));
        assert_eq!(
            infinite.step(later, Duration::from_secs(5)).unwrap(),
            later + Duration::from_secs(5)
        );
        assert_eq!(
            infinite.backoff(later, Duration::from_millis(500)),
            Duration::from_millis(500)
        );
        let finite = WaitDeadline::new(start, Some(3000)).unwrap();
        let near_end = start + Duration::from_millis(2800);
        assert_eq!(
            finite.step(near_end, Duration::from_secs(5)).unwrap(),
            start + Duration::from_secs(3)
        );
        assert_eq!(
            finite.backoff(near_end, Duration::from_millis(500)),
            Duration::from_millis(200)
        );
        assert!(finite.expired(start + Duration::from_secs(3)));
    }
}
