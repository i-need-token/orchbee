//! When one repository's status runs. Pure: callers pass the time and the events, and carry out the
//! returned effects; the next wake comes from [`Refresh::next_wake`].
//!
//! A file change waits out a debounce, then for no write to run, no status to run, a focused
//! subscriber and the cooldown after the previous status. A finished write refreshes at once and
//! supersedes a running status. Agent completion and a manual refresh run at once, skipping the
//! debounce, the focus wait and the cooldown, after a running write, and once more after a running
//! status. With no subscriber nothing runs on its own.
use super::ports::Change;
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(1000);
const COOLDOWN: Duration = Duration::from_millis(5000);

/// What the caller does next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Effect {
    /// Start status run `generation`; its end comes back through [`Refresh::status_ended`].
    Start(u64),
    /// Cancel status run `generation`; its end is ignored.
    Cancel(u64),
}

#[derive(Debug)]
pub(super) struct Refresh {
    subscribed: bool,
    focused: bool,
    /// The last status passed the entry limit: content changes no longer refresh.
    large: bool,
    writes: u32,
    running: Option<u64>,
    generation: u64,
    debounce: Option<Instant>,
    /// A debounced change waits for its conditions.
    due: bool,
    /// A refresh that skips the debounce, the focus wait and the cooldown is wanted.
    urgent: bool,
    cooldown: Option<Instant>,
}

impl Refresh {
    pub(super) fn new() -> Self {
        Self {
            subscribed: false,
            focused: false,
            large: false,
            writes: 0,
            running: None,
            generation: 0,
            debounce: None,
            due: false,
            urgent: false,
            cooldown: None,
        }
    }

    /// Whether some Client subscribes to this Workspace's summary. Without one, only explicit
    /// refreshes run.
    pub(super) fn set_subscribed(&mut self, now: Instant, subscribed: bool) -> Vec<Effect> {
        self.subscribed = subscribed;
        if !subscribed {
            self.debounce = None;
            self.due = false;
        }
        self.pump(now)
    }

    /// Whether a subscribed Client that views this Workspace has focus.
    pub(super) fn set_focused(&mut self, now: Instant, focused: bool) -> Vec<Effect> {
        self.focused = focused;
        self.pump(now)
    }

    pub(super) fn changed(&mut self, now: Instant, change: Change) -> Vec<Effect> {
        // A running write refreshes when it ends.
        if !self.subscribed || self.writes > 0 || (self.large && change == Change::Content) {
            return Vec::new();
        }
        self.debounce = Some(now + DEBOUNCE);
        Vec::new()
    }

    /// Agent completion in the Workspace, a manual refresh, or a repository just opened.
    pub(super) fn refresh(&mut self, now: Instant) -> Vec<Effect> {
        self.urgent = true;
        self.pump(now)
    }

    pub(super) fn write_started(&mut self) {
        self.writes += 1;
    }

    /// A write ended, whether or not it succeeded: what it changed is refreshed at once, and a
    /// status that started before it no longer answers.
    pub(super) fn write_ended(&mut self, now: Instant) -> Vec<Effect> {
        self.writes = self.writes.saturating_sub(1);
        let mut effects = Vec::new();
        if self.writes == 0
            && let Some(running) = self.running.take()
        {
            effects.push(Effect::Cancel(running));
        }
        self.urgent = true;
        effects.extend(self.pump(now));
        effects
    }

    /// Status run `generation` ended. `truncated` is whether it passed the entry limit; a cancelled
    /// or failed run keeps the previous answer.
    pub(super) fn status_ended(
        &mut self,
        now: Instant,
        generation: u64,
        truncated: Option<bool>,
    ) -> Vec<Effect> {
        if self.running != Some(generation) {
            return Vec::new();
        }
        self.running = None;
        if let Some(truncated) = truncated {
            self.large = truncated;
        }
        self.cooldown = Some(now + COOLDOWN);
        self.pump(now)
    }

    /// The timers fired that were due at `now`.
    pub(super) fn wake(&mut self, now: Instant) -> Vec<Effect> {
        self.pump(now)
    }

    /// The status run whose end still answers, if one runs.
    pub(super) const fn running(&self) -> Option<u64> {
        self.running
    }

    /// When the caller calls [`Self::wake`] next, if anything waits for time.
    pub(super) fn next_wake(&self) -> Option<Instant> {
        let waiting = self.due && self.focused && self.running.is_none() && self.writes == 0;
        let cooling = waiting.then_some(self.cooldown).flatten();
        [self.debounce, cooling].into_iter().flatten().min()
    }

    fn pump(&mut self, now: Instant) -> Vec<Effect> {
        if self.debounce.is_some_and(|until| until <= now) {
            self.debounce = None;
            self.due = true;
        }
        if self.cooldown.is_some_and(|until| until <= now) {
            self.cooldown = None;
        }
        if self.writes > 0 || self.running.is_some() {
            return Vec::new();
        }
        let automatic = self.subscribed && self.due && self.focused && self.cooldown.is_none();
        if !self.urgent && !automatic {
            return Vec::new();
        }
        // The run reads the tree after every change seen so far, so none of them waits any more.
        self.urgent = false;
        self.due = false;
        self.debounce = None;
        self.generation += 1;
        self.running = Some(self.generation);
        vec![Effect::Start(self.generation)]
    }
}

#[cfg(test)]
mod tests;
