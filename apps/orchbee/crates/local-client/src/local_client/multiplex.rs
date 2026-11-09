//! The sole TUI connection owner. Lanes submit complete records and never own a Server socket.
use super::LocalClientOrigin;
use orchbee_connection_handshake::CapabilitySet;
use orchbee_local_client_protocol::ClientConnection;
use orchbee_local_ipc::{AsLocalIo, LocalIo, LocalStream, PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    MAXIMUM_MULTIPLEX_BYTES, MAXIMUM_MULTIPLEX_LANES, MAXIMUM_MULTIPLEX_STREAMS, MultiplexDecoder,
    MultiplexLaneId, MultiplexMessage, MultiplexStreamId, MultiplexViewerId, StreamCloseReason,
    StreamFrameKind, StreamMode, encode_multiplex_message,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{self, Read, Write},
    num::NonZeroU64,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const QUEUED: u8 = 0;
const HANDED_OFF: u8 = 1;
const CANCELLED: u8 = 2;
const EVENT_RECORDS: usize = 32;
mod health;

/// One aggregate retained-byte budget covers admitted requests, their encoded output and inboxes.
#[derive(Debug)]
struct Reservation {
    budget: Arc<AtomicUsize>,
    streams: Arc<AtomicUsize>,
    bytes: usize,
}
impl Reservation {
    fn acquire(
        budget: &Arc<AtomicUsize>,
        streams: &Arc<AtomicUsize>,
        bytes: usize,
    ) -> io::Result<Self> {
        streams
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAXIMUM_MULTIPLEX_STREAMS).then_some(count + 1)
            })
            .map_err(|_| io::ErrorKind::WouldBlock)?;
        if budget
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(bytes)
                    .filter(|next| *next <= MAXIMUM_MULTIPLEX_BYTES)
            })
            .is_err()
        {
            streams.fetch_sub(1, Ordering::AcqRel);
            return Err(io::ErrorKind::WouldBlock.into());
        }
        Ok(Self {
            budget: Arc::clone(budget),
            streams: Arc::clone(streams),
            bytes,
        })
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.budget.fetch_sub(self.bytes, Ordering::AcqRel);
        self.streams.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
pub enum StreamEvent {
    Record(Vec<u8>),
    Gap,
    Disconnected,
}
#[derive(Debug, Default)]
struct DeliveryState {
    events: VecDeque<StreamEvent>,
    bytes: usize,
    closed: bool,
    failure: Option<orchbee_workbench_protocol::MultiplexDisconnectReason>,
    /// A drained Server refused this stream's request before it took effect.
    refused: bool,
}
/// Runs on the connection actor or the thread cancelling the stream, so it must not block.
struct ReadyListener(Box<dyn Fn() + Send + Sync>);

impl std::fmt::Debug for ReadyListener {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ReadyListener")
    }
}

