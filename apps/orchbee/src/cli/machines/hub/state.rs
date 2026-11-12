//! The hub's state under its one lock: the lifetime, the catalog, the attached connections with
//! their protocol state machines, and the counts admission and closing wait on.

use super::{catalog::Machines, lifetime::Lifetime};
use orchbee_local_ipc::LocalStream;
use orchbee_machine_protocol::{
    Connection,
    hub::{self, Event},
};
use std::{collections::BTreeMap, net::Shutdown};

/// The state under its lock, and the signal that wakes the supervisor, closing, every writer and
/// every paused reader.
pub(super) type Shared = super::super::shared_carrier::Shared<Hub>;

/// Attached connections.
pub(super) const PEERS: usize = 256;
/// Control requests served at once.
pub(super) const CONTROLS: usize = 4;
/// Candidates read at once.
pub(super) const CANDIDATES: usize = 64;

pub(super) struct Hub {
    pub(super) lifetime: Lifetime,
    pub(super) machines: Machines,
    pub(super) peers: BTreeMap<u64, Peer>,
    next_peer: u64,
    pub(super) controls: usize,
    pub(super) candidates: usize,
    /// Catalog reads in flight, which closing waits for.
    pub(super) reloads: usize,
    /// The order catalog reads started in, so an older read never replaces a newer one.
    reload_started: u64,
    reload_applied: u64,
}

/// One attached connection.
pub(super) struct Peer {
    pub(super) connection: Connection<hub::Hub>,
    stream: LocalStream,
    pub(super) presence: bool,
    pub(super) watch: bool,
    /// False once the connection is over at the hub: the client left, sent `bye`, or a write
    /// failed. A connection that committed a violation stays open until its `bye` is written.
    pub(super) open: bool,
    pub(super) writer_done: bool,
    /// The reader and the writer that still run.
    threads: u8,
}

impl Hub {
    pub(super) fn new(lifetime: Lifetime, machines: Machines) -> Self {
        Self {
            lifetime,
            machines,
            peers: BTreeMap::new(),
            next_peer: 0,
            controls: 0,
            candidates: 0,
            reloads: 0,
            reload_started: 0,
            reload_applied: 0,
        }
    }

    /// Registers an admitted connection whose reader and writer are about to run.
    pub(super) fn register(&mut self, stream: LocalStream) -> u64 {
        self.next_peer += 1;
        self.peers.insert(
            self.next_peer,
            Peer {
                connection: Connection::<hub::Hub>::acceptor(),
                stream,
                presence: false,
                watch: false,
                open: true,
                writer_done: false,
                threads: 2,
            },
        );
        self.next_peer
    }

    /// Ends the connection `id` at the hub: it stops holding the hub and both directions close.
    pub(super) fn close(&mut self, id: u64) {
        let Some(peer) = self.peers.get_mut(&id) else {
            return;
        };
        let held = std::mem::take(&mut peer.presence);
        peer.open = false;
        let _closed = peer.stream.shutdown(Shutdown::Both);
        if held {
            self.lifetime.detach();
        }
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

    /// Queues `event` to every open connection attached with `watch`.
    pub(super) fn broadcast(&mut self, event: &Event) {
        for peer in self
            .peers
            .values_mut()
            .filter(|peer| peer.open && peer.watch)
        {
            let _queued = peer.connection.event(event.clone());
        }
    }

    /// Queues the lifetime state to every watcher.
    pub(super) fn announce(&mut self) {
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
            for event in self.machines.replace(read) {
                self.broadcast(&event);
            }
        }
    }

    /// Ends a catalog read that failed.
    pub(super) fn abandon_reload(&mut self) {
        self.reloads = self.reloads.saturating_sub(1);
    }
}
