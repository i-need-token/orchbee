//! Three-thread admitted carrier: bounded input, one dispatch/output owner, one owner of reads
//! that wait on a Worker job, and event wakeups.
use super::application_connection::{ApplicationConnectionError, ApplicationRequest};
use orchbee_local_ipc::LocalStream;
use orchbee_server::SessionChangeSignal;
use orchbee_workbench_protocol::{
    MAXIMUM_MULTIPLEX_BYTES, MAXIMUM_MULTIPLEX_LANES, MAXIMUM_MULTIPLEX_STREAMS,
    MultiplexDisconnectReason, MultiplexLaneId, MultiplexMessage, MultiplexStreamId,
    MultiplexViewerId, StreamCloseReason, StreamFrameKind, StreamMode, read_multiplex_message,
    write_multiplex_message,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{self, Cursor, Read, Write},
    num::NonZeroU64,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const INPUT_RECORDS: usize = 64;
const PERSISTENT_OVERRUN: u8 = 3;

struct Claim {
    budget: Arc<AtomicUsize>,
    bytes: usize,
    streams: Option<Arc<AtomicUsize>>,
}
impl Claim {
    fn reserve(budget: &Arc<AtomicUsize>, bytes: usize) -> io::Result<Self> {
        budget
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(bytes)
                    .filter(|next| *next <= MAXIMUM_MULTIPLEX_BYTES)
            })
            .map_err(|_| io::ErrorKind::OutOfMemory)?;
        Ok(Self {
            budget: Arc::clone(budget),
            bytes,
            streams: None,
        })
    }
    fn message(
        budget: &Arc<AtomicUsize>,
        streams: &Arc<AtomicUsize>,
        message: &MultiplexMessage,
    ) -> io::Result<Self> {
        let mut claim = Self::reserve(budget, size(message).saturating_mul(3))?;
        if matches!(message, MultiplexMessage::StreamOpen { .. }) {
            streams
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                    (count < MAXIMUM_MULTIPLEX_STREAMS).then_some(count + 1)
                })
                .map_err(|_| io::ErrorKind::OutOfMemory)?;
            claim.streams = Some(Arc::clone(streams));
        }
        Ok(claim)
    }
}
impl Drop for Claim {
    fn drop(&mut self) {
        self.budget.fetch_sub(self.bytes, Ordering::AcqRel);
        if let Some(streams) = &self.streams {
            streams.fetch_sub(1, Ordering::AcqRel);
        }
    }
}
struct Queued {
    message: MultiplexMessage,
    claim: Claim,
}

#[derive(Default)]
struct Inbox {
    messages: VecDeque<Queued>,
    bytes: usize,
    closed: bool,
    failure: Option<MultiplexDisconnectReason>,
}
struct Subscription {
    lane: MultiplexLaneId,
    request: ApplicationRequest,
    continuation: ApplicationRequest,
    more: bool,
    mode: StreamMode,
    sequence: u64,
    last: Vec<u8>,
    observed: u64,
    overruns: u8,
    last_overrun: Option<Instant>,
    _request_claim: Claim,
    last_claim: Claim,
}

