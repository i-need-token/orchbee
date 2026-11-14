//! The command logins the hub offers to every interface that asks. Each waits as a notice, the
//! lowest login first, until the person accepts it here or declines it, or the hub reports it
//! claimed elsewhere or ended. One offered for a Machine this client has not read yet waits unseen
//! and shows once the Machine is known. An accepted login keeps its notice until its question
//! opens; one the hub takes, ends or loses before that stays as an ended notice until the person
//! closes it. `apps/orchbee/docs/machines.md#login-questions` owns the rules.

use super::Ending;
use orchbee_machine_protocol::Id;

/// Why a login the person accepted here closed before its question opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Withdrawn {
    /// Another interface claimed it, or this client's claim did not take it.
    Taken,
    /// The hub ended it, the link to the hub ended, or this client could not show it.
    Ended(Ending),
}

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
    /// The accepted login closed before its question: the notice says why and takes nothing but
    /// the person's close.
    pub(crate) ended: Option<Withdrawn>,
}

/// An offer for a Machine this client cannot name yet.
struct Unknown {
    login: Id,
    machine: String,
    waiters: u64,
}

/// The offered logins, ascending by login.
#[derive(Default)]
pub(super) struct Offers {
    offered: Vec<Offered>,
    unknown: Vec<Unknown>,
    /// The accepted login that closed before its question, until the person closes its notice.
    ended: Option<Offered>,
}

impl Offers {
    /// The hub offers `login`, or counts its waiters again; `target` is unknown while this client
    /// has not read the Machine.
    pub(super) fn offer(
        &mut self,
        login: Id,
        machine: String,
        target: Option<String>,
        waiters: u64,
    ) {
        if let Some(offer) = self.offered.iter_mut().find(|offer| offer.login == login) {
            offer.waiters = waiters;
            return;
        }
        self.unknown.retain(|unknown| unknown.login != login);
        match target {
            Some(target) => self.insert(login, machine, target, waiters),
            None => self.unknown.push(Unknown {
                login,
                machine,
                waiters,
            }),
        }
    }

    /// Shows every offer whose Machine `target` now names.
    pub(super) fn resolve(&mut self, target: impl Fn(&str) -> Option<String>) {
        for unknown in std::mem::take(&mut self.unknown) {
            match target(&unknown.machine) {
                Some(found) => self.insert(unknown.login, unknown.machine, found, unknown.waiters),
                None => self.unknown.push(unknown),
            }
        }
    }

    fn insert(&mut self, login: Id, machine: String, target: String, waiters: u64) {
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
                ended: None,
            },
        );
    }

    /// The hub took `login` from this client, ended it or lost it: an accepted notice stays as an
    /// ended one saying `why`; any other goes.
    pub(super) fn withdraw(&mut self, login: Id, why: Withdrawn) {
        self.unknown.retain(|unknown| unknown.login != login);
        let Some(index) = self.offered.iter().position(|offer| offer.login == login) else {
            return;
        };
        let mut offer = self.offered.remove(index);
        if offer.accepted {
            offer.ended = Some(why);
            self.ended = Some(offer);
        }
    }

    /// `login` is no longer offered: its question opened, the person's selection claimed it, or
    /// the person declined it.
    pub(super) fn remove(&mut self, login: Id) {
        self.offered.retain(|offer| offer.login != login);
        self.unknown.retain(|unknown| unknown.login != login);
    }

    /// The link to the hub ended: every offer goes, and an accepted one stays as ended.
    pub(super) fn clear(&mut self, why: Withdrawn) {
        if let Some(login) = self
            .offered
            .iter()
            .find(|offer| offer.accepted)
            .map(|offer| offer.login)
        {
            self.withdraw(login, why);
        }
        self.offered.clear();
        self.unknown.clear();
    }

    /// The person closed the ended notice of `login`.
    pub(super) fn close(&mut self, login: Id) {
        self.ended.take_if(|offer| offer.login == login);
    }

    pub(super) fn get(&self, login: Id) -> Option<&Offered> {
        self.offered.iter().find(|offer| offer.login == login)
    }

    pub(super) const fn ended(&self) -> Option<&Offered> {
        self.ended.as_ref()
    }

    /// The notice to show: an ended one until the person closes it, then the accepted login until
    /// its question opens, otherwise the lowest.
    pub(super) fn first(&self) -> Option<&Offered> {
        self.ended
            .as_ref()
            .or_else(|| self.offered.iter().find(|offer| offer.accepted))
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
