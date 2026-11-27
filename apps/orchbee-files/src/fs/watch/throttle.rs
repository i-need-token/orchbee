//! VS Code's `ThrottledWorker` as its recursive watcher configures it: changes pass on in chunks of
//! at most 500, the first at once and each further one 200 ms after the one before, and at most
//! 30,000 wait. A batch that would pass that bound is refused whole; VS Code drops it, and the
//! watch reports a rescan instead. Time is the caller's, so the rules hold under any clock.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// Changes passed on at once.
const CHUNK: usize = 500;
/// The rest between two chunks.
const REST: Duration = Duration::from_millis(200);
/// Changes that may wait.
const BUFFERED: usize = 30_000;

/// The changes that wait to pass on.
pub(crate) struct Throttle<T> {
    waiting: VecDeque<T>,
    /// When the next chunk passes; absent while not throttling.
    resting_until: Option<Instant>,
}

impl<T> Throttle<T> {
    pub(crate) const fn new() -> Self {
        Self {
            waiting: VecDeque::new(),
            resting_until: None,
        }
    }

    /// Offers `batch` at `now` and returns the chunk that passes on at once, which is empty while
    /// throttling.
    ///
    /// # Errors
    ///
    /// The batch, refused whole, when it would leave more than 30,000 changes waiting; what already
    /// waits stays.
    pub(crate) fn offer(&mut self, batch: Vec<T>, now: Instant) -> Result<Vec<T>, Vec<T>> {
        let total = self.waiting.len() + batch.len();
        // Unthrottled, the first chunk passes at once rather than waiting.
        let refused = match self.resting_until {
            Some(_) => total > BUFFERED,
            None => total.saturating_sub(CHUNK) > BUFFERED,
        };
        if refused {
            return Err(batch);
        }
        self.waiting.extend(batch);
        Ok(match self.resting_until {
            Some(_) => Vec::new(),
            None => self.pass(now),
        })
    }

    /// When the next chunk passes, while throttling.
    pub(crate) const fn due(&self) -> Option<Instant> {
        self.resting_until
    }

    /// The chunk that passes when the rest that ended at `now` is over.
    pub(crate) fn resume(&mut self, now: Instant) -> Vec<T> {
        self.resting_until = None;
        self.pass(now)
    }

    fn pass(&mut self, now: Instant) -> Vec<T> {
        let chunk: Vec<T> = self
            .waiting
            .drain(..self.waiting.len().min(CHUNK))
            .collect();
        if !self.waiting.is_empty() {
            self.resting_until = Some(now + REST);
        }
        chunk
    }
}

#[cfg(test)]
mod tests;
