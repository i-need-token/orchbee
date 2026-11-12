//! The one retry policy for saved machines, with the time an argument: the delay after each
//! consecutive failure and the online span that forgets them. The machine hub and the TUI's
//! machine discovery both schedule their attempts with it.
use std::time::{Duration, Instant};

/// The first delay, doubled after each consecutive failure.
const FIRST_DELAY: Duration = Duration::from_millis(500);
/// The longest delay.
const LONGEST_DELAY: Duration = Duration::from_secs(120);
/// A failure after this long online starts the count again.
const SUSTAINED_ONLINE: Duration = Duration::from_secs(60);

/// One machine's failure history.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Backoff {
    failures: u32,
    online_since: Option<Instant>,
}

impl Backoff {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            failures: 0,
            online_since: None,
        }
    }

    /// The machine is online at `now`; the first call after a failure starts the online span.
    pub fn online(&mut self, now: Instant) {
        self.online_since.get_or_insert(now);
    }

    /// The machine left online without a failure being counted yet.
    pub const fn interrupted(&mut self) {
        self.online_since = None;
    }

    /// Counts a failure at `now` and returns the delay before the next attempt: 500 ms after the
    /// first consecutive failure, doubled after each further one, at most 120 seconds. A failure
    /// that ends 60 seconds or more online counts as the first again.
    pub fn fail(&mut self, now: Instant) -> Duration {
        if self
            .online_since
            .take()
            .is_some_and(|since| now.saturating_duration_since(since) >= SUSTAINED_ONLINE)
        {
            self.failures = 0;
        }
        let delay = FIRST_DELAY
            .saturating_mul(1 << self.failures.min(8))
            .min(LONGEST_DELAY);
        self.failures = self.failures.saturating_add(1);
        delay
    }

    /// Forgets every failure, as an explicit disconnect does.
    pub const fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests;
