//! Exact-viewer clear admission and local feedback; the Server remains the screen-effect owner.
use super::{
    ClientActionDisposition, QueuedCommand, Tagged, WorkbenchClientNotice, WorkbenchClientUi,
};
use crate::workbench_state::PaneRef;
use orchbee_client_server_protocol::TerminalControlOutcome;
use std::time::Duration;

impl WorkbenchClientUi {
    /// A clear that worked says nothing; it only replaces an earlier clear notice.
    fn note_terminal_clear(&mut self, notice: Option<WorkbenchClientNotice>) {
        let until = self.clock.now().checked_add(Duration::from_secs(5));
        self.command_notice.set(notice.zip(until));
    }

    pub(super) fn request_terminal_clear(&mut self) -> ClientActionDisposition {
        let key = self
            .state
            .as_ref()
            .filter(|state| state.ready().is_some())
            .and_then(|state| state.focus_pane_ref());
        let admitted = !self.navigation_pending()
            && self.retirement_failure.is_none()
            && key.is_some_and(|key| {
                self.mailbox.as_ref().is_some_and(|mailbox| {
                    mailbox
                        .try_enqueue(QueuedCommand::ClearTerminal(Tagged::new(key, ())))
                        .is_ok()
                })
            });
        self.note_terminal_clear(
            (!admitted).then_some(WorkbenchClientNotice::TerminalClearUnavailable),
        );
        ClientActionDisposition::DismissNavigator
    }

    pub(super) fn apply_terminal_clear_outcome(
        &mut self,
        key: PaneRef,
        outcome: TerminalControlOutcome,
    ) -> bool {
        if self
            .state
            .as_ref()
            .and_then(|state| state.focus_pane_ref())
            .as_ref()
            != Some(&key)
        {
            return false;
        }
        let notice = match outcome {
            TerminalControlOutcome::Cleared {
                cleanup_confirmed: true,
                ..
            } => None,
            TerminalControlOutcome::Cleared {
                cleanup_confirmed: false,
                ..
            } => Some(WorkbenchClientNotice::TerminalClearUnconfirmed),
            TerminalControlOutcome::Rejected { .. } => {
                Some(WorkbenchClientNotice::TerminalClearRefused)
            }
            TerminalControlOutcome::Unavailable { .. } => {
                Some(WorkbenchClientNotice::TerminalClearUnavailable)
            }
            TerminalControlOutcome::OutcomeUnknown {} => {
                Some(WorkbenchClientNotice::TerminalClearUnknown)
            }
            _ => {
                self.retire(crate::workbench_state::CarrierFailure::Protocol);
                return true;
            }
        };
        self.note_terminal_clear(notice);
        true
    }
}

#[cfg(test)]
mod tests;
