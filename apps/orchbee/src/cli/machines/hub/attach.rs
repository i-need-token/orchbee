//! Attach or start: the one way a client reaches the hub of its data root, starting it when no
//! endpoint exists. Concurrent callers each may start a hub; the lock lets one serve and every
//! caller connects to it.

use orchbee_connection_handshake::{
    AcceptorMessage, Build, ConnectionKind, ControlRequest, Extensions, InitiatorMessage, Offer,
    OfferFailure, ProductVersion, QueryResult, RejectionReason, RequestFailure, RequestKind,
    RequestReply, conclude_offer, conclude_request, encode_initiator_frame, read_acceptor_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::{
    DataRoot, RetainedChild, create_private_directory, machine_hub_endpoint, spawn_retained,
};
use std::{
    ffi::OsStr,
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// The hidden entry of the hub process.
pub(super) const HUB_ENTRY: &str = "--machine-hub-v1";
/// Hubs one call may start.
const STARTS: u8 = 8;
/// The pause before connecting again.
const PAUSE: Duration = Duration::from_millis(20);
/// How long a call waits after a hub it started exited 0, having lost the lock to another hub that
/// has not bound yet, before it starts one again; it doubles with each loss up to the maximum.
const RESTART_PAUSE: Duration = Duration::from_millis(200);
const RESTART_PAUSE_MAX: Duration = Duration::from_millis(1_600);
/// One connection attempt's bound, within the call's deadline.
const CONNECT: Duration = Duration::from_secs(2);
/// How long a call tries again, every [`PAUSE`], after its first `capacity` answer.
const CAPACITY_RETRY: Duration = Duration::from_secs(1);

/// Why a call reached no hub.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttachFailure {
    /// The data root or its `internal/machines/` folder is unavailable.
    DataRoot,
    /// The hub kept answering `capacity` for [`CAPACITY_RETRY`], or the call could not start one.
    Unavailable,
    /// The endpoint refused to open, or its server runs as another user.
    Denied,
    /// The running hub belongs to another environment or version.
    Incompatible,
    /// The deadline passed.
    Timeout,
}

/// The endpoint of this process's data root, after creating `internal/machines/` privately.
pub(super) fn endpoint() -> Result<PathBuf, AttachFailure> {
    let root =
        DataRoot::resolve(|name| std::env::var_os(name)).map_err(|_| AttachFailure::DataRoot)?;
    drop(create_private_directory(&root.machines()).map_err(|_| AttachFailure::DataRoot)?);
    machine_hub_endpoint(&root).map_err(|_| AttachFailure::DataRoot)
}

/// Connects to the hub of this process's data root and offers `machine-hub`, starting the hub when
/// none runs, within `deadline`. The answered connection carries `OBMX` next.
pub(super) fn attach_or_start(deadline: Instant) -> Result<LocalStream, AttachFailure> {
    let endpoint = endpoint()?;
    let executable = std::env::current_exe().map_err(|_| AttachFailure::Unavailable)?;
    let mut child: Option<RetainedChild> = None;
    let mut starts = 0;
    let mut pause = RESTART_PAUSE;
    let mut rest_until: Option<Instant> = None;
    let mut first_capacity: Option<Instant> = None;
    loop {
        if Instant::now() >= deadline {
            return Err(AttachFailure::Timeout);
        }
        match offer(&endpoint, deadline) {
            Ok(stream) => return Ok(stream),
            Err(Reached::Refused(RejectionReason::Stopping) | Reached::Lost) => {}
            Err(Reached::Refused(RejectionReason::Capacity)) => {
                if !capacity_retry_left(&mut first_capacity) {
                    return Err(AttachFailure::Unavailable);
                }
            }
            Err(Reached::Refused(_)) => return Err(AttachFailure::Incompatible),
            Err(Reached::Denied) => return Err(AttachFailure::Denied),
            Err(Reached::Nothing) => {
                if let Some(running) = child.as_mut() {
                    match running.try_wait() {
                        Ok(None) => {}
                        // It lost the lock to another hub, or closed at once: connect again.
                        Ok(Some(status)) if status.success() => {
                            child = None;
                            rest_until = Some(Instant::now() + pause);
                            pause = (pause * 2).min(RESTART_PAUSE_MAX);
                        }
                        Ok(Some(_)) | Err(_) => return Err(AttachFailure::Unavailable),
                    }
                }
                let rested = rest_until.is_none_or(|until| Instant::now() >= until);
                if child.is_none() && rested {
                    if starts == STARTS {
                        return Err(AttachFailure::Unavailable);
                    }
                    starts += 1;
                    child = Some(
                        spawn_retained(&executable, &[OsStr::new(HUB_ENTRY)])
                            .map_err(|_| AttachFailure::Unavailable)?,
                    );
                }
            }
        }
        thread::sleep(PAUSE);
    }
}

/// What one connection attempt reached.
enum Reached {
    /// No endpoint: no hub runs, or one has taken the lock and not yet bound.
    Nothing,
    /// The hub refused the offer.
    Refused(RejectionReason),
    /// The connection or its answer failed; another attempt may succeed.
    Lost,
    /// The endpoint refused this process: no attempt can succeed.
    Denied,
}

fn offer(endpoint: &Path, deadline: Instant) -> Result<LocalStream, Reached> {
    let stream = connect(endpoint, deadline)?;
    let offer = Offer::local(
        ConnectionKind::MachineHub,
        ProductVersion::new(env!("CARGO_PKG_VERSION"))
            .expect("the package version is printable ASCII"),
        Build::default(),
        orchbee_local_runtime::environment_name(),
    );
    let reply = exchange(&stream, &InitiatorMessage::Offer(offer.clone()), deadline)
        .map_err(|_| Reached::Lost)?;
    match conclude_offer(&offer, reply) {
        Ok(_) => stream
            .set_read_timeout(None)
            .map(|()| stream)
            .map_err(|_| Reached::Lost),
        Err(OfferFailure::Rejected(rejection)) => Err(Reached::Refused(rejection.reason)),
        Err(OfferFailure::InvalidAnswer | OfferFailure::UnexpectedReply) => {
            Err(Reached::Refused(RejectionReason::Unknown))
        }
    }
}

fn connect(endpoint: &Path, deadline: Instant) -> Result<LocalStream, Reached> {
    let bound = deadline.min(Instant::now() + CONNECT);
    LocalStream::connect_before(endpoint, bound).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => Reached::Nothing,
        // The pipe's DACL or an elevated creator's integrity label refused this process, or the
        // same-user proof found another user's server.
        io::ErrorKind::PermissionDenied => Reached::Denied,
        _ => Reached::Lost,
    })
}

