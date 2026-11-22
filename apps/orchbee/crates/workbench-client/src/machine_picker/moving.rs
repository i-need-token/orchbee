//! A Workspace's move as the picker shows it: the question a move asks before it stops a program,
//! `Moving…` while the move runs, and the one notice a move leaves when it did not land as asked.
//! The machine manager runs the move and asks through here; this module keeps what is shown.
//!
//! The question appears on its own once the move's check answers, not under the person's key, so
//! like a command's login notice its Enter and Move button act only once frames have shown it whole
//! for a second in which no key or paste was dropped. Esc and Cancel act at once: they lose
//! nothing. Otherwise it behaves as the close-Workspace confirmation does: Move holds the focus,
//! Enter moves, Esc cancels. `apps/orchbee/docs/machines.md#change-machine` owns the rules.

use super::Picker;
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::machine_change::{Notice, Question};
use crate::transient_notice::NoticeTimer;
use std::time::{Duration, Instant};

/// How long frames show the question whole, with no key or paste dropped, before Enter or Move acts.
const ARMING: Duration = Duration::from_secs(1);
const CONFIRM: Binding = Binding::new(KeyCode::Enter, Modifiers::NONE);
const CANCEL: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);

/// The question shown, by the token its painted controls carry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MoveConfirmation {
    pub(crate) token: u64,
    pub(crate) question: Question,
    /// The last frame showed the whole question.
    drawn: bool,
    /// The first tick that found it drawn, since it last was not or last dropped a key or paste.
    since: Option<Instant>,
    armed: bool,
}

impl MoveConfirmation {
    /// A key or paste went nowhere, or the window changed: its second starts again.
    const fn disarm(&mut self) {
        self.since = None;
        self.armed = false;
    }
}

/// What the status line says about a move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MoveStatus<'a> {
    Moving,
    Notice(&'a Notice),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Moving {
    confirmation: Option<MoveConfirmation>,
    tokens: u64,
    /// The person's answer until the machine manager takes it: the question's token and whether
    /// to move.
    answer: Option<(u64, bool)>,
    running: bool,
    notice: Option<(u64, Notice)>,
    timer: NoticeTimer<u64>,
}

impl Picker {
    /// Shows `question` over everything but a login; its token comes back with the answer.
    pub fn ask_move(&mut self, question: Question) -> Option<u64> {
        let token = self.moving.tokens.checked_add(1)?;
        self.moving.tokens = token;
        self.moving.answer = None;
        self.moving.confirmation = Some(MoveConfirmation {
            token,
            question,
            drawn: false,
            since: None,
            armed: false,
        });
        Some(token)
    }

    /// The person's answer to the question with its token, once: whether to move.
    pub fn take_move_answer(&mut self) -> Option<(u64, bool)> {
        self.moving.answer.take()
    }

    /// Whether the question is shown.
    pub const fn move_shown(&self) -> bool {
        self.moving.confirmation.is_some()
    }

    pub(crate) const fn move_confirmation(&self) -> Option<&MoveConfirmation> {
        self.moving.confirmation.as_ref()
    }

    /// Whether a move runs; the status line says so while no question is shown. True when that
    /// changed.
    pub fn set_moving(&mut self, running: bool) -> bool {
        let changed = self.moving.running != running;
        self.moving.running = running;
        if running {
            self.moving.notice = None;
        }
        changed
    }

    /// The notice a move leaves when it did not land as asked; it passes as other notices do.
    pub fn move_notice(&mut self, notice: Notice) {
        let generation = self
            .moving
            .notice
            .as_ref()
            .map_or(0, |(generation, _)| generation.wrapping_add(1));
        self.moving.notice = Some((generation, notice));
    }

    pub(crate) fn move_status(&self) -> Option<MoveStatus<'_>> {
        if self.moving.running && self.moving.confirmation.is_none() {
            return Some(MoveStatus::Moving);
        }
        self.moving
            .notice
            .as_ref()
            .map(|(_, notice)| MoveStatus::Notice(notice))
    }

    /// Arms a question frames showed whole for a second and lets a notice pass after its time.
    /// True when anything shown changed.
    pub fn tick_move(&mut self, now: Instant) -> bool {
        if let Some(confirmation) = self.moving.confirmation.as_mut() {
            if confirmation.drawn {
                let since = *confirmation.since.get_or_insert(now);
                confirmation.armed = now.saturating_duration_since(since) >= ARMING;
            } else {
                confirmation.disarm();
            }
        }
        let shown = self
            .moving
            .notice
            .as_ref()
            .map(|(generation, _)| *generation);
        if self.moving.timer.expired(shown, now) {
            self.moving.notice = None;
            return true;
        }
        false
    }

    fn answer_move(&mut self, confirm: bool) {
        let Some(confirmation) = self.moving.confirmation.as_ref() else {
            return;
        };
        if confirm && !confirmation.armed {
            return;
        }
        self.moving.answer = Some((confirmation.token, confirm));
        self.moving.confirmation = None;
    }

    /// Routes a key to the shown question, which takes an armed Enter and any Esc and starts its
    /// second again for every other key; false when none is shown.
    pub(crate) fn move_key(&mut self, key: KeyEvent) -> bool {
        let Some(confirmation) = self.moving.confirmation.as_mut() else {
            return false;
        };
        if key.kind == KeyKind::Release {
            return true;
        }
        if CANCEL.matches(key) {
            self.answer_move(false);
        } else if CONFIRM.matches(key) && confirmation.armed {
            self.answer_move(true);
        } else {
            confirmation.disarm();
        }
        true
    }

    /// A paste while the question is shown goes nowhere and starts its second again; otherwise it
    /// is given back.
    pub(crate) fn move_paste(&mut self, text: String) -> Option<String> {
        match self.moving.confirmation.as_mut() {
            Some(confirmation) => {
                confirmation.disarm();
                None
            }
            None => Some(text),
        }
    }

    /// A painted control of the question with `token`; a stale one changes nothing.
    pub(crate) fn move_control(&mut self, token: u64, confirm: bool) -> bool {
        if self
            .moving
            .confirmation
            .as_ref()
            .is_none_or(|confirmation| confirmation.token != token)
        {
            return false;
        }
        self.answer_move(confirm);
        true
    }

    /// Records whether the frame just painted showed the whole question.
    pub(crate) fn set_move_drawn(&mut self, drawn: bool) {
        if let Some(confirmation) = self.moving.confirmation.as_mut() {
            confirmation.drawn = drawn;
        }
    }

    /// The window changed: the question counts as off screen until a frame shows it whole again.
    pub fn move_undrawn(&mut self) {
        self.set_move_drawn(false);
    }

    /// Lets go of the question in a view that is no longer presented; the move's run decides.
    pub fn forget_move(&mut self) {
        self.moving.confirmation = None;
    }
}
