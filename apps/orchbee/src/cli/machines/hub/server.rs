//! The hub process: the lock before the endpoint, the listener, the admission of each candidate
//! under the candidate deadline, the supervisor that owns the expiry and the Session reads, and the
//! closing that ends with `bye`, the endpoint closed and the lock released.

use super::super::shared_carrier::{acceptor_profile, answer, query};
use super::{
    catalog::Machines,
    environment::Environment,
    lifetime::{Lifetime, Moment, ReadDue},
    peer,
    state::{CANDIDATES, CONTROLS, Hub, Line, PEERS, Shared},
};
use orchbee_connection_handshake::{
    AcceptorLoad, AcceptorMessage, AcceptorProfile, CANDIDATE_DEADLINE, ConnectionKind, Decision,
    DrainResult, ExitResult, Extensions, ProcessRole, ProcessState, ReadError, RejectionReason,
    RequestKind, read_initiator_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::{DataRoot, create_private_directory, machine_hub_endpoint};
use orchbee_machine_protocol::ByeReason;
use orchbee_workbench_client::application::machine_hub;
use std::{
    fs::{File, TryLockError},
    io::{self, Read},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// The status after closing, or when another hub holds the lock.
const CLOSED: u8 = 0;
/// The status when the hub cannot serve: no data root, no lock, or no endpoint.
const CANNOT_SERVE: u8 = 1;
/// The expiry, and the bound of the development override.
const EXPIRY: Duration = Duration::from_secs(600);
/// One Session read covers every Session within this.
const SESSION_READ: Duration = Duration::from_secs(5);
/// How long a name the hub cannot take is retried.
const BIND_RETRY: Duration = Duration::from_secs(2);
const RETRY_PAUSE: Duration = Duration::from_millis(20);
/// The listener's pause between accept attempts.
const ACCEPT_PAUSE: Duration = Duration::from_millis(10);
/// How long closing waits for catalog reads in flight.
const ALLOWANCE: Duration = Duration::from_secs(60);
/// How long closing lets the writers finish with `bye`.
const FAREWELL: Duration = Duration::from_secs(2);
/// A bound on writing one refusal to a candidate.
const REFUSAL_WRITE: Duration = Duration::from_secs(1);
/// How long the supervisor sleeps with nothing due.
const IDLE_WAKE: Duration = Duration::from_secs(60);

/// Runs the hub until it closes and returns its exit status.
pub(super) fn serve() -> u8 {
    end_on_panic();
    let environment = Environment::take();
    let Ok(root) = DataRoot::resolve(|name| environment.get(name)) else {
        return CANNOT_SERVE;
    };
    let lock = match take_lock(&root) {
        Ok(Some(lock)) => lock,
        Ok(None) => return CLOSED,
        Err(_) => return CANNOT_SERVE,
    };
    let Ok(endpoint) = machine_hub_endpoint(&root) else {
        return CANNOT_SERVE;
    };
    // Bound at once, so a starting client finds the endpoint instead of starting more hubs; its
    // connection waits in the pipe until the listener accepts it after the reads below.
    let Some(listener) = bind(&endpoint) else {
        return CANNOT_SERVE;
    };
    environment.start_test_child();
    let machines = Machines::read(&root).unwrap_or_else(|_| Machines::empty());
    let base = machine_hub::runtime_base(|name| environment.get(name));
    let live = any_live(base.as_deref());
    let lifetime = Lifetime::start(environment.expiry().unwrap_or(EXPIRY), live, Moment::now());
    let shared = Arc::new(Shared::new(Hub::new(root, lifetime, machines)));
    let profile = Arc::new(acceptor_profile(ConnectionKind::MachineHub));
    let stop = Arc::new(AtomicBool::new(false));
    let listening = {
        let (shared, profile, stop) = (shared.clone(), profile.clone(), stop.clone());
        thread::Builder::new()
            .name("orchbee-hub-listener".into())
            .spawn(move || listen(&shared, &profile, &listener, &stop))
    };
    let Ok(listening) = listening else {
        return CANNOT_SERVE;
    };
    supervise(&shared, base.as_deref());
    close(&shared);
    stop.store(true, Ordering::Release);
    // The endpoint closes before the lock is released, so a successor binds a free name.
    let _listener_ended = listening.join();
    drop(lock);
    CLOSED
}

/// Whether any Session in the runtime base lives, within the read's bound.
fn any_live(base: Option<&Path>) -> bool {
    base.is_some_and(|base| machine_hub::any_live(base, Instant::now() + SESSION_READ))
}

/// Makes a panic in any of the hub's threads end the process after the usual report, as a kill
/// would. Unwinding one thread would leave the rest serving wrongly: a listener's panic drops the
/// endpoint while the lock stays held, and a reader's leaves its `presence` counted for ever.
fn end_on_panic() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        report(panic);
        std::process::abort();
    }));
}

