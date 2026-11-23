//! The question a click on Update asks when the active version needs a new Terminal Host: before
//! the restart when that version's Worker cannot adopt the Host, or once the Session runs that
//! version and terminals that could not move stay on the Host. Replacing it restarts every terminal,
//! so nothing happens until the person confirms. It appears once a background read answers, not
//! under the person's key, so like a move's question its Enter and Restart button act only once
//! frames have shown it whole for a second in which no key or paste was dropped; Esc and Cancel act
//! at once. The updater runs the replacement; this module keeps what is shown.

use super::Picker;
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};
use std::time::{Duration, Instant};

/// How long frames show the question whole, with no key or paste dropped, before Enter or Restart
/// acts.
const ARMING: Duration = Duration::from_secs(1);
const CONFIRM: Binding = Binding::new(KeyCode::Enter, Modifiers::NONE);
const CANCEL: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);

/// The question shown, by the token its painted controls carry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostReplacementQuestion {
    pub(crate) token: u64,
    /// The terminals that could not move, once the Session runs the new version; `None` before
    /// the restart.
    pub(crate) staying: Option<u32>,
    /// The last frame showed the whole question.
    drawn: bool,
    /// The first tick that found it drawn, since it last was not or last dropped a key or paste.
    since: Option<Instant>,
    armed: bool,
}

impl HostReplacementQuestion {
    /// A key or paste went nowhere, or the window changed: its second starts again.
    const fn disarm(&mut self) {
        self.since = None;
        self.armed = false;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct HostReplacement {
    question: Option<HostReplacementQuestion>,
    tokens: u64,
    /// Whether the person confirmed, until the shell takes it.
    answer: Option<bool>,
}

impl Picker {
    /// Shows the question over everything but a login or a move's question, naming the
    /// terminals `staying` once the Session runs the new version.
    pub fn ask_host_replacement(&mut self, staying: Option<u32>) {
        let token = self.host_replacement.tokens.wrapping_add(1);
        self.host_replacement.tokens = token;
        self.host_replacement.answer = None;
        self.host_replacement.question = Some(HostReplacementQuestion {
            token,
            staying,
            drawn: false,
            since: None,
            armed: false,
        });
    }

    /// Whether the person confirmed the shown question, once.
    pub fn take_host_replacement_answer(&mut self) -> Option<bool> {
        self.host_replacement.answer.take()
    }

    pub const fn host_replacement_shown(&self) -> bool {
        self.host_replacement.question.is_some()
    }

    pub(crate) const fn host_replacement_question(&self) -> Option<&HostReplacementQuestion> {
        self.host_replacement.question.as_ref()
    }

    /// Arms a question frames showed whole for a second. True when that changed.
    pub fn tick_host_replacement(&mut self, now: Instant) -> bool {
        let Some(question) = self.host_replacement.question.as_mut() else {
            return false;
        };
        let armed = question.armed;
        if question.drawn {
            let since = *question.since.get_or_insert(now);
            question.armed = now.saturating_duration_since(since) >= ARMING;
        } else {
            question.disarm();
        }
        armed != question.armed
    }

    fn answer_host_replacement(&mut self, confirm: bool) {
        let Some(question) = self.host_replacement.question.as_ref() else {
            return;
        };
        if confirm && !question.armed {
            return;
        }
        self.host_replacement.answer = Some(confirm);
        self.host_replacement.question = None;
    }

    /// Routes a key to the shown question, which takes an armed Enter and any Esc and starts its
    /// second again for every other key; false when none is shown.
    pub(crate) fn host_replacement_key(&mut self, key: KeyEvent) -> bool {
        let Some(question) = self.host_replacement.question.as_mut() else {
            return false;
        };
        if key.kind == KeyKind::Release {
            return true;
        }
        if CANCEL.matches(key) {
            self.answer_host_replacement(false);
        } else if CONFIRM.matches(key) && question.armed {
            self.answer_host_replacement(true);
        } else {
            question.disarm();
        }
        true
    }

    /// A paste while the question is shown goes nowhere and starts its second again; otherwise it
    /// is given back.
    pub(crate) fn host_replacement_paste(&mut self, text: String) -> Option<String> {
        match self.host_replacement.question.as_mut() {
            Some(question) => {
                question.disarm();
                None
            }
            None => Some(text),
        }
    }

    /// A painted control of the question with `token`; a stale one changes nothing.
    pub(crate) fn host_replacement_control(&mut self, token: u64, confirm: bool) -> bool {
        if self
            .host_replacement
            .question
            .as_ref()
            .is_none_or(|question| question.token != token)
        {
            return false;
        }
        self.answer_host_replacement(confirm);
        true
    }

    /// Records whether the frame just painted showed the whole question.
    pub(crate) fn set_host_replacement_drawn(&mut self, drawn: bool) {
        if let Some(question) = self.host_replacement.question.as_mut() {
            question.drawn = drawn;
        }
    }
}