/// Writes one `OBHS` frame and reads the reply within `deadline`.
fn exchange(
    stream: &LocalStream,
    message: &InitiatorMessage,
    deadline: Instant,
) -> io::Result<AcceptorMessage> {
    let frame = encode_initiator_frame(message).map_err(io::Error::other)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::ErrorKind::TimedOut.into());
    }
    stream.set_write_timeout(Some(remaining))?;
    stream.set_read_timeout(Some(remaining))?;
    let mut output = stream;
    output.write_all(&frame)?;
    let mut input = stream;
    read_acceptor_message(&mut input).map_err(io::Error::other)
}

/// The running hub's `query` answer, on a control connection of its own.
pub(super) fn query(deadline: Instant) -> Result<QueryResult, AttachFailure> {
    let endpoint = endpoint()?;
    let request = InitiatorMessage::Request(ControlRequest {
        kind: RequestKind::Query,
        environment: orchbee_local_runtime::environment_name(),
        extensions: Extensions::new(),
    });
    let mut first_capacity: Option<Instant> = None;
    loop {
        let stream = connect(&endpoint, deadline).map_err(|reached| match reached {
            Reached::Denied => AttachFailure::Denied,
            Reached::Nothing | Reached::Refused(_) | Reached::Lost => AttachFailure::Unavailable,
        })?;
        let reply =
            exchange(&stream, &request, deadline).map_err(|_| AttachFailure::Unavailable)?;
        match conclude_request(RequestKind::Query, reply) {
            Ok(RequestReply::Query(result)) => return Ok(result),
            Err(RequestFailure::Rejected(rejection))
                if rejection.reason == RejectionReason::Capacity =>
            {
                if !capacity_retry_left(&mut first_capacity) || Instant::now() + PAUSE >= deadline {
                    return Err(AttachFailure::Unavailable);
                }
            }
            Ok(_) | Err(_) => return Err(AttachFailure::Incompatible),
        }
        drop(stream);
        thread::sleep(PAUSE);
    }
}

/// Whether a call that has just been answered `capacity` may try again: every client retries
/// within [`CAPACITY_RETRY`] of its first such answer, since a slot frees when a request ends.
fn capacity_retry_left(first: &mut Option<Instant>) -> bool {
    first.get_or_insert_with(Instant::now).elapsed() < CAPACITY_RETRY
}