#[derive(Debug)]
struct Delivery {
    state: Mutex<DeliveryState>,
    ready: LocalStream,
    notify: LocalStream,
    listener: OnceLock<ReadyListener>,
    handoff: AtomicU8,
    generation: AtomicU64,
    cancelled: AtomicBool,
    budget: Arc<AtomicUsize>,
}
impl Delivery {
    fn new(budget: Arc<AtomicUsize>) -> io::Result<Self> {
        let (ready, notify) = LocalStream::pair()?;
        ready.set_nonblocking(true)?;
        notify.set_nonblocking(true)?;
        Ok(Self {
            state: Mutex::default(),
            ready,
            notify,
            listener: OnceLock::new(),
            handoff: AtomicU8::new(QUEUED),
            generation: AtomicU64::new(0),
            cancelled: AtomicBool::new(false),
            budget,
        })
    }
    fn signal(&self) {
        let _notified = (&self.notify).write(&[1]);
        if let Some(listener) = self.listener.get() {
            (listener.0)();
        }
    }
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    fn cancel(&self) -> bool {
        self.cancelled.store(true, Ordering::Release);
        let submitted = self
            .handoff
            .compare_exchange(QUEUED, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_err_and(|previous| previous == HANDED_OFF);
        if let Ok(mut state) = self.state.lock() {
            self.budget.fetch_sub(state.bytes, Ordering::AcqRel);
            state.bytes = 0;
            state.events.clear();
            state.closed = true;
        }
        self.signal();
        submitted
    }
    fn publish(&self, event: StreamEvent, mode: StreamMode) -> io::Result<bool> {
        let length = match &event {
            StreamEvent::Record(bytes) => bytes.len(),
            _ => 0,
        };
        let mut state = self.state.lock().map_err(|_| io::ErrorKind::BrokenPipe)?;
        if state.closed || self.cancelled() {
            return Ok(false);
        }
        let overflow = mode == StreamMode::Events && state.events.len() >= EVENT_RECORDS;
        if matches!(mode, StreamMode::State | StreamMode::Invalidations) || overflow {
            let barrier = matches!(&event, StreamEvent::Record(_))
                .then(|| {
                    state.events.iter().rev().find_map(|event| match event {
                        StreamEvent::Gap => Some(StreamEvent::Gap),
                        StreamEvent::Disconnected => Some(StreamEvent::Disconnected),
                        StreamEvent::Record(_) => None,
                    })
                })
                .flatten();
            self.budget.fetch_sub(state.bytes, Ordering::AcqRel);
            state.bytes = 0;
            state.events.clear();
            if overflow {
                state.events.push_back(StreamEvent::Gap);
                drop(state);
                self.signal();
                return Ok(true);
            }
            if let Some(barrier) = barrier {
                state.events.push_back(barrier);
            }
        }
        if self
            .budget
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(length)
                    .filter(|next| *next <= MAXIMUM_MULTIPLEX_BYTES)
            })
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                Disconnect(orchbee_workbench_protocol::MultiplexDisconnectReason::Capacity),
            ));
        }
        state.bytes += length;
        state.events.push_back(event);
        drop(state);
        self.signal();
        Ok(overflow)
    }
    fn take(&self) -> io::Result<Option<StreamEvent>> {
        // Drain before checking state: a publication between checking and draining would lose
        // its only readiness edge and strand a queued result.
        let mut buffer = [0; 128];
        while matches!((&self.ready).read(&mut buffer), Ok(count) if count > 0) {}
        let mut state = self.state.lock().map_err(|_| io::ErrorKind::BrokenPipe)?;
        let result = state.events.pop_front();
        if let Some(StreamEvent::Record(bytes)) = &result {
            state.bytes -= bytes.len();
            self.budget.fetch_sub(bytes.len(), Ordering::AcqRel);
        }
        if result.is_none() && state.closed {
            if let Some(reason) = state.failure {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    Disconnect(reason),
                ));
            }
            if state.refused {
                return Err(io::Error::new(io::ErrorKind::ConnectionRefused, Refused));
            }
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        drop(state);
        Ok(result)
    }
    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
        }
        self.signal();
    }
    fn refuse(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.refused = true;
            state.closed = true;
        }
        self.signal();
    }
    fn fail(&self, reason: orchbee_workbench_protocol::MultiplexDisconnectReason) {
        if let Ok(mut state) = self.state.lock() {
            state.failure = Some(reason);
            state.closed = true;
        }
        self.signal();
    }
    fn wait(
        &self,
        deadline: Option<Instant>,
        clock: &dyn super::MonotonicClock,
        wake: Option<LocalIo<'_>>,
        cancel: Option<LocalIo<'_>>,
    ) -> io::Result<StreamEvent> {
        loop {
            if let Some(event) = self.take()? {
                return Ok(event);
            }
            let timeout = match deadline {
                Some(deadline) => super::poll_timeout(
                    deadline
                        .checked_duration_since(clock.now())
                        .filter(|remaining| !remaining.is_zero())
                        .ok_or(io::ErrorKind::TimedOut)?,
                ),
                None => PollTimeout::NONE,
            };
            let mut fds = vec![PollFd::new(self.ready.as_local_io(), PollFlags::POLLIN)];
            if let Some(wake) = wake {
                fds.push(PollFd::new(wake, PollFlags::POLLIN));
            }
            let cancel_index = fds.len();
            if let Some(cancel) = cancel {
                fds.push(PollFd::new(cancel, PollFlags::POLLIN));
            }
            poll(&mut fds, timeout)?;
            if cancel.is_some()
                && fds[cancel_index]
                    .revents()
                    .is_some_and(|flags| !flags.is_empty())
            {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            if wake.is_some()
                && let Some(wake) = fds.get(1)
            {
                let flags = wake.revents().unwrap_or(PollFlags::empty());
                if flags.contains(PollFlags::POLLIN) {
                    return Err(io::Error::other(super::ExchangeWakeReady));
                }
                if flags.intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL) {
                    return Err(io::Error::other(super::ExchangeWakeFault(flags)));
                }
            }
        }
    }
}
impl Drop for Delivery {
    fn drop(&mut self) {
        if let Ok(state) = self.state.get_mut() {
            self.budget.fetch_sub(state.bytes, Ordering::AcqRel);
        }
    }
}

#[derive(Debug)]
enum Command {
    Probe {
        id: MultiplexStreamId,
        request: Vec<u8>,
    },
    Open {
        id: MultiplexStreamId,
        lane: MultiplexLaneId,
        mode: StreamMode,
        request: Vec<u8>,
        delivery: Arc<Delivery>,
        reservation: Arc<Reservation>,
    },
    Cancel(MultiplexStreamId),
    LaneOpen(MultiplexLaneId),
    LaneClose(MultiplexLaneId),
}
#[derive(Debug, Default)]
struct Commands {
    next: u64,
    next_lane: u64,
    lanes: usize,
    owners: usize,
    queue: VecDeque<Command>,
}
type ViewerReadyListener = Arc<dyn Fn(MultiplexViewerId) + Send + Sync>;

