//! What the hub knows of a Machine's remembered answers, kind by kind: within one attempt, each is
//! tried at most once and one `ssh` refused is never tried again in it; across attempts, a kind
//! handed to `ssh` by an attempt that then ended before the link handshake is held back until the
//! Machine has been online or a person asks, and a kind whose deletion failed is held back until a
//! person logs in themselves. The hub holds no answer, only these marks and the question held while
//! the helper reads the store. `docs/machines.md#remembered-answers` owns the rules.

use super::Raised;
use orchbee_machine_protocol::Prompt;
use orchbee_workbench_client::application::machine_credentials::SecretKind;

/// Where a kind's remembered answer stands in the attempt.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Use {
    /// Not tried yet.
    #[default]
    Untried,
    /// The helper reads it now, and may already have handed it to `ssh`.
    Reading,
    /// It went to `ssh`, which has not refused it.
    Given,
    /// It went to `ssh`, which refused it: not tried again in this attempt.
    Refused,
    /// Nothing was remembered, or it was not tried: not tried again in this attempt.
    Spent,
}

impl Use {
    /// The answer reached `ssh`, or may have.
    const fn handed(self) -> bool {
        matches!(self, Self::Reading | Self::Given | Self::Refused)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Remembered {
    password: Use,
    passphrase: Use,
    /// The prompt the remembered passphrase was given for: only the same prompt again refuses it,
    /// since another key, or another hop's `ssh`, asks in other words.
    passphrase_prompt: Option<Prompt>,
    /// The question held while its helper reads the store.
    pub(super) parked: Option<Raised>,
}

impl Remembered {
    pub(super) const fn get(&self, kind: SecretKind) -> Use {
        match kind {
            SecretKind::Password => self.password,
            SecretKind::KeyPassphrase => self.passphrase,
        }
    }

    pub(super) const fn set(&mut self, kind: SecretKind, mark: Use) {
        match kind {
            SecretKind::Password => self.password = mark,
            SecretKind::KeyPassphrase => self.passphrase = mark,
        }
    }

    /// The parked question's answer went to `ssh`.
    pub(super) fn given(&mut self, raised: &Raised, kind: SecretKind) {
        self.set(kind, Use::Given);
        if kind == SecretKind::KeyPassphrase {
            self.passphrase_prompt = Some(raised.prompt.clone());
        }
    }

    /// Whether `ssh` asking `prompt` again for `kind` refuses the remembered answer it was given: any
    /// password question does; a passphrase question only in the words it was given for.
    pub(super) fn refuses(&self, kind: SecretKind, prompt: &Prompt) -> bool {
        self.get(kind) == Use::Given
            && match kind {
                SecretKind::Password => true,
                SecretKind::KeyPassphrase => self.passphrase_prompt.as_ref() == Some(prompt),
            }
    }

    /// The kinds whose remembered answer reached `ssh` in this attempt, or may have.
    pub(super) fn handed(&self) -> impl Iterator<Item = SecretKind> + '_ {
        SecretKind::ALL
            .into_iter()
            .filter(|kind| self.get(*kind).handed())
    }
}

/// One mark per kind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Marks {
    password: bool,
    passphrase: bool,
}

impl Marks {
    const fn get(self, kind: SecretKind) -> bool {
        match kind {
            SecretKind::Password => self.password,
            SecretKind::KeyPassphrase => self.passphrase,
        }
    }

    const fn set(&mut self, kind: SecretKind) {
        match kind {
            SecretKind::Password => self.password = true,
            SecretKind::KeyPassphrase => self.passphrase = true,
        }
    }
}

/// The kinds a Machine does not recall across attempts, for the hub's life.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Holds {
    /// Handed to `ssh` by an attempt that ended before the link handshake: an automatic attempt
    /// would send the same answer again, a lockout loop.
    withheld: Marks,
    /// Refused, and its deletion failed or could not start.
    undeleted: Marks,
}

impl Holds {
    /// Whether an attempt may recall the remembered answer of `kind`.
    pub(super) const fn recallable(self, kind: SecretKind) -> bool {
        !self.withheld.get(kind) && !self.undeleted.get(kind)
    }

    /// An attempt that handed `kind` to `ssh` ended before the link handshake.
    pub(super) const fn withhold(&mut self, kind: SecretKind) {
        self.withheld.set(kind);
    }

    /// The deletion of a refused `kind` failed.
    pub(super) const fn undeleted(&mut self, kind: SecretKind) {
        self.undeleted.set(kind);
    }

    /// The Machine reached `online`, or a person asked for the connection.
    pub(super) fn release(&mut self) {
        self.withheld = Marks::default();
    }

    /// A person logged in with answers of their own.
    pub(super) fn logged_in(&mut self) {
        *self = Self::default();
    }
}
