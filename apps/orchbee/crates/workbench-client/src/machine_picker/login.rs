//! The login overlay the picker owns while a question waits or a login ended without the person:
//! what the episode presents, whether the last frame showed the whole question, and the person's
//! act until the shell carries it to discovery. Typed and pasted characters, a new host's `yes`
//! included, go straight into the field's fixed buffer and nowhere else; the overlay keeps only how
//! many there are.

use super::{Choice, Picker};
use crate::client_view::{KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::machine_discovery::Discovery;
use crate::machine_login::{Act, Presented, Token, YES};
use orchbee_machine_protocol::{hub::QuestionKind, wipe};

#[cfg(test)]
mod tests;

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
    /// The new host's question was submitted without `yes`.
    pub(crate) type_yes: bool,
    /// The last frame showed the whole question: the asking sentence, a new host's whole
    /// fingerprint, the field and the buttons. Until a frame does, only Esc acts.
    drawn: bool,
    /// Counts edits, so a copy of the view taken before a keystroke compares unequal after it.
    edits: u64,
    act: Option<(Token, Act)>,
}

impl LoginOverlay {
    fn new(presented: Presented) -> Self {
        Self {
            presented,
            type_yes: false,
            drawn: false,
            edits: 0,
            act: None,
        }
    }

    pub(crate) const fn host_key(&self) -> bool {
        matches!(self.presented.kind, QuestionKind::HostKey { .. })
    }

    /// The login ended without the person: the overlay shows how, takes no answer, and closes on
    /// Esc or Enter.
    pub(crate) const fn ended(&self) -> bool {
        self.presented.outcome.is_some()
    }

    /// What a new host's field shows in clear: the typed text while it begins `yes`; anything
    /// else is masked like a secret.
    pub(crate) fn shown_yes(&self) -> Option<String> {
        self.host_key()
            .then(|| self.presented.field.prefix_of(YES))
            .flatten()
    }

    fn act(&mut self, act: Act) {
        self.act = Some((self.presented.token, act));
    }

    /// The answer is on its way, or an act waits for discovery: input changes nothing until the
    /// next question, so nothing typed after Enter joins the answer.
    const fn settled(&self) -> bool {
        self.presented.sending || self.act.is_some()
    }

    /// Whether typing reaches the field: a question on screen that waits for its answer.
    const fn editable(&self) -> bool {
        self.drawn && !self.ended() && !self.settled()
    }

    fn submit(&mut self) {
        if !self.editable() {
            return;
        }
        match &self.presented.kind {
            QuestionKind::HostKey { .. } if self.presented.field.reads(YES.as_bytes()) => {
                self.act(Act::Trust);
            }
            QuestionKind::HostKey { .. } => self.type_yes = true,
            // An empty password or passphrase would cost an attempt.
            QuestionKind::Password | QuestionKind::KeyPassphrase
                if self.presented.field.characters() == 0 => {}
            _ => self.act(Act::Submit),
        }
    }

    fn push(&mut self, character: char) {
        if self.editable() && self.presented.field.push(character) {
            self.type_yes = false;
        }
    }

    fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyKind::Release {
            return;
        }
        self.edits = self.edits.wrapping_add(1);
        let shift = Modifiers::NONE.with_shift();
        let (code, mods) = key.normalized();
        let escape = code == KeyCode::Esc && mods == Modifiers::NONE;
        let enter = code == KeyCode::Char('m') && mods == Modifiers::NONE.with_ctrl();
        if self.ended() {
            // Every other key of an ended login's overlay is discarded.
            if escape || enter {
                self.act(Act::Close);
            }
            return;
        }
        match (code, mods) {
            _ if escape => self.act(Act::Cancel),
            _ if enter => self.submit(),
            (KeyCode::Backspace, Modifiers::NONE) if self.editable() => {
                self.presented.field.pop();
            }
            (KeyCode::Char('u'), mods)
                if mods == Modifiers::NONE.with_ctrl() && self.editable() =>
            {
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
            _ if login.ended() => login.act(Act::Close),
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

    /// Records whether the frame just painted showed the whole question.
    pub(crate) fn set_login_drawn(&mut self, drawn: bool) {
        if let Some(login) = self.login.as_mut() {
            login.drawn = drawn;
        }
    }

    /// The window changed: the question counts as off screen until a frame shows it whole again.
    /// The field keeps what was typed.
    pub fn login_undrawn(&mut self) {
        self.set_login_drawn(false);
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
            // The same question, or the overlay of its login that ended, keeps the overlay: an
            // overlay with a field closes only by the person's act.
            (Some(mut login), Some(presented)) if login.presented.token == presented.token => {
                if login.presented.notice != presented.notice
                    || login.presented.sending != presented.sending
                    || login.presented.outcome != presented.outcome
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
