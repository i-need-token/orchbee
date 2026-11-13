//! The owner of one attachment to the hub. A reader thread hands frames to the connection's state
//! machine and re-attaches after the hub goes away, starting a hub only when that hub was not asked
//! to stop and the owner is not closing; a writer thread writes what the state machine queues;
//! callers send requests, take notices, and open, write, end and read channels. One lock guards the
//! owner's state, with one signal for callers and one for the writer; no thread holds it while it
//! reads or writes the carrier or attaches. A channel's deliveries wait in the owner, bounded by
//! the channel's credit, which returns only as the caller takes its data; a channel belongs to the
//! connection it was opened on and reads `lost` when that connection is lost.

use super::{
    AttachFailure, Attached, Attachment,
    attach::{self, Cancel, Link, READ_BUFFER_BYTES, Start},
    data_root,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::DataRoot;
use orchbee_machine_protocol::{
    ByeReason, Connection, Delivery, FrameDecoder, Id, Message, ResetReason, SendError,
    hub::{self, AttachOutcome, Event, HubState, Outcome, Reply, Request},
};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Write},
    net::Shutdown,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// One re-attachment attempt's bound.
const ATTEMPT: Duration = Duration::from_secs(10);
/// How long after a loss re-attachment goes on while the hub is still closing: the hub's 60-second
/// allowance, its writers' 2 seconds, and a margin.
const CLOSING_ALLOWANCE: Duration = Duration::from_secs(75);
/// The loss within [`LOSS_WINDOW`] that ends the owner instead of re-attaching.
const LOSSES: usize = 5;
const LOSS_WINDOW: Duration = Duration::from_secs(60);
/// The reader stops reading while this many notices wait for `next`.
const PACE: usize = 256;
/// How long `close` gives `bye` and the threads.
const FAREWELL: Duration = Duration::from_secs(2);
/// The most bytes of whole frames one write carries.
const WRITE_BATCH_BYTES: usize = 64 * 1024;

/// What the owner tells its caller, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// A hub event.
    Event(Event),
    /// The owner attached to a hub again after a loss, restoring the attachment. Everything read
    /// before, the hub state, the catalog and each Machine, must be read again.
    Reattached(Attached),
    /// The owner ended; every later request fails with the same failure.
    Ended(AttachFailure),
}

/// Why a request has no reply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestFailure {
    /// The request cannot be sent, such as one too large for a frame; the hub never saw it.
    Unsendable,
    /// The connection was lost with the request in flight; whether the hub acted on it is unknown.
    Lost,
    /// The hub does not serve the request.
    Unsupported,
    /// No reply within the deadline.
    Timeout,
    /// The owner ended.
    Ended(AttachFailure),
}

/// A channel the owner opened, on the connection it was opened on.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChannelHandle {
    connection: u64,
    channel: u32,
}

/// What a channel delivers, in order. `Closed`, `Reset` and `Lost` are the last.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelDelivery {
    /// The hub accepted the channel.
    Accepted,
    /// Bytes from the far end; taking them returns their credit.
    Data(Vec<u8>),
    /// The far end sends no more bytes.
    Ended,
    /// The channel's result.
    Closed(Outcome),
    /// The hub reset the channel with its reason: no effect. A `session` channel also reads
    /// `session-not-running` when no Server of that Session answers on the Machine, and
    /// `invalid-session` when the Machine's platform refuses the Session's name.
    Reset(ResetReason),
    /// The connection was lost with the channel open.
    Lost,
}

/// Why a channel operation failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelFailure {
    /// No connection would send it.
    Unsendable,
    /// The channel is over, or its connection is gone; its deliveries tell how.
    Closed,
    /// No connection, or no room for another channel, within the deadline.
    Timeout,
    /// The owner ended.
    Ended(AttachFailure),
}

/// Whether [`HubClient::open_with`] may start a hub.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opening {
    /// Start one when none runs, once a closing hub is gone.
    Start,
    /// Start one when none runs, as the owner's re-attachment does; a hub answering `stopping`
    /// fails the call `closing` at once, since it was asked to stop.
    Restart,
    /// Attach only to a hub somebody else started; a hub of another version is reported, not
    /// replaced, and no hub at the deadline fails `closing`.
    Join,
}