/// Runs on the connection actor, so a listener must not block.
#[derive(Default)]
struct ViewerReadyListeners {
    next: u64,
    listeners: Vec<(u64, ViewerReadyListener)>,
}

impl std::fmt::Debug for ViewerReadyListeners {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ViewerReadyListeners")
            .field("count", &self.listeners.len())
            .finish()
    }
}

/// Registered interest in this connection's viewer hints; dropping it withdraws the listener.
#[derive(Debug)]
pub struct ViewerReadyWatch {
    owner: Arc<Connection>,
    token: u64,
}

impl Drop for ViewerReadyWatch {
    fn drop(&mut self) {
        self.owner
            .shared
            .viewer_ready
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .listeners
            .retain(|(token, _)| *token != self.token);
    }
}

#[derive(Debug)]
struct Shared {
    commands: Mutex<Commands>,
    viewer_ready: Mutex<ViewerReadyListeners>,
    wake: LocalStream,
    stopped: AtomicBool,
    generation: AtomicU64,
    /// The capabilities the current carrier's handshake answer listed; a reconnect replaces them.
    capabilities: Mutex<CapabilitySet>,
    budget: Arc<AtomicUsize>,
    streams: Arc<AtomicUsize>,
    current_until: Mutex<Option<Instant>>,
}
impl Shared {
    fn signal(&self) {
        let _notified = (&self.wake).write(&[1]);
    }
    fn cancel(&self, id: MultiplexStreamId) {
        if let Ok(mut commands) = self.commands.lock() {
            // Cancellation capacity is separate and bounded by the active stream bound.
            if commands
                .queue
                .iter()
                .any(|command| matches!(command, Command::Cancel(queued) if *queued == id))
            {
                return;
            }
            if commands.queue.len() < MAXIMUM_MULTIPLEX_STREAMS * 2 {
                commands.queue.push_back(Command::Cancel(id));
            } else {
                self.stopped.store(true, Ordering::Release);
            }
        }
        self.signal();
    }
}