/// Opens `hub.lock` and takes it, or `None` when another hub holds it. The file is created when
/// missing and never removed; the directories that pin it are released once it is open.
fn take_lock(root: &DataRoot) -> io::Result<Option<File>> {
    let pins = create_private_directory(&root.machines())?;
    let path = root.machine_hub_lock();
    let file = match orchbee_local_runtime::private_filesystem::open_file(&path, false, true) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match orchbee_local_runtime::private_filesystem::open_file(&path, true, true) {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    orchbee_local_runtime::private_filesystem::open_file(&path, false, true)
                }
                opened => opened,
            }
        }
        opened => opened,
    }?;
    drop(pins);
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// Binds the endpoint, retrying a name a killed predecessor's pipe still holds for a moment.
fn bind(endpoint: &Path) -> Option<LocalListener> {
    let deadline = Instant::now() + BIND_RETRY;
    loop {
        match LocalListener::bind(endpoint) {
            Ok(listener) => return listener.set_nonblocking(true).ok().map(|()| listener),
            Err(_) if Instant::now() < deadline => thread::sleep(RETRY_PAUSE),
            Err(_) => return None,
        }
    }
}

/// Accepts connections until closing ends; each proves its user before a byte is read.
fn listen(
    shared: &Arc<Shared>,
    profile: &Arc<AcceptorProfile>,
    listener: &LocalListener,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, ())) => admit(shared, profile, stream),
            // Nothing waits, or a client failed the same-user proof and was dropped unread.
            Err(_) => thread::sleep(ACCEPT_PAUSE),
        }
    }
}

/// Hands an accepted connection to its own admission thread, or refuses it `capacity` unread.
fn admit(shared: &Arc<Shared>, profile: &Arc<AcceptorProfile>, stream: LocalStream) {
    let accepted = Instant::now();
    {
        let mut hub = shared.lock();
        if hub.candidates >= CANDIDATES {
            drop(hub);
            refuse(&stream, profile, RejectionReason::Capacity);
            return;
        }
        hub.candidates += 1;
    }
    let started = {
        let (shared, profile) = (shared.clone(), profile.clone());
        thread::Builder::new()
            .name("orchbee-hub-candidate".into())
            .spawn(move || candidate(&shared, &profile, stream, accepted))
    };
    if started.is_err() {
        shared.lock().candidates -= 1;
    }
}

fn refuse(stream: &LocalStream, profile: &AcceptorProfile, reason: RejectionReason) {
    let _bounded = stream.set_write_timeout(Some(REFUSAL_WRITE));
    answer(
        &mut &*stream,
        &AcceptorMessage::Rejection(profile.reject(reason)),
    );
}

/// Reads one `OBHS` frame within the candidate deadline and answers it; an admitted connection's
/// reader then runs on this thread.
fn candidate(
    shared: &Arc<Shared>,
    profile: &AcceptorProfile,
    stream: LocalStream,
    accepted: Instant,
) {
    let admitted = decide(shared, profile, &stream, accepted + CANDIDATE_DEADLINE);
    shared.lock().candidates -= 1;
    if let Some((id, line)) = admitted {
        peer::read(shared, &line, id, &stream);
    }
}