/// Input and frames must not queue behind a Git process, so these reads run on their own owner.
/// A one-shot read has no order against its lane; its result returns through the dispatcher.
struct JobRead {
    stream_id: MultiplexStreamId,
    request: ApplicationRequest,
    claim: Claim,
}
struct FinishedJobRead {
    stream_id: MultiplexStreamId,
    outcome: Result<Vec<u8>, ApplicationConnectionError>,
    _claim: Claim,
}
#[derive(Default)]
struct JobReadState {
    waiting: VecDeque<JobRead>,
    finished: VecDeque<FinishedJobRead>,
    closed: bool,
}
#[derive(Default)]
struct JobReads {
    state: Mutex<JobReadState>,
    ready: Condvar,
}
impl JobReads {
    fn state(&self) -> std::sync::MutexGuard<'_, JobReadState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn close(&self) {
        self.state().closed = true;
        self.ready.notify_all();
    }
    fn run(
        &self,
        signal: &SessionChangeSignal,
        execute: &impl Fn(&ApplicationRequest) -> Result<Vec<u8>, ApplicationConnectionError>,
    ) {
        loop {
            let mut state = self.state();
            let read = loop {
                if state.closed {
                    return;
                }
                if let Some(read) = state.waiting.pop_front() {
                    break read;
                }
                state = self
                    .ready
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            };
            drop(state);
            let outcome = execute(&read.request);
            self.state().finished.push_back(FinishedJobRead {
                stream_id: read.stream_id,
                outcome,
                _claim: read.claim,
            });
            signal.wake();
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each callback is a distinct Server authority the carrier must not own"
)]
pub(super) fn run(
    stream: &LocalStream,
    first: MultiplexMessage,
    signal: Arc<SessionChangeSignal>,
    mut execute: impl FnMut(
        MultiplexLaneId,
        &ApplicationRequest,
    ) -> Result<Vec<u8>, ApplicationConnectionError>,
    mut viewer_ready: impl FnMut() -> Vec<MultiplexViewerId>,
    execute_job_read: impl Fn(&ApplicationRequest) -> Result<Vec<u8>, ApplicationConnectionError> + Sync,
    mut close_lane: impl FnMut(MultiplexLaneId),
    mut cancel: impl FnMut(&ApplicationRequest),
    mut maintenance: impl FnMut() -> io::Result<Option<Instant>>,
) -> io::Result<()> {
    let inbox = Arc::new(Mutex::new(Inbox::default()));
    let budget = Arc::new(AtomicUsize::new(0));
    let streams = Arc::new(AtomicUsize::new(0));
    let first = Queued {
        claim: Claim::message(&budget, &streams, &first)?,
        message: first,
    };
    let reader = stream.try_clone()?;
    let mut writer = super::record_stream::ClientRecordStream::new(stream)?;
    let job_reads = JobReads::default();
    thread::scope(|scope| {
        let read_inbox = Arc::clone(&inbox);
        let read_signal = Arc::clone(&signal);
        let read_budget = Arc::clone(&budget);
        let reader_task = thread::Builder::new()
            .name("orchbee-client-read".into())
            .spawn_scoped(scope, move || {
                let outcome = read_loop(&reader, &read_inbox, &read_signal, &read_budget, &streams);
                let mut state = read_inbox
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.closed = true;
                if outcome.is_err() && state.failure.is_none() {
                    state.failure = Some(MultiplexDisconnectReason::InvalidStream);
                }
                drop(state);
                read_signal.wake();
            })?;
        let job_task = thread::Builder::new()
            .name("orchbee-client-job-read".into())
            .spawn_scoped(scope, || job_reads.run(&signal, &execute_job_read))?;
        let result = dispatch(
            &mut writer,
            first,
            &inbox,
            &signal,
            &job_reads,
            &mut execute,
            &mut viewer_ready,
            &mut cancel,
            &mut close_lane,
            &budget,
            &mut maintenance,
        );
        if let Err(error) = &result {
            let reason = match error.kind() {
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                    MultiplexDisconnectReason::SlowConsumer
                }
                io::ErrorKind::OutOfMemory => MultiplexDisconnectReason::Capacity,
                io::ErrorKind::InvalidData => MultiplexDisconnectReason::InvalidStream,
                _ => MultiplexDisconnectReason::Unavailable,
            };
            // The peer receives a typed reason if it resumes draining within the final bounded
            // write window; an unresponsive socket is then closed regardless of delivery.
            let _notice =
                write_multiplex_message(&mut writer, &MultiplexMessage::Disconnect { reason });
        }
        // This owner closes both before joining; no detached bridge or lane thread exists. A read
        // in progress returns by its own Worker job deadline.
        let shutdown = stream.shutdown(std::net::Shutdown::Both);
        job_reads.close();
        let joined = reader_task
            .join()
            .map_err(|_| io::Error::other("Client reader panicked"));
        let job_joined = job_task
            .join()
            .map_err(|_| io::Error::other("Client job reader panicked"));
        result.and(shutdown).and(joined).and(job_joined)
    })
}

fn size(message: &MultiplexMessage) -> usize {
    match message {
        MultiplexMessage::StreamOpen { request, .. } => request.len() + 32,
        _ => 32,
    }
}

