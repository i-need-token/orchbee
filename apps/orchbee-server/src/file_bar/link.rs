//! One admitted link to `orchbee-files`: a writer that sends the Server's frames in order, a reader
//! that checks every frame the process sends and relays it to its call's Client stream, and the
//! routes from calls to streams. A malformed frame, a frame for a call never sent, or a write the
//! process does not take within 30 seconds ends the link. When it ends, a stream still open closes
//! as the process would have closed it had it lived: a relayed answer completes, a mutation's
//! outcome is unknown, an unanswered call fails with `unavailable{restarting}`, and a
//! subscription stops.

use super::{
    mailbox::{Full, Mailbox},
    scope::Stated,
};
use orchbee_capability_node_protocol::FilesProcessIdentity;
use orchbee_files_protocol::{
    CallClose, CallId, FrameKind, LinkFrame, LinkMessage, LinkSide, RecordPlace,
    check_files_record, encode_link_message, read_link_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{MultiplexStreamId, StreamCloseReason, StreamFrameKind};
use std::{
    collections::{HashMap, VecDeque},
    io::{self, BufReader, Write},
    net::Shutdown,
    num::NonZeroU64,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread,
    time::Duration,
};

/// How long one frame may take to reach a process that stopped reading.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
/// The encoded frames the writer may hold; a process this far behind has stopped reading.
const MAXIMUM_QUEUED_BYTES: usize = 32 * 1024 * 1024;
const READ_BUFFER_BYTES: usize = 64 * 1024;

/// What the link reports to its owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Event {
    /// A routed call ended.
    CallEnded,
    /// Whether the process holds work no open call covers.
    Activity(bool),
    /// The link ended; every route it held was closed first.
    Ended,
}

/// Where one call's frames go.
pub(super) struct Route {
    pub(super) mailbox: Arc<Mailbox>,
    pub(super) stream: MultiplexStreamId,
    /// An Events call, which streams a Snapshot and Updates; otherwise a request.
    pub(super) events: bool,
    /// A mutation, whose effect is unknown once the process is lost before answering.
    pub(super) mutation: bool,
    /// The record a call the link loses before it answers ends with.
    pub(super) restarting: Arc<[u8]>,
    answered: bool,
    finished: bool,
    sequence: u64,
}

impl Route {
    pub(super) const fn new(
        mailbox: Arc<Mailbox>,
        stream: MultiplexStreamId,
        events: bool,
        mutation: bool,
        restarting: Arc<[u8]>,
    ) -> Self {
        Self {
            mailbox,
            stream,
            events,
            mutation,
            restarting,
            answered: false,
            finished: false,
            sequence: 0,
        }
    }
}

#[derive(Default)]
struct Routes {
    calls: HashMap<CallId, Route>,
    highest: u64,
    closed: bool,
}

#[derive(Default)]
struct Queue {
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
    closed: bool,
}

/// One admitted link.
pub(super) struct Link {
    id: u64,
    generation: NonZeroU64,
    process: FilesProcessIdentity,
    stream: LocalStream,
    queue: Mutex<Queue>,
    queued: Condvar,
    routes: Mutex<Routes>,
    scopes: Mutex<Stated>,
    events: Arc<dyn Fn(u64, Event) + Send + Sync>,
}

impl Link {
    /// Serves the admitted `stream` of `process`, which the Worker generation `generation`
    /// started, as link `id`, reporting to `events` until the link ends.
    pub(super) fn serve(
        id: u64,
        generation: NonZeroU64,
        process: FilesProcessIdentity,
        stream: LocalStream,
        events: Arc<dyn Fn(u64, Event) + Send + Sync>,
    ) -> io::Result<Arc<Self>> {
        let mut writer = stream.try_clone()?;
        writer.set_write_timeout(Some(WRITE_TIMEOUT))?;
        let reader = BufReader::with_capacity(READ_BUFFER_BYTES, stream.try_clone()?);
        let link = Arc::new(Self {
            id,
            generation,
            process,
            stream,
            queue: Mutex::default(),
            queued: Condvar::new(),
            routes: Mutex::default(),
            scopes: Mutex::default(),
            events,
        });
        let writing = Arc::clone(&link);
        thread::Builder::new()
            .name("orchbee-files-link-write".into())
            .spawn(move || writing.write(&mut writer))?;
        let reading = Arc::clone(&link);
        if let Err(error) = thread::Builder::new()
            .name("orchbee-files-link-read".into())
            .spawn(move || reading.read(reader))
        {
            link.close();
            return Err(error);
        }
        Ok(link)
    }

    pub(super) const fn id(&self) -> u64 {
        self.id
    }

    /// The Worker generation whose start created the process.
    pub(super) const fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    pub(super) const fn process(&self) -> FilesProcessIdentity {
        self.process
    }

    /// The scopes stated on this link. Holding the guard orders the updates sent under it.
    pub(super) fn scopes(&self) -> MutexGuard<'_, Stated> {
        self.scopes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues `message`; false once the link ended or its writer fell too far behind, which ends
    /// it.
    pub(super) fn send(&self, message: &LinkMessage) -> bool {
        let Ok(frame) = encode_link_message(message) else {
            return false;
        };
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        if queue.closed {
            return false;
        }
        if queue.bytes.saturating_add(frame.len()) > MAXIMUM_QUEUED_BYTES {
            drop(queue);
            self.close();
            return false;
        }
        queue.bytes += frame.len();
        queue.frames.push_back(frame);
        drop(queue);
        self.queued.notify_one();
        true
    }

    /// Routes `call`'s frames to `route`; false once the link ended.
    pub(super) fn route(&self, call: CallId, route: Route) -> bool {
        let mut routes = self.lock_routes();
        if routes.closed {
            return false;
        }
        routes.highest = routes.highest.max(call.get());
        routes.calls.insert(call, route);
        true
    }

