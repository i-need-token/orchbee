//! The sole TUI connection owner. Lanes submit complete records and never own a Server socket.
use super::LocalClientOrigin;
use orchbee_local_client_protocol::ClientConnection;
use orchbee_local_ipc::{AsLocalIo, LocalIo, LocalStream, PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    MAXIMUM_MULTIPLEX_BYTES, MAXIMUM_MULTIPLEX_STREAMS, MultiplexDecoder, MultiplexMessage,
    MultiplexStreamId, StreamCloseReason, StreamFrameKind, StreamMode, encode_multiplex_message,
};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Write},
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const QUEUED: u8 = 0;
const HANDED_OFF: u8 = 1;
const CANCELLED: u8 = 2;
const EVENT_RECORDS: usize = 32;

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
pub(crate) enum StreamEvent {
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
}
#[derive(Debug)]
struct Delivery {
    state: Mutex<DeliveryState>,
    ready: LocalStream,
    notify: LocalStream,
    handoff: AtomicU8,
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
            handoff: AtomicU8::new(QUEUED),
            budget,
        })
    }
    fn signal(&self) {
        let _notified = (&self.notify).write(&[1]);
    }
    fn cancelled(&self) -> bool {
        self.handoff.load(Ordering::Acquire) == CANCELLED
    }
    fn cancel(&self) -> bool {
        self.handoff
            .compare_exchange(QUEUED, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_err_and(|previous| previous == HANDED_OFF)
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
            self.budget.fetch_sub(state.bytes, Ordering::AcqRel);
            state.bytes = 0;
            state.events.clear();
            if overflow {
                state.events.push_back(StreamEvent::Gap);
                drop(state);
                self.signal();
                return Ok(true);
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
            return Err(io::ErrorKind::OutOfMemory.into());
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
    fn fail(&self, reason: orchbee_workbench_protocol::MultiplexDisconnectReason) {
        if let Ok(mut state) = self.state.lock() {
            state.failure = Some(reason);
            state.closed = true;
        }
        self.signal();
    }
    fn wait(
        &self,
        deadline: Instant,
        clock: &dyn super::MonotonicClock,
        wake: Option<LocalIo<'_>>,
        cancel: Option<LocalIo<'_>>,
    ) -> io::Result<StreamEvent> {
        loop {
            if let Some(event) = self.take()? {
                return Ok(event);
            }
            let remaining = deadline
                .checked_duration_since(clock.now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or(io::ErrorKind::TimedOut)?;
            let mut fds = vec![PollFd::new(self.ready.as_local_io(), PollFlags::POLLIN)];
            if let Some(wake) = wake {
                fds.push(PollFd::new(wake, PollFlags::POLLIN));
            }
            let cancel_index = fds.len();
            if let Some(cancel) = cancel {
                fds.push(PollFd::new(cancel, PollFlags::POLLIN));
            }
            poll(&mut fds, super::poll_timeout(remaining))?;
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
    Open {
        id: MultiplexStreamId,
        mode: StreamMode,
        request: Vec<u8>,
        delivery: Arc<Delivery>,
        reservation: Arc<Reservation>,
    },
    Cancel(MultiplexStreamId),
}
#[derive(Debug, Default)]
struct Commands {
    next: u64,
    queue: VecDeque<Command>,
}
#[derive(Debug)]
struct Shared {
    commands: Mutex<Commands>,
    wake: LocalStream,
    stopped: AtomicBool,
    generation: AtomicU64,
    budget: Arc<AtomicUsize>,
    streams: Arc<AtomicUsize>,
}
impl Shared {
    fn signal(&self) {
        let _notified = (&self.wake).write(&[1]);
    }
    fn cancel(&self, id: MultiplexStreamId) {
        if let Ok(mut commands) = self.commands.lock() {
            // Cancellation capacity is separate and bounded by the active stream bound.
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
impl Connection {
    pub(super) fn start(
        connection: ClientConnection,
        mut origin: LocalClientOrigin,
    ) -> io::Result<Arc<Self>> {
        let (reader, wake) = LocalStream::pair()?;
        reader.set_nonblocking(true)?;
        wake.set_nonblocking(true)?;
        origin.multiplex = None;
        let shared = Arc::new(Shared {
            commands: Mutex::default(),
            wake,
            stopped: AtomicBool::new(false),
            generation: AtomicU64::new(connection.generation().get()),
            budget: Arc::new(AtomicUsize::new(0)),
            streams: Arc::new(AtomicUsize::new(0)),
        });
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-client-connection".into())
            .spawn(move || actor(connection, origin, reader, state))?;
        Ok(Arc::new(Self {
            shared,
            task: Mutex::new(Some(task)),
        }))
    }
    pub(super) fn generation(&self) -> NonZeroU64 {
        NonZeroU64::new(self.shared.generation.load(Ordering::Acquire))
            .expect("admitted connection generation is nonzero")
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
    fn open(self: &Arc<Self>, frame: Vec<u8>, mode: StreamMode) -> io::Result<Subscription> {
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
        self.open(frame, mode)
    }
    pub(super) fn exchange(
        self: &Arc<Self>,
        frame: &[u8],
        deadline: Instant,
        clock: &dyn super::MonotonicClock,
        wake: Option<LocalIo<'_>>,
        cancel: Option<LocalIo<'_>>,
    ) -> Result<Vec<u8>, (bool, io::Error)> {
        if clock.now() >= deadline {
            return Err((false, io::ErrorKind::TimedOut.into()));
        }
        let subscription = self
            .open(frame.to_vec(), StreamMode::Request)
            .map_err(|error| (false, error))?;
        match subscription.delivery.wait(deadline, clock, wake, cancel) {
            Ok(StreamEvent::Record(bytes)) => Ok(bytes),
            Ok(StreamEvent::Gap | StreamEvent::Disconnected) => Err((
                subscription.delivery.cancel(),
                io::ErrorKind::ConnectionAborted.into(),
            )),
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
pub(crate) struct Subscription {
    owner: Arc<Connection>,
    id: MultiplexStreamId,
    delivery: Arc<Delivery>,
}
impl Subscription {
    pub(crate) fn poll(&self) -> io::Result<Option<StreamEvent>> {
        self.delivery.take()
    }
    pub(crate) fn readiness(&self) -> LocalIo<'_> {
        self.delivery.ready.as_local_io()
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.delivery.cancel();
        self.delivery.close();
        self.owner.shared.cancel(self.id);
    }
}

struct Active {
    mode: StreamMode,
    request: Vec<u8>,
    delivery: Arc<Delivery>,
    sequence: u64,
    needs_snapshot: bool,
    _reservation: Arc<Reservation>,
}
struct Outgoing {
    id: MultiplexStreamId,
    bytes: Vec<u8>,
    offset: usize,
    deadline: Instant,
    _reservation: Arc<Reservation>,
}

fn actor(
    connection: ClientConnection,
    origin: LocalClientOrigin,
    wake: LocalStream,
    shared: Arc<Shared>,
) {
    let mut connection = Some(connection);
    let mut active = BTreeMap::<MultiplexStreamId, Active>::new();
    let mut outgoing = VecDeque::new();
    let mut decoder = MultiplexDecoder::default();
    let mut reconnect_at = Instant::now();
    let mut partial_at = None;
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
                Command::Open {
                    id,
                    mode,
                    request,
                    delivery,
                    reservation,
                } => {
                    if delivery.cancelled() {
                        continue;
                    }
                    if active.len() >= MAXIMUM_MULTIPLEX_STREAMS {
                        delivery.close();
                        continue;
                    }
                    if connection.is_some() {
                        if let Ok(bytes) = encode_multiplex_message(&MultiplexMessage::StreamOpen {
                            stream_id: id,
                            mode,
                            request: request.clone(),
                        }) {
                            outgoing.push_back(Outgoing {
                                id,
                                bytes,
                                offset: 0,
                                deadline: Instant::now() + Duration::from_secs(2),
                                _reservation: Arc::clone(&reservation),
                            });
                        } else {
                            delivery.close();
                            continue;
                        }
                    }
                    active.insert(
                        id,
                        Active {
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
                    outgoing.retain(|item| item.id != id || item.offset != 0);
                    if let Some(removed) = active.remove(&id)
                        && connection.is_some()
                    {
                        if let Ok(bytes) =
                            encode_multiplex_message(&MultiplexMessage::StreamClose {
                                stream_id: id,
                                reason: StreamCloseReason::Cancelled,
                            })
                        {
                            outgoing.push_back(Outgoing {
                                id,
                                bytes,
                                offset: 0,
                                deadline: Instant::now() + Duration::from_secs(2),
                                _reservation: removed._reservation,
                            });
                        }
                    }
                }
            }
        }
        if connection.is_none() && !active.is_empty() && Instant::now() >= reconnect_at {
            match ClientConnection::connect_before_cancellable(
                &origin.endpoint,
                &origin.cohort,
                Instant::now() + Duration::from_secs(2),
                Some(wake.as_local_io()),
            ) {
                Ok(next) => {
                    shared
                        .generation
                        .store(next.generation().get(), Ordering::Release);
                    connection = Some(next);
                    for (id, item) in &mut active {
                        item.sequence = 0;
                        item.needs_snapshot = item.mode != StreamMode::Request;
                        if let Ok(bytes) = encode_multiplex_message(&MultiplexMessage::StreamOpen {
                            stream_id: *id,
                            mode: item.mode,
                            request: item.request.clone(),
                        }) {
                            outgoing.push_back(Outgoing {
                                id: *id,
                                bytes,
                                offset: 0,
                                deadline: Instant::now() + Duration::from_secs(2),
                                _reservation: Arc::clone(&item._reservation),
                            });
                        }
                    }
                }
                Err(_) => {
                    reconnect_at = Instant::now() + Duration::from_millis(250);
                }
            }
        }
        let result = match connection.as_mut() {
            Some(connection) => drive(
                connection,
                &wake,
                &mut active,
                &mut outgoing,
                &mut decoder,
                &mut partial_at,
            ),
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
            if let Some(Disconnect(reason)) = error
                .get_ref()
                .and_then(|error| error.downcast_ref::<Disconnect>())
            {
                // A typed admission/budget violation is terminal. Reconnecting would evade the
                // peer's sustained-overload policy; ordinary transport loss still rebuilds reads.
                shared.stopped.store(true, Ordering::Release);
                for item in active.values() {
                    item.delivery.fail(*reason);
                }
                break;
            }
            connection.take();
            outgoing.clear();
            decoder = MultiplexDecoder::default();
            partial_at = None;
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

fn drive(
    connection: &mut ClientConnection,
    wake: &LocalStream,
    active: &mut BTreeMap<MultiplexStreamId, Active>,
    outgoing: &mut VecDeque<Outgoing>,
    decoder: &mut MultiplexDecoder,
    partial_at: &mut Option<Instant>,
) -> io::Result<()> {
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
    let timeout = deadline.map_or(PollTimeout::NONE, |deadline| {
        super::poll_timeout(deadline.saturating_duration_since(Instant::now()))
    });
    let mut fds = [
        PollFd::new(connection.as_local_io(), interest),
        PollFd::new(wake.as_local_io(), PollFlags::POLLIN),
    ];
    poll(&mut fds, timeout)?;
    let flags = fds[0].revents().unwrap_or(PollFlags::empty());
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(io::ErrorKind::TimedOut.into());
    }
    if flags.contains(PollFlags::POLLIN) {
        let mut buffer = [0; 16384];
        match connection.read(&mut buffer) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => {
                decoder.push(&buffer[..count])?;
                while let Some(message) = decoder.next_message()? {
                    deliver(message, active, outgoing)?;
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
            && let Some(active) = active.get(&item.id)
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
                Ok(_) | Err(_) => {}
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
    active: &mut BTreeMap<MultiplexStreamId, Active>,
    outgoing: &mut VecDeque<Outgoing>,
) -> io::Result<()> {
    match message {
        MultiplexMessage::StreamFrame {
            stream_id,
            sequence,
            kind,
            record,
        } => {
            let Some(item) = active.get_mut(&stream_id) else {
                return Ok(());
            }; // cancelled stream's in-flight frame
            if item.needs_snapshot && kind == StreamFrameKind::Update {
                // Already-buffered updates preceding the resnapshot request cannot fill the gap.
                return Ok(());
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
                    id: stream_id,
                    bytes: encode_multiplex_message(&MultiplexMessage::Resnapshot { stream_id })?,
                    offset: 0,
                    deadline: Instant::now() + Duration::from_secs(2),
                    _reservation: Arc::clone(&item._reservation),
                });
            }
            if item.mode == StreamMode::Request {
                active.remove(&stream_id);
            }
        }
        MultiplexMessage::Gap { stream_id, .. } => {
            if let Some(item) = active.get_mut(&stream_id) {
                item.needs_snapshot = true;
                item.delivery
                    .publish(StreamEvent::Gap, StreamMode::Events)?;
            }
        }
        MultiplexMessage::StreamClose { stream_id, .. } => {
            if let Some(item) = active.remove(&stream_id) {
                item.delivery.close();
            }
        }
        MultiplexMessage::Disconnect { reason } => {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                Disconnect(reason),
            ));
        }
        MultiplexMessage::StreamOpen { .. } | MultiplexMessage::Resnapshot { .. } => {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Disconnect(orchbee_workbench_protocol::MultiplexDisconnectReason);
impl std::fmt::Display for Disconnect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "OBWP disconnect: {:?}", self.0)
    }
}
impl std::error::Error for Disconnect {}
