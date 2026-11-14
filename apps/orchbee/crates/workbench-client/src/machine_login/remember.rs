//! Remembering a typed answer: the box beside a password or passphrase field, the person's choice,
//! and the copy of each answer kept until the login succeeds. Only after `succeeded` does a kept
//! answer leave as an effect to be written to the credential store; a refusal of its kind, any
//! other end, and dropping the episode wipe it. `apps/orchbee/docs/machines.md#remembered-answers`
//! owns the rules.

use super::{Answer, Effect};
use crate::machine_credentials::{Remembering, SecretKind};
use orchbee_machine_protocol::hub::QuestionKind;

/// What the overlay shows beside the field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RememberBox {
    /// The question's answer is never remembered: no box.
    Absent,
    /// No store can be used in this login: one line says so instead of the box.
    Unavailable,
    /// The box, ticked or not; `until_restart` when the store keeps answers only until this
    /// computer restarts.
    Offered { ticked: bool, until_restart: bool },
}

/// The person's choice for one episode, and the answers kept for it.
#[derive(Default)]
pub(super) struct Remember {
    ticked: bool,
    kept: Vec<(SecretKind, Answer)>,
}

impl Remember {
    pub(super) const fn choose(&mut self, ticked: bool) {
        self.ticked = ticked;
    }

    /// The box for a question of `kind`, while the store keeps answers as `remembering` says.
    pub(super) fn shown(
        &self,
        kind: &QuestionKind,
        remembering: Option<Remembering>,
    ) -> RememberBox {
        if SecretKind::of(kind).is_none() {
            return RememberBox::Absent;
        }
        match remembering {
            Some(Remembering::Lasting) => RememberBox::Offered {
                ticked: self.ticked,
                until_restart: false,
            },
            Some(Remembering::UntilRestart) => RememberBox::Offered {
                ticked: self.ticked,
                until_restart: true,
            },
            Some(Remembering::Unavailable) | None => RememberBox::Unavailable,
        }
    }

    /// Keeps a copy of `answer`, just submitted to a question of `kind`, while the box is ticked
    /// and a store can keep it; it replaces an earlier copy of the same kind.
    pub(super) fn keep(
        &mut self,
        kind: &QuestionKind,
        answer: &Answer,
        remembering: Option<Remembering>,
    ) {
        let Some(kind) = SecretKind::of(kind) else {
            return;
        };
        self.kept.retain(|(kept, _)| *kept != kind);
        let usable = matches!(
            remembering,
            Some(Remembering::Lasting | Remembering::UntilRestart)
        );
        if self.ticked && usable {
            self.kept.push((kind, answer.copy()));
        }
    }

    /// `ssh` asked a question of `kind` again: the answer kept for it was refused, and goes.
    pub(super) fn refused(&mut self, kind: &QuestionKind) {
        if let Some(kind) = SecretKind::of(kind) {
            self.kept.retain(|(kept, _)| *kept != kind);
        }
    }

    /// The login succeeded: each kept answer leaves to be written as `machine`'s.
    pub(super) fn saved(&mut self, machine: &str) -> Vec<Effect> {
        std::mem::take(&mut self.kept)
            .into_iter()
            .map(|(kind, answer)| Effect::Remember {
                machine: machine.to_owned(),
                kind,
                answer,
            })
            .collect()
    }
}
