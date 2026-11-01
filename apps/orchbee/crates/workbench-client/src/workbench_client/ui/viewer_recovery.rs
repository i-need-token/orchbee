//! Re-attaches a focused pane whose viewer was lost. A lost viewer does not prove that its
//! terminal ended, so after a short pause the Client admits the same-Tab switch a reselection
//! would, with the same panel focused: a live terminal recovers without a key press, and a
//! terminal that ended settles through the switch's own snapshot and attach. Attempts for one
//! pane are spaced out and bounded per window, so a viewer that keeps failing settles on the
//! unavailable status instead of cycling; a reselection still retries at once.

use super::{
    ActionBuildError, AdmissionError, CarrierFailure, CommandMailbox, OperationDeadlines,
    SwitchMarker, WorkbenchClientState, WorkbenchClientUi, WorkbenchPanelId, WorkbenchTabId,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The pause before each automatic attempt for one pane, measured from the loss it answers;
/// their count bounds the attempts in one window.
const PAUSES: [Duration; 3] = [
    Duration::from_millis(300),
    Duration::from_secs(1),
    Duration::from_secs(3),
];
/// How long one pane's attempt budget lasts, from the loss that opened it.
const WINDOW: Duration = Duration::from_secs(60);
/// The wait before asking again when local work holds the switch slot; it spends no attempt.
const BLOCKED_RETRY: Duration = Duration::from_millis(100);

/// The attempt budget of the pane most recently left without its viewer.
#[derive(Default)]
pub(super) struct ViewerRecovery {
    episode: Option<Episode>,
}

struct Episode {
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
    opened: Instant,
    attempts: usize,
    /// When the next attempt may start; unset until a loss is observed.
    due: Option<Instant>,
}

impl ViewerRecovery {
    /// Follows the focused pane whose viewer is lost, if any, and answers whether its next
    /// attempt is due within its budget.
    fn due(&mut self, lost: Option<(&WorkbenchTabId, &WorkbenchPanelId)>, now: Instant) -> bool {
        let Some((tab, panel)) = lost else {
            // A live or moved focus ends the pause; the spent attempts stay with their window.
            if let Some(episode) = &mut self.episode {
                episode.due = None;
            }
            return false;
        };
        let episode = match &mut self.episode {
            Some(episode)
                if &episode.tab == tab
                    && &episode.panel == panel
                    && now.saturating_duration_since(episode.opened) < WINDOW =>
            {
                episode
            }
            slot => slot.insert(Episode {
                tab: tab.clone(),
                panel: panel.clone(),
                opened: now,
                attempts: 0,
                due: None,
            }),
        };
        let Some(pause) = PAUSES.get(episode.attempts) else {
            return false;
        };
        now >= *episode.due.get_or_insert(now + *pause)
    }

    /// One attempt was admitted; the next one waits for the next loss.
    fn admitted(&mut self) {
        if let Some(episode) = &mut self.episode {
            episode.attempts += 1;
            episode.due = None;
        }
    }

    /// Local work held the switch slot; the same attempt is asked again shortly.
    fn blocked(&mut self, now: Instant) {
        if let Some(episode) = &mut self.episode {
            episode.due = Some(now + BLOCKED_RETRY);
        }
    }
}

impl WorkbenchClientUi {
    /// Admits the due re-attach of a lost focused viewer. A refusal by local work asks again
    /// shortly without spending an attempt and never shows a busy notice; only a broken Client
    /// invariant, identity, or carrier ends the Client.
    pub(super) fn drive_viewer_recovery(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        let now = self.clock.now();
        let focus = self
            .state
            .as_ref()
            .filter(|state| state.focused_viewer_lost())
            .and_then(WorkbenchClientState::focus_pane_ref);
        let lost = focus
            .as_ref()
            .map(|focus| (focus.pane().anchor().tab_id(), focus.pane().panel()));
        if !self.viewer_recovery.due(lost, now) {
            return false;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(now) else {
            self.retire(CarrierFailure::Deadline);
            return true;
        };
        let Some(state) = self.state.as_mut() else {
            return false;
        };
        let result = mailbox.try_enqueue_switch_for(
            state,
            |state, generation| build_recovery(state, generation, deadlines),
            |state, result, facts| {
                if result.is_ok() {
                    state.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                }
            },
        );
        match result {
            Ok(_generation) => {
                self.viewer_recovery.admitted();
                self.pending_draws.clear();
                self.ordinary_poll_serial = None;
                true
            }
            Err(
                AdmissionError::BootstrapPending
                | AdmissionError::Busy
                | AdmissionError::Full
                | AdmissionError::StaleAttachment
                | AdmissionError::Build(
                    ActionBuildError::Busy
                    | ActionBuildError::NoOp
                    | ActionBuildError::OutOfRange(_),
                ),
            ) => {
                self.viewer_recovery.blocked(now);
                false
            }
            Err(error) => {
                self.retire_for_admission(&error);
                true
            }
        }
    }
}

fn build_recovery(
    state: &mut WorkbenchClientState,
    generation: u64,
    deadlines: OperationDeadlines,
) -> Result<SwitchMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    let target = state
        .target_for_lost_focus_reattach(generation, deadlines)
        .ok_or(ActionBuildError::Busy)?;
    if !state.begin_switch(target.clone()) {
        return Err(ActionBuildError::Protocol);
    }
    Ok(SwitchMarker::new(target))
}

#[cfg(test)]
mod tests;
