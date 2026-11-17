//! The notice for one controller action that did not take effect. The pane keeps or loses control
//! by its own fact; this owner only says what became of the action, and never replays it.
use super::{WorkbenchClientNotice, WorkbenchClientUi};
use crate::workbench_client::mailbox::{ControlAction, ControlActionFailure};
use crate::workbench_state::PaneRef;
use std::time::Duration;

impl WorkbenchClientUi {
    /// Shows the failure for five seconds while `key` is still the pane's incarnation; a late
    /// answer for a departed incarnation says nothing.
    pub(super) fn note_control_action_failure(
        &mut self,
        key: &PaneRef,
        action: ControlAction,
        failure: ControlActionFailure,
    ) -> bool {
        let current = self.state.as_ref().is_some_and(|state| {
            state
                .pane_record(key.pane())
                .is_some_and(|record| record.pane_ref() == *key)
        });
        if !current {
            return false;
        }
        let notice = match (action, failure) {
            (ControlAction::Input, ControlActionFailure::NotDone) => {
                WorkbenchClientNotice::TerminalInputNotSent
            }
            (ControlAction::Resize, ControlActionFailure::NotDone) => {
                WorkbenchClientNotice::TerminalResizeNotDone
            }
            (_, ControlActionFailure::Unknown) => WorkbenchClientNotice::TerminalActionUnknown,
        };
        let until = self.clock.now().checked_add(Duration::from_secs(5));
        self.command_notice.set(until.map(|until| (notice, until)));
        true
    }
}
