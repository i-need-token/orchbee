//! The login overlay the picker owns while a question waits: what the episode presents, the typed
//! `yes` of a new host's question, and the person's act until the shell carries it to discovery.
//! Typed and pasted characters of an answer go straight into the field's fixed buffer and nowhere
//! else; the overlay keeps only how many there are.

use super::{Choice, Picker};
use crate::client_view::{KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::machine_discovery::Discovery;
use crate::machine_login::{Act, Presented, Token};
use orchbee_machine_protocol::{hub::QuestionKind, wipe};

#[cfg(test)]
mod tests;

/// The most characters the `yes` field of a new host's question holds.
const YES_CHARACTERS: usize = 8;

/// A control of the painted overlay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginControl {
    Close,
    Cancel,
    Submit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LoginOverlay {
    pub(crate) presented: Presented,
    /// What was typed for a new host's question; it is no secret and never sent: Orchbee answers
    /// `yes` itself once it reads `yes`.
    pub(crate) yes: String,
    /// The new host's question was submitted without `yes`.
    pub(crate) type_yes: bool,
    /// Counts edits, so a copy of the view taken before a keystroke compares unequal after it.
    edits: u64,
    act: Option<(Token, Act)>,
}

impl LoginOverlay {
    fn new(presented: Presented) -> Self {
        Self {
            presented,
            yes: String::new(),
            type_yes: false,
            edits: 0,
            act: None,
        }
    }

    pub(crate) const fn host_key(&self) -> bool {
        matches!(self.presented.kind, QuestionKind::HostKey { .. })
    }

    fn act(&mut self, act: Act) {
        self.act = Some((self.presented.token, act));
    }

    /// The answer is on its way, or an act waits for discovery: input changes nothing until the
    /// next question, so nothing typed after Enter joins the answer.
    const fn settled(&self) -> bool {
        self.presented.sending || self.act.is_some()
    }

    fn submit(&mut self) {
        if self.settled() {
            return;
        }
        if !self.host_key() {
            self.act(Act::Submit);
        } else if self.yes.trim().eq_ignore_ascii_case("yes") {
            self.act(Act::Trust);
        } else {
            self.type_yes = true;
        }
    }

    fn push(&mut self, character: char) {
        if self.settled() {
            return;
        }
        if self.host_key() {
            if !character.is_control() && self.yes.chars().count() < YES_CHARACTERS {
                self.yes.push(character);
                self.type_yes = false;
            }
        } else {
            self.presented.field.push(character);
        }
    }

    fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyKind::Release {
            return;
        }
        self.edits = self.edits.wrapping_add(1);
        let shift = Modifiers::NONE.with_shift();
        match key.normalized() {
            (KeyCode::Esc, Modifiers::NONE) => self.act(Act::Cancel),
            (KeyCode::Char('m'), mods) if mods == Modifiers::NONE.with_ctrl() => self.submit(),
            (KeyCode::Backspace, Modifiers::NONE) if !self.settled() => {
                if self.host_key() {
                    self.yes.pop();
                } else {
                    self.presented.field.pop();
                }
            }
            (KeyCode::Char('u'), mods)
                if mods == Modifiers::NONE.with_ctrl() && !self.settled() =>
            {
                self.yes.clear();
                self.presented.field.clear();
            }
            (KeyCode::Char(character), mods) if mods == Modifiers::NONE || mods == shift => {
                self.push(if mods.shift {
                    character.to_ascii_uppercase()
                } else {
                    character
                });
            }
            _ => {}
        }
    }

    /// Takes a pasted run into the field and wipes the run's own bytes after.
    fn paste(&mut self, text: String) {
        self.edits = self.edits.wrapping_add(1);
        let mut bytes = text.into_bytes();
        if let Ok(text) = std::str::from_utf8(&bytes) {
            for character in text.chars() {
                self.push(character);
            }
        }
        wipe(&mut bytes);
    }
}

impl Picker {
    /// Whether the picker or its login overlay owns input now.
    pub fn captures(&self) -> bool {
        self.open || self.login.is_some()
    }

    pub(crate) const fn login(&self) -> Option<&LoginOverlay> {
        self.login.as_ref()
    }

    /// Whether a login question is shown.
    pub const fn login_shown(&self) -> bool {
        self.login.is_some()
    }

    /// The name the picker shows for the Machine the login is for.
    pub(crate) fn login_label(&self) -> Option<&str> {
        let machine = &self.login.as_ref()?.presented.machine;
        self.rows
            .iter()
            .find(|row| {
                row.context.is_none()
                    && row.agent.is_none()
                    && matches!(&row.choice, Choice::Remote(id) if id.as_str() == machine)
            })
            .map(|row| row.label.as_str())
    }

    /// Routes a key to the open login overlay; false when none is open.
    pub(crate) fn login_key(&mut self, key: KeyEvent) -> bool {
        let Some(login) = self.login.as_mut() else {
            return false;
        };
        login.key(key);
        self.settle_login_act();
        true
    }

    /// Routes a paste to the open login overlay; otherwise it is given back.
    pub(crate) fn login_paste(&mut self, text: String) -> Option<String> {
        match self.login.as_mut() {
            Some(login) => {
                login.paste(text);
                None
            }
            None => Some(text),
        }
    }

    /// A painted control of the overlay for question `token`; a stale one changes nothing.
    pub(crate) fn login_control(&mut self, token: Token, control: LoginControl) -> bool {
        let Some(login) = self
            .login
            .as_mut()
            .filter(|login| login.presented.token == token)
        else {
            return false;
        };
        match control {
            LoginControl::Close | LoginControl::Cancel => login.act(Act::Cancel),
            LoginControl::Submit => login.submit(),
        }
        self.settle_login_act();
        true
    }

    /// Lets go of the overlay in a view that is no longer presented; the episode keeps its field.
    pub fn forget_login(&mut self) {
        self.login = None;
    }

    /// Wipes the field, as a lost focus does.
    pub(crate) fn clear_login_field(&mut self) {
        if let Some(login) = self.login.as_mut() {
            login.presented.field.clear();
            login.edits = login.edits.wrapping_add(1);
        }
    }

    /// Cancelling the login cancels its selection too: the person stays where they were, and the
    /// Machine shows the state the hub gives it.
    fn settle_login_act(&mut self) {
        if self
            .login
            .as_ref()
            .is_some_and(|login| matches!(login.act, Some((_, Act::Cancel))))
        {
            // The person stays where they were: the picker under the overlay closes with the
            // selection, since a picker without its selection draws nothing yet holds the keys.
            self.close_picker();
        }
    }

    /// Carries the person's act to discovery and shows what discovery presents now. True when the
    /// overlay changed.
    pub fn exchange_login(&mut self, discovery: &mut Discovery) -> bool {
        if let Some((token, act)) = self.login.as_mut().and_then(|login| login.act.take()) {
            discovery.login_act(token, act);
        }
        let presented = discovery.login_presented();
        let before = self.login.clone();
        self.login = match (self.login.take(), presented) {
            (Some(mut login), Some(presented)) if login.presented.token == presented.token => {
                if login.presented.notice != presented.notice
                    || login.presented.sending != presented.sending
                {
                    login.type_yes = false;
                }
                login.presented = presented;
                Some(login)
            }
            (_, presented) => presented.map(LoginOverlay::new),
        };
        before != self.login
    }
}
