//! A login episode inside one attempt: its id, its claimant, its one open question and what was
//! answered. It holds no answer and no clock of its own; the Machine passes the time in.
//! `docs/machines.md#login-episode` owns the transitions.

use super::Waiter;
use orchbee_machine_protocol::{Id, hub::QuestionKind};
use std::time::Instant;

/// What a question asks, without the offered key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Tag {
    Password,
    KeyPassphrase,
    HostKey,
    Other,
}

impl Tag {
    pub(super) const fn of(kind: &QuestionKind) -> Self {
        match kind {
            QuestionKind::Password => Self::Password,
            QuestionKind::KeyPassphrase => Self::KeyPassphrase,
            QuestionKind::HostKey { .. } => Self::HostKey,
            QuestionKind::Other => Self::Other,
        }
    }

    /// A password, a passphrase or another secret, which `ssh` can refuse.
    const fn secret(self) -> bool {
        !matches!(self, Self::HostKey)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Open {
    tag: Tag,
    since: Instant,
    /// When it goes unanswered.
    due: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Episode {
    login: Id,
    claimant: Waiter,
    open: Option<Open>,
    /// The last question id given out.
    asked: u32,
    /// The kinds whose answers the helper took.
    answered: Vec<Tag>,
}

impl Episode {
    pub(super) const fn new(login: Id, claimant: Waiter) -> Self {
        Self {
            login,
            claimant,
            open: None,
            asked: 0,
            answered: Vec::new(),
        }
    }

    pub(super) const fn login(&self) -> Id {
        self.login
    }

    pub(super) const fn claimant(&self) -> &Waiter {
        &self.claimant
    }

    /// When the open question goes unanswered, if one is open.
    pub(super) fn due(&self) -> Option<Instant> {
        self.open.as_ref().map(|open| open.due)
    }

    /// Opens the next question of kind `tag` at `now`, unanswered at `due`: its id and whether an
    /// answer of that kind was taken before, which `ssh` asking again means it refused. `None`
    /// while one is open.
    pub(super) fn open(&mut self, tag: Tag, now: Instant, due: Instant) -> Option<(Id, bool)> {
        if self.open.is_some() {
            return None;
        }
        let question = Id::new(self.asked.checked_add(1)?)?;
        self.asked = question.get();
        self.open = Some(Open {
            tag,
            since: now,
            due,
        });
        Some((question, self.answered.contains(&tag)))
    }

    /// The helper took the answer: the open question closes, and when it was asked is returned.
    pub(super) fn take(&mut self) -> Option<Instant> {
        let open = self.open.take()?;
        self.answered.push(open.tag);
        Some(open.since)
    }

    /// The helper took a secret answer in this episode, so a refusal by `ssh` is of answers.
    pub(super) fn answered_secret(&self) -> bool {
        self.answered.iter().any(|tag| tag.secret())
    }
}
