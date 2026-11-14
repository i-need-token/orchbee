//! "Forget password" on a selected remote Machine: its key, the request the shell carries to
//! discovery, and the outcome the picker's status line reports. The action is offered, and its key
//! taken, only while it has a target: a Machine's own row while this platform's credential store
//! can be used. It deletes both remembered answers of the Machine without reading either.
//! `apps/orchbee/docs/machines.md#remembered-answers` owns the rule.

use super::{Choice, Picker};
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use crate::machine_credentials::Remembering;
use crate::machine_discovery::{Discovery, Forgotten};
use crate::machines::ProfileId;
use crate::terminal_events::keys::Binding;

/// The key that forgets the selected remote Machine's saved password; the handler and the painted
/// action both read it.
pub(crate) const FORGET_KEY: Binding = Binding::new(KeyCode::Char('g'), Modifiers::CTRL);

impl Picker {
    /// The remote Machine whose saved password the row at `index` can forget: a Machine's own row,
    /// while the store can be used.
    pub(crate) fn forget_target(&self, index: usize) -> Option<&ProfileId> {
        if !self.forgettable {
            return None;
        }
        let row = self.visible_rows().get(index)?;
        match &row.choice {
            Choice::Remote(id) if row.context.is_none() && row.agent.is_none() => Some(id),
            _ => None,
        }
    }

    /// Takes the forget key while the selected row can forget; any key first clears the last
    /// outcome. Without a target the key is not this action's and goes on to the picker's others.
    pub(super) fn forget_key(&mut self, key: KeyEvent) -> bool {
        self.forget_notice = None;
        if !FORGET_KEY.matches(key) {
            return false;
        }
        let Some(target) = self.forget_target(self.selected).cloned() else {
            return false;
        };
        self.forget_request = Some(target);
        true
    }

    /// Asks discovery to forget the requested Machine's saved password and reports how the last
    /// forget ended. True when the picker changed.
    pub fn exchange_forget(&mut self, discovery: &mut Discovery) -> bool {
        let mut changed = false;
        // The store is asked whether it can be used only once a remote row is selected, off the
        // interface thread; the action appears once it answered.
        if self.open && !self.forgettable {
            let selected = self.visible_rows().get(self.selected);
            if selected.is_some_and(|row| matches!(row.choice, Choice::Remote(_))) {
                self.forgettable = discovery
                    .remembering()
                    .is_some_and(|remembering| remembering != Remembering::Unavailable);
                changed |= self.forgettable;
            }
        }
        if let Some(machine) = self.forget_request.take()
            && !discovery.forget_password(&machine)
        {
            self.forget_notice = Some(Forgotten::Failed);
            changed = true;
        }
        if let Some(outcome) = discovery.forgotten() {
            self.forget_notice = Some(outcome);
            changed = true;
        }
        changed
    }
}
