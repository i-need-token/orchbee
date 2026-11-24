//! The catalog the hub serves and one carried [`Machine`] per record: a disabled record reads
//! `disabled`, a connection kind this build does not know `unsupported`, and an SSH record starts
//! `offline` without a cause or a next attempt. A reread keeps each record whose id stays, so its
//! carrier and history survive a reload that does not change it.
//!
//! Draft Machines live beside the records: each belongs to one connection, which alone sees it and
//! its events. A reread that holds a draft's id with the same connection and Session makes it an
//! ordinary Machine; one that holds the id otherwise drops it, as its owner's leaving does.

use super::{
    lifetime::Moment,
    machine::{Context, Effect, Kind, Machine, Route, Serials},
};
use orchbee_local_runtime::DataRoot;
use orchbee_machine_protocol::{
    Count, Id, MachineId, SessionName,
    hub::{Catalog, Event, MachineState, ReloadOutcome},
};
use orchbee_workbench_client::application::machine_credentials::SecretKind;
use orchbee_workbench_client::application::machine_spaces::LastSeen;
use orchbee_workbench_client::application::machines::{Connection, Error, Store};
use std::time::{Duration, Instant};

/// How many Machines a `presence` connection attempts at once; the rest wait for a slot.
const SILENT_STARTS: usize = 4;

#[derive(Debug)]
pub(super) struct Machines {
    revision: Count,
    entries: Vec<Machine>,
    /// The last carrier number and login id given out in the hub's life.
    serials: Serials,
    /// The Machines whose last-seen entry lists a Workspace, as the catalog was read: they take
    /// the first start slots.
    preferred: Vec<MachineId>,
}

/// What the hub knows around a transition, beside the carrier numbers the catalog keeps.
#[derive(Clone, Debug)]
pub(super) struct Conditions {
    pub(super) now: Moment,
    pub(super) presence: bool,
    pub(super) closing: bool,
    /// The connections that `asks`, in ascending order.
    pub(super) askers: Vec<u64>,
    /// How long a question put to a claimant stays open unanswered, and how long a `command`
    /// request waits for a person.
    pub(super) question: Duration,
}

/// What a transition gives the hub: the `machine` events for watchers, a draft's events for its
/// owner alone, while it watches, and the effects to apply.
#[derive(Debug, Default)]
pub(super) struct Outcome {
    pub(super) events: Vec<Event>,
    pub(super) owned: Vec<(u64, Event)>,
    pub(super) effects: Vec<Effect>,
}

impl Machines {
    /// The catalog served while `machines.json` cannot be read at start.
    pub(super) fn empty() -> Self {
        Self {
            revision: Count::new(0).expect("zero is a count"),
            entries: Vec::new(),
            serials: Serials::default(),
            preferred: Vec::new(),
        }
    }