/// Ends an open in progress from another thread. Once [`OpenCancel::cancel`] returns, no start is
/// under way and none begins, and every open that takes it fails `timeout` at once.
#[derive(Default)]
pub struct OpenCancel(Cancel);

impl OpenCancel {
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

/// One attachment to the hub of this process's data root, kept across the hub going away.
pub struct HubClient {
    inner: Arc<Inner>,
    threads: Vec<JoinHandle<()>>,
}

struct Inner {
    root: DataRoot,
    /// Ends a re-attachment in progress when `close` begins.
    cancel: Cancel,
    state: Mutex<State>,
    /// Wakes callers waiting for a reply, a notice or a connection, the paused reader, and `close`.
    changed: Condvar,
    /// Wakes the writer.
    transmit: Condvar,
}

struct State {
    /// The attached connection; absent while re-attaching and after the end.
    current: Option<Current>,
    /// What a re-attachment restores.
    attachment: Attachment,
    notices: VecDeque<Notice>,
    /// Each request's answer by ticket, `None` while it waits.
    answers: BTreeMap<u64, Option<Result<Reply, RequestFailure>>>,
    next_ticket: u64,
    losses: VecDeque<Instant>,
    /// The attached hub was seen stopping: its state event said `closing`, or it answered an
    /// `attach` `stopping`. A loss after that is its ending, not a crash.
    stopping: bool,
    ended: Option<AttachFailure>,
    closing: bool,
    reader_done: bool,
    writer_done: bool,
    /// Each open channel's deliveries not yet taken, by its connection's number and its id.
    channels: BTreeMap<(u64, u32), Slot>,
    /// The number of the last connection installed.
    connections: u64,
}

#[derive(Default)]
struct Slot {
    deliveries: VecDeque<ChannelDelivery>,
    /// The last delivery is queued.
    over: bool,
}

struct Current {
    /// Its number among the owner's connections.
    number: u64,
    connection: Connection<hub::Hub>,
    /// The writer's handle; shutting it down cancels the reader's read too.
    stream: Arc<LocalStream>,
    /// Requests in flight: the ticket, and the attachment an `attach` states.
    pending: BTreeMap<Id, (u64, Option<Attachment>)>,
}

/// Why the reader stopped reading a connection.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Loss {
    /// `bye` `stopping`: the hub was asked to stop, or expired.
    Stopped,
    /// `bye` other than `stopping` or `violation`, or the carrier ended.
    Gone,
    /// Either side broke the protocol.
    Violation,
    /// `close` began.
    Closed,
}

/// What the reader needs to read one connection.
struct Reading {
    stream: LocalStream,
    decoder: FrameDecoder,
    early: Vec<Delivery<hub::Hub>>,
}

impl HubClient {
    /// Attaches as `attachment` to the hub of this process's data root, starting it when none
    /// runs, within `deadline`.
    ///
    /// # Errors
    ///
    /// [`AttachFailure`] as attach or start answers; `closing` when the hub was still closing at
    /// the deadline.
    pub fn open(
        attachment: Attachment,
        deadline: Instant,
    ) -> Result<(Self, Attached), AttachFailure> {
        Self::open_with(attachment, deadline, Opening::Start, &OpenCancel::default())
    }

