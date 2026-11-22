//! Re-attaches a focused pane whose viewer was lost. A lost viewer does not prove that its
//! terminal ended, so after a short pause the Client admits the same-Tab switch a reselection
//! would, with the same panel focused: a live terminal recovers without a key press, and a
//! terminal that ended settles through the switch's own snapshot and attach. Attempts for one
//! pane are spaced out and bounded per window, so a viewer that keeps failing settles on the
//! unavailable status instead of cycling; a reselection still retries at once.
//!
//! While the Session's Server replaces a lost Worker, or once it gave up, no attach can succeed,
//! so no attempt starts. When it serves again, the shown Tab re-attaches at once under a fresh
//! recovery deadline, the attempt budget starts over, and the Agent streams the Server closed open
//! again; nothing announces the return.

use super::{
    ActionBuildError, AdmissionError, CarrierFailure, CommandMailbox, OperationDeadlines,
    SwitchMarker, WorkbenchClientState, WorkbenchClientUi, WorkbenchPanelId, WorkbenchTabId,
};
use orchbee_workbench_protocol::TerminalRuntimeState;
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
    /// Settles the Session's terminal-runtime stream. Serving again after a replacement arms the
    /// shown Tab's re-attach and reopens the Agent streams the Server closed. A shared connection
    /// that lost its Server retires the Client: what reconnects may be another Server, which holds
    /// none of its viewers or registration, so the shell starts a new Client in its place.
    pub(super) fn drive_terminal_runtime(&mut self) -> bool {
        let session = self
            .state
            .as_ref()
            .map(|state| state.cache().session_id().clone());
        let state = self.terminal_runtime.poll(session.as_ref(), &self.clock);
        if self.terminal_runtime.take_carrier_lost() {
            self.retire(CarrierFailure::Transport);
            return true;
        }
        let previous = std::mem::replace(&mut self.terminal_runtime_state, state);
        // A replacement too quick for its frames to arrive still ends in a later serving revision.
        let served_again = self.terminal_runtime.take_served_again();
        if previous == state && !served_again {
            return false;
        }
        self.runtime_reattach = state == TerminalRuntimeState::Serving;
        if self.runtime_reattach {
            self.agent_catalog.reopen_now(&self.clock);
            self.runtime_served = true;
        }
        true
    }

    /// Whether the Server served again since the composition last asked, so it reopens the title
    /// streams the Server closed.
    pub(in crate::workbench_client) fn take_runtime_served(&mut self) -> bool {
        std::mem::take(&mut self.runtime_served)
    }

    /// Admits the due re-attach of a lost focused viewer. A refusal by local work asks again
    /// shortly without spending an attempt and never shows a busy notice; only a broken Client
    /// invariant, identity, or carrier ends the Client.
    pub(super) fn drive_viewer_recovery(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        if self.terminal_runtime_state != TerminalRuntimeState::Serving {
            // Spent attempts belonged to a Worker that is gone; the replacement starts afresh.
            self.viewer_recovery = ViewerRecovery::default();
            return false;
        }
        if self.runtime_reattach {
            return self.drive_runtime_reattach(mailbox);
        }
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

impl WorkbenchClientUi {
    /// Admits the shown Tab's re-attach once the Server serves again. Nothing to re-attach ends
    /// it; a refusal by local work asks again on the next tick.
    fn drive_runtime_reattach(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        if !self
            .state
            .as_ref()
            .is_some_and(WorkbenchClientState::runtime_reattach_needed)
        {
            self.runtime_reattach = false;
            return false;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return true;
        };
        let Some(state) = self.state.as_mut() else {
            return false;
        };
        let result = mailbox.try_enqueue_switch_for(
            state,
            |state, generation| build_runtime_reattach(state, generation, deadlines),
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
                self.runtime_reattach = false;
                self.viewer_recovery = ViewerRecovery::default();
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
            ) => false,
            Err(error) => {
                self.retire_for_admission(&error);
                true
            }
        }
    }
}

fn build_runtime_reattach(
    state: &mut WorkbenchClientState,
    generation: u64,
    deadlines: OperationDeadlines,
) -> Result<SwitchMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    let target = state
        .target_for_runtime_reattach(generation, deadlines)
        .ok_or(ActionBuildError::Busy)?;
    if !state.begin_switch(target.clone()) {
        return Err(ActionBuildError::Protocol);
    }
    Ok(SwitchMarker::new(target))
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