fn decide(
    shared: &Arc<Shared>,
    profile: &AcceptorProfile,
    stream: &LocalStream,
    deadline: Instant,
) -> Option<(u64, Arc<Line>)> {
    let message = match read_initiator_message(&mut Deadlined { stream, deadline }) {
        Ok(message) => message,
        Err(ReadError::Closed) => return None,
        Err(ReadError::Io(_)) => {
            refuse(stream, profile, RejectionReason::Timeout);
            return None;
        }
        Err(ReadError::ForeignProtocol | ReadError::Malformed(_)) => {
            refuse(stream, profile, RejectionReason::Malformed);
            return None;
        }
    };
    let mut hub = shared.lock();
    let state = if hub.lifetime.is_closing() {
        ProcessState::Stopping
    } else {
        ProcessState::Ready
    };
    let load = AcceptorLoad {
        session_slot_free: hub.peers.len() < PEERS,
        control_slot_free: hub.controls < CONTROLS,
        state,
    };
    // The slot is taken with the decision, so closing finds every admitted connection.
    let reply = match profile.decide(&message, load) {
        Decision::Accept(reply) => {
            // The reader waits without a bound; the candidate deadline ends here.
            if stream.set_read_timeout(None).is_err() {
                return None;
            }
            let (id, line) = hub.register(stream.clone());
            drop(hub);
            if answer(&mut &*stream, &AcceptorMessage::Answer(reply))
                && peer::start_writer(shared, &line, id, stream.clone())
            {
                return Some((id, line));
            }
            let mut hub = shared.lock();
            hub.close(id);
            hub.release(id);
            hub.release(id);
            shared.wake();
            return None;
        }
        Decision::Reject(rejection) => AcceptorMessage::Rejection(rejection),
        Decision::Unsupported(reply) => AcceptorMessage::Unsupported(reply),
        Decision::Serve(kind) => {
            hub.controls += 1;
            let reply = control(shared, &mut hub, profile, kind, state);
            drop(hub);
            answer(&mut &*stream, &reply);
            if matches!(kind, RequestKind::Exit) {
                // `exit` ends the process at once; the system releases the lock and the endpoint.
                std::process::exit(i32::from(CLOSED));
            }
            shared.lock().controls -= 1;
            return None;
        }
    };
    drop(hub);
    answer(&mut &*stream, &reply);
    None
}

fn control(
    shared: &Shared,
    hub: &mut Hub,
    profile: &AcceptorProfile,
    kind: RequestKind,
    state: ProcessState,
) -> AcceptorMessage {
    match kind {
        RequestKind::Query => query(profile, ProcessRole::MachineHub, state).map_or_else(
            || AcceptorMessage::Rejection(profile.reject(RejectionReason::Capacity)),
            AcceptorMessage::QueryResult,
        ),
        RequestKind::Drain => {
            if hub.lifetime.drain() {
                hub.announce();
                shared.wake();
            }
            AcceptorMessage::DrainResult(DrainResult {
                state: ProcessState::Stopping,
                extensions: Extensions::new(),
            })
        }
        RequestKind::Exit => AcceptorMessage::ExitResult(ExitResult::default()),
    }
}

/// A candidate's reads, each bounded by what remains of the candidate deadline.
struct Deadlined<'a> {
    stream: &'a LocalStream,
    deadline: Instant,
}

impl Read for Deadlined<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.stream.set_read_timeout(Some(remaining))?;
        let mut stream = self.stream;
        stream.read(buffer)
    }
}

/// Owns the Session reads, the one at the expiry deadline included, until the hub enters `closing`.
fn supervise(shared: &Shared, base: Option<&Path>) {
    let mut hub = shared.lock();
    loop {
        if hub.lifetime.is_closing() {
            return;
        }
        let now = Moment::now();
        let due = hub.lifetime.read_due();
        let read_now = match due {
            Some(ReadDue::Now) => true,
            Some(ReadDue::At(at)) => at <= now.at,
            None => false,
        };
        if read_now {
            let ticket = hub.lifetime.begin_read();
            drop(hub);
            let live = any_live(base);
            hub = shared.lock();
            if hub.lifetime.finish_read(ticket, live, Moment::now()) {
                hub.announce();
            }
            continue;
        }
        let wake_at = match due {
            Some(ReadDue::At(at)) => at,
            _ => now.at + IDLE_WAKE,
        };
        hub = shared.wait_until(hub, wake_at);
    }
}

/// Lets catalog reads in flight finish, then sends `bye` `stopping` on every connection and gives
/// the writers their bound.
fn close(shared: &Shared) {
    let mut hub = shared.lock();
    let allowance = Instant::now() + ALLOWANCE;
    while hub.reloads > 0 && Instant::now() < allowance {
        hub = shared.wait_until(hub, allowance);
    }
    for peer in hub.peers.values() {
        let mut wire = peer.line.lock();
        if wire.open {
            let _farewell = wire.connection.bye(ByeReason::Stopping);
            drop(wire);
            peer.line.wake();
        }
    }
    let farewell = Instant::now() + FAREWELL;
    while hub.peers.values().any(|peer| !peer.writer_done) && Instant::now() < farewell {
        hub = shared.wait_until(hub, farewell);
    }
}