    /// Attaches as `attachment` within `deadline`, starting a hub only as `opening` allows, until
    /// `cancel` ends the attempt.
    ///
    /// # Errors
    ///
    /// As [`HubClient::open`]; `timeout` once cancelled.
    pub fn open_with(
        attachment: Attachment,
        deadline: Instant,
        opening: Opening,
        cancel: &OpenCancel,
    ) -> Result<(Self, Attached), AttachFailure> {
        let root = data_root()?;
        let start = match opening {
            Opening::Start => Start::Always,
            Opening::Restart => Start::UntilStopping,
            Opening::Join => Start::Never,
        };
        let link = attach::attach(&root, attachment, deadline, start, &cancel.0)?;
        let attached = link.attached;
        let inner = Arc::new(Inner {
            root,
            cancel: Cancel::default(),
            state: Mutex::new(State {
                current: None,
                attachment,
                notices: VecDeque::new(),
                answers: BTreeMap::new(),
                next_ticket: 0,
                losses: VecDeque::new(),
                stopping: false,
                ended: None,
                closing: false,
                reader_done: false,
                writer_done: false,
                channels: BTreeMap::new(),
                connections: 0,
            }),
            changed: Condvar::new(),
            transmit: Condvar::new(),
        });
        let reading = {
            let mut state = inner.lock();
            install(&mut state, link).ok_or(AttachFailure::Unavailable)?
        };
        let mut client = Self {
            inner,
            threads: Vec::new(),
        };
        let reader = {
            let inner = client.inner.clone();
            thread::Builder::new()
                .name("orchbee-hub-client-reader".into())
                .spawn(move || inner.read(reading))
        };
        client
            .threads
            .push(reader.map_err(|_| AttachFailure::Unavailable)?);
        let writer = {
            let inner = client.inner.clone();
            thread::Builder::new()
                .name("orchbee-hub-client-writer".into())
                .spawn(move || inner.write())
        };
        client
            .threads
            .push(writer.map_err(|_| AttachFailure::Unavailable)?);
        Ok((client, attached))
    }

    /// Sends `request` and waits for its reply within `deadline`. While the owner re-attaches, the
    /// request waits for the new connection. An `attach` that is answered `attached` becomes what a
    /// later re-attachment restores.
    ///
    /// # Errors
    ///
    /// [`RequestFailure`] when no reply arrives; at once when the request cannot be sent.
    pub fn request(&self, request: Request, deadline: Instant) -> Result<Reply, RequestFailure> {
        let attachment = match &request {
            Request::Attach(attach) => Some(Attachment {
                presence: attach.presence,
                asks: attach.asks,
                watch: attach.watch,
            }),
            _ => None,
        };
        let mut state = self.inner.lock();
        let ticket = loop {
            if let Some(failure) = &state.ended {
                return Err(RequestFailure::Ended(failure.clone()));
            }
            let ticket = state.next_ticket;
            let sent = match state.current.as_mut() {
                Some(current) => match current.connection.request(request.clone()) {
                    Ok(id) => {
                        current.pending.insert(id, (ticket, attachment));
                        true
                    }
                    // 64 requests already wait, or the connection is ending.
                    Err(SendError::TooManyRequests | SendError::Closed) => false,
                    // No connection would send it.
                    Err(_) => return Err(RequestFailure::Unsendable),
                },
                None => false,
            };
            if sent {
                state.next_ticket += 1;
                state.answers.insert(ticket, None);
                break ticket;
            }
            // Re-attaching, 64 requests already wait, or the connection is ending: a reply or a
            // connection wakes this.
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out {
                return Err(RequestFailure::Timeout);
            }
        };
        self.inner.transmit.notify_all();
        loop {
            if matches!(state.answers.get(&ticket), Some(Some(_)))
                && let Some(Some(answer)) = state.answers.remove(&ticket)
            {
                return answer;
            }
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out && !matches!(state.answers.get(&ticket), Some(Some(_))) {
                // A late reply finds no ticket and is dropped.
                state.answers.remove(&ticket);
                return Err(RequestFailure::Timeout);
            }
        }
    }

    /// The next notice, waiting until `deadline`; `None` when none arrived, and at once after the
    /// owner's end was taken.
    pub fn next(&self, deadline: Instant) -> Option<Notice> {
        let mut state = self.inner.lock();
        loop {
            if let Some(notice) = state.notices.pop_front() {
                let paused = state.notices.len() + 1 >= PACE;
                drop(state);
                if paused {
                    self.inner.changed.notify_all();
                }
                return Some(notice);
            }
            if state.ended.is_some() {
                return None;
            }
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out && state.notices.is_empty() {
                return None;
            }
        }
    }

    /// Whether the owner holds a connection to a hub not seen stopping: false from a loss until the
    /// re-attachment, once the hub said it is closing, and after the end. Never waits.
    pub fn connected(&self) -> bool {
        let state = self.inner.lock();
        state.current.is_some() && !state.stopping && state.ended.is_none()
    }

