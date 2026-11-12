//! The hub's state under its lock: the lifetime, the catalog, the registry of attached connections,
//! and the counts admission and closing wait on. Each connection's protocol state machine lives
//! under that connection's own lock. Lock order: the hub state's lock before a connection's; a
//! thread holding a connection's lock takes no other, and neither is held across I/O.

use super::{catalog::Machines, lifetime::Lifetime};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::DataRoot;
use orchbee_machine_protocol::{
    Connection,
    hub::{self, Event},
};
use std::{collections::BTreeMap, net::Shutdown, sync::Arc};

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
    pub(super) fn new(root: DataRoot, lifetime: Lifetime, machines: Machines) -> Self {
        Self {
            root,
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
        peer.line.lock().open = false;
        peer.line.wake();
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
