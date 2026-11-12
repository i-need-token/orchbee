//! The catalog the hub serves and one carried [`Machine`] per record: a disabled record reads
//! `disabled`, a connection kind this build does not know `unsupported`, and an SSH record starts
//! `offline` without a cause or a next attempt. A reread keeps each record whose id stays, so its
//! carrier and history survive a reload that does not change it.

use super::{
    lifetime::Moment,
    machine::{Context, Effect, Kind, Machine, Route},
};
use orchbee_local_runtime::DataRoot;
use orchbee_machine_protocol::{
    Count, MachineId, SessionName,
    hub::{Catalog, Event, MachineState, ReloadOutcome},
};
use orchbee_workbench_client::application::machines::{Connection, Error, Store};
use std::time::Instant;

#[derive(Debug)]
pub(super) struct Machines {
    revision: Count,
    entries: Vec<Machine>,
    /// The last carrier number given out in the hub's life.
    carriers: u64,
}

/// What the hub knows around a transition, beside the carrier numbers the catalog keeps.
#[derive(Clone, Copy, Debug)]
pub(super) struct Conditions {
    pub(super) now: Moment,
    pub(super) presence: bool,
    pub(super) closing: bool,
}

/// What a transition gives the hub: the `machine` events for watchers and the effects to apply.
#[derive(Debug, Default)]
pub(super) struct Outcome {
    pub(super) events: Vec<Event>,
    pub(super) effects: Vec<Effect>,
}

impl Machines {
    /// The catalog served while `machines.json` cannot be read at start.
    pub(super) fn empty() -> Self {
        Self {
            revision: Count::new(0).expect("zero is a count"),
            entries: Vec::new(),
            carriers: 0,
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
        let machines = Self {
            revision,
            entries,
            carriers: 0,
        };
        machines
            .catalog()
            .map_err(|()| ReloadOutcome::InvalidCatalog)?;
        Ok(machines)
    }

    pub(super) const fn revision(&self) -> Count {
        self.revision
    }

    /// The state of the Machine `id`, when the catalog lists it.
    pub(super) fn state(&self, id: &MachineId) -> Option<&MachineState> {
        self.entries
            .iter()
            .find(|machine| machine.id() == id)
            .map(Machine::state)
    }

    fn catalog(&self) -> Result<Catalog, ()> {
        Catalog::new(
            self.revision,
            self.entries
                .iter()
                .map(|machine| machine.id().clone())
                .collect(),
        )
        .map_err(drop)
    }

    /// `catalog`, then one `machine` per id in catalog order: what turning `watch` on sends.
    pub(super) fn events(&self) -> Vec<Event> {
        let mut events = Vec::with_capacity(self.entries.len() + 1);
        events.extend(self.catalog().ok().map(Event::Catalog));
        events.extend(self.entries.iter().map(Machine::event));
        events
    }

    /// The Machine whose carrier is `carrier`.
    pub(super) fn owner(&self, carrier: u64) -> Option<&MachineId> {
        self.entries
            .iter()
            .find(|machine| machine.carrier() == Some(carrier))
            .map(Machine::id)
    }

    /// When the earliest timed transition of any Machine is due.
    pub(super) fn next_wake(&self) -> Option<Instant> {
        self.entries.iter().filter_map(Machine::next_wake).min()
    }

    /// Applies `transition` to the Machine `id`, when the catalog lists it.
    pub(super) fn update(
        &mut self,
        id: &MachineId,
        conditions: Conditions,
        transition: impl FnOnce(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) -> Outcome {
        let mut outcome = Outcome::default();
        if let Some(machine) = self.entries.iter_mut().find(|machine| machine.id() == id) {
            step(
                machine,
                &mut self.carriers,
                conditions,
                &mut outcome,
                transition,
            );
        }
        outcome
    }

    /// Applies `transition` to every Machine in catalog order.
    pub(super) fn each(
        &mut self,
        conditions: Conditions,
        mut transition: impl FnMut(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) -> Outcome {
        let mut outcome = Outcome::default();
        for machine in &mut self.entries {
            step(
                machine,
                &mut self.carriers,
                conditions,
                &mut outcome,
                &mut transition,
            );
        }
        outcome
    }

    /// Replaces the catalog with `next`. A record whose id stays keeps its Machine, reconciled to
    /// its new content; a removed record's carrier ends and its waiters read `not-found`; a new or
    /// changed one [arrives](Machine::arrived). Watchers receive `catalog` and every Machine that
    /// is new or reads differently, or nothing when neither changed.
    pub(super) fn replace(&mut self, next: Self, conditions: Conditions) -> Outcome {
        let listed = |machines: &Self| -> Vec<MachineId> {
            machines
                .entries
                .iter()
                .map(|machine| machine.id().clone())
                .collect()
        };
        let catalog_changed = self.revision != next.revision || listed(self) != listed(&next);
        let mut outcome = Outcome::default();
        let mut previous = std::mem::take(&mut self.entries);
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
            match position {
                Some(position) => {
                    let mut machine = previous.swap_remove(position);
                    let kind = incoming.into_kind();
                    step(
                        &mut machine,
                        &mut self.carriers,
                        conditions,
                        &mut outcome,
                        |machine, cx| machine.reconcile(kind, cx),
                    );
                    self.entries.push(machine);
                }
                None => {
                    let mut machine = incoming;
                    let mut cx = context(conditions, &mut self.carriers);
                    outcome.effects.extend(machine.arrived(&mut cx));
                    outcome.events.push(machine.event());
                    self.entries.push(machine);
                }
            }
        }
        self.revision = next.revision;
        if catalog_changed && let Ok(catalog) = self.catalog() {
            outcome.events.insert(0, Event::Catalog(catalog));
        }
        outcome
    }

    /// The connection `peer` closed: its requests wait no longer.
    pub(super) fn forget(&mut self, peer: u64) {
        for machine in &mut self.entries {
            machine.forget(peer);
        }
    }
}

/// Applies `transition` to `machine` and records its effects and, when it reads differently, its
/// `machine` event.
fn step(
    machine: &mut Machine,
    carriers: &mut u64,
    conditions: Conditions,
    outcome: &mut Outcome,
    transition: impl FnOnce(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
) {
    let before = machine.event();
    outcome
        .effects
        .extend(transition(machine, &mut context(conditions, carriers)));
    let after = machine.event();
    if after != before {
        outcome.events.push(after);
    }
}

fn context(conditions: Conditions, carriers: &mut u64) -> Context<'_> {
    Context {
        now: conditions.now,
        presence: conditions.presence,
        closing: conditions.closing,
        carriers,
    }
}
