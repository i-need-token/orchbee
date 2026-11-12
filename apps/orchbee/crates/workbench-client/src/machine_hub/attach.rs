//! Attach or start: the one way a client reaches the hub of its data root, starting it when no
//! endpoint exists and replacing an unheld hub of another version. Concurrent callers each may
//! start a hub; the lock lets one serve and every caller connects to it.

use super::{AttachFailure, Attached, Attachment, HUB_ENTRY, Incompatible, control, holders};
use orchbee_connection_handshake::{
    AcceptorMessage, Build, ConnectionKind, InitiatorMessage, Offer, OfferFailure, ProductVersion,
    Rejection, RejectionReason, conclude_offer, encode_initiator_frame, read_acceptor_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::{
    DataRoot, RetainedChild, create_private_directory, machine_hub_endpoint, spawn_retained,
};
use orchbee_machine_protocol::{
    Connection, Delivery, FrameDecoder, Message,
    hub::{self, AttachOutcome, Reply, Request},
};
use std::{
    ffi::OsStr,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// Hubs one call may start.
const STARTS: u8 = 8;
/// The pause before connecting again.
const PAUSE: Duration = Duration::from_millis(20);
/// The pause after a `stopping` answer doubles from [`PAUSE`] up to this.
const CLOSING_PAUSE_MAX: Duration = Duration::from_millis(250);
/// How long a call waits after a hub it started exited 0, having lost the lock to another hub that
/// has not bound yet, before it starts one again; it doubles with each loss up to the maximum.
const RESTART_PAUSE: Duration = Duration::from_millis(200);
const RESTART_PAUSE_MAX: Duration = Duration::from_millis(1_600);
/// One connection attempt's bound, within the call's deadline.
pub(super) const CONNECT: Duration = Duration::from_secs(2);
/// How long a call tries again, every [`PAUSE`], after its first `capacity` answer.
const CAPACITY_RETRY: Duration = Duration::from_secs(1);
/// One read of the carrier.
pub(super) const READ_BUFFER_BYTES: usize = 64 * 1024;

/// An attached connection: its carrier, its protocol state, and whatever arrived after the reply in
/// the same reads.
pub(super) struct Link {
    pub(super) stream: LocalStream,
    pub(super) connection: Connection<hub::Hub>,
    pub(super) decoder: FrameDecoder,
    pub(super) early: Vec<Delivery<hub::Hub>>,
    pub(super) attached: Attached,
}

/// The endpoint of `root`, after creating `internal/machines/` privately.
fn endpoint(root: &DataRoot) -> Result<PathBuf, AttachFailure> {
    drop(create_private_directory(&root.machines()).map_err(|_| AttachFailure::DataRoot)?);
    machine_hub_endpoint(root).map_err(|_| AttachFailure::DataRoot)
}

/// Connects to the hub of `root`, offers `machine-hub` and attaches as `attachment`, starting the
/// hub when none runs, within `deadline`. `cancelled` ends the call early with `timeout`.
pub(super) fn attach(
    root: &DataRoot,
    attachment: Attachment,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Link, AttachFailure> {
    let endpoint = endpoint(root)?;
    let executable = std::env::current_exe().map_err(|_| AttachFailure::Unavailable)?;
    let mut child: Option<RetainedChild> = None;
    let mut starts = 0;
    let mut restart_pause = RESTART_PAUSE;
    let mut rest_until: Option<Instant> = None;
    let mut first_capacity: Option<Instant> = None;
    // The last definite answer was `stopping`, and the pause before the next attempt.
    let mut closing = false;
    let mut closing_pause = PAUSE;
    // An incompatible hub this call drained answers as before until it exits.
    let mut replaced = false;
    loop {
        if Instant::now() >= deadline || cancelled() {
            return Err(if closing {
                AttachFailure::Closing
            } else {
                AttachFailure::Timeout
            });
        }
        let mut stopping = false;
        match offer(&endpoint, deadline) {
            Ok(stream) => match exchange_attach(stream, attachment, deadline) {
                Ok((stream, connection, decoder, early, hub)) => {
                    return Ok(Link {
                        stream,
                        connection,
                        decoder,
                        early,
                        attached: Attached { hub, starts },
                    });
                }
                Err(Exchange::Stopping) => stopping = true,
                Err(Exchange::Lost) => {}
                Err(Exchange::Violation) => return Err(AttachFailure::Lost),
            },
            Err(Reached::Refused(rejection)) => match rejection.reason {
                RejectionReason::Stopping => stopping = true,
                RejectionReason::Capacity => {
                    closing = false;
                    if !capacity_retry_left(&mut first_capacity) {
                        return Err(AttachFailure::Unavailable);
                    }
                }
                RejectionReason::InitiatorTooOld
                | RejectionReason::InitiatorTooNew
                | RejectionReason::FingerprintMismatch => {
                    if !replaced {
                        replace(&endpoint, &rejection, deadline)?;
                        replaced = true;
                    }
                    stopping = true;
                }
                RejectionReason::EnvironmentMismatch => {
                    return Err(AttachFailure::Incompatible(Incompatible::Environment {
                        environment: rejection.environment.as_str().to_owned(),
                    }));
                }
                _ => return Err(AttachFailure::Incompatible(Incompatible::Protocol)),
            },
            Err(Reached::Foreign) => {
                return Err(AttachFailure::Incompatible(Incompatible::Protocol));
            }
            Err(Reached::Lost) => {}
            Err(Reached::Denied) => return Err(AttachFailure::Denied),
            Err(Reached::Nothing) => {
                closing = false;
                if let Some(running) = child.as_mut() {
                    match running.try_wait() {
                        Ok(None) => {}
                        // It lost the lock to another hub, or closed at once: connect again.
                        Ok(Some(status)) if status.success() => {
                            child = None;
                            rest_until = Some(Instant::now() + restart_pause);
                            restart_pause = (restart_pause * 2).min(RESTART_PAUSE_MAX);
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
        let pause = if stopping {
            closing = true;
            let pause = closing_pause;
            closing_pause = (closing_pause * 2).min(CLOSING_PAUSE_MAX);
            pause
        } else {
            closing_pause = PAUSE;
            PAUSE
        };
        thread::sleep(pause.min(deadline.saturating_duration_since(Instant::now())));
    }
}

/// Drains an incompatible hub of this environment when nothing holds it; otherwise reports it.
fn replace(endpoint: &Path, rejection: &Rejection, deadline: Instant) -> Result<(), AttachFailure> {
    let held = || {
        AttachFailure::Incompatible(Incompatible::Held {
            version: rejection.product_version.as_str().to_owned(),
        })
    };
    let Ok(Some(process_id)) = control::hub_process(endpoint, deadline) else {
        // Whatever cannot be proved unheld stays.
        return Err(held());
    };
    if holders::held(process_id, deadline) {
        return Err(held());
    }
    control::drain(endpoint, deadline).map_err(|_| AttachFailure::Unavailable)
}

/// What one connection attempt reached.
pub(super) enum Reached {
    /// No endpoint: no hub runs, or one has taken the lock and not yet bound.
    Nothing,
    /// The hub refused the offer.
    Refused(Rejection),
    /// The answer was not a handshake answer to this offer.
    Foreign,
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
        Ok(_) => Ok(stream),
        Err(OfferFailure::Rejected(rejection)) => Err(Reached::Refused(rejection)),
        Err(OfferFailure::InvalidAnswer | OfferFailure::UnexpectedReply) => Err(Reached::Foreign),
    }
}

pub(super) fn connect(endpoint: &Path, deadline: Instant) -> Result<LocalStream, Reached> {
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
pub(super) fn exchange(
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

/// How the `attach` request on an answered connection ended, short of `attached`.
enum Exchange {
    /// The hub answered `stopping`, sent `bye`, or closed in `closing`.
    Stopping,
    /// The carrier failed or the deadline passed; another attempt may succeed.
    Lost,
    /// Either side broke the protocol.
    Violation,
}

type Exchanged = (
    LocalStream,
    Connection<hub::Hub>,
    FrameDecoder,
    Vec<Delivery<hub::Hub>>,
    hub::HubState,
);

/// Sends `attach` on an answered connection and reads until its reply; later deliveries read with
/// it are kept for the owner.
fn exchange_attach(
    stream: LocalStream,
    attachment: Attachment,
    deadline: Instant,
) -> Result<Exchanged, Exchange> {
    let mut connection = Connection::<hub::Hub>::initiator();
    let id = connection
        .request(Request::Attach(hub::Attach {
            presence: attachment.presence,
            asks: attachment.asks,
            watch: attachment.watch,
        }))
        .map_err(|_| Exchange::Violation)?;
    let mut bytes = Vec::new();
    while let Some(frame) = connection.poll_transmit() {
        bytes.extend_from_slice(&frame.encode());
    }
    let mut output = &stream;
    output.write_all(&bytes).map_err(|_| Exchange::Lost)?;
    let mut decoder = FrameDecoder::new();
    let mut buffer = vec![0; READ_BUFFER_BYTES];
    let mut early = Vec::new();
    let mut hub = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Exchange::Lost);
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| Exchange::Lost)?;
        let mut input = &stream;
        let read = match input.read(&mut buffer) {
            Ok(0) | Err(_) => return Err(Exchange::Lost),
            Ok(read) => read,
        };
        let mut offset = 0;
        while let Some(rest) = buffer.get(offset..read).filter(|rest| !rest.is_empty()) {
            let (consumed, frame) = decoder.decode(rest).map_err(|_| Exchange::Violation)?;
            offset += consumed;
            if let Some(frame) = frame {
                let delivery = connection
                    .receive(&frame)
                    .map_err(|_| Exchange::Violation)?
                    .delivery;
                match delivery {
                    Some(Delivery::Message(Message::Reply {
                        id: answered,
                        reply: Reply::Attach(outcome),
                        ..
                    })) if answered == id => match outcome {
                        AttachOutcome::Attached { hub: state } => hub = Some(state),
                        AttachOutcome::Stopping | AttachOutcome::Unknown => {
                            return Err(Exchange::Stopping);
                        }
                    },
                    Some(Delivery::Message(Message::Bye { .. })) if hub.is_none() => {
                        return Err(Exchange::Stopping);
                    }
                    Some(delivery) if hub.is_some() => early.push(delivery),
                    _ => {}
                }
            }
            if consumed == 0 {
                break;
            }
        }
        if let Some(hub) = hub {
            stream.set_read_timeout(None).map_err(|_| Exchange::Lost)?;
            stream.set_write_timeout(None).map_err(|_| Exchange::Lost)?;
            return Ok((stream, connection, decoder, early, hub));
        }
    }
}

/// Whether a call that has just been answered `capacity` may try again: every client retries
/// within [`CAPACITY_RETRY`] of its first such answer, since a slot frees when a request ends.
pub(super) fn capacity_retry_left(first: &mut Option<Instant>) -> bool {
    first.get_or_insert_with(Instant::now).elapsed() < CAPACITY_RETRY
}

/// The pause between attempts.
pub(super) const RETRY_PAUSE: Duration = PAUSE;
