//! The card over the terminals of a machine that is not connected: shown for a Workspace or Agent
//! the person chose on such a machine, until the choice lands, Not now or another choice, and for
//! the machine on screen while its connection is lost or needs a login. It says the machine's
//! state as Settings › Machines does and offers that state's one action and Not now.

use super::{Choice, Picker, Selection, SelectionOrigin};
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::machine_settings::{Action, Machine, State};
use crate::machines::ProfileId;

const ACT: Binding = Binding::new(KeyCode::Enter, Modifiers::NONE);
const NOT_NOW: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Cards {
    /// A choice of a Workspace or Agent on a machine that was not connected when chosen.
    pub(super) chosen: Option<Selection>,
    /// What Settings › Machines knows of the machine a card would show: the chosen one, otherwise
    /// the machine on screen.
    facts: Option<Machine>,
    /// The machine on screen in the state whose card Not now put away, until that state changes.
    dismissed: Option<(String, State)>,
    /// Names the card painted, so a press on a card that has since changed does nothing.
    token: u64,
    /// The card the token names: whether it shows a choice, and for which machine.
    shown: Option<(bool, String)>,
}

/// The card to paint.
pub(crate) struct Card<'a> {
    pub(crate) token: u64,
    pub(crate) machine: &'a Machine,
    /// For a choice still on its way, rather than for the machine on screen.
    pub(crate) chosen: bool,
    /// The state's one action, if it has one the card offers.
    pub(crate) action: Option<Action>,
}

/// The connection states in which the machine on screen cannot reach its terminals.
const fn lost(state: State) -> bool {
    matches!(
        state,
        State::Reconnecting | State::Offline | State::NeedsLogin
    )
}

impl Picker {
    /// The saved machine whose facts the card needs: the chosen machine, otherwise the remote
    /// machine on screen.
    pub fn card_machine(&self) -> Option<ProfileId> {
        let chosen = self
            .cards
            .chosen
            .as_ref()
            .map(|chosen| &chosen.machine)
            .filter(|machine| **machine != self.current);
        match chosen.unwrap_or(&self.current) {
            Choice::Remote(id) => Some(id.clone()),
            Choice::Local => None,
        }
    }

    /// Takes what Settings › Machines knows of the card's machine; true when the card changed.
    pub fn observe_card(&mut self, facts: Option<Machine>) -> bool {
        // A choice that landed, or of a machine no longer saved and enabled, has no card.
        if self.cards.chosen.as_ref().is_some_and(|chosen| {
            chosen.machine == self.current
                || !self
                    .rows
                    .iter()
                    .any(|row| row.enabled && row.choice == chosen.machine)
        }) {
            self.cards.chosen = None;
        }
        if self.cards.dismissed.as_ref().is_some_and(|(id, state)| {
            facts
                .as_ref()
                .is_none_or(|f| &f.id != id || f.state != *state)
        }) {
            self.cards.dismissed = None;
        }
        let before = self.card_identity();
        let changed = self.cards.facts != facts;
        self.cards.facts = facts;
        let after = self.card_identity();
        if after != self.cards.shown {
            self.cards.token = self.cards.token.wrapping_add(1);
            self.cards.shown = after.clone();
        }
        changed || before != after
    }

    fn card_identity(&self) -> Option<(bool, String)> {
        self.card()
            .map(|card| (card.chosen, card.machine.id.clone()))
    }

    /// The card over the terminals, if one shows.
    pub(crate) fn card(&self) -> Option<Card<'_>> {
        let machine = self.cards.facts.as_ref()?;
        let named =
            |choice: &Choice| matches!(choice, Choice::Remote(id) if id.as_str() == machine.id);
        let chosen = self
            .cards
            .chosen
            .as_ref()
            .is_some_and(|chosen| chosen.machine != self.current && named(&chosen.machine));
        let on_screen = !chosen
            && named(&self.current)
            && lost(machine.state)
            && self.cards.dismissed != Some((machine.id.clone(), machine.state));
        (chosen || on_screen).then(|| Card {
            token: self.cards.token,
            machine,
            chosen,
            action: machine.action().filter(|action| {
                matches!(action, Action::Retry | Action::RetryNow | Action::LogIn)
            }),
        })
    }

    /// Whether the card shows and takes the keyboard: the picker, when open, takes it first.
    pub fn card_shown(&self) -> bool {
        !self.open && self.card().is_some()
    }

    /// Notes the person's choice of `selected` on a machine that is not connected, which shows its
    /// card until the choice lands; any other choice puts the card away.
    pub(super) fn note_choice(&mut self, selected: &Selection) {
        self.cards.chosen = self
            .unconnected_state(&selected.machine)
            .map(|_| selected.clone());
    }

    /// Routes a key to the card, which takes Enter for its action and Esc for Not now and keeps
    /// every other key from the terminals beneath; false when no card shows.
    pub(super) fn card_key(&mut self, key: KeyEvent) -> bool {
        if !self.card_shown() {
            return false;
        }
        if key.kind == KeyKind::Release {
            return true;
        }
        if NOT_NOW.matches(key) {
            self.card_not_now();
        } else if ACT.matches(key) {
            self.card_act();
        }
        true
    }

    /// A painted control of the card with `token`: its action, or Not now; a stale one changes
    /// nothing.
    pub(crate) fn card_control(&mut self, token: u64, act: bool) -> bool {
        if !self.card_shown() || self.cards.token != token {
            return false;
        }
        if act {
            self.card_act();
        } else {
            self.card_not_now();
        }
        true
    }

    /// Connects again: the choice is made again, or the machine on screen is chosen again, so the
    /// hub tries at once and a login asks this TUI.
    fn card_act(&mut self) {
        let Some(card) = self.card() else {
            return;
        };
        if card.action.is_none() {
            return;
        }
        let selection = match &self.cards.chosen {
            Some(chosen) if card.chosen => chosen.clone(),
            _ => Selection::from(self.current.clone()),
        };
        let chosen = self.cards.chosen.take();
        if self.begin_selection(SelectionOrigin::Sidebar) {
            self.request = Some(selection);
            self.error = None;
        }
        self.cards.chosen = chosen;
    }

    /// Puts the card away: a choice is given up, and the machine on screen keeps its terminals in
    /// view until its state changes.
    fn card_not_now(&mut self) {
        let Some(card) = self.card() else {
            return;
        };
        if card.chosen {
            self.cancel_selection();
        } else {
            self.cards.dismissed = Some((card.machine.id.clone(), card.machine.state));
        }
    }
}

#[cfg(test)]
mod tests;