#[derive(Debug)]
pub(crate) struct Connection {
    shared: Arc<Shared>,
    task: Mutex<Option<JoinHandle<()>>>,
}
#[derive(Debug)]
pub(super) struct Lane {
    owner: Arc<Connection>,
    id: MultiplexLaneId,
    closed: AtomicBool,
}
impl Lane {
    pub(super) fn id(&self) -> MultiplexLaneId {
        self.id
    }
    pub(super) fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut commands) = self.owner.shared.commands.lock() {
            commands.lanes -= 1;
            commands.queue.push_back(Command::LaneClose(self.id));
        }
        self.owner.shared.signal();
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        self.close();
    }
}
/// One Client surface's claim on a shared carrier. A surface derived from another surface's
/// origin reuses that carrier, so retiring the older surface must not stop it: only the last
/// released claim stops the carrier. Releasing is idempotent.
#[derive(Debug)]
pub(super) struct Owner {
    connection: Arc<Connection>,
    released: AtomicBool,
}
impl Owner {
    pub(super) fn release(&self) -> io::Result<()> {
        if self.released.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let last = {
            let mut commands = self
                .connection
                .shared
                .commands
                .lock()
                .map_err(|_| io::ErrorKind::BrokenPipe)?;
            commands.owners = commands.owners.saturating_sub(1);
            // Decided under the command lock, so no claim can start on a stopping carrier.
            let last = commands.owners == 0;
            if last {
                self.connection
                    .shared
                    .stopped
                    .store(true, Ordering::Release);
            }
            last
        };
        if last {
            self.connection.shutdown()
        } else {
            Ok(())
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _released = self.release();
    }
}
impl Connection {
    /// Claims this carrier for one more surface; a stopped carrier admits no new claim.
    pub(super) fn own(self: &Arc<Self>) -> io::Result<Owner> {
        let mut commands = self
            .shared
            .commands
            .lock()
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
        if self.shared.stopped.load(Ordering::Acquire) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        commands.owners += 1;
        drop(commands);
        Ok(Owner {
            connection: Arc::clone(self),
            released: AtomicBool::new(false),
        })
    }
    pub(super) fn lane(self: &Arc<Self>) -> io::Result<Arc<Lane>> {
        let mut commands = self
            .shared
            .commands
            .lock()
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
        if self.shared.stopped.load(Ordering::Acquire) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if commands.lanes >= MAXIMUM_MULTIPLEX_LANES
            || commands.queue.len() >= MAXIMUM_MULTIPLEX_STREAMS * 2
        {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let id = commands
            .next_lane
            .max(1)
            .checked_add(1)
            .and_then(MultiplexLaneId::new)
            .ok_or(io::ErrorKind::OutOfMemory)?;
        commands.next_lane = id.value();
        commands.lanes += 1;
        commands.queue.push_back(Command::LaneOpen(id));
        drop(commands);
        self.shared.signal();
        Ok(Arc::new(Lane {
            owner: Arc::clone(self),
            id,
            closed: AtomicBool::new(false),
        }))
    }
    pub(super) fn start(
        connection: ClientConnection,
        origin: LocalClientOrigin,
    ) -> io::Result<Arc<Self>> {
        Self::start_with_timing(connection, origin, health::Timing::default())
    }
    fn start_with_timing(
        connection: ClientConnection,
        mut origin: LocalClientOrigin,
        timing: health::Timing,
    ) -> io::Result<Arc<Self>> {
        let (reader, wake) = LocalStream::pair()?;
        reader.set_nonblocking(true)?;
        wake.set_nonblocking(true)?;
        origin.multiplex = None;
        let shared = Arc::new(Shared {
            commands: Mutex::default(),
            viewer_ready: Mutex::default(),
            wake,
            stopped: AtomicBool::new(false),
            generation: AtomicU64::new(connection.generation().get()),
            capabilities: Mutex::new(connection.capabilities().clone()),
            // The control reservation is part of, not additional to, the public connection cap.
            budget: Arc::new(AtomicUsize::new(health::RESERVED_BYTES)),
            streams: Arc::new(AtomicUsize::new(health::RESERVED_STREAMS)),
            current_until: Mutex::new(Some(Instant::now() + timing.idle + timing.response)),
        });
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-client-connection".into())
            .spawn(move || actor(connection, origin, reader, state, timing))?;
        Ok(Arc::new(Self {
            shared,
            task: Mutex::new(Some(task)),
        }))
    }
    /// Calls `listener` on the connection actor for each viewer hint until the watch drops.
    pub(super) fn watch_viewer_ready(
        self: &Arc<Self>,
        listener: impl Fn(MultiplexViewerId) + Send + Sync + 'static,
    ) -> ViewerReadyWatch {
        let mut listeners = self
            .shared
            .viewer_ready
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let token = listeners.next;
        listeners.next = token.wrapping_add(1);
        listeners.listeners.push((token, Arc::new(listener)));
        ViewerReadyWatch {
            owner: Arc::clone(self),
            token,
        }
    }
    pub(super) fn generation(&self) -> NonZeroU64 {
        NonZeroU64::new(self.shared.generation.load(Ordering::Acquire))
            .expect("admitted connection generation is nonzero")
    }
    /// Whether the current carrier's handshake answer listed `capability`.
    pub(super) fn serves(&self, capability: &str) -> bool {
        self.shared
            .capabilities
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(capability)
    }
    pub(super) fn is_current(&self, now: Instant) -> bool {
        !self.shared.stopped.load(Ordering::Acquire)
            && self
                .shared
                .current_until
                .lock()
                .ok()
                .is_some_and(|until| until.is_some_and(|until| now < until))
    }
    pub(super) fn shutdown(&self) -> io::Result<()> {
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.signal();
        let task = self
            .task
            .lock()
            .map_err(|_| io::ErrorKind::BrokenPipe)?
            .take();
        if let Some(task) = task {
            task.join()
                .map_err(|_| io::Error::other("Client connection actor panicked"))?;
        }
        Ok(())
    }
    fn open(
        self: &Arc<Self>,
        lane: MultiplexLaneId,
        frame: Vec<u8>,
        mode: StreamMode,
    ) -> io::Result<Subscription> {
        if self.shared.stopped.load(Ordering::Acquire) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let delivery = Arc::new(Delivery::new(Arc::clone(&self.shared.budget))?);
        let mut commands = self
            .shared
            .commands
            .lock()
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
        if commands.queue.len() >= MAXIMUM_MULTIPLEX_STREAMS {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let reservation = Arc::new(Reservation::acquire(
            &self.shared.budget,
            &self.shared.streams,
            frame.len().saturating_mul(2).saturating_add(64),
        )?);
        let next = commands
            .next
            .checked_add(1)
            .ok_or(io::ErrorKind::OutOfMemory)?;
        let id = MultiplexStreamId::new(next).ok_or(io::ErrorKind::InvalidData)?;
        commands.next = next;
        commands.queue.push_back(Command::Open {
            id,
            lane,
            mode,
            request: frame,
            delivery: Arc::clone(&delivery),
            reservation,
        });
        drop(commands);
        self.shared.signal();
        Ok(Subscription {
            owner: Arc::clone(self),
            id,
            delivery,
        })
    }
    pub(crate) fn subscribe(
        self: &Arc<Self>,
        frame: Vec<u8>,
        mode: StreamMode,
    ) -> io::Result<Subscription> {
        self.open(MultiplexLaneId::ROOT, frame, mode)
    }
    pub(super) fn exchange(
        self: &Arc<Self>,
        lane: MultiplexLaneId,
        frame: &[u8],
        deadline: Instant,
        clock: &dyn super::MonotonicClock,
        wake: Option<LocalIo<'_>>,
        cancel: Option<LocalIo<'_>>,
    ) -> Result<(Vec<u8>, NonZeroU64), (bool, io::Error)> {
        if clock.now() >= deadline {
            return Err((false, io::ErrorKind::TimedOut.into()));
        }
        let subscription = self
            .open(lane, frame.to_vec(), StreamMode::Request)
            .map_err(|error| (false, error))?;
        match subscription
            .delivery
            .wait(Some(deadline), clock, wake, cancel)
        {
            Ok(StreamEvent::Record(bytes)) => {
                NonZeroU64::new(subscription.delivery.generation.load(Ordering::Acquire))
                    .map(|generation| (bytes, generation))
                    .ok_or_else(|| (true, io::ErrorKind::InvalidData.into()))
            }
            Ok(StreamEvent::Gap | StreamEvent::Disconnected) => Err((
                subscription.delivery.cancel(),
                io::ErrorKind::ConnectionAborted.into(),
            )),
            // The Server refused the request before it took effect, so it was never submitted.
            Err(error) if refused(&error) => {
                subscription.delivery.cancel();
                Err((false, error))
            }
            Err(error) => Err((subscription.delivery.cancel(), error)),
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        let _closed = self.shutdown();
    }
}

#[derive(Debug)]
pub struct Subscription {
    owner: Arc<Connection>,
    id: MultiplexStreamId,
    delivery: Arc<Delivery>,
}
impl Subscription {
    /// Wait on the shared actor's mailbox and the caller's cancellation source, without polling.
    pub fn wait(&self, wake: Option<LocalIo<'_>>) -> io::Result<StreamEvent> {
        self.delivery
            .wait(None, &super::SystemMonotonicClock, wake, None)
    }
    pub fn cancel(&self) -> bool {
        let submitted = self.delivery.cancel();
        self.owner.shared.cancel(self.id);
        submitted
    }
    pub fn poll(&self) -> io::Result<Option<StreamEvent>> {
        self.delivery.take()
    }
    pub fn readiness(&self) -> LocalIo<'_> {
        self.delivery.ready.as_local_io()
    }
    /// For a reader that cannot wait on [`Self::readiness`]: calls `listener` after each event
    /// arrives and when the stream ends, on the connection actor or the thread cancelling the
    /// stream, so it must not block. The first listener stays.
    pub fn on_ready(&self, listener: impl Fn() + Send + Sync + 'static) {
        let _first = self
            .delivery
            .listener
            .set(ReadyListener(Box::new(listener)));
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct Active {
    lane: MultiplexLaneId,
    mode: StreamMode,
    request: Vec<u8>,
    delivery: Arc<Delivery>,
    sequence: u64,
    needs_snapshot: bool,
    _reservation: Arc<Reservation>,
}
struct Outgoing {
    id: Option<MultiplexStreamId>,
    bytes: Vec<u8>,
    offset: usize,
    deadline: Instant,
    _reservation: Option<Arc<Reservation>>,
}
// Buffered frame bytes and their partial-frame deadline belong to the same carrier.
#[derive(Default)]
struct Incoming {
    decoder: MultiplexDecoder,
    partial_at: Option<Instant>,
}

fn actor(
    connection: ClientConnection,
    origin: LocalClientOrigin,
    wake: LocalStream,
    shared: Arc<Shared>,
    timing: health::Timing,
) {
    let mut connection = Some(connection);
    let mut active = BTreeMap::<MultiplexStreamId, Active>::new();
    let mut lanes = BTreeSet::from([MultiplexLaneId::ROOT]);
    let mut outgoing = VecDeque::new();
    let mut incoming = Incoming::default();
    let mut reconnect_at = Instant::now();
    let mut health = health::Health::new(timing, Instant::now());
    loop {
        if shared.stopped.load(Ordering::Acquire) {
            break;
        }
        let commands = match shared.commands.lock() {
            Ok(mut commands) => std::mem::take(&mut commands.queue),
            Err(_) => break,
        };
        for command in commands {
            match command {
                Command::Probe { id, request } => {
                    if connection.is_some()
                        && let Some(deadline) = health.queued(id)
                    {
                        let Ok(bytes) = encode_multiplex_message(&MultiplexMessage::StreamOpen {
                            stream_id: id,
                            lane_id: MultiplexLaneId::ROOT,
                            mode: StreamMode::Request,
                            request,
                        }) else {
                            break;
                        };
                        outgoing.push_back(Outgoing {
                            id: Some(id),
                            bytes,
                            offset: 0,
                            deadline,
                            _reservation: None,
                        });
                    }
                }
                Command::Open {
                    id,
                    lane,
                    mode,
                    request,
                    delivery,
                    reservation,
                } => {
                    if delivery.cancelled() || !lanes.contains(&lane) {
                        delivery.close();
                        continue;
                    }
                    if active.len() >= MAXIMUM_MULTIPLEX_STREAMS {
                        delivery.close();
                        continue;
                    }
                    if connection.is_some() {
                        if let Ok(bytes) = encode_multiplex_message(&MultiplexMessage::StreamOpen {
                            stream_id: id,
                            lane_id: lane,
                            mode,
                            request: request.clone(),
                        }) {
                            outgoing.push_back(Outgoing {
                                id: Some(id),
                                bytes,
                                offset: 0,
                                deadline: Instant::now() + Duration::from_secs(2),
                                _reservation: Some(Arc::clone(&reservation)),
                            });
                        } else {
                            delivery.close();
                            continue;
                        }
                    }
                    active.insert(
                        id,
                        Active {
                            lane,
                            mode,
                            request,
                            delivery,
                            sequence: 0,
                            needs_snapshot: mode != StreamMode::Request,
                            _reservation: reservation,
                        },
                    );
                }
                Command::Cancel(id) => {
                    // A queued request cancelled before the handoff CAS must never reach the
                    // Server. A partial frame is completed before the cancellation record.
                    outgoing.retain(|item| item.id != Some(id) || item.offset != 0);
                    if let Some(removed) = active.remove(&id)
                        && connection.is_some()
                        && let Ok(bytes) =
                            encode_multiplex_message(&MultiplexMessage::StreamClose {
                                stream_id: id,
                                reason: StreamCloseReason::Cancelled,
                            })
                    {
                        outgoing.push_back(Outgoing {
                            id: Some(id),
                            bytes,
                            offset: 0,
                            deadline: Instant::now() + Duration::from_secs(2),
                            _reservation: Some(removed._reservation),
                        });
                    }
                }
                Command::LaneOpen(id) => {
                    lanes.insert(id);
                    if connection.is_some() {
                        queue_lane(&mut outgoing, id, true);
                    }
                }
                Command::LaneClose(id) => {
                    lanes.remove(&id);
                    let removed = active
                        .iter()
                        .filter_map(|(stream, item)| (item.lane == id).then_some(*stream))
                        .collect::<Vec<_>>();
                    for stream in removed {
                        outgoing.retain(|item| item.id != Some(stream) || item.offset != 0);
                        if let Some(item) = active.remove(&stream) {
                            item.delivery.close();
                        }
                    }
                    if connection.is_some() {
                        queue_lane(&mut outgoing, id, false);
                    }
                }
            }
        }
        if connection.is_none() && !active.is_empty() && Instant::now() >= reconnect_at {
            match ClientConnection::connect_before_cancellable(
                &origin.endpoint,
                &origin.offer,
                Instant::now() + Duration::from_secs(2),
                Some(wake.as_local_io()),
            ) {
                Ok(next) => {
                    health.reset(Instant::now());
                    shared
                        .generation
                        .store(next.generation().get(), Ordering::Release);
                    next.capabilities().clone_into(
                        &mut shared
                            .capabilities
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner()),
                    );
                    connection = Some(next);
                    for lane in lanes.iter().filter(|lane| **lane != MultiplexLaneId::ROOT) {
                        queue_lane(&mut outgoing, *lane, true);
                    }
                    for (id, item) in &mut active {
                        item.sequence = 0;
                        item.needs_snapshot = item.mode != StreamMode::Request;
                        if let Ok(bytes) = encode_multiplex_message(&MultiplexMessage::StreamOpen {
                            stream_id: *id,
                            lane_id: item.lane,
                            mode: item.mode,
                            request: item.request.clone(),
                        }) {
                            outgoing.push_back(Outgoing {
                                id: Some(*id),
                                bytes,
                                offset: 0,
                                deadline: Instant::now() + Duration::from_secs(2),
                                _reservation: Some(Arc::clone(&item._reservation)),
                            });
                        }
                    }
                }
                Err(error) if retryable_admission(&error) => {
                    reconnect_at = Instant::now() + Duration::from_millis(250);
                }
                Err(_) => {
                    shared.stopped.store(true, Ordering::Release);
                    for item in active.values() {
                        item.delivery.fail(
                            orchbee_workbench_protocol::MultiplexDisconnectReason::Unavailable,
                        );
                    }
                    break;
                }
            }
        }
        if outgoing.len() > MAXIMUM_MULTIPLEX_STREAMS * 4 {
            shared.stopped.store(true, Ordering::Release);
            for item in active.values() {
                item.delivery
                    .fail(orchbee_workbench_protocol::MultiplexDisconnectReason::Capacity);
            }
            break;
        }
        let result = match connection.as_mut() {
            Some(connection) => {
                let probing = !active.is_empty();
                let queued = if probing {
                    health.enqueue(&shared, Instant::now())
                } else {
                    Ok(())
                };
                *shared
                    .current_until
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(health.valid_until());
                queued.and_then(|()| {
                    drive(
                        connection,
                        &shared,
                        &wake,
                        &mut active,
                        &mut outgoing,
                        &mut incoming,
                        &mut health,
                        probing,
                    )
                })
            }
            None => {
                let timeout = if active.is_empty() {
                    PollTimeout::NONE
                } else {
                    super::poll_timeout(reconnect_at.saturating_duration_since(Instant::now()))
                };
                poll(
                    &mut [PollFd::new(wake.as_local_io(), PollFlags::POLLIN)],
                    timeout,
                )
                .map(|_| ())
            }
        };
        let mut bytes = [0; 256];
        while matches!((&wake).read(&mut bytes), Ok(count) if count > 0) {}
        if let Err(error) = result {
            let failure = error
                .get_ref()
                .and_then(|error| error.downcast_ref::<Disconnect>())
                .map(|failure| failure.0)
                .or_else(|| {
                    (error.kind() == io::ErrorKind::InvalidData).then_some(
                        orchbee_workbench_protocol::MultiplexDisconnectReason::InvalidStream,
                    )
                });
            if let Some(reason) = failure {
                // A typed admission/budget violation is terminal. Reconnecting would evade the
                // peer's sustained-overload policy; ordinary transport loss still rebuilds reads.
                shared.stopped.store(true, Ordering::Release);
                for item in active.values() {
                    item.delivery.fail(reason);
                }
                break;
            }
            connection.take();
            health.reset(Instant::now());
            *shared
                .current_until
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
            outgoing.clear();
            incoming = Incoming::default();
            active.retain(|_, item| {
                if item.mode == StreamMode::Request {
                    item.delivery.close();
                    false
                } else {
                    let _reported = item
                        .delivery
                        .publish(StreamEvent::Disconnected, StreamMode::State);
                    true
                }
            });
            reconnect_at = Instant::now() + Duration::from_millis(250);
        }
    }
    *shared
        .current_until
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    for item in active.into_values() {
        item.delivery.close();
    }
    if let Ok(mut commands) = shared.commands.lock() {
        for command in commands.queue.drain(..) {
            if let Command::Open { delivery, .. } = command {
                delivery.close();
            }
        }
    }
}

fn retryable_admission(error: &orchbee_local_client_protocol::ConnectClientError) -> bool {
    use orchbee_local_client_protocol::ConnectClientError;
    match error {
        ConnectClientError::CancellationReady => true,
        ConnectClientError::ConnectFailed(error) | ConnectClientError::Io(error) => matches!(
            error.kind(),
            io::ErrorKind::NotFound
                | io::ErrorKind::ConnectionRefused
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::UnexpectedEof
                | io::ErrorKind::TimedOut
                | io::ErrorKind::Interrupted
                | io::ErrorKind::WouldBlock
        ),
        _ => false,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the actor loop lends each piece of its state to one bounded step"
)]
fn drive(
    connection: &mut ClientConnection,
    shared: &Shared,
    wake: &LocalStream,
    active: &mut BTreeMap<MultiplexStreamId, Active>,
    outgoing: &mut VecDeque<Outgoing>,
    incoming: &mut Incoming,
    health: &mut health::Health,
    probing: bool,
) -> io::Result<()> {
    let Incoming {
        decoder,
        partial_at,
    } = incoming;
    connection.set_nonblocking(true)?;
    let interest = if outgoing.is_empty() {
        PollFlags::POLLIN
    } else {
        PollFlags::POLLIN | PollFlags::POLLOUT
    };
    let deadline = outgoing
        .front()
        .map(|item| item.deadline)
        .into_iter()
        .chain(*partial_at)
        .min();
    let wait_until = deadline
        .into_iter()
        .chain((probing || health.pending()).then(|| health.deadline()))
        .min();
    let timeout = wait_until.map_or(PollTimeout::NONE, |deadline| {
        super::poll_timeout(deadline.saturating_duration_since(Instant::now()))
    });
    let mut fds = [
        PollFd::new(connection.as_local_io(), interest),
        PollFd::new(wake.as_local_io(), PollFlags::POLLIN),
    ];
    poll(&mut fds, timeout)?;
    let flags = fds[0].revents().unwrap_or(PollFlags::empty());
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) || health.expired(Instant::now())
    {
        return Err(io::ErrorKind::TimedOut.into());
    }
    if flags.contains(PollFlags::POLLIN) {
        let mut buffer = [0; 16384];
        match connection.read(&mut buffer) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => {
                decoder.push(&buffer[..count])?;
                while let Some(message) = decoder.next_message()? {
                    if !health.receive(&message, connection.generation(), Instant::now())?
                        && deliver(message, shared, active, outgoing)?
                    {
                        health.business_progress(Instant::now());
                    }
                }
                if decoder.is_empty() {
                    *partial_at = None;
                } else if partial_at.is_none() {
                    *partial_at = Some(Instant::now() + Duration::from_secs(2));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
    }
    if flags.contains(PollFlags::POLLOUT)
        && let Some(item) = outgoing.front_mut()
    {
        if item.offset == 0
            && let Some(id) = item.id
        {
            health.handoff(id, connection.generation());
        }
        if item.offset == 0
            && let Some(active) = item.id.and_then(|id| active.get(&id))
        {
            match active.delivery.handoff.compare_exchange(
                QUEUED,
                HANDED_OFF,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Err(CANCELLED) => {
                    outgoing.pop_front();
                    return Ok(());
                }
                Ok(_) | Err(_) => {
                    // Bind correlation to the carrier that actually starts this request. It
                    // may differ from the generation visible while the caller queued offline.
                    active
                        .delivery
                        .generation
                        .store(connection.generation().get(), Ordering::Release);
                }
            }
        }
        match connection.write(&item.bytes[item.offset..]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => {
                item.offset += count;
                if item.offset == item.bytes.len() {
                    outgoing.pop_front();
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
    }
    if flags.intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL) {
        return Err(io::ErrorKind::ConnectionAborted.into());
    }
    Ok(())
}

fn deliver(
    message: MultiplexMessage,
    shared: &Shared,
    active: &mut BTreeMap<MultiplexStreamId, Active>,
    outgoing: &mut VecDeque<Outgoing>,
) -> io::Result<bool> {
    match message {
        MultiplexMessage::StreamFrame {
            stream_id,
            sequence,
            kind,
            record,
        } => {
            let Some(item) = active.get_mut(&stream_id) else {
                return Ok(false);
            }; // cancelled stream's in-flight frame
            if item.needs_snapshot && kind == StreamFrameKind::Update {
                // Already-buffered updates preceding the resnapshot request cannot fill the gap.
                return Ok(false);
            }
            if sequence.get() <= item.sequence
                || (item.needs_snapshot && kind != StreamFrameKind::Snapshot)
                || (item.mode == StreamMode::Request && kind != StreamFrameKind::Result)
            {
                return Err(io::ErrorKind::InvalidData.into());
            }
            item.sequence = sequence.get();
            item.needs_snapshot = false;
            if item
                .delivery
                .publish(StreamEvent::Record(record), item.mode)?
            {
                item.needs_snapshot = true;
                outgoing.push_back(Outgoing {
                    id: Some(stream_id),
                    bytes: encode_multiplex_message(&MultiplexMessage::Resnapshot { stream_id })?,
                    offset: 0,
                    deadline: Instant::now() + Duration::from_secs(2),
                    _reservation: Some(Arc::clone(&item._reservation)),
                });
            }
            if item.mode == StreamMode::Request {
                active.remove(&stream_id);
            }
        }
        MultiplexMessage::Gap { stream_id, through } => {
            if let Some(item) = active.get_mut(&stream_id) {
                if item.mode == StreamMode::Request || through.get() < item.sequence {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                item.needs_snapshot = true;
                item.delivery
                    .publish(StreamEvent::Gap, StreamMode::Events)?;
            } else {
                return Ok(false);
            }
        }
        MultiplexMessage::StreamClose { stream_id, reason } => {
            if let Some(item) = active.remove(&stream_id) {
                if reason == StreamCloseReason::Stopping {
                    item.delivery.refuse();
                } else {
                    item.delivery.close();
                }
            } else {
                return Ok(false);
            }
        }
        MultiplexMessage::Disconnect { reason } => {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                Disconnect(reason),
            ));
        }
        MultiplexMessage::ViewerRecordReady { viewer_id } => {
            let listeners = shared
                .viewer_ready
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for (_, listener) in &listeners.listeners {
                listener(viewer_id);
            }
        }
        MultiplexMessage::StreamOpen { .. }
        | MultiplexMessage::Resnapshot { .. }
        | MultiplexMessage::LaneOpen { .. }
        | MultiplexMessage::LaneClose { .. } => {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
    Ok(true)
}

fn queue_lane(outgoing: &mut VecDeque<Outgoing>, lane_id: MultiplexLaneId, open: bool) {
    let message = if open {
        MultiplexMessage::LaneOpen { lane_id }
    } else {
        MultiplexMessage::LaneClose { lane_id }
    };
    outgoing.push_back(Outgoing {
        id: None,
        bytes: encode_multiplex_message(&message).expect("fixed valid lane frame"),
        offset: 0,
        deadline: Instant::now() + Duration::from_secs(2),
        _reservation: None,
    });
}

#[derive(Debug)]
struct Disconnect(orchbee_workbench_protocol::MultiplexDisconnectReason);
impl std::fmt::Display for Disconnect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "OBWP disconnect: {:?}", self.0)
    }
}
impl std::error::Error for Disconnect {}

/// A drained Server refused one request before it took effect; the connection stays usable.
#[derive(Debug)]
struct Refused;
impl std::fmt::Display for Refused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the Session is stopping and accepts no new intent")
    }
}
impl std::error::Error for Refused {}

/// Whether the Server refused this request because it is stopping; nothing was applied.
fn refused(error: &io::Error) -> bool {
    error.get_ref().is_some_and(|source| source.is::<Refused>())
}

/// A typed peer rejection retires this carrier; an outer profile owner must not retry around it.
pub fn terminal_failure(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|source| source.is::<Disconnect>())
}

#[cfg(test)]
mod tests;
