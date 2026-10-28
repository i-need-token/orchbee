//! Holds the closing presentation of a pane whose Shell exited with code 0. Server closes the
//! only Presentation of such a Panel through the canonical Close, so for a bounded moment the
//! pane keeps its final pixels and label and raises no exit, read-only or viewer-loss cue until
//! it leaves membership. A pane still present at the bound returns to the ordinary settlement
//! presentation. Holds never change attachment, input, selection, membership or Server state.

use crate::localization::TerminalEnd;
use orchbee_terminal_protocol::ExitResult;
use std::time::{Duration, Instant};

/// Server's Close removes a closed Shell's pane well within this bound. It also limits how long
/// an exit that Server keeps, such as a non-interactive program's, waits for its settlement cue.
pub(super) const BOUND: Duration = Duration::from_secs(1);

/// Closing holds keyed by the exact pane incarnation `K` whose terminal settled.
pub(super) struct NormalExitHolds<K> {
    held: Vec<(K, Instant)>,
}

impl<K> Default for NormalExitHolds<K> {
    fn default() -> Self {
        Self { held: Vec::new() }
    }
}

impl<K: Clone + PartialEq> NormalExitHolds<K> {
    /// Starts a hold for the pane that just settled with `end`. Only exit code 0 of a Panel's
    /// `sole` Presentation qualifies, because only that pane closes with its Shell.
    pub(super) fn observe(&mut self, key: &K, end: TerminalEnd, sole: bool, now: Instant) {
        if end != TerminalEnd::Exited(ExitResult::ExitCode(0)) || !sole || self.holds(key) {
            return;
        }
        if let Some(until) = now.checked_add(BOUND) {
            self.held.push((key.clone(), until));
        }
    }

    pub(super) fn holds(&self, key: &K) -> bool {
        self.held.iter().any(|(held, _)| held == key)
    }

    pub(super) fn active(&self) -> bool {
        !self.held.is_empty()
    }

    /// Ends every hold that reached its bound or whose pane is no longer `present`. Returns
    /// whether any presentation changed.
    pub(super) fn release(&mut self, now: Instant, present: impl Fn(&K) -> bool) -> bool {
        let before = self.held.len();
        self.held
            .retain(|(key, until)| now < *until && present(key));
        self.held.len() != before
    }

    pub(super) fn clear(&mut self) {
        self.held.clear();
    }
}

#[cfg(test)]
mod tests;