    /// Opens `channel` on the attached connection, waiting for one while re-attaching and for room
    /// while 256 channels are open.
    ///
    /// # Errors
    ///
    /// [`ChannelFailure`] when no connection takes it within `deadline`, or none could.
    pub fn open_channel(
        &self,
        channel: &hub::Channel,
        deadline: Instant,
    ) -> Result<ChannelHandle, ChannelFailure> {
        let mut state = self.inner.lock();
        loop {
            if let Some(failure) = &state.ended {
                return Err(ChannelFailure::Ended(failure.clone()));
            }
            if let Some(current) = state.current.as_mut() {
                match current.connection.open(channel.clone()) {
                    Ok(id) => {
                        let handle = ChannelHandle {
                            connection: current.number,
                            channel: id,
                        };
                        state
                            .channels
                            .insert((handle.connection, id), Slot::default());
                        drop(state);
                        self.inner.transmit.notify_all();
                        return Ok(handle);
                    }
                    // 256 channels are open, or the connection is ending.
                    Err(SendError::Capacity | SendError::Closed) => {}
                    Err(_) => return Err(ChannelFailure::Unsendable),
                }
            }
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out {
                return Err(ChannelFailure::Timeout);
            }
        }
    }

    /// Commits as many leading bytes of `bytes` as the channel's credit admits and returns their
    /// count, waiting for credit until `deadline` while none is left; 0 when none came in time.
    ///
    /// # Errors
    ///
    /// [`ChannelFailure::Closed`] once the channel is over or its connection is gone.
    pub fn write(
        &self,
        handle: ChannelHandle,
        bytes: &[u8],
        deadline: Instant,
    ) -> Result<usize, ChannelFailure> {
        let mut state = self.inner.lock();
        loop {
            let current = state
                .current
                .as_mut()
                .filter(|current| current.number == handle.connection)
                .ok_or(ChannelFailure::Closed)?;
            match current.connection.write(handle.channel, bytes) {
                Ok(0) if !bytes.is_empty() => {}
                Ok(committed) => {
                    drop(state);
                    self.inner.transmit.notify_all();
                    return Ok(committed);
                }
                Err(_) => return Err(ChannelFailure::Closed),
            }
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out {
                return Ok(0);
            }
        }
    }

    /// Sends `end` on the channel, after the bytes it committed.
    ///
    /// # Errors
    ///
    /// [`ChannelFailure::Closed`] once the channel is over or its connection is gone.
    pub fn end(&self, handle: ChannelHandle) -> Result<(), ChannelFailure> {
        let mut state = self.inner.lock();
        let current = state
            .current
            .as_mut()
            .filter(|current| current.number == handle.connection)
            .ok_or(ChannelFailure::Closed)?;
        current
            .connection
            .end(handle.channel)
            .map_err(|_| ChannelFailure::Closed)?;
        drop(state);
        self.inner.transmit.notify_all();
        Ok(())
    }

    /// Resets the channel `cancelled` while it is open, and forgets it.
    pub fn cancel(&self, handle: ChannelHandle) {
        let mut state = self.inner.lock();
        state.channels.remove(&(handle.connection, handle.channel));
        if let Some(current) = state
            .current
            .as_mut()
            .filter(|current| current.number == handle.connection)
        {
            let _reset = current
                .connection
                .reset(handle.channel, ResetReason::Cancelled);
        }
        drop(state);
        self.inner.transmit.notify_all();
    }

    /// The channel's next delivery, waiting until `deadline`; `None` when none arrived. Taking data
    /// returns its credit. After its last delivery the channel is forgotten and reads `lost`.
    pub fn receive(&self, handle: ChannelHandle, deadline: Instant) -> Option<ChannelDelivery> {
        let key = (handle.connection, handle.channel);
        let mut state = self.inner.lock();
        loop {
            let Some(slot) = state.channels.get_mut(&key) else {
                return Some(ChannelDelivery::Lost);
            };
            if let Some(delivery) = slot.deliveries.pop_front() {
                if slot.over && slot.deliveries.is_empty() {
                    state.channels.remove(&key);
                }
                if let ChannelDelivery::Data(bytes) = &delivery
                    && let Some(current) = state
                        .current
                        .as_mut()
                        .filter(|current| current.number == handle.connection)
                {
                    let _taken = current.connection.take(handle.channel, bytes.len());
                    drop(state);
                    self.inner.transmit.notify_all();
                }
                return Some(delivery);
            }
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out {
                return None;
            }
        }
    }

