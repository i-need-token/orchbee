//! The catalog the hub serves and each Machine's state while the hub cannot connect: `disabled`
//! for a disabled record, `unsupported` for a connection kind this build does not know, otherwise
//! `offline` without a cause or a next attempt.

use orchbee_machine_protocol::{
    Count, MachineId,
    hub::{Catalog, Event, MachineEvent, MachineState, ReloadOutcome, UnsupportedCause},
};
use orchbee_workbench_client::application::machines::{Connection, Error, Store};

#[derive(Debug)]
pub(super) struct Machines {
    revision: Count,
    entries: Vec<(MachineId, MachineState)>,
}

impl Machines {
    /// The catalog served while `machines.json` cannot be read at start.
    pub(super) fn empty() -> Self {
        Self {
            revision: Count::new(0).expect("zero is a count"),
            entries: Vec::new(),
        }
    }

    /// Reads `machines.json`, or the `reload-result` failure that keeps the catalog in use.
    pub(super) fn read() -> Result<Self, ReloadOutcome> {
        let catalog = Store::configured()
            .and_then(|store| store.read())
            .map_err(|error| match error {
                Error::InvalidCatalog => ReloadOutcome::InvalidCatalog,
                _ => ReloadOutcome::Storage,
            })?;
        let revision = Count::new(catalog.revision).map_err(|_| ReloadOutcome::InvalidCatalog)?;
        let entries = catalog
            .profiles
            .iter()
            .map(|profile| {
                let state = if !profile.enabled {
                    MachineState::Disabled
                } else if matches!(profile.draft.connection, Connection::Unsupported(_)) {
                    MachineState::Unsupported {
                        cause: Some(UnsupportedCause::ConnectionKind),
                    }
                } else {
                    MachineState::Offline {
                        cause: None,
                        next_attempt: None,
                    }
                };
                MachineId::new(profile.id.as_str()).map(|id| (id, state))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ReloadOutcome::InvalidCatalog)?;
        let machines = Self { revision, entries };
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
            .find(|(entry, _)| entry == id)
            .map(|(_, state)| state)
    }

    fn catalog(&self) -> Result<Catalog, ()> {
        Catalog::new(
            self.revision,
            self.entries.iter().map(|(id, _)| id.clone()).collect(),
        )
        .map_err(drop)
    }

    /// `catalog`, then one `machine` per id in catalog order: what turning `watch` on sends.
    pub(super) fn events(&self) -> Vec<Event> {
        let mut events = Vec::with_capacity(self.entries.len() + 1);
        events.extend(self.catalog().ok().map(Event::Catalog));
        events.extend(
            self.entries
                .iter()
                .map(|(id, state)| machine(id.clone(), state.clone())),
        );
        events
    }

    /// Replaces the catalog with `next`, returning what watchers receive: `catalog` and every
    /// Machine whose state is new or changed, or nothing when neither changed.
    pub(super) fn replace(&mut self, next: Self) -> Vec<Event> {
        let listed = |machines: &Self| -> Vec<MachineId> {
            machines.entries.iter().map(|(id, _)| id.clone()).collect()
        };
        let changed: Vec<Event> = next
            .entries
            .iter()
            .filter(|(id, state)| self.state(id) != Some(state))
            .map(|(id, state)| machine(id.clone(), state.clone()))
            .collect();
        let catalog_changed = self.revision != next.revision || listed(self) != listed(&next);
        *self = next;
        if !catalog_changed && changed.is_empty() {
            return Vec::new();
        }
        let mut events = Vec::with_capacity(changed.len() + 1);
        events.extend(self.catalog().ok().map(Event::Catalog));
        events.extend(changed);
        events
    }
}

fn machine(machine: MachineId, state: MachineState) -> Event {
    Event::Machine(MachineEvent {
        machine,
        state,
        last_attempt: None,
        remote: None,
        draft: false,
    })
}