fn read_loop(
    stream: &LocalStream,
    inbox: &Mutex<Inbox>,
    signal: &SessionChangeSignal,
    budget: &Arc<AtomicUsize>,
    streams: &Arc<AtomicUsize>,
) -> io::Result<()> {
    let mut records = super::record_stream::ClientRecordStream::new(stream)?;
    loop {
        records.begin_read();
        let message = read_multiplex_message(&mut records)?;
        if !matches!(
            message,
            MultiplexMessage::StreamOpen { .. }
                | MultiplexMessage::Resnapshot { .. }
                | MultiplexMessage::LaneOpen { .. }
                | MultiplexMessage::LaneClose { .. }
                | MultiplexMessage::StreamClose {
                    reason: StreamCloseReason::Cancelled,
                    ..
                }
        ) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut state = inbox.lock().map_err(|_| io::ErrorKind::BrokenPipe)?;
        let bytes = size(&message);
        let claim = Claim::message(budget, streams, &message);
        if state.messages.len() == INPUT_RECORDS || claim.is_err() {
            state.failure = Some(MultiplexDisconnectReason::Capacity);
            drop(state);
            signal.wake();
            return Ok(());
        }
        state.bytes += bytes;
        state.messages.push_back(Queued {
            message,
            claim: claim?,
        });
        drop(state);
        signal.wake();
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the dispatcher borrows the connection loop's callbacks and shared state without owning them"
)]
fn dispatch(
    writer: &mut impl Write,
    first: Queued,
    inbox: &Mutex<Inbox>,
    signal: &SessionChangeSignal,
    job_reads: &JobReads,
    execute: &mut impl FnMut(
        MultiplexLaneId,
        &ApplicationRequest,
    ) -> Result<Vec<u8>, ApplicationConnectionError>,
    viewer_ready: &mut impl FnMut() -> Vec<MultiplexViewerId>,
    cancel: &mut impl FnMut(&ApplicationRequest),
    close_lane: &mut impl FnMut(MultiplexLaneId),
    budget: &Arc<AtomicUsize>,
    maintenance: &mut impl FnMut() -> io::Result<Option<Instant>>,
) -> io::Result<()> {
    let mut subscriptions = BTreeMap::new();
    let mut high_water = 0;
    let mut lanes = BTreeSet::from([MultiplexLaneId::ROOT]);
    let mut lane_high_water = 1;
    let mut pending = Some(first);
    // Closing a stream or lane withdraws interest; its read finishes unobserved.
    let mut open_job_reads: BTreeMap<MultiplexStreamId, MultiplexLaneId> = BTreeMap::new();
    loop {
        let wake_revision = signal.wake_revision();
        // A hint delivered after this take wakes the signal this pass waits on below.
        for viewer_id in viewer_ready() {
            write_multiplex_message(writer, &MultiplexMessage::ViewerRecordReady { viewer_id })?;
        }
        // Take bounded work out of the mailbox before semantic execution or socket I/O.
        let (messages, failure, closed) = {
            let mut state = inbox.lock().map_err(|_| io::ErrorKind::BrokenPipe)?;
            state.bytes = 0;
            (
                std::mem::take(&mut state.messages),
                state.failure,
                state.closed,
            )
        };
        if let Some(reason) = failure {
            return write_multiplex_message(writer, &MultiplexMessage::Disconnect { reason });
        }
        if closed {
            return Ok(());
        }
        let finished = std::mem::take(&mut job_reads.state().finished);
        for read in finished {
            if open_job_reads.remove(&read.stream_id).is_none() {
                continue;
            }
            let record = read.outcome.map_err(io::Error::other)?;
            let Ok(_frame_claim) = Claim::reserve(budget, record.len().saturating_mul(3)) else {
                return write_multiplex_message(
                    writer,
                    &MultiplexMessage::Disconnect {
                        reason: MultiplexDisconnectReason::Capacity,
                    },
                );
            };
            write_multiplex_message(
                writer,
                &MultiplexMessage::StreamFrame {
                    stream_id: read.stream_id,
                    sequence: NonZeroU64::MIN,
                    kind: StreamFrameKind::Result,
                    record,
                },
            )?;
        }
        for Queued { message, claim } in pending.take().into_iter().chain(messages) {
            match message {
                MultiplexMessage::StreamOpen {
                    stream_id,
                    lane_id,
                    mode,
                    request,
                } => {
                    if stream_id.value() <= high_water || !lanes.contains(&lane_id) {
                        return write_multiplex_message(
                            writer,
                            &MultiplexMessage::Disconnect {
                                reason: MultiplexDisconnectReason::InvalidStream,
                            },
                        );
                    }
                    high_water = stream_id.value();
                    if subscriptions.len() >= MAXIMUM_MULTIPLEX_STREAMS {
                        write_multiplex_message(
                            writer,
                            &MultiplexMessage::StreamClose {
                                stream_id,
                                reason: StreamCloseReason::Capacity,
                            },
                        )?;
                        continue;
                    }
                    let parsed = ApplicationRequest::decode(&request).map_err(io::Error::other)?;
                    if !parsed.multiplex_safe()
                        || (mode != StreamMode::Request && !parsed.subscribable())
                    {
                        write_multiplex_message(
                            writer,
                            &MultiplexMessage::StreamClose {
                                stream_id,
                                reason: StreamCloseReason::Unsupported,
                            },
                        )?;
                        continue;
                    }
                    // A stream owns its event cursor until cancellation. Another stream or
                    // one-shot Read/Close must not advance or destroy that cursor underneath it.
                    if subscriptions
                        .values()
                        .any(|current: &Subscription| current.request.shares_event_cursor(&parsed))
                    {
                        write_multiplex_message(
                            writer,
                            &MultiplexMessage::StreamClose {
                                stream_id,
                                reason: StreamCloseReason::InvalidRequest,
                            },
                        )?;
                        continue;
                    }
                    // These listeners belong to the authenticated Client, not to independent
                    // foreground identities. Replacement retires the old evaluation first.
                    if mode != StreamMode::Request {
                        subscriptions.retain(|_, current: &mut Subscription| {
                            !current.request.same_subscription_owner(&parsed)
                        });
                    }
                    if mode == StreamMode::Request && parsed.waits_for_worker_job() {
                        open_job_reads.insert(stream_id, lane_id);
                        job_reads.state().waiting.push_back(JobRead {
                            stream_id,
                            request: parsed,
                            claim,
                        });
                        job_reads.ready.notify_one();
                        continue;
                    }
                    // Registration precedes capture: publication during capture changes the signal
                    // revision and forces another evaluation before this connection can sleep.
                    let captured_at = stream_revision(signal, mode, &parsed);
                    let record = match execute(lane_id, &parsed) {
                        Ok(record) => record,
                        // A drain refuses this intent alone; the connection keeps serving reads.
                        Err(ApplicationConnectionError::Draining) => {
                            write_multiplex_message(
                                writer,
                                &MultiplexMessage::StreamClose {
                                    stream_id,
                                    reason: StreamCloseReason::Stopping,
                                },
                            )?;
                            continue;
                        }
                        Err(error) => return Err(io::Error::other(error)),
                    };
                    let Ok(last_claim) = Claim::reserve(budget, record.len().saturating_mul(3))
                    else {
                        return write_multiplex_message(
                            writer,
                            &MultiplexMessage::Disconnect {
                                reason: MultiplexDisconnectReason::Capacity,
                            },
                        );
                    };
                    let kind = if mode == StreamMode::Request {
                        StreamFrameKind::Result
                    } else {
                        StreamFrameKind::Snapshot
                    };
                    write_multiplex_message(
                        writer,
                        &MultiplexMessage::StreamFrame {
                            stream_id,
                            sequence: NonZeroU64::MIN,
                            kind,
                            record: record.clone(),
                        },
                    )?;
                    if mode != StreamMode::Request {
                        subscriptions.insert(
                            stream_id,
                            Subscription {
                                lane: lane_id,
                                continuation: parsed.continuation(),
                                request: parsed,
                                more: ApplicationRequest::event_page(&record).0,
                                mode,
                                sequence: 1,
                                last: record,
                                observed: captured_at,
                                overruns: 0,
                                last_overrun: None,
                                _request_claim: claim,
                                last_claim,
                            },
                        );
                    }
                }
                MultiplexMessage::StreamClose { stream_id, .. } => {
                    open_job_reads.remove(&stream_id);
                    if let Some(removed) = subscriptions.remove(&stream_id)
                        && !subscriptions.values().any(|current| {
                            current.request.same_subscription_owner(&removed.request)
                        })
                    {
                        cancel(&removed.request);
                    }
                }
                MultiplexMessage::Resnapshot { stream_id } => {
                    if let Some(subscription) = subscriptions.get_mut(&stream_id) {
                        let now = Instant::now();
                        if subscription
                            .last_overrun
                            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(30))
                        {
                            subscription.overruns = 0;
                        }
                        subscription.last_overrun = Some(now);
                        subscription.overruns = subscription.overruns.saturating_add(1);
                        if subscription.overruns >= PERSISTENT_OVERRUN {
                            return write_multiplex_message(
                                writer,
                                &MultiplexMessage::Disconnect {
                                    reason: MultiplexDisconnectReason::SlowConsumer,
                                },
                            );
                        }
                        resnapshot(
                            writer,
                            stream_id,
                            subscription,
                            execute,
                            cancel,
                            signal,
                            budget,
                        )?;
                    }
                }
                MultiplexMessage::LaneOpen { lane_id } => {
                    if lane_id.value() <= lane_high_water || lanes.len() > MAXIMUM_MULTIPLEX_LANES {
                        return write_multiplex_message(
                            writer,
                            &MultiplexMessage::Disconnect {
                                reason: MultiplexDisconnectReason::Capacity,
                            },
                        );
                    }
                    lane_high_water = lane_id.value();
                    lanes.insert(lane_id);
                }
                MultiplexMessage::LaneClose { lane_id } => {
                    if lane_id == MultiplexLaneId::ROOT {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    if lanes.remove(&lane_id) {
                        open_job_reads.retain(|_, lane| *lane != lane_id);
                        subscriptions.retain(|_, subscription| {
                            if subscription.lane == lane_id {
                                cancel(&subscription.request);
                                false
                            } else {
                                true
                            }
                        });
                        close_lane(lane_id);
                    }
                }
                _ => return Err(io::ErrorKind::InvalidData.into()),
            }
        }
        // Maintenance publishes expired metadata before evaluating subscriptions. Its lock is
        // released before any write/wait, and reports wake this owner to recompute the deadline.
        let expiry = maintenance()?;
        let mut retained = 0usize;
        for (stream_id, subscription) in &mut subscriptions {
            // Domain journals retain ordered events and report their own cursor gaps. A commit
            // count is only invalidation: unrelated commits must never imply event loss.
            let observed = stream_revision(signal, subscription.mode, &subscription.request);
            let backlog = observed.wrapping_sub(subscription.observed);
            if backlog > 0 || subscription.more || subscription.request.requires_sampling() {
                let record = if subscription.mode == StreamMode::Invalidations {
                    observed.to_be_bytes().to_vec()
                } else {
                    execute(subscription.lane, &subscription.continuation)
                        .map_err(io::Error::other)?
                };
                let (more, gap) = ApplicationRequest::event_page(&record);
                subscription.more = more;
                if gap {
                    resnapshot(
                        writer,
                        *stream_id,
                        subscription,
                        execute,
                        cancel,
                        signal,
                        budget,
                    )?;
                    continue;
                }
                if record != subscription.last
                    || (subscription.mode == StreamMode::Events
                        && ApplicationRequest::has_event(&record))
                {
                    let Ok(last_claim) = Claim::reserve(budget, record.len().saturating_mul(3))
                    else {
                        return write_multiplex_message(
                            writer,
                            &MultiplexMessage::Disconnect {
                                reason: MultiplexDisconnectReason::Capacity,
                            },
                        );
                    };
                    subscription.sequence = subscription
                        .sequence
                        .checked_add(1)
                        .ok_or(io::ErrorKind::InvalidData)?;
                    write_multiplex_message(
                        writer,
                        &MultiplexMessage::StreamFrame {
                            stream_id: *stream_id,
                            sequence: NonZeroU64::new(subscription.sequence)
                                .ok_or(io::ErrorKind::InvalidData)?,
                            kind: StreamFrameKind::Update,
                            record: record.clone(),
                        },
                    )?;
                    subscription.last = record;
                    subscription.last_claim = last_claim;
                }
            }
            subscription.observed = observed;
            retained = retained
                .saturating_add(subscription.last.len())
                .saturating_add(subscription.request.encoded_length());
        }
        if retained > MAXIMUM_MULTIPLEX_BYTES {
            return write_multiplex_message(
                writer,
                &MultiplexMessage::Disconnect {
                    reason: MultiplexDisconnectReason::Capacity,
                },
            );
        }
        // OS/Host probes retain their existing cadence.
        let sampling = subscriptions
            .values()
            .any(|subscription| subscription.request.requires_sampling())
            .then_some(Duration::from_millis(100));
        if !subscriptions.values().any(|subscription| subscription.more) {
            let timeout = sampling
                .into_iter()
                .chain(expiry.map(|at| at.saturating_duration_since(Instant::now())))
                .min();
            signal.wait(wake_revision, timeout);
        }
    }
}