    /// Sends `bye` `done`, cancels a re-attachment in progress, which then starts no hub, gives
    /// `bye` and the threads at most 2 seconds, and ends the owner.
    pub fn close(mut self) {
        self.shut();
    }

    fn shut(&mut self) {
        let deadline = Instant::now() + FAREWELL;
        {
            let mut state = self.inner.lock();
            if state.closing {
                return;
            }
            state.closing = true;
            if let Some(current) = state.current.as_mut() {
                let _farewell = current.connection.bye(ByeReason::Done);
            }
        }
        // After `closing` is set, so a reader whose attempt this ends finds the owner closed.
        self.inner.cancel.cancel();
        self.inner.transmit.notify_all();
        self.inner.changed.notify_all();
        let mut state = self.inner.lock();
        while !state.writer_done {
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out {
                break;
            }
        }
        // Cancels the reader's read of the attached connection.
        if let Some(current) = state.current.as_ref() {
            let _cancelled = current.stream.shutdown(Shutdown::Both);
        }
        while !state.reader_done {
            let (next, waited_out) = self.inner.wait(state, deadline);
            state = next;
            if waited_out {
                break;
            }
        }
        drop(state);
        self.inner.transmit.notify_all();
        for thread in self.threads.drain(..) {
            // A thread past the bound ends on its own; the owner does not wait for it.
            if thread.is_finished() {
                let _joined = thread.join();
            }
        }
    }
}

