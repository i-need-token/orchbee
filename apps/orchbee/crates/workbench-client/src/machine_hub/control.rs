//! Control requests to the hub, `query`, `drain` and `exit`, each on a connection of its own, the
//! one path that ends a hub on purpose, and watching a hub until it goes.

use super::attach::{self, Reached};
use orchbee_connection_handshake::{
    ControlRequest, Extensions, InitiatorMessage, ProcessRole, RejectionReason, RequestFailure,
    RequestKind, RequestReply, conclude_request,
};
use orchbee_local_runtime::{DataRoot, machine_hub_endpoint};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use winsafe::{self as w, co, guard::CloseHandleGuard};

/// How long `end` waits for a drained hub to exit before it sends `exit`.
const DRAIN_WAIT: Duration = Duration::from_secs(10);
/// How long `end` waits after `exit`.
const EXIT_WAIT: Duration = Duration::from_secs(5);
/// Hubs one `end` ends, counting any a client started while it ran.
const ROUNDS: usize = 3;
/// How often a [`HubWatch`] without the hub's process looks at its endpoint.
const LOOK: Duration = Duration::from_secs(1);
/// How long that look's `query` may take, so a watch still answers its caller about once a look.
const LOOK_QUERY: Duration = Duration::from_millis(500);

/// Why `running` or `end` could not finish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EndFailure {
    /// The endpoint refused this process, or its server runs as another user; for `end`, also a
    /// hub process this one may not wait for, such as an elevated one.
    Denied,
    /// A build of another environment serves this data root and refuses control requests.
    Foreign {
        /// That build's environment name.
        environment: String,
    },
    /// The hub answered outside the contract or kept answering `capacity`; for `end`, also a hub
    /// process that could not be opened for another reason.
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

/// Ends the hub running for `root`: `drain`, a wait for the process to exit, then `exit`. An owner
/// whose hub this ends starts none in reaction, so a hub found afterwards was started by a client
/// that was starting anyway, and it is ended the same way. Returns the last hub ended, or `None`
/// when none ran; once it returns, no hub's program file is held by that process.
///
/// # Errors
///
/// [`EndFailure`] when a hub refuses, cannot be waited for, or outlasts `deadline`.
pub fn end(root: &DataRoot, deadline: Instant) -> Result<Option<u32>, EndFailure> {
    let Ok(endpoint) = machine_hub_endpoint(root) else {
        return Ok(None);
    };
    let mut ended = None;
    for _ in 0..ROUNDS {
        let Some(process_id) = hub_process(&endpoint, deadline)? else {
            return Ok(ended);
        };
        let process = match w::HPROCESS::OpenProcess(
            co::PROCESS::SYNCHRONIZE | co::PROCESS::QUERY_LIMITED_INFORMATION,
            false,
            process_id,
        ) {
            Ok(process) => process,
            // Gone already: the next query tells.
            Err(error) if error == co::ERROR::INVALID_PARAMETER => {
                pause(deadline)?;
                continue;
            }
            // An elevated hub, or one of another user: without waiting for its exit this process
            // could not tell when its program file is free, so it leaves the hub as it is.
            Err(error) if error == co::ERROR::ACCESS_DENIED => return Err(EndFailure::Denied),
            Err(_) => return Err(EndFailure::Unavailable),
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

/// The hub serving a data root when the watch began, watched until it goes: through its process
/// when a `query` names it, which costs nothing while it runs, otherwise through its endpoint. It
/// starts nothing and creates nothing.
pub struct HubWatch {
    endpoint: Option<PathBuf>,
    process: Option<CloseHandleGuard<w::HPROCESS>>,
    /// The process id the hub named when the watch began, if it named one.
    named: Option<u32>,
    /// When a watch without the process looks at the endpoint next.
    look: Cell<Instant>,
}

impl HubWatch {
    /// Watches the hub serving `root` now; the query that names its process ends by `deadline`.
    pub fn new(root: &DataRoot, deadline: Instant) -> Self {
        let look = Cell::new(Instant::now());
        let Ok(endpoint) = machine_hub_endpoint(root) else {
            return Self {
                endpoint: None,
                process: None,
                named: None,
                look,
            };
        };
        let named = hub_process(&endpoint, deadline).ok().flatten();
        let process = named.and_then(|process_id| {
            let process =
                w::HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, process_id).ok()?;
            // The same id once the handle is open proves the handle is the hub's.
            (hub_process(&endpoint, deadline).ok().flatten() == Some(process_id)).then_some(process)
        });
        Self {
            endpoint: Some(endpoint),
            process,
            named,
            look,
        }
    }

    /// Waits until `until` for the hub to go; true once it is gone. Without its process it looks at
    /// the endpoint every [`LOOK`]: a missing name, or a `query` that names another process than
    /// the watched hub named, is a later hub's. A later hub that refuses `query` as the watched one
    /// did reads as this one.
    pub fn gone(&self, until: Instant) -> bool {
        match (&self.process, &self.endpoint) {
            (Some(process), _) => exited(process, until),
            (None, Some(endpoint)) => {
                if Instant::now() >= self.look.get() {
                    self.look.set(Instant::now() + LOOK);
                    if !orchbee_local_ipc::endpoint_exists(endpoint).unwrap_or(true) {
                        return true;
                    }
                    let now = hub_process(endpoint, Instant::now() + LOOK_QUERY)
                        .ok()
                        .flatten();
                    if now.is_some() && now != self.named {
                        return true;
                    }
                }
                thread::sleep(until.saturating_duration_since(Instant::now()));
                false
            }
            (None, None) => true,
        }
    }
}

/// Whether a hub serves `root` now, by its endpoint's name alone: nothing is opened or created.
pub fn present(root: &DataRoot) -> bool {
    machine_hub_endpoint(root)
        .is_ok_and(|endpoint| orchbee_local_ipc::endpoint_exists(&endpoint).unwrap_or(false))
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
