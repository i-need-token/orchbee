//! How the hub answers `capacity`: this computer's at once, a Machine's from its link's
//! bookkeeping, and each waiting request once its link answers, its bound passes, its link ends or
//! the hub closes. Nothing here connects a Machine.

use super::{
    capacities::Waiter,
    carrier::Handle,
    state::{Hub, Line},
};
use orchbee_machine_protocol::{
    Id, MachineId,
    capacity::Capacity,
    hub::{CapacityOutcome, Reply},
    link,
};
use std::{collections::BTreeMap, time::Instant};

impl Hub {
    /// Answers a `capacity` from the connection `peer` at once, or returns `None` when it waits
    /// for the Machine's link. The caller wakes the hub state's signal, since the supervisor may
    /// have a new wait to keep.
    pub(super) fn capacity_request(
        &mut self,
        peer: u64,
        request: Id,
        machine: Option<MachineId>,
    ) -> Option<CapacityOutcome> {
        if self.lifetime.is_closing() {
            return Some(CapacityOutcome::Stopping);
        }
        let Some(machine) = machine else {
            return Some(self.capacities.local());
        };
        if self.machines.state(&machine).is_none() {
            return Some(CapacityOutcome::NotFound);
        }
        let Some((carrier, _)) = self.machines.online_link(&machine) else {
            return Some(CapacityOutcome::NotOnline);
        };
        let carriers = &self.carriers;
        self.capacities
            .request(carrier, peer, request, || ask(carriers, carrier))
    }

    /// The link on `carrier` answered the ask `request`; `None` is `unsupported`.
    pub(super) fn capacity_result(
        &mut self,
        carrier: u64,
        request: Id,
        capacity: Option<Capacity>,
    ) {
        if let Some((waiters, outcome)) = self.capacities.result(carrier, request, capacity) {
            self.answer_capacity(waiters, &outcome);
        }
    }

    /// Every request waiting past its bound at `now` reads `unknown`.
    pub(super) fn expire_capacity(&mut self, now: Instant) {
        let expired = self.capacities.expire(now);
        self.answer_capacity(expired, &CapacityOutcome::Unknown);
    }

    /// The carrier `carrier` ended: its Machine is no longer `online`.
    pub(super) fn end_capacity_on(&mut self, carrier: u64) {
        let waiters = self.capacities.end(carrier);
        self.answer_capacity(waiters, &CapacityOutcome::NotOnline);
    }

    /// The hub is closing: every waiting request reads `stopping`.
    pub(super) fn end_capacity_closing(&mut self) {
        let waiters = self.capacities.drain();
        self.answer_capacity(waiters, &CapacityOutcome::Stopping);
    }

    /// The connection `peer` is over: its waiting requests go.
    pub(super) fn forget_capacity_waiters(&mut self, peer: u64) {
        self.capacities.forget(peer);
    }

    fn answer_capacity(&self, waiters: Vec<Waiter>, outcome: &CapacityOutcome) {
        for waiter in waiters {
            if let Some(peer) = self.peers.get(&waiter.peer) {
                reply(&peer.line, waiter.request, outcome);
            }
        }
    }
}

/// Sends `capacity` on the link of `carrier` while it is open: the ask's id, or `None`.
fn ask(carriers: &BTreeMap<u64, Handle>, carrier: u64) -> Option<Id> {
    let line = carriers.get(&carrier)?.line();
    let mut wire = line.lock();
    let asked = if wire.open {
        wire.connection.request(link::Request::Capacity).ok()
    } else {
        None
    };
    drop(wire);
    line.wake();
    asked
}

fn reply(line: &Line, request: Id, outcome: &CapacityOutcome) {
    let mut wire = line.lock();
    if wire.open {
        let _replied = wire
            .connection
            .reply(request, Reply::Capacity(outcome.clone()));
    }
    drop(wire);
    line.wake();
}
