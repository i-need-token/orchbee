//! The idle move: a Server older than its store's active version ends its Session through the
//! ordinary stop once the Session has stayed idle for one quiet minute, so the next Client start
//! runs the active version. A connected Client does not keep the Session: its record names the
//! version the Session moved to, and the Client starts that version. This rule owns only the
//! countdown and the order of the closing checks; the managed Session looks, asks and stops.
use std::{
    cmp::Ordering,
    time::{Duration, Instant},
};

/// How long the Session stays idle, without a break, before the closing checks run.
pub(crate) const QUIET: Duration = Duration::from_secs(60);

/// What one look at the Session found. Anything found restarts the countdown.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Activity {
    /// A terminal runs, or a launch has not settled.
    pub(crate) terminal: bool,
    /// The update helper runs.
    pub(crate) helper: bool,
    /// A Workbench operation is pending.
    pub(crate) operation: bool,
}

/// The checks that close a completed countdown, asked in this order and only then: the first reads
/// the store and the second asks the Worker.
pub(crate) trait Closing {
    /// Whether this Server is older than its store's active version; unknown reads as not older.
    fn superseded(&mut self) -> bool;
    /// Whether the Worker runs no plugin job and no configured command; any failure reads as
    /// running.
    fn worker_idle(&mut self) -> bool;
    /// Begins the ordinary stop unless anything arrived since the last look; true once begun.
    fn begin(&mut self) -> bool;
}

/// Idle time counted since the first idle look; a new countdown has none.
#[derive(Debug, Default)]
pub(crate) struct Countdown {
    since: Option<Instant>,
}

impl Countdown {
    /// Feeds one look taken at `now` and answers whether the stop began. Activity restarts the
    /// count. Once the Session has been idle for [`QUIET`], the closing checks run, and a check
    /// that fails starts a new countdown from `now`, so they run at most once a minute.
    pub(crate) fn observe(
        &mut self,
        now: Instant,
        activity: Activity,
        closing: &mut impl Closing,
    ) -> bool {
        if activity != Activity::default() {
            self.since = None;
            return false;
        }
        let since = *self.since.get_or_insert(now);
        if now.saturating_duration_since(since) < QUIET {
            return false;
        }
        self.since = Some(now);
        closing.superseded() && closing.worker_idle() && closing.begin()
    }
}

/// Whether a Server of version `own` is older than the store's `active` version. Only an older
/// Server moves: an equal one has nowhere to go, and a newer one would go back.
pub(crate) fn older(own: &str, active: &str) -> bool {
    orchbee_local_runtime::compare_product_versions(own, active) == Ordering::Less
}

#[cfg(test)]
mod tests;