impl Drop for HubClient {
    fn drop(&mut self) {
        self.shut();
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Waits on `changed` until `deadline`; true when the deadline had passed.
    fn wait<'a>(
        &self,
        state: MutexGuard<'a, State>,
        deadline: Instant,
    ) -> (MutexGuard<'a, State>, bool) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return (state, true);
        }
        let (state, _) = self
            .changed
            .wait_timeout(state, remaining)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state, Instant::now() >= deadline)
    }

    /// The reader: each connection until it is lost, then a re-attachment, until the end.
    fn read(&self, mut reading: Reading) {
        let mut buffer = vec![0; READ_BUFFER_BYTES];
        loop {
            let loss = self.pump(&mut reading, &mut buffer);
            match self.recover(loss) {
                Some(next) => reading = next,
                None => break,
            }
        }
        self.lock().reader_done = true;
        self.changed.notify_all();
        self.transmit.notify_all();
    }

    /// Reads one connection until it is lost.
    fn pump(&self, reading: &mut Reading, buffer: &mut [u8]) -> Loss {
        {
            let mut state = self.lock();
            for delivery in std::mem::take(&mut reading.early) {
                if let Some(loss) = state.deliver(delivery) {
                    return loss;
                }
            }
        }
        self.changed.notify_all();
        loop {
            {
                let mut state = self.lock();
                while state.notices.len() >= PACE && !state.closing {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                if state.closing {
                    return Loss::Closed;
                }
            }
            let read = match reading.stream.read(buffer) {
                Ok(0) => return Loss::Gone,
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Loss::Gone,
            };
            let mut state = self.lock();
            let mut offset = 0;
            while let Some(rest) = buffer.get(offset..read).filter(|rest| !rest.is_empty()) {
                let Ok((consumed, frame)) = reading.decoder.decode(rest) else {
                    return Loss::Violation;
                };
                offset += consumed;
                if let Some(frame) = frame {
                    let Some(current) = state.current.as_mut() else {
                        return Loss::Gone;
                    };
                    let Ok(receipt) = current.connection.receive(&frame) else {
                        return Loss::Violation;
                    };
                    if let Some(delivery) = receipt.delivery
                        && let Some(loss) = state.deliver(delivery)
                    {
                        return loss;
                    }
                }
                if consumed == 0 {
                    break;
                }
            }
            let transmit = state
                .current
                .as_ref()
                .is_some_and(|current| current.connection.queued_frames() > 0);
            drop(state);
            self.changed.notify_all();
            if transmit {
                self.transmit.notify_all();
            }
        }
    }

    /// After a loss: ends the owner, or re-attaches and returns the new connection to read. A hub
    /// that was stopping is never restarted in reaction: the owner attaches only to a hub somebody
    /// else starts, and ends `closing` when none appears.
    fn recover(&self, loss: Loss) -> Option<Reading> {
        let (attachment, stopped) = {
            let mut state = self.lock();
            if let Some(current) = state.current.take() {
                let _closed = current.stream.shutdown(Shutdown::Both);
                state.fail(
                    current.pending.values().map(|(ticket, _)| *ticket),
                    &RequestFailure::Lost,
                );
                state.lose_channels(Some(current.number));
            }
            if state.closing || loss == Loss::Closed {
                return None;
            }
            if loss == Loss::Violation {
                state.end(AttachFailure::Lost);
                drop(state);
                self.changed.notify_all();
                return None;
            }
            let now = Instant::now();
            while state
                .losses
                .front()
                .is_some_and(|at| now.duration_since(*at) >= LOSS_WINDOW)
            {
                state.losses.pop_front();
            }
            state.losses.push_back(now);
            if state.losses.len() >= LOSSES {
                state.end(AttachFailure::Unavailable);
                drop(state);
                self.changed.notify_all();
                return None;
            }
            (state.attachment, loss == Loss::Stopped || state.stopping)
        };
        self.changed.notify_all();
        let lost = Instant::now();
        let (mut start, mut deadline) = if stopped {
            (Start::Never, lost + CLOSING_ALLOWANCE)
        } else {
            (Start::UntilStopping, lost + ATTEMPT)
        };
        loop {
            let attached = attach::attach(&self.root, attachment, deadline, start, &self.cancel);
            let mut state = self.lock();
            if state.closing {
                if let Ok(link) = attached {
                    let _closed = link.stream.shutdown(Shutdown::Both);
                }
                return None;
            }
            match attached {
                Ok(link) => {
                    let notice = Notice::Reattached(link.attached);
                    let Some(reading) = install(&mut state, link) else {
                        state.end(AttachFailure::Unavailable);
                        drop(state);
                        self.changed.notify_all();
                        return None;
                    };
                    state.notices.push_back(notice);
                    drop(state);
                    self.changed.notify_all();
                    self.transmit.notify_all();
                    return Some(reading);
                }
                // The hub that answered is closing, so it was asked to stop: wait it out, starting
                // none.
                Err(AttachFailure::Closing)
                    if start == Start::UntilStopping
                        && Instant::now() < lost + CLOSING_ALLOWANCE =>
                {
                    start = Start::Never;
                    deadline = lost + CLOSING_ALLOWANCE;
                }
                Err(failure) => {
                    state.end(failure);
                    drop(state);
                    self.changed.notify_all();
                    return None;
                }
            }
        }
    }

    /// The writer: whole frames in the state machine's order, up to a batch per write, until the
    /// owner closes or ends.
    fn write(&self) {
        loop {
            let batch = {
                let mut state = self.lock();
                loop {
                    let closing = state.closing;
                    if let Some(current) = state.current.as_mut() {
                        let mut bytes = Vec::new();
                        while bytes.len() < WRITE_BATCH_BYTES {
                            match current.connection.poll_transmit() {
                                Some(frame) => bytes.extend_from_slice(&frame.encode()),
                                None => break,
                            }
                        }
                        if !bytes.is_empty() {
                            break Some((current.stream.clone(), bytes));
                        }
                        if closing {
                            break None;
                        }
                    } else if closing || state.ended.is_some() {
                        break None;
                    }
                    state = self
                        .transmit
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            };
            let Some((stream, bytes)) = batch else {
                break;
            };
            // A failed write loses the connection, which the reader finds.
            let mut output = &*stream;
            let _written = output.write_all(&bytes);
        }
        self.lock().writer_done = true;
        self.changed.notify_all();
    }
}

impl State {
    /// Takes one delivery; a loss when it ends the connection.
    fn deliver(&mut self, delivery: Delivery<hub::Hub>) -> Option<Loss> {
        match delivery {
            Delivery::Message(Message::Reply { id, reply, .. }) => {
                // A reply to no pending request is one the owner already failed as `lost`.
                if let Some((ticket, attachment)) = self
                    .current
                    .as_mut()
                    .and_then(|current| current.pending.remove(&id))
                {
                    match (attachment, &reply) {
                        (Some(attachment), Reply::Attach(AttachOutcome::Attached { .. })) => {
                            self.attachment = attachment;
                        }
                        (_, Reply::Attach(AttachOutcome::Stopping)) => self.stopping = true,
                        _ => {}
                    }
                    if let Some(answer) = self.answers.get_mut(&ticket) {
                        *answer = Some(Ok(reply));
                    }
                }
                None
            }
            Delivery::Message(Message::Unsupported { id, .. }) => {
                if let Some((ticket, _)) = self
                    .current
                    .as_mut()
                    .and_then(|current| current.pending.remove(&id))
                    && let Some(answer) = self.answers.get_mut(&ticket)
                {
                    *answer = Some(Err(RequestFailure::Unsupported));
                }
                None
            }
            Delivery::Message(Message::Event { event, .. }) => {
                if matches!(event, Event::Hub(HubState::Closing)) {
                    self.stopping = true;
                }
                self.notices.push_back(Notice::Event(event));
                None
            }
            Delivery::Message(Message::Bye { reason, .. }) => Some(match reason {
                ByeReason::Stopping => Loss::Stopped,
                ByeReason::Violation => Loss::Violation,
                _ => Loss::Gone,
            }),
            Delivery::Accept { channel, .. } => {
                self.queue(channel, ChannelDelivery::Accepted, false);
                None
            }
            Delivery::Data { channel, bytes } => {
                self.queue(channel, ChannelDelivery::Data(bytes), false);
                None
            }
            Delivery::End { channel } => {
                self.queue(channel, ChannelDelivery::Ended, false);
                None
            }
            Delivery::Close {
                channel, outcome, ..
            } => {
                self.queue(channel, ChannelDelivery::Closed(outcome), true);
                None
            }
            Delivery::Reset {
                channel, reason, ..
            } => {
                self.queue(channel, ChannelDelivery::Reset(reason), true);
                None
            }
            // A writer waiting for credit wakes with every batch the reader takes.
            _ => None,
        }
    }

    /// Queues `delivery` for the attached connection's channel `channel` while the owner keeps it.
    fn queue(&mut self, channel: u32, delivery: ChannelDelivery, last: bool) {
        let Some(number) = self.current.as_ref().map(|current| current.number) else {
            return;
        };
        if let Some(slot) = self.channels.get_mut(&(number, channel))
            && !slot.over
        {
            slot.deliveries.push_back(delivery);
            slot.over = last;
        }
    }

    /// Ends each open channel of the connection `number`, or of every connection, with `lost`.
    fn lose_channels(&mut self, number: Option<u64>) {
        for ((connection, _), slot) in &mut self.channels {
            if number.is_none_or(|number| number == *connection) && !slot.over {
                slot.deliveries.push_back(ChannelDelivery::Lost);
                slot.over = true;
            }
        }
    }

    /// Answers the waiting requests `tickets` with `failure`.
    fn fail(&mut self, tickets: impl Iterator<Item = u64>, failure: &RequestFailure) {
        for ticket in tickets {
            if let Some(answer) = self.answers.get_mut(&ticket) {
                *answer = Some(Err(failure.clone()));
            }
        }
    }

    /// Ends the owner with `failure`: the last notice, and the answer to every waiting request.
    fn end(&mut self, failure: AttachFailure) {
        let waiting: Vec<u64> = self
            .answers
            .iter()
            .filter(|(_, answer)| answer.is_none())
            .map(|(ticket, _)| *ticket)
            .collect();
        self.fail(waiting.into_iter(), &RequestFailure::Ended(failure.clone()));
        self.lose_channels(None);
        self.notices.push_back(Notice::Ended(failure.clone()));
        self.ended = Some(failure);
    }
}

/// Makes `link` the attached connection and returns what the reader needs to read it; `None` when
/// the carrier cannot be shared between the reader and the writer.
fn install(state: &mut State, link: Link) -> Option<Reading> {
    let writer = link.stream.try_clone().ok()?;
    state.stopping = false;
    state.connections += 1;
    state.current = Some(Current {
        number: state.connections,
        connection: link.connection,
        stream: Arc::new(writer),
        pending: BTreeMap::new(),
    });
    Some(Reading {
        stream: link.stream,
        decoder: link.decoder,
        early: link.early,
    })
}
