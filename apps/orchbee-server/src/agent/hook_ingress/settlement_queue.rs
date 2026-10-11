//! Caller-driven settlement deadlines owned by hook ingress.

#[derive(Clone, Debug)]
struct SettlementTimer {
    event_expires_at: f64,
    event_key: String,
    fire_at: f64,
}

/// The settlement scheduler clock. `advance` moves only this clock; `pop_due`
/// yields timers whose `fire_at` is reached — the caller fires them with the
/// injected semantic `now`, keeping the two clocks independent.
#[derive(Default)]
pub(super) struct SettlementQueue {
    now: f64,
    timers: Vec<SettlementTimer>,
}

impl SettlementQueue {
    /// Repeating the same key and expiry preserves its deadline; a different
    /// expiry replaces the previously scheduled timer.
    pub(super) fn schedule(&mut self, event_key: String, event_expires_at: f64, milliseconds: f64) {
        if let Some(existing) = self
            .timers
            .iter()
            .find(|timer| timer.event_key == event_key)
            && existing.event_expires_at == event_expires_at
        {
            return;
        }
        self.timers.retain(|timer| timer.event_key != event_key);
        self.timers.push(SettlementTimer {
            event_expires_at,
            event_key,
            fire_at: self.now + milliseconds,
        });
    }

    /// `cancel-settlement`: keyed on `(event_key, event_expires_at)`.
    pub(super) fn cancel(&mut self, event_key: &str, event_expires_at: f64) {
        let Some(existing) = self
            .timers
            .iter()
            .find(|timer| timer.event_key == event_key)
        else {
            return;
        };
        if existing.event_expires_at != event_expires_at {
            return;
        }
        self.timers.retain(|timer| timer.event_key != event_key);
    }

    #[cfg(test)]
    pub(super) fn advance(&mut self, milliseconds: f64) {
        self.now += milliseconds;
    }

    pub(super) fn pop_due(&mut self) -> Option<(String, f64)> {
        let index = self
            .timers
            .iter()
            .position(|timer| timer.fire_at <= self.now)?;
        let timer = self.timers.remove(index);
        Some((timer.event_key, timer.event_expires_at))
    }

    /// Monotone sync toward the freshest known time — the production
    /// call-time clock adapter. Only the new Instant-domain seams reach
    /// this; the corpus `advance`/`pop_due` pair can never, so the
    /// recorded two-clock contract is preserved by non-reachability
    /// rather than by any claimed invariance. A stale or out-of-order
    /// `now` can never move the scheduler backward.
    pub(super) fn sync(&mut self, now: f64) {
        self.now = self.now.max(now);
    }

    /// Earliest pending `fire_at` — the production poll deadline source.
    /// Returns `None` while no timer producer has scheduled anything.
    pub(super) fn next_fire_at(&self) -> Option<f64> {
        self.timers
            .iter()
            .map(|timer| timer.fire_at)
            .reduce(f64::min)
    }
}