    /// Stops routing `call`, whose Client closed its stream; true when the link routed it.
    pub(super) fn unroute(&self, call: CallId) -> bool {
        self.lock_routes().calls.remove(&call).is_some()
    }

    /// Whether the link ended, so it routes nothing more.
    pub(super) fn ended(&self) -> bool {
        self.lock_routes().closed
    }

    /// Ends the link: the process reads its end and exits, and the reader closes every route.
    pub(super) fn close(&self) {
        self.queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .closed = true;
        self.queued.notify_all();
        let _ = self.stream.shutdown(Shutdown::Both);
    }

    fn lock_routes(&self) -> MutexGuard<'_, Routes> {
        self.routes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self, writer: &mut LocalStream) {
        loop {
            let frame = {
                let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
                loop {
                    if queue.closed {
                        return;
                    }
                    if let Some(frame) = queue.frames.pop_front() {
                        queue.bytes -= frame.len();
                        break frame;
                    }
                    queue = self
                        .queued
                        .wait(queue)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            };
            if writer.write_all(&frame).is_err() {
                self.close();
                return;
            }
        }
    }

    fn read(&self, mut reader: BufReader<LocalStream>) {
        loop {
            let relayed = match read_link_message(&mut reader, LinkSide::Files) {
                Ok(LinkMessage::Frame(frame)) => self.relay_frame(frame),
                Ok(LinkMessage::Close { call, reason }) => self.relay_close(call, reason),
                Ok(LinkMessage::Activity { holds }) => {
                    (self.events)(self.id, Event::Activity(holds));
                    Ok(())
                }
                // A second hello, or a read that failed or ended.
                Ok(_) | Err(_) => Err(()),
            };
            if relayed.is_err() {
                break;
            }
        }
        self.close();
        self.fail_routes();
        (self.events)(self.id, Event::Ended);
    }

    /// Relays one frame after checking it may follow what its call sent before.
    fn relay_frame(&self, frame: LinkFrame) -> Result<(), ()> {
        let mut routes = self.lock_routes();
        if frame.call.get() > routes.highest {
            return Err(());
        }
        let Some(route) = routes.calls.get_mut(&frame.call) else {
            // The call's Client closed its stream first.
            return Ok(());
        };
        let (place, kind) = match (route.events, route.answered, frame.kind) {
            (_, false, FrameKind::Result) if !route.finished => {
                (RecordPlace::ResultFrame, StreamFrameKind::Result)
            }
            (true, false, FrameKind::Snapshot) => {
                (RecordPlace::NoticeFrame, StreamFrameKind::Snapshot)
            }
            (true, true, FrameKind::Update) if !route.finished => {
                (RecordPlace::NoticeFrame, StreamFrameKind::Update)
            }
            _ => return Err(()),
        };
        check_files_record(&frame.record, place).map_err(|_| ())?;
        route.answered = true;
        route.finished |= kind == StreamFrameKind::Result;
        route.sequence += 1;
        let sequence = NonZeroU64::new(route.sequence).ok_or(())?;
        if let Err(Full) = route
            .mailbox
            .frame(route.stream, sequence, kind, frame.record)
        {
            // The Client stopped reading: its stream closes, and the call is cancelled.
            let Some(route) = routes.calls.remove(&frame.call) else {
                return Ok(());
            };
            drop(routes);
            let reason = if route.mutation {
                StreamCloseReason::OutcomeUnknown
            } else {
                StreamCloseReason::Capacity
            };
            route.mailbox.close(route.stream, reason, true);
            self.send(&LinkMessage::Cancel { call: frame.call });
            (self.events)(self.id, Event::CallEnded);
        }
        Ok(())
    }

    fn relay_close(&self, call: CallId, reason: CallClose) -> Result<(), ()> {
        let mut routes = self.lock_routes();
        if call.get() > routes.highest {
            return Err(());
        }
        let Some(route) = routes.calls.remove(&call) else {
            return Ok(());
        };
        drop(routes);
        route
            .mailbox
            .close(route.stream, stream_close(reason), false);
        (self.events)(self.id, Event::CallEnded);
        Ok(())
    }

    fn fail_routes(&self) {
        let routes = {
            let mut routes = self.lock_routes();
            routes.closed = true;
            std::mem::take(&mut routes.calls)
        };
        for route in routes.into_values() {
            let reason = if route.finished {
                StreamCloseReason::Complete
            } else if route.mutation {
                StreamCloseReason::OutcomeUnknown
            } else if route.answered {
                StreamCloseReason::Stopping
            } else if route
                .mailbox
                .frame(
                    route.stream,
                    NonZeroU64::MIN,
                    StreamFrameKind::Result,
                    route.restarting.to_vec(),
                )
                .is_ok()
            {
                StreamCloseReason::Complete
            } else {
                StreamCloseReason::Capacity
            };
            route.mailbox.close(route.stream, reason, false);
        }
    }
}

/// The multiplex close of a call the process closed.
const fn stream_close(reason: CallClose) -> StreamCloseReason {
    match reason {
        CallClose::Complete => StreamCloseReason::Complete,
        CallClose::Cancelled => StreamCloseReason::Cancelled,
        CallClose::Unsupported => StreamCloseReason::Unsupported,
        CallClose::InvalidRequest => StreamCloseReason::InvalidRequest,
        CallClose::Capacity => StreamCloseReason::Capacity,
        CallClose::Stopping => StreamCloseReason::Stopping,
        CallClose::OutcomeUnknown => StreamCloseReason::OutcomeUnknown,
    }
}

#[cfg(test)]
mod tests;
