//! What one attempt did with a Machine's remembered answers, kind by kind: each is tried at most
//! once per attempt, and one `ssh` refused is never tried again in it. The hub holds no answer, only
//! these marks and the question held while the helper reads the store.
//! `docs/machines.md#remembered-answers` owns the rule.

use super::Raised;
use orchbee_workbench_client::application::machine_credentials::SecretKind;

/// Where a kind's remembered answer stands in the attempt.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Use {
    /// Not tried yet.
    #[default]
    Untried,
    /// The helper reads it now.
    Reading,
    /// It went to `ssh`, which has not refused it.
    Given,
    /// Nothing was remembered, or `ssh` refused it: not tried again in this attempt.
    Spent,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Remembered {
    password: Use,
    passphrase: Use,
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

    /// The kinds whose remembered answer went to `ssh` and was not refused yet.
    pub(super) fn given(&self) -> impl Iterator<Item = SecretKind> + '_ {
        SecretKind::ALL
            .into_iter()
            .filter(|kind| self.get(*kind) == Use::Given)
    }
}
