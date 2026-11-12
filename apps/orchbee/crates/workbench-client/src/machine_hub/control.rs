//! Control requests to the hub, `query`, `drain` and `exit`, each on a connection of its own, and the
//! one path that ends a hub on purpose.

use super::attach::{self, Reached};
use orchbee_connection_handshake::{
    ControlRequest, Extensions, InitiatorMessage, ProcessRole, RejectionReason, RequestFailure,
    RequestKind, RequestReply, conclude_request,
};
use orchbee_local_runtime::{DataRoot, machine_hub_endpoint};
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};
use winsafe::{self as w, co};

/// How long `end` waits for a drained hub to exit before it sends `exit`.
const DRAIN_WAIT: Duration = Duration::from_secs(10);
/// How long `end` waits after `exit`.
const EXIT_WAIT: Duration = Duration::from_secs(5);
/// Hubs one `end` ends, counting any a client started while it ran.
const ROUNDS: usize = 3;

/// Why `running` or `end` could not finish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EndFailure {
    /// The endpoint refused this process, or its server runs as another user.
    Denied,
    /// A build of another environment serves this data root and refuses control requests.
    Foreign {
        /// That build's environment name.
        environment: String,
    },
    /// The hub answered outside the contract or kept answering `capacity`.
    Unavailable,
    /// The deadline passed with the hub still running.
    Timeout {
        /// The running hub's process id, when known.
        process_id: Option<u32>,
    },
}

/// The process id of the hub running for `root`, through `query`; `None` when none runs. It starts
/// nothing and creates nothing.
///
/// # Errors
///
/// [`EndFailure`] when a hub answers but its process id cannot be read.
pub fn running(root: &DataRoot, deadline: Instant) -> Result<Option<u32>, EndFailure> {
    let Ok(endpoint) = machine_hub_endpoint(root) else {
        // A data root that does not exist yet has no hub.
        return Ok(None);
    };
    hub_process(&endpoint, deadline)
}

/// Ends the hub running for `root`: `drain`, a wait for the process to exit, then `exit`. A hub that
/// a client started meanwhile is ended the same way. Returns the last hub ended, or `None` when
/// none ran; once it returns, no hub's program file is held by that process.
///
/// # Errors
///
/// [`EndFailure`] when a hub refuses or outlasts `deadline`.
pub fn end(root: &DataRoot, deadline: Instant) -> Result<Option<u32>, EndFailure> {
    let Ok(endpoint) = machine_hub_endpoint(root) else {
        return Ok(None);
    };
    let mut ended = None;
    for _ in 0..ROUNDS {
        let Some(process_id) = hub_process(&endpoint, deadline)? else {
            return Ok(ended);
        };
        let Ok(process) = w::HPROCESS::OpenProcess(
            co::PROCESS::SYNCHRONIZE | co::PROCESS::QUERY_LIMITED_INFORMATION,
            false,
            process_id,
        ) else {
            // Gone already, or not ours to wait for: the next query tells.
            pause(deadline)?;
            continue;
        };
        // The same id once the handle is open proves the handle is the hub's, not a reused id's.
        match hub_process(&endpoint, deadline)? {
            Some(again) if again == process_id => {}
            Some(_) => continue,
            None => {
                ended = Some(process_id);
                continue;
            }
        }
        let _drained = request(&endpoint, RequestKind::Drain, deadline);
        if !exited(&process, deadline.min(Instant::now() + DRAIN_WAIT)) {
            let _exited = request(&endpoint, RequestKind::Exit, deadline);
            if !exited(&process, deadline.min(Instant::now() + EXIT_WAIT)) {
                return Err(EndFailure::Timeout {
                    process_id: Some(process_id),
                });
            }
        }
        ended = Some(process_id);
    }
    match hub_process(&endpoint, deadline)? {
        None => Ok(ended),
        Some(process_id) => Err(EndFailure::Timeout {
            process_id: Some(process_id),
        }),
    }
}

/// Asks a hub to enter `closing`.
pub(super) fn drain(endpoint: &Path, deadline: Instant) -> Result<(), EndFailure> {
    request(endpoint, RequestKind::Drain, deadline).map(drop)
}

/// The process id the hub at `endpoint` reports; `None` when no endpoint exists.
pub(super) fn hub_process(endpoint: &Path, deadline: Instant) -> Result<Option<u32>, EndFailure> {
    match request(endpoint, RequestKind::Query, deadline) {
        Ok(Some(RequestReply::Query(result))) => result
            .processes()
            .first()
            .filter(|process| process.role == ProcessRole::MachineHub)
            .map(|process| Some(process.pid.get()))
            .ok_or(EndFailure::Unavailable),
        Ok(None) => Ok(None),
        Ok(Some(_)) => Err(EndFailure::Unavailable),
        Err(failure) => Err(failure),
    }
}

/// One control request, retrying `capacity` as every client does; `None` when no endpoint exists.
fn request(
    endpoint: &Path,
    kind: RequestKind,
    deadline: Instant,
) -> Result<Option<RequestReply>, EndFailure> {
    let message = InitiatorMessage::Request(ControlRequest {
        kind,
        environment: orchbee_local_runtime::environment_name(),
        extensions: Extensions::new(),
    });
    let mut first_capacity: Option<Instant> = None;
    loop {
        let stream = match attach::connect(endpoint, deadline) {
            Ok(stream) => stream,
            Err(Reached::Nothing) => return Ok(None),
            Err(Reached::Denied) => return Err(EndFailure::Denied),
            Err(_) if Instant::now() < deadline => {
                pause(deadline)?;
                continue;
            }
            Err(_) => return Err(EndFailure::Timeout { process_id: None }),
        };
        let Ok(reply) = attach::exchange(&stream, &message, deadline) else {
            // `exit` ends the hub as it answers, and a closing hub may go before it answers.
            if matches!(kind, RequestKind::Exit | RequestKind::Drain) {
                return Ok(None);
            }
            pause(deadline)?;
            continue;
        };
        match conclude_request(kind, reply) {
            Ok(reply) => return Ok(Some(reply)),
            Err(RequestFailure::Rejected(rejection)) => match rejection.reason {
                RejectionReason::Capacity => {
                    if !attach::capacity_retry_left(&mut first_capacity) {
                        return Err(EndFailure::Unavailable);
                    }
                }
                RejectionReason::EnvironmentMismatch => {
                    return Err(EndFailure::Foreign {
                        environment: rejection.environment.as_str().to_owned(),
                    });
                }
                _ => return Err(EndFailure::Unavailable),
            },
            Err(_) => return Err(EndFailure::Unavailable),
        }
        drop(stream);
        pause(deadline)?;
    }
}

/// Sleeps the retry pause, or fails when the deadline has passed.
fn pause(deadline: Instant) -> Result<(), EndFailure> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(EndFailure::Timeout { process_id: None });
    }
    thread::sleep(attach::RETRY_PAUSE.min(remaining));
    Ok(())
}

/// Whether `process` exited by `until`.
fn exited(process: &w::HPROCESS, until: Instant) -> bool {
    loop {
        let remaining = until.saturating_duration_since(Instant::now());
        let milliseconds = u32::try_from(remaining.as_millis()).unwrap_or(u32::MAX);
        match process.WaitForSingleObject(Some(milliseconds)) {
            Ok(co::WAIT::OBJECT_0) => return true,
            Ok(_) if remaining.is_zero() => return false,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}
