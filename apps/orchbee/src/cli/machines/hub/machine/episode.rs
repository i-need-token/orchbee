//! A login episode inside one attempt: its id, its trigger, its claimant once claimed, the clients
//! it is shown to, its one open question and what was answered. It holds no answer and no clock of
//! its own; the Machine passes the time in. `docs/machines.md#login-episode` owns the transitions.

use super::Raised;
use orchbee_machine_protocol::{
    Id,
    hub::{Question, QuestionKind, Trigger},
};
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

/// The client that claimed an episode, and its own `user` request when it came by one rather than
/// by `claim`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Holder {
    pub(super) peer: u64,
    pub(super) request: Option<Id>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Open {
    question: Id,
    tag: Tag,
    raised: Raised,
    repeated: bool,
    /// `ssh` refused the remembered answer to the same question just before.
    rejected: bool,
    /// When `ssh` asked it: the attempt's clock stops from then until its answer.
    since: Instant,
    /// When it goes unanswered: only once it was put to a claimant.
    due: Option<Instant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Episode {
    login: Id,
    trigger: Trigger,
    /// `None` while the episode is offered.
    claimant: Option<Holder>,
    /// The connections its `login` events go to.
    audience: Vec<u64>,
    /// Boxed: a question is the bulk of an episode, and an attempt holds one inline.
    open: Option<Box<Open>>,
    /// The last question id given out.
    asked: u32,
    /// The kinds whose answers the helper took.
    answered: Vec<Tag>,
}

impl Episode {
    /// A `user` episode, claimed by `claimant` and shown to it alone.
    pub(super) fn claimed(login: Id, claimant: Holder) -> Self {
        Self {
            login,
            trigger: Trigger::User,
            claimant: Some(claimant),
            audience: vec![claimant.peer],
            open: None,
            asked: 0,
            answered: Vec::new(),
        }
    }

    /// A `command` episode offered to `audience`, every connection that asks.
    pub(super) fn offered(login: Id, audience: &[u64]) -> Self {
        Self {
            login,
            trigger: Trigger::Command,
            claimant: None,
            audience: audience.to_vec(),
            open: None,
            asked: 0,
            answered: Vec::new(),
        }
    }

    pub(super) const fn login(&self) -> Id {
        self.login
    }

    pub(super) const fn trigger(&self) -> Trigger {
        self.trigger
    }

    pub(super) const fn claimant(&self) -> Option<Holder> {
        self.claimant
    }

    pub(super) fn audience(&self) -> &[u64] {
        &self.audience
    }

    /// Whether a helper waits for the answer to an open question.
    pub(super) const fn question_open(&self) -> bool {
        self.open.is_some()
    }

    /// When the open question goes unanswered, if one is open and was put to a claimant.
    pub(super) fn due(&self) -> Option<Instant> {
        self.open.as_ref().and_then(|open| open.due)
    }

    /// Opens the next question, `raised` at `now` and marked `rejected` when `ssh` refused the
    /// remembered answer to it, held until it is put to a claimant. `None` while one is open.
    pub(super) fn open(&mut self, raised: Raised, rejected: bool, now: Instant) -> Option<()> {
        if self.open.is_some() {
            return None;
        }
        let question = Id::new(self.asked.checked_add(1)?)?;
        self.asked = question.get();
        let tag = Tag::of(&raised.kind);
        self.open = Some(Box::new(Open {
            question,
            tag,
            raised,
            // `ssh` asking a kind whose answer it took before means it refused that answer.
            repeated: self.answered.contains(&tag),
            rejected,
            since: now,
            due: None,
        }));
        Some(())
    }

    /// Puts the open question to the claimant, unanswered at `due`: the `question` event and the
    /// connection it goes to. `None` while offered or with no question open.
    pub(super) fn put(&mut self, due: Instant) -> Option<(u64, Question)> {
        let claimant = self.claimant?;
        let open = self.open.as_mut()?;
        open.due = Some(due);
        Some((
            claimant.peer,
            Question {
                login: self.login,
                question: open.question,
                kind: open.raised.kind.clone(),
                prompt: open.raised.prompt.clone(),
                repeated: open.repeated,
                remembered_rejected: open.rejected,
                answer: open.raised.answer.clone(),
            },
        ))
    }

    /// `holder` claims the episode, which it also shows from now on.
    pub(super) fn claim(&mut self, holder: Holder) {
        self.claimant = Some(holder);
        if !self.audience.contains(&holder.peer) {
            self.audience.push(holder.peer);
        }
    }

    /// The claimant left: the episode is offered again and an open question is held again, without
    /// a bound.
    pub(super) fn release(&mut self) {
        self.claimant = None;
        if let Some(open) = &mut self.open {
            open.due = None;
        }
    }

    /// `peer` shows the episode from now on; false when it did already.
    pub(super) fn join(&mut self, peer: u64) -> bool {
        if self.audience.contains(&peer) {
            return false;
        }
        self.audience.push(peer);
        true
    }

    /// `peer` no longer shows the episode.
    pub(super) fn leave(&mut self, peer: u64) {
        self.audience.retain(|shown| *shown != peer);
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
