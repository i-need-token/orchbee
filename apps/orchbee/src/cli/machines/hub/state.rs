//! The hub's state under its lock: the lifetime, the catalog with each carried Machine, the
//! carriers, the registry of attached connections, the open askpass helpers, the relay table, the
//! `stop-session` runs, and the counts admission and closing wait on.
//! Each connection's and each link's protocol state machine lives under its own lock. Lock order:
//! the hub state's lock before a connection's or a link's; a thread holding one of those takes no
//! other, and none is held across I/O.

use super::{
    carrier::Handle,
    catalog::{Conditions, Machines, Outcome},
    lifetime::{Lifetime, Moment},
    machine::{Context, Effect, Machine, Route},
    relay::Relays,
    stop::Stops,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::DataRoot;
use orchbee_machine_protocol::{
    Connection, MachineId,
    hub::{self, ConnectOutcome, Event, Reply},
};
use std::{collections::BTreeMap, net::Shutdown, sync::Arc, time::Duration};

/// The hub state under its lock, and the signal that wakes the supervisor and closing.
pub(super) type Shared = super::super::shared_carrier::Shared<Hub>;
/// One connection's state under its own lock, and the signal that wakes its writer and its reader
/// paused at the backlog.
pub(super) type Line = super::super::shared_carrier::Shared<Wire>;

/// Attached connections.
pub(super) const PEERS: usize = 256;
/// Control requests served at once.
pub(super) const CONTROLS: usize = 4;
/// Candidates read at once.
pub(super) const CANDIDATES: usize = 64;

pub(super) struct Hub {
    /// The data root the hub serves, resolved once from its view of the environment.
    pub(super) root: DataRoot,
    pub(super) lifetime: Lifetime,
    pub(super) machines: Machines,
    pub(super) peers: BTreeMap<u64, Peer>,
    next_peer: u64,
    /// Every carrier from its start until its owner reports its end.
    pub(super) carriers: BTreeMap<u64, Handle>,
    /// Carriers whose owners the supervisor starts next, outside the lock.
    pub(super) starting: Vec<(u64, Route)>,
    /// The open askpass helper of each carrier that has one, under its registration number.
    askers: BTreeMap<u64, (u64, LocalStream)>,
    next_asker: u64,
    /// Helper connections being served.
    pub(super) helpers: usize,
    /// How long a question stays open unanswered.
    question: Duration,
    /// Every channel relayed to a link, a `stop-session` run's current exchange among them.
    pub(super) relays: Relays,
    /// Every `stop-session` run until its outcome.
    pub(super) stops: Stops,
    pub(super) controls: usize,
    pub(super) candidates: usize,
    /// Catalog reads in flight, which closing waits for.
    pub(super) reloads: usize,
    /// The order catalog reads started in, so an older read never replaces a newer one.
    reload_started: u64,
    reload_applied: u64,
}

/// One attached connection as the hub state knows it.
pub(super) struct Peer {
    pub(super) line: Arc<Line>,
    stream: LocalStream,
    pub(super) presence: bool,
    /// The connection shows login questions.
    pub(super) asks: bool,
    pub(super) watch: bool,
    pub(super) writer_done: bool,
    /// The reader and the writer that still run.
    threads: u8,
}

/// One connection's own state.
pub(super) struct Wire {
    pub(super) connection: Connection<hub::Hub>,
    /// False once the connection is over at the hub: the client left, sent `bye`, or a write
    /// failed. A connection that committed a violation stays open until its `bye` is written.
    pub(super) open: bool,
}

impl Hub {
    pub(super) fn new(
        root: DataRoot,
        lifetime: Lifetime,
        machines: Machines,
        question: Duration,
    ) -> Self {
        Self {
            root,
            lifetime,
            machines,
            peers: BTreeMap::new(),
            next_peer: 0,
            carriers: BTreeMap::new(),
            starting: Vec::new(),
            askers: BTreeMap::new(),
            next_asker: 0,
            helpers: 0,
            question,
            relays: Relays::default(),
            stops: Stops::default(),
            controls: 0,
            candidates: 0,
            reloads: 0,
            reload_started: 0,
            reload_applied: 0,
        }
    }

    /// Registers an admitted connection whose reader and writer are about to run.
    pub(super) fn register(&mut self, stream: LocalStream) -> (u64, Arc<Line>) {
        self.next_peer += 1;
        let line = Arc::new(Line::new(Wire {
            connection: Connection::<hub::Hub>::acceptor(),
            open: true,
        }));
        self.peers.insert(
            self.next_peer,
            Peer {
                line: line.clone(),
                stream,
                presence: false,
                asks: false,
                watch: false,
                writer_done: false,
                threads: 2,
            },
        );
        (self.next_peer, line)
    }

    /// Ends the connection `id` at the hub: it stops holding the hub and both directions close.
    /// The caller wakes the hub state's signal, since the lifetime may have changed.
    pub(super) fn close(&mut self, id: u64) {
        let Some(peer) = self.peers.get_mut(&id) else {
            return;
        };
        let held = std::mem::take(&mut peer.presence);
        peer.asks = false;
        peer.line.lock().open = false;
        peer.line.wake();
        let _closed = peer.stream.shutdown(Shutdown::Both);
        if held {
            self.lifetime.detach();
        }
        // An episode the connection claimed ends with its attempt.
        self.machines(|machine, cx| machine.forget(id, cx));
        self.peer_left(id);
    }

    /// One thread of the connection `id` ended; the last one forgets it.
    pub(super) fn release(&mut self, id: u64) {
        if let Some(peer) = self.peers.get_mut(&id) {
            peer.threads = peer.threads.saturating_sub(1);
            if peer.threads == 0 {
                self.close(id);
                self.peers.remove(&id);
            }
        }
    }

    /// Queues `event` to every open connection attached with `watch`, waking only those writers.
    /// The caller holds no connection's lock.
    pub(super) fn broadcast(&self, event: &Event) {
        for peer in self.peers.values().filter(|peer| peer.watch) {
            let mut wire = peer.line.lock();
            if wire.open {
                let _queued = wire.connection.event(event.clone());
                drop(wire);
                peer.line.wake();
            }
        }
    }

    /// Queues the lifetime state to every watcher.
    pub(super) fn announce(&self) {
        self.broadcast(&Event::Hub(self.lifetime.state()));
    }

    /// Starts a catalog read outside the lock; returns its place in the start order.
    pub(super) fn begin_reload(&mut self) -> u64 {
        self.reloads += 1;
        self.reload_started += 1;
        self.reload_started
    }

    /// Ends the catalog read `order`, applying `read` unless a newer read was applied already.
    pub(super) fn finish_reload(&mut self, order: u64, read: Machines) {
        self.reloads = self.reloads.saturating_sub(1);
        if order > self.reload_applied {
            self.reload_applied = order;
            let conditions = self.conditions();
            let outcome = self.machines.replace(read, conditions);
            self.publish(outcome);
        }
    }

    /// Ends a catalog read that failed.
    pub(super) fn abandon_reload(&mut self) {
        self.reloads = self.reloads.saturating_sub(1);
    }

    /// What a transition of a Machine knows now.
    pub(super) fn conditions(&self) -> Conditions {
        Conditions {
            now: Moment::now(),
            presence: self.peers.values().any(|peer| peer.presence),
            closing: self.lifetime.is_closing(),
            asking: self.peers.values().any(|peer| peer.asks),
            question: self.question,
        }
    }

    /// Registers the open askpass helper of `carrier`; its registration number, or `None` while the
    /// carrier has one.
    pub(super) fn admit_asker(&mut self, carrier: u64, stream: LocalStream) -> Option<u64> {
        if self.askers.contains_key(&carrier) {
            return None;
        }
        self.next_asker += 1;
        self.askers.insert(carrier, (self.next_asker, stream));
        Some(self.next_asker)
    }

    /// Forgets the helper `serial` of `carrier`; false when the hub already closed it.
    pub(super) fn release_asker(&mut self, carrier: u64, serial: u64) -> bool {
        match self.askers.get(&carrier) {
            Some((registered, _)) if *registered == serial => {
                self.askers.remove(&carrier);
                true
            }
            _ => false,
        }
    }

    /// Applies `transition` to the Machine `id`, sends what changed to watchers and applies the
    /// effects. The caller wakes the hub state's signal, since a carrier may wait to start.
    pub(super) fn machine(
        &mut self,
        id: &MachineId,
        transition: impl FnOnce(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) {
        let conditions = self.conditions();
        let outcome = self.machines.update(id, conditions, transition);
        self.publish(outcome);
    }

    /// As [`Self::machine`], for the Machine the carrier `carrier` belongs to.
    pub(super) fn machine_of(
        &mut self,
        carrier: u64,
        transition: impl FnOnce(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) {
        if let Some(machine) = self
            .carriers
            .get(&carrier)
            .map(|handle| handle.machine.clone())
        {
            self.machine(&machine, transition);
        }
    }

    /// As [`Self::machine`], for every Machine.
    pub(super) fn machines(
        &mut self,
        transition: impl FnMut(&mut Machine, &mut Context<'_>) -> Vec<Effect>,
    ) {
        let conditions = self.conditions();
        let outcome = self.machines.each(conditions, transition);
        self.publish(outcome);
    }

    /// Sends a transition's events to every watcher and applies its effects.
    pub(super) fn publish(&mut self, outcome: Outcome) {
        for event in &outcome.events {
            self.broadcast(event);
        }
        for effect in outcome.effects {
            self.apply(effect);
        }
    }

    fn apply(&mut self, effect: Effect) {
        match effect {
            Effect::Start { carrier, route } => {
                if let Some(machine) = self.machines.owner(carrier).cloned() {
                    self.carriers.insert(carrier, Handle::new(machine));
                    self.starting.push((carrier, route));
                }
            }
            Effect::End { carrier } => {
                self.carrier_ended(carrier);
                let queued = self.starting.iter().any(|(queued, _)| *queued == carrier);
                if queued {
                    // Its owner never started, so nothing else would forget it.
                    self.starting.retain(|(queued, _)| *queued != carrier);
                    self.carriers.remove(&carrier);
                } else if let Some(handle) = self.carriers.get_mut(&carrier) {
                    handle.end();
                }
            }
            Effect::EnsureSession { carrier, session } => {
                if let Some(handle) = self.carriers.get(&carrier) {
                    handle.ensure_session(session);
                }
            }
            Effect::Ping { carrier } => {
                if let Some(handle) = self.carriers.get(&carrier) {
                    handle.ping();
                }
            }
            Effect::Notify { peer, event } => {
                if let Some(peer) = self.peers.get(&peer) {
                    let mut wire = peer.line.lock();
                    if wire.open {
                        let _queued = wire.connection.event(event);
                        drop(wire);
                        peer.line.wake();
                    }
                }
            }
            Effect::Close { carrier } => {
                if let Some((_, stream)) = self.askers.remove(&carrier) {
                    let _closed = stream.shutdown(Shutdown::Both);
                }
            }
            Effect::Reply { waiters, result } => {
                // A `connect` that ends `online` starts the expiry countdown again.
                if matches!(result.outcome, ConnectOutcome::Online { .. })
                    && !waiters.is_empty()
                    && self.lifetime.connected(Moment::now())
                {
                    self.announce();
                }
                for waiter in waiters {
                    if let Some(peer) = self.peers.get(&waiter.peer) {
                        let mut wire = peer.line.lock();
                        if wire.open {
                            let _replied = wire
                                .connection
                                .reply(waiter.request, Reply::Connect(result.clone()));
                            drop(wire);
                            peer.line.wake();
                        }
                    }
                }
            }
        }
    }
}
