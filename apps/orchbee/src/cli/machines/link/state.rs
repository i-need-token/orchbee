//! The link's shared state: the connection state machine, the table of live channels, and why the
//! link ended, under one lock that no thread holds across I/O. The carrier's threads wait on one
//! signal; each channel's workers wait on their channel's own.

use orchbee_local_ipc::LocalStream;
use orchbee_machine_protocol::{ByeReason, Connection, ResetReason, link};
use std::{
    collections::{BTreeMap, VecDeque},
    net::Shutdown,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

/// Why the link ended; the first cause is kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum End {
    /// Standard input ended or failed.
    InputClosed,
    /// The hub sent `bye`.
    Hub,
    /// 90 seconds passed without a received frame.
    Idle,
    /// Standard output failed.
    OutputFailed,
    /// The hub broke the contract.
    Violation,
}

/// Everything the link's threads share.
pub(super) struct State {
    pub(super) connection: Connection<link::Link>,
    pub(super) channels: Table,
    pub(super) end: Option<End>,
    /// When the reader last took a frame, for the idle rule.
    pub(super) last_received: Instant,
    /// The writer stopped: `bye` was written, nothing was left, or standard output failed.
    pub(super) writer_done: bool,
}

impl State {
    pub(super) fn new(connection: Connection<link::Link>) -> Self {
        Self {
            connection,
            channels: Table::default(),
            end: None,
            last_received: Instant::now(),
            writer_done: false,
        }
    }

    /// Whether the link still runs.
    pub(super) fn running(&self) -> bool {
        self.end.is_none()
    }

    /// Ends the link once: every channel ends without a result and `bye` is queued when the link
    /// can still carry it.
    pub(super) fn end(&mut self, end: End) {
        if self.end.is_some() {
            return;
        }
        self.end = Some(end);
        let farewell = match end {
            End::InputClosed | End::Hub => Some(ByeReason::Done),
            End::Idle => Some(ByeReason::Idle),
            End::OutputFailed | End::Violation => None,
        };
        self.channels
            .end_all(&mut self.connection, farewell.is_some());
        if let Some(reason) = farewell {
            let _queued = self.connection.bye(reason);
        }
    }
}

/// The state under its lock and the signal that wakes the writer, the reader's pause, and the
/// supervisor.
pub(super) struct Shared {
    state: Mutex<State>,
    carrier: Condvar,
}

impl Shared {
    pub(super) fn new(state: State) -> Self {
        Self {
            state: Mutex::new(state),
            carrier: Condvar::new(),
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wakes the writer, the reader, and the supervisor to look at the state again.
    pub(super) fn wake(&self) {
        self.carrier.notify_all();
    }

    pub(super) fn wait<'a>(&self, state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        self.carrier
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn wait_until<'a>(
        &self,
        state: MutexGuard<'a, State>,
        deadline: Instant,
    ) -> MutexGuard<'a, State> {
        let timeout = deadline.saturating_duration_since(Instant::now());
        self.carrier
            .wait_timeout(state, timeout)
            .unwrap_or_else(PoisonError::into_inner)
            .0
    }
}

/// What a channel's workers do.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Work {
    Session,
    Route,
    Start,
}

/// One channel the hub opened, from `open` until its last worker settles.
pub(super) struct Entry {
    pub(super) work: Work,
    /// Wakes this channel's workers: data, credit, a written frame, the opener's `end`, the end.
    pub(super) signal: Arc<Condvar>,
    /// The channel is over at the link: closed by this end, reset by either end, or the link ended.
    gone: bool,
    /// Set with `gone`: a running routed command ends.
    pub(super) cancel: Arc<AtomicBool>,
    /// The Session connection, to close it when the channel ends.
    pub(super) endpoint: Option<LocalStream>,
    /// `session`: the hub's bytes for the endpoint, bounded by the credit this end granted.
    pub(super) inbound: VecDeque<Vec<u8>>,
    /// `session`: the opener sent `end`.
    pub(super) opener_ended: bool,
    /// `route`: the request packet until the opener's `end`, bounded by the state machine.
    pub(super) request: Vec<u8>,
    /// Workers that still run for this channel.
    pub(super) workers: u8,
}

impl Entry {
    pub(super) fn new(work: Work) -> Self {
        Self {
            work,
            signal: Arc::new(Condvar::new()),
            gone: false,
            cancel: Arc::new(AtomicBool::new(false)),
            endpoint: None,
            inbound: VecDeque::new(),
            opener_ended: false,
            request: Vec::new(),
            workers: 0,
        }
    }

    /// Marks the channel over and releases what blocks its workers.
    fn finish(&mut self) {
        self.gone = true;
        self.cancel.store(true, Ordering::Release);
        self.inbound.clear();
        if let Some(endpoint) = self.endpoint.take() {
            let _closed = endpoint.shutdown(Shutdown::Both);
        }
        self.signal.notify_all();
    }
}

/// The live channels of the link.
#[derive(Default)]
pub(super) struct Table {
    entries: BTreeMap<u32, Entry>,
}

impl Table {
    /// The channels kept, counting closed ones whose workers still settle.
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Records a channel the hub opened.
    pub(super) fn insert(&mut self, channel: u32, entry: Entry) {
        self.entries.insert(channel, entry);
    }

    /// Wakes `channel`'s workers.
    pub(super) fn signal(&self, channel: u32) {
        if let Some(entry) = self.entries.get(&channel) {
            entry.signal.notify_all();
        }
    }

    /// Routed commands still running.
    pub(super) fn commands(&self) -> usize {
        self.running(Work::Route)
    }

    /// Session starts still running.
    pub(super) fn starts(&self) -> usize {
        self.running(Work::Start)
    }

    fn running(&self, work: Work) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.work == work && entry.workers > 0)
            .count()
    }

    /// Ends every channel because the link ended; `reset` resets those still open with
    /// `stopping`, so `bye` does not wait behind their data.
    fn end_all(&mut self, connection: &mut Connection<link::Link>, reset: bool) {
        for (channel, entry) in &mut self.entries {
            if reset && !entry.gone {
                let _reset = connection.reset(*channel, ResetReason::Stopping);
            }
            entry.finish();
        }
        self.entries.retain(|_, entry| entry.workers > 0);
    }

    /// `channel`'s entry while the channel is not over.
    pub(super) fn live(&mut self, channel: u32) -> Option<&mut Entry> {
        self.entries.get_mut(&channel).filter(|entry| !entry.gone)
    }

    /// Ends `channel` at this end, and forgets it once no worker runs.
    pub(super) fn finish(&mut self, channel: u32) {
        if let Some(entry) = self.entries.get_mut(&channel) {
            entry.finish();
            if entry.workers == 0 {
                self.entries.remove(&channel);
            }
        }
    }

    /// One worker of `channel` left; the last one ends the channel and forgets it.
    pub(super) fn release(&mut self, channel: u32) {
        if let Some(entry) = self.entries.get_mut(&channel) {
            entry.workers = entry.workers.saturating_sub(1);
            if entry.workers == 0 {
                entry.finish();
                self.entries.remove(&channel);
            }
        }
    }

    /// Forgets a channel that never got a worker.
    pub(super) fn remove(&mut self, channel: u32) {
        self.entries.remove(&channel);
    }
}