/// Invalidation streams count commits. A terminal output frame only concerns the streams that
/// deliver semantic values, so a printing terminal cannot make one Client reread its checkpoints.
/// A published-title read follows commits, which move bindings, and title changes, never output;
/// no other stream follows title changes. An Agent catalog read follows only the catalog's own
/// changes.
fn stream_revision(
    signal: &SessionChangeSignal,
    mode: StreamMode,
    request: &ApplicationRequest,
) -> u64 {
    if request.follows_agent_catalog() {
        signal.agent_catalog_revision()
    } else if request.follows_titles() {
        signal
            .commit_revision()
            .wrapping_add(signal.titles_revision())
    } else if mode == StreamMode::Invalidations {
        signal.commit_revision()
    } else {
        signal.revision()
    }
}

fn resnapshot(
    writer: &mut impl Write,
    stream_id: MultiplexStreamId,
    subscription: &mut Subscription,
    execute: &mut impl FnMut(
        MultiplexLaneId,
        &ApplicationRequest,
    ) -> Result<Vec<u8>, ApplicationConnectionError>,
    cancel: &mut impl FnMut(&ApplicationRequest),
    signal: &SessionChangeSignal,
    budget: &Arc<AtomicUsize>,
) -> io::Result<()> {
    write_multiplex_message(
        writer,
        &MultiplexMessage::Gap {
            stream_id,
            through: NonZeroU64::new(subscription.sequence).ok_or(io::ErrorKind::InvalidData)?,
        },
    )?;
    // The event journal owns its cursor. Reset only this subscription, then capture its new
    // baseline while the connection's commit signal remains registered.
    if matches!(
        subscription.request.workbench(),
        Some(orchbee_workbench_protocol::WorkbenchRequest::Events(_))
    ) {
        cancel(&subscription.request);
    }
    let observed = stream_revision(signal, subscription.mode, &subscription.request);
    let fresh = subscription.request.fresh_snapshot();
    let record = execute(subscription.lane, &fresh).map_err(io::Error::other)?;
    let last_claim = Claim::reserve(budget, record.len().saturating_mul(3))?;
    subscription.sequence = subscription
        .sequence
        .checked_add(1)
        .ok_or(io::ErrorKind::InvalidData)?;
    write_multiplex_message(
        writer,
        &MultiplexMessage::StreamFrame {
            stream_id,
            sequence: NonZeroU64::new(subscription.sequence).ok_or(io::ErrorKind::InvalidData)?,
            kind: StreamFrameKind::Snapshot,
            record: record.clone(),
        },
    )?;
    subscription.continuation = fresh.continuation();
    subscription.more = ApplicationRequest::event_page(&record).0;
    subscription.last = record;
    subscription.last_claim = last_claim;
    subscription.observed = observed;
    Ok(())
}

pub(super) struct PrefixedStream<'a, S> {
    pub(super) prefix: Cursor<Vec<u8>>,
    pub(super) stream: &'a mut S,
}
impl<S: Read> Read for PrefixedStream<'_, S> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.prefix.position() < self.prefix.get_ref().len() as u64 {
            self.prefix.read(buffer)
        } else {
            self.stream.read(buffer)
        }
    }
}
impl<S: Write> Write for PrefixedStream<'_, S> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.write(buffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[cfg(test)]
mod tests;
