//! Holds the closing presentation of a pane whose Shell exited with code 0. Server closes the
//! only Presentation of such a Panel through the canonical Close, so for a bounded moment the
//! pane keeps its final pixels and label and raises no exit, read-only or viewer-loss cue until
//! it leaves membership. A pane still present at the bound returns to the ordinary settlement
//! presentation. Holds never change attachment, input, selection, membership or Server state.

use crate::localization::TerminalEnd;
use crate::workbench_client::state::{PaneRef, WorkbenchClientState};
use orchbee_terminal_protocol::ExitResult;
use std::time::{Duration, Instant};

/// Server's Close removes a closed Shell's pane well within this bound. It also limits how long
/// an exit that Server keeps, such as a non-interactive program's, waits for its settlement cue.
const BOUND: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct NormalExitHolds {
    held: Vec<(PaneRef, Instant)>,
}

impl NormalExitHolds {
    /// Starts a hold for the exact pane incarnation that just settled with `end`. Only exit
    /// code 0 of a Panel's only Presentation qualifies, because only that pane can close.
    pub(super) fn observe(&mut self, key: &PaneRef, end: TerminalEnd, sole: bool, now: Instant) {
        if end != TerminalEnd::Exited(ExitResult::ExitCode(0)) || !sole || self.holds(key) {
            return;
        }
        if let Some(until) = now.checked_add(BOUND) {
            self.held.push((key.clone(), until));
        }
    }

    pub(super) fn holds(&self, key: &PaneRef) -> bool {
        self.held.iter().any(|(held, _)| held == key)
    }

    pub(super) fn active(&self) -> bool {
        !self.held.is_empty()
    }

    /// Ends every hold that reached its bound or whose exact pane incarnation is no longer a
    /// `member`. Returns whether any presentation changed.
    pub(super) fn release(&mut self, now: Instant, member: impl Fn(&PaneRef) -> bool) -> bool {
        let before = self.held.len();
        self.held.retain(|(key, until)| now < *until && member(key));
        self.held.len() != before
    }

    pub(super) fn clear(&mut self) {
        self.held.clear();
    }
}

/// Whether `key` presents the only Presentation of its Panel.
pub(super) fn sole_presentation(state: &WorkbenchClientState, key: &PaneRef) -> bool {
    let pane = key.pane();
    state
        .cache()
        .tabs()
        .iter()
        .find(|tab| tab.tab_id() == pane.anchor().tab_id())
        .and_then(|tab| tab.panel(tab.panel_index(pane.panel())?))
        .is_some_and(|panel| panel.presentation_count() == 1)
}

/// Whether `key` is still the exact pane incarnation `state` holds.
pub(super) fn member(state: Option<&WorkbenchClientState>, key: &PaneRef) -> bool {
    state
        .and_then(|state| state.pane_record(key.pane()))
        .is_some_and(|record| record.pane_ref() == *key)
}

#[cfg(test)]
mod tests;
