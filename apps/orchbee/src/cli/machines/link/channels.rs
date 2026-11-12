//! The workers that serve the link's channels. A `session` channel relays a Session's Client
//! endpoint through two workers, a `route` channel runs one routed command after the opener's
//! `end`, and an `ensure-session` channel starts a Session. Workers commit link data only below a
//! per-channel mark and wait on their channel's signal.

use super::state::{End, Entry, Shared, State, Work};
use crate::{
    cli::machines::routing,
    local_activation::{self, ActivationCause, StartupFailure},
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::SessionName;
use orchbee_machine_protocol::{Delivery, MAXIMUM_DATA_PAYLOAD_BYTES, Message, ResetReason, link};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    sync::{
        Arc, Condvar, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Channels the table keeps, counting closed ones whose workers still settle; beyond it an `open`
/// is reset `unavailable`.
const MAXIMUM_ENTRIES: usize = 512;
/// A channel's workers commit link data only while fewer of its bytes wait to be written.
const UNWRITTEN_MARK: usize = 64 * 1024;
/// One read of a Session endpoint, and the most one commit adds.
const CHUNK_BYTES: usize = MAXIMUM_DATA_PAYLOAD_BYTES;
/// How long a `session` channel waits for its Session's endpoint to take the connection.
const CONNECT_WINDOW: Duration = Duration::from_secs(20);
/// How long an `ensure-session` channel waits for its Session to report ready.
const START_WINDOW: Duration = Duration::from_secs(30);

/// Applies one delivery of the state machine.
pub(super) fn deliver(link: &Arc<Shared>, state: &mut State, delivery: Delivery<link::Link>) {
    match delivery {
        Delivery::Open { channel, open, .. } => open_channel(link, state, channel, open),
        Delivery::Data { channel, bytes } => {
            let length = bytes.len();
            if let Some(entry) = state.channels.live(channel) {
                match entry.work {
                    Work::Session => {
                        entry.inbound.push_back(bytes);
                        entry.signal.notify_all();
                    }
                    Work::Route => {
                        entry.request.extend_from_slice(&bytes);
                        let _taken = state.connection.take(channel, length);
                    }
                    Work::Start => {}
                }
            }
        }
        Delivery::End { channel } => {
            let Some(entry) = state.channels.live(channel) else {
                return;
            };
            match entry.work {
                Work::Session => {
                    entry.opener_ended = true;
                    entry.signal.notify_all();
                }
                Work::Route => start_route(link, state, channel),
                Work::Start => {}
            }
        }
        Delivery::Writable { channel } => state.channels.signal(channel),
        Delivery::Reset { channel, .. } | Delivery::TooLarge { channel } => {
            state.channels.finish(channel);
        }
        Delivery::Message(Message::Bye { .. }) => state.end(End::Hub),
        // The hub accepts and closes nothing here, and the state machine answers its requests.
        Delivery::Message(_)
        | Delivery::Accept { .. }
        | Delivery::Close { .. }
        | Delivery::Pong => {}
    }
}

fn open_channel(link: &Arc<Shared>, state: &mut State, channel: u32, open: link::Channel) {
    if state.channels.len() >= MAXIMUM_ENTRIES {
        let _reset = state.connection.reset(channel, ResetReason::Unavailable);
        return;
    }
    match open {
        link::Channel::Session { session } => {
            let Ok(name) = SessionName::parse(session.as_str()) else {
                let _reset = state.connection.reset(channel, ResetReason::InvalidSession);
                return;
            };
            let worker = link.clone();
            start_worker(state, channel, Entry::new(Work::Session), move || {
                relay_endpoint(&worker, channel, &name);
            });
        }
        link::Channel::Route => {
            if state.connection.accept(channel).is_ok() {
                state.channels.insert(channel, Entry::new(Work::Route));
            }
        }
        link::Channel::EnsureSession { session } => {
            let Ok(name) = SessionName::parse(session.as_str()) else {
                let _reset = state.connection.reset(channel, ResetReason::InvalidSession);
                return;
            };
            if state.connection.accept(channel).is_err() {
                return;
            }
            let worker = link.clone();
            start_worker(state, channel, Entry::new(Work::Start), move || {
                start_session(&worker, channel, &name);
            });
        }
    }
}

/// Records `entry` with one worker running `work`, or resets the channel `unavailable` when no
/// thread starts.
fn start_worker(
    state: &mut State,
    channel: u32,
    mut entry: Entry,
    work: impl FnOnce() + Send + 'static,
) {
    entry.workers = 1;
    state.channels.insert(channel, entry);
    if thread::Builder::new()
        .name("orchbee-link-channel".into())
        .spawn(work)
        .is_err()
    {
        state.channels.remove(channel);
        let _reset = state.connection.reset(channel, ResetReason::Unavailable);
    }
}

fn wait<'a>(signal: &Condvar, state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
    signal.wait(state).unwrap_or_else(PoisonError::into_inner)
}

/// Commits `bytes` as `channel`'s data in pieces of at most one chunk, each only while fewer than
/// the mark's bytes of the channel wait to be written and within the hub's credit. False when the
/// channel ended first.
fn commit(link: &Shared, channel: u32, bytes: &[u8]) -> bool {
    let mut offset = 0;
    let mut state = link.lock();
    loop {
        let Some(entry) = state.channels.live(channel) else {
            return false;
        };
        let signal = entry.signal.clone();
        let Some(rest) = bytes.get(offset..).filter(|rest| !rest.is_empty()) else {
            return true;
        };
        if state.connection.queued_bytes(channel) < UNWRITTEN_MARK {
            let piece = rest.get(..CHUNK_BYTES).unwrap_or(rest);
            match state.connection.write(channel, piece) {
                Ok(0) => {}
                Ok(written) => {
                    offset += written;
                    link.wake();
                    continue;
                }
                Err(_) => return false,
            }
        }
        state = wait(&signal, state);
    }
}

/// Closes `channel` with `outcome` unless it already ended, then ends it at this end.
fn settle(state: &mut State, channel: u32, outcome: link::Outcome) {
    if state.channels.live(channel).is_some() {
        let _closed = state.connection.close(channel, outcome);
    }
    state.channels.finish(channel);
}

/// The `session` worker that connects and then relays the endpoint's bytes to the link.
fn relay_endpoint(link: &Arc<Shared>, channel: u32, name: &SessionName) {
    let stream = match connect(name) {
        Ok(stream) => stream,
        Err(reason) => {
            let mut state = link.lock();
            if state.channels.live(channel).is_some() {
                let _reset = state.connection.reset(channel, reason);
            }
            state.channels.release(channel);
            link.wake();
            return;
        }
    };
    if !adopt(link, channel, &stream) {
        let _closed = stream.shutdown(Shutdown::Both);
        link.lock().channels.release(channel);
        link.wake();
        return;
    }
    let mut buffer = vec![0; CHUNK_BYTES];
    let outcome = loop {
        match (&stream).read(&mut buffer) {
            Ok(0) => break link::Outcome::Ended,
            Ok(read) => {
                if !buffer
                    .get(..read)
                    .is_some_and(|bytes| commit(link, channel, bytes))
                {
                    break link::Outcome::Ended;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break link::Outcome::Failed,
        }
    };
    let mut state = link.lock();
    settle(&mut state, channel, outcome);
    state.channels.release(channel);
    link.wake();
}

/// Accepts the connected channel and starts its second worker, which writes the hub's bytes to
/// the endpoint. False when the channel ended while connecting or no thread starts.
fn adopt(link: &Arc<Shared>, channel: u32, stream: &LocalStream) -> bool {
    let Ok(writer) = stream.try_clone() else {
        return false;
    };
    let Ok(closer) = stream.try_clone() else {
        return false;
    };
    let mut state = link.lock();
    if state.channels.live(channel).is_none() || state.connection.accept(channel).is_err() {
        return false;
    }
    let worker = link.clone();
    let started = thread::Builder::new()
        .name("orchbee-link-channel".into())
        .spawn(move || relay_link(&worker, channel, &writer));
    let Some(entry) = state.channels.live(channel) else {
        return false;
    };
    entry.endpoint = Some(closer);
    if started.is_ok() {
        entry.workers += 1;
    } else {
        let _reset = state.connection.reset(channel, ResetReason::Unavailable);
        state.channels.finish(channel);
    }
    link.wake();
    started.is_ok()
}

/// Connects to `name`'s Client endpoint as a local Client of the same user would.
fn connect(name: &SessionName) -> Result<LocalStream, ResetReason> {
    let reason = |error: io::Error| match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => {
            ResetReason::SessionNotRunning
        }
        _ => ResetReason::Unavailable,
    };
    let endpoint = local_activation::session_client_endpoint(name).map_err(reason)?;
    let stream =
        LocalStream::connect_before(&endpoint, Instant::now() + CONNECT_WINDOW).map_err(reason)?;
    stream
        .require_same_user()
        .map_err(|_| ResetReason::Unavailable)?;
    Ok(stream)
}

enum Step {
    Write(Vec<u8>),
    OpenerEnded,
    Gone,
}

/// The `session` worker that writes the hub's bytes to the endpoint and returns credit for them.
fn relay_link(link: &Shared, channel: u32, mut stream: &LocalStream) {
    loop {
        let step = {
            let mut state = link.lock();
            loop {
                let Some(entry) = state.channels.live(channel) else {
                    break Step::Gone;
                };
                if let Some(bytes) = entry.inbound.pop_front() {
                    break Step::Write(bytes);
                }
                if entry.opener_ended {
                    break Step::OpenerEnded;
                }
                let signal = entry.signal.clone();
                state = wait(&signal, state);
            }
        };
        match step {
            Step::Write(bytes) => {
                if let Err(error) = stream.write_all(&bytes) {
                    // A Session that closed its end is the other worker's `ended`; any other
                    // write error is the endpoint's I/O failure.
                    if error.kind() == io::ErrorKind::BrokenPipe {
                        let _closed = stream.shutdown(Shutdown::Both);
                    } else {
                        settle(&mut link.lock(), channel, link::Outcome::Failed);
                    }
                    break;
                }
                let _taken = link.lock().connection.take(channel, bytes.len());
                link.wake();
            }
            // Everything the hub sent is delivered: closing the endpoint ends the other worker's
            // read, which closes the channel `ended`.
            Step::OpenerEnded => {
                let _closed = stream.shutdown(Shutdown::Both);
                break;
            }
            Step::Gone => break,
        }
    }
    link.lock().channels.release(channel);
    link.wake();
}

/// Takes `channel`'s request packet and starts its command, or resets it `unavailable`.
fn start_route(link: &Arc<Shared>, state: &mut State, channel: u32) {
    let Some(entry) = state.channels.live(channel) else {
        return;
    };
    let request = std::mem::take(&mut entry.request);
    let cancel = entry.cancel.clone();
    entry.workers = 1;
    let worker = link.clone();
    if thread::Builder::new()
        .name("orchbee-link-route".into())
        .spawn(move || run_route(&worker, channel, &request, &cancel))
        .is_err()
    {
        let _reset = state.connection.reset(channel, ResetReason::Unavailable);
        state.channels.release(channel);
    }
}

/// Runs one routed command, ending it when its channel or the link ends, and sends its reply.
fn run_route(link: &Shared, channel: u32, request: &[u8], cancel: &AtomicBool) {
    let reply = routing::answer_packet(request, || cancel.load(Ordering::Acquire));
    if commit(link, channel, &reply) {
        settle(&mut link.lock(), channel, link::Outcome::Replied);
    }
    link.lock().channels.release(channel);
    link.wake();
}

/// Starts `name` as `server start` would and closes the channel with what became of it.
fn start_session(link: &Shared, channel: u32, name: &SessionName) {
    let outcome =
        match local_activation::activate_named_session(name, Instant::now() + START_WINDOW) {
            // The admitted Client only proved readiness; dropping it disconnects it.
            Ok(started) => link::Outcome::Ready {
                started: started.spawned,
            },
            // A start that ran out of time, or whose Server may have been spawned, is unconfirmed.
            Err(failure)
                if matches!(failure.failure, StartupFailure::TimedOut)
                    || matches!(failure.cause, Some(ActivationCause::SpawnIo)) =>
            {
                link::Outcome::Unknown
            }
            Err(_) => link::Outcome::Failed,
        };
    let mut state = link.lock();
    settle(&mut state, channel, outcome);
    state.channels.release(channel);
    link.wake();
}
