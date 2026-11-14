//! The command logins the hub offers to every interface that asks. Each waits as a notice, the
//! lowest login first, until the person accepts it here or declines it, or the hub reports it
//! claimed elsewhere or ended. An accepted login keeps its notice until its question opens.
//! `apps/orchbee/docs/machines.md#login-questions` owns the rules.

use orchbee_machine_protocol::Id;

/// What a notice shows of one offered login.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Offered {
    pub(crate) login: Id,
    pub(crate) machine: String,
    /// The Machine's `ssh` target.
    pub(crate) target: String,
    /// The commands waiting on the login, as the hub last counted them.
    pub(crate) waiters: u64,
    /// The person accepted it here: the claim is on its way, and its question follows.
    pub(crate) accepted: bool,
}

/// The offered logins, ascending by login.
#[derive(Default)]
pub(super) struct Offers {
    offered: Vec<Offered>,
}

impl Offers {
    /// The hub offers `login`, or counts its waiters again.
    pub(super) fn offer(&mut self, login: Id, machine: String, target: String, waiters: u64) {
        if let Some(offer) = self.offered.iter_mut().find(|offer| offer.login == login) {
            offer.waiters = waiters;
            return;
        }
        let index = self
            .offered
            .partition_point(|offer| u32::from(offer.login) < u32::from(login));
        self.offered.insert(
            index,
            Offered {
                login,
                machine,
                target,
                waiters,
                accepted: false,
            },
        );
    }

    /// `login` is no longer offered here: claimed elsewhere, declined, asked or ended.
    pub(super) fn withdraw(&mut self, login: Id) {
        self.offered.retain(|offer| offer.login != login);
    }

    pub(super) fn clear(&mut self) {
        self.offered.clear();
    }

    pub(super) fn get(&self, login: Id) -> Option<&Offered> {
        self.offered.iter().find(|offer| offer.login == login)
    }

    /// The notice to show: the accepted login until its question opens, otherwise the lowest.
    pub(super) fn first(&self) -> Option<&Offered> {
        self.offered
            .iter()
            .find(|offer| offer.accepted)
            .or_else(|| self.offered.first())
    }

    /// Whether the person accepted `login` here.
    pub(super) fn accepted(&self, login: Id) -> bool {
        self.get(login).is_some_and(|offer| offer.accepted)
    }

    /// The person accepts `login`; false unless it is the notice shown and not accepted yet.
    pub(super) fn accept(&mut self, login: Id) -> bool {
        if self
            .first()
            .is_none_or(|offer| offer.login != login || offer.accepted)
        {
            return false;
        }
        if let Some(offer) = self.offered.iter_mut().find(|offer| offer.login == login) {
            offer.accepted = true;
        }
        true
    }
}