    /// Reads the `machines.json` of `root`, or the `reload-result` failure that keeps the catalog
    /// in use.
    pub(super) fn read(root: &DataRoot) -> Result<Self, ReloadOutcome> {
        let catalog = Store::in_data_root(root.clone())
            .read()
            .map_err(|error| match error {
                Error::InvalidCatalog => ReloadOutcome::InvalidCatalog,
                _ => ReloadOutcome::Storage,
            })?;
        let revision = Count::new(catalog.revision).map_err(|_| ReloadOutcome::InvalidCatalog)?;
        let entries = catalog
            .profiles
            .iter()
            .map(|profile| {
                let kind = match &profile.draft.connection {
                    _ if !profile.enabled => Kind::Disabled,
                    Connection::Unsupported(_) => Kind::Unsupported,
                    Connection::Ssh { target } => Kind::Ssh(Route {
                        target: target.clone(),
                        session: SessionName::new(profile.draft.session.as_str())
                            .map_err(|_| ReloadOutcome::InvalidCatalog)?,
                    }),
                };
                MachineId::new(profile.id.as_str())
                    .map(|id| Machine::new(id, kind))
                    .map_err(|_| ReloadOutcome::InvalidCatalog)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let preferred = LastSeen::in_data_root(root.clone())
            .machines_with_workspaces()
            .iter()
            .filter_map(|id| MachineId::new(id).ok())
            .collect();
        Ok(Self {
            revision,
            entries,
            serials: Serials::default(),
            preferred,
        })
    }

    pub(super) const fn revision(&self) -> Count {
        self.revision
    }

    /// The Machines the catalog lists, drafts left out.
    fn listed(&self) -> impl Iterator<Item = &Machine> {
        self.entries
            .iter()
            .filter(|machine| machine.owner().is_none())
    }

    /// The carrier and the Session of the Machine `id` while it reads `online`; a draft offers
    /// none to anyone.
    pub(super) fn online_link(&self, id: &MachineId) -> Option<(u64, SessionName)> {
        self.listed()
            .find(|machine| machine.id() == id)
            .and_then(Machine::online_link)
            .map(|(carrier, session)| (carrier, session.clone()))
    }

    /// The state of the Machine `id`, when the catalog lists it.
    pub(super) fn state(&self, id: &MachineId) -> Option<&MachineState> {
        self.listed()
            .find(|machine| machine.id() == id)
            .map(Machine::state)
    }

    /// Whether the catalog lists the Machine `id` as `kind`.
    pub(super) fn saved_as(&self, id: &MachineId, kind: &Kind) -> bool {
        self.listed()
            .any(|machine| machine.id() == id && machine.kind() == kind)
    }

    /// The draft Machine `id`, whoever owns it.
    pub(super) fn draft(&self, id: &MachineId) -> Option<&Machine> {
        self.entries
            .iter()
            .find(|machine| machine.owner().is_some() && machine.id() == id)
    }

    /// Every draft Machine ID currently held beside the catalog.
    pub(super) fn draft_ids(&self) -> impl Iterator<Item = &MachineId> {
        self.entries
            .iter()
            .filter(|machine| machine.owner().is_some())
            .map(Machine::id)
    }

    /// The draft the connection `peer` owns; a connection keeps one at a time.
    pub(super) fn owned_draft(&self, peer: u64) -> Option<&MachineId> {
        self.entries
            .iter()
            .find(|machine| machine.owner() == Some(peer))
            .map(Machine::id)
    }

    /// Adds a draft Machine; the caller checked that no Machine has its id.
    pub(super) fn add_draft(&mut self, machine: Machine) {
        debug_assert!(
            machine.owner().is_some(),
            "only a draft is added beside the catalog"
        );
        self.entries.push(machine);
    }

    /// Drops the draft `id` after `transition`, its link closed. Remembered answers saved while
    /// it was a draft go with it: no record will ever name them.
    pub(super) fn drop_draft(
        &mut self,
        id: &MachineId,
        conditions: &Conditions,
        transition: impl FnOnce(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) -> Outcome {
        let mut outcome = Outcome::default();
        if let Some(position) = self
            .entries
            .iter()
            .position(|machine| machine.owner().is_some() && machine.id() == id)
        {
            let mut machine = self.entries.remove(position);
            let mut cx = context(&machine, conditions, &mut self.serials);
            outcome.effects.extend(transition(&mut machine, &mut cx));
            outcome.effects.extend(forget_all(machine.id()));
        }
        outcome
    }

    /// The connection `peer` closed: every draft it owned is dropped.
    pub(super) fn drop_owned(&mut self, peer: u64, conditions: &Conditions) -> Outcome {
        let owned: Vec<MachineId> = self
            .entries
            .iter()
            .filter(|machine| machine.owner() == Some(peer))
            .map(|machine| machine.id().clone())
            .collect();
        let mut outcome = Outcome::default();
        for id in owned {
            let dropped = self.drop_draft(&id, conditions, |machine, _| machine.remove());
            outcome.effects.extend(dropped.effects);
        }
        outcome
    }

    fn catalog(&self) -> Catalog {
        Catalog::new(
            self.revision,
            self.listed().map(|machine| machine.id().clone()).collect(),
        )
    }

    /// `catalog`, then one `machine` per id in catalog order: what turning `watch` on sends. The
    /// connection `peer` then reads its own drafts.
    pub(super) fn events(&self, peer: u64) -> Vec<Event> {
        let mut events = Vec::with_capacity(self.entries.len() + 1);
        events.push(Event::Catalog(self.catalog()));
        events.extend(self.listed().map(Machine::event));
        events.extend(
            self.entries
                .iter()
                .filter(|machine| machine.owner() == Some(peer))
                .map(Machine::event),
        );
        events
    }

    /// The Machine whose carrier is `carrier`.
    pub(super) fn owner(&self, carrier: u64) -> Option<&MachineId> {
        self.entries
            .iter()
            .find(|machine| machine.carrier() == Some(carrier))
            .map(Machine::id)
    }

    /// The Machine whose running login episode is `login`, as the connection `peer` may reach it:
    /// a draft's episode is its owner's alone.
    pub(super) fn login_owner(&self, login: Id, peer: u64) -> Option<&Machine> {
        self.entries.iter().find(|machine| {
            machine.login() == Some(login) && machine.owner().is_none_or(|owner| owner == peer)
        })
    }

    /// When the earliest timed transition of any Machine is due.
    pub(super) fn next_wake(&self) -> Option<Instant> {
        self.entries.iter().filter_map(Machine::next_wake).min()
    }

    /// Applies `transition` to the Machine `id`, when the catalog lists it.
    pub(super) fn update(
        &mut self,
        id: &MachineId,
        conditions: &Conditions,
        transition: impl FnOnce(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) -> Outcome {
        let mut outcome = Outcome::default();
        if let Some(machine) = self.entries.iter_mut().find(|machine| machine.id() == id) {
            step(
                machine,
                &mut self.serials,
                conditions,
                &mut outcome,
                transition,
            );
        }
        self.admit(conditions, &mut outcome);
        outcome
    }

    /// Applies `transition` to every Machine in catalog order.
    pub(super) fn each(
        &mut self,
        conditions: &Conditions,
        mut transition: impl FnMut(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) -> Outcome {
        let mut outcome = Outcome::default();
        for machine in &mut self.entries {
            step(
                machine,
                &mut self.serials,
                conditions,
                &mut outcome,
                &mut transition,
            );
        }
        self.admit(conditions, &mut outcome);
        outcome
    }

    /// Starts queued Machines while fewer than [`SILENT_STARTS`] attempts are under way: those
    /// that listed a Workspace first, then catalog order.
    fn admit(&mut self, conditions: &Conditions, outcome: &mut Outcome) {
        let mut open = self.entries.iter().filter(|m| m.opening()).count();
        if open >= SILENT_STARTS || !conditions.presence || conditions.closing {
            return;
        }
        let mut queued: Vec<usize> = (0..self.entries.len())
            .filter(|&at| self.entries[at].queued())
            .collect();
        queued.sort_by_key(|&at| !self.preferred.contains(self.entries[at].id()));
        for at in queued {
            if open >= SILENT_STARTS {
                break;
            }
            let machine = &mut self.entries[at];
            step(
                machine,
                &mut self.serials,
                conditions,
                outcome,
                Machine::admit,
            );
            open += usize::from(self.entries[at].opening());
        }
    }

    /// Replaces the catalog with `next`. A record whose id stays keeps its Machine, reconciled to
    /// its new content; a removed record's carrier ends and its waiters read `not-found`; a new or
    /// changed one [arrives](Machine::arrived). Watchers receive `catalog` and every Machine that
    /// is new or reads differently, or nothing when neither changed.
    pub(super) fn replace(&mut self, next: Self, conditions: &Conditions) -> Outcome {
        let listed = |machines: &Self| -> Vec<MachineId> {
            machines
                .listed()
                .map(|machine| machine.id().clone())
                .collect()
        };
        let catalog_changed = self.revision != next.revision || listed(self) != listed(&next);
        let mut outcome = Outcome::default();
        let (mut drafts, mut previous): (Vec<Machine>, Vec<Machine>) =
            std::mem::take(&mut self.entries)
                .into_iter()
                .partition(|machine| machine.owner().is_some());
        for removed in previous
            .iter_mut()
            .filter(|machine| next.state(machine.id()).is_none())
        {
            outcome.effects.extend(removed.remove());
        }
        for incoming in next.entries {
            let position = previous
                .iter()
                .position(|machine| machine.id() == incoming.id());
            let drafted = drafts
                .iter()
                .position(|machine| machine.id() == incoming.id());
            match (position, drafted) {
                (None, Some(drafted)) if drafts[drafted].drafts(incoming.kind()) => {
                    // Saved as it was connected: the draft keeps its link, seen by everyone now.
                    let mut machine = drafts.swap_remove(drafted);
                    machine.promote();
                    outcome.events.push(machine.event());
                    self.entries.push(machine);
                }
                (None, drafted) => {
                    // A draft saved otherwise goes with its remembered answers; the record
                    // arrives as itself.
                    if let Some(drafted) = drafted {
                        let mut draft = drafts.swap_remove(drafted);
                        outcome.effects.extend(draft.remove());
                        outcome.effects.extend(forget_all(draft.id()));
                    }
                    let mut machine = incoming;
                    let mut cx = context(&machine, conditions, &mut self.serials);
                    outcome.effects.extend(machine.arrived(&mut cx));
                    outcome.events.push(machine.event());
                    self.entries.push(machine);
                }
                (Some(position), drafted) => {
                    if let Some(drafted) = drafted {
                        let mut draft = drafts.swap_remove(drafted);
                        outcome.effects.extend(draft.remove());
                        if !draft.drafts(incoming.kind()) {
                            outcome.effects.extend(forget_all(draft.id()));
                        }
                    }
                    let mut machine = previous.swap_remove(position);
                    let kind = incoming.into_kind();
                    step(
                        &mut machine,
                        &mut self.serials,
                        conditions,
                        &mut outcome,
                        |machine, cx| machine.reconcile(kind, cx),
                    );
                    self.entries.push(machine);
                }
            }
        }
        // Drafts the reread does not name stay their owners'.
        self.entries.append(&mut drafts);
        self.revision = next.revision;
        self.preferred = next.preferred;
        self.admit(conditions, &mut outcome);
        if catalog_changed {
            outcome.events.insert(0, Event::Catalog(self.catalog()));
        }
        outcome
    }
}

/// Applies `transition` to `machine` and records its effects and, when it reads differently, its
/// `machine` event.
fn step(
    machine: &mut Machine,
    serials: &mut Serials,
    conditions: &Conditions,
    outcome: &mut Outcome,
    transition: impl FnOnce(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
) {
    let before = machine.event();
    let mut cx = context(machine, conditions, serials);
    outcome.effects.extend(transition(machine, &mut cx));
    let after = machine.event();
    if after != before {
        match machine.owner() {
            Some(owner) => outcome.owned.push((owner, after)),
            None => outcome.events.push(after),
        }
    }
}

/// What `machine` knows around a transition. A draft is its owner's alone: only its owner is
/// asked, and no `presence` attempts it silently.
fn context<'a>(
    machine: &Machine,
    conditions: &'a Conditions,
    serials: &'a mut Serials,
) -> Context<'a> {
    let askers: &'a [u64] = match machine.owner() {
        None => &conditions.askers,
        Some(owner) => conditions
            .askers
            .binary_search(&owner)
            .map_or(&[], |at| &conditions.askers[at..=at]),
    };
    Context {
        now: conditions.now,
        presence: conditions.presence && machine.owner().is_none(),
        closing: conditions.closing,
        askers,
        question: conditions.question,
        serials,
    }
}

/// Deletes both kinds of remembered answer the Machine `id` may hold.
fn forget_all(id: &MachineId) -> impl Iterator<Item = Effect> + '_ {
    SecretKind::ALL.into_iter().map(|kind| Effect::Forget {
        machine: id.clone(),
        kind,
    })
}

#[cfg(test)]
mod tests;
