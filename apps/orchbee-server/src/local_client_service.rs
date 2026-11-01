//! Owns bounded local Client connection admission, same-user and cohort checks, connection
//! registration, and transfer into application service dispatch. It does not own canonical
//! Workbench transitions or infer product Session identity from a Client connection
//! generation.

use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "macos")]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use orchbee_local_client_protocol::{
    LocalClientMessage, ProtocolError, ReadMessageError, RejectionReason, read_message,
    write_message,
};
use orchbee_local_peer_credentials::require_same_effective_user;

use self::application_connection::ApplicationConnectionError;
use self::diagnostic::Diagnostic;
use crate::agent_service::{AgentPolicy, AgentService};
use crate::client_connection_registry::{ClientConnectionAdmission, ClientConnectionRegistry};

use crate::local_terminal_service::LocalTerminalService;
use crate::workbench_service::{WorkbenchPolicy, WorkbenchService};

mod application_connection;
mod diagnostic;
#[cfg(windows)]
mod managed_windows;
#[cfg(windows)]
mod metadata;
mod multiplex;
mod record_stream;
mod stop_control;
#[cfg(windows)]
pub(crate) use managed_windows::{ManagedClients, start as start_managed_windows};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
const MAXIMUM_CLIENT_CONNECTIONS: usize = 48;

pub(crate) fn start(
    endpoint: &Path,
    expected_cohort: &str,
    terminals: Arc<LocalTerminalService>,
) -> io::Result<thread::JoinHandle<io::Result<()>>> {
    let listener = UnixListener::bind(endpoint)?;
    #[cfg(unix)]
    fs::set_permissions(endpoint, fs::Permissions::from_mode(0o600))?;
    let agents = Arc::new(AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })?);
    start_listener(listener, expected_cohort, terminals, agents)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn start_managed(
    endpoint: &Path,
    expected_cohort: &str,
    terminals: Arc<LocalTerminalService>,
    agents: Arc<AgentService>,
) -> io::Result<thread::JoinHandle<io::Result<()>>> {
    start_listener(bind_managed(endpoint)?, expected_cohort, terminals, agents)
}

#[cfg(target_os = "linux")]
fn bind_managed(endpoint: &Path) -> io::Result<UnixListener> {
    use nix::fcntl::{OFlag, RenameFlags, open, renameat2};
    use orchbee_local_runtime::CLIENT_PENDING_SOCKET_NAME;
    let parent = endpoint.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let name = endpoint.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let directory = open(
        parent,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        nix::sys::stat::Mode::empty(),
    )?;
    let pending = parent.join(CLIENT_PENDING_SOCKET_NAME);
    let listener = UnixListener::bind(&pending)?;
    #[cfg(unix)]
    fs::set_permissions(&pending, fs::Permissions::from_mode(0o600))?;
    renameat2(
        &directory,
        CLIENT_PENDING_SOCKET_NAME,
        &directory,
        name,
        RenameFlags::RENAME_NOREPLACE,
    )?;
    Ok(listener)
}

#[cfg(target_os = "macos")]
fn bind_managed(endpoint: &Path) -> io::Result<UnixListener> {
    let listener = UnixListener::bind(endpoint)?;
    use nix::unistd::geteuid;

    // The private runtime directory is already mode-checked. macOS applies
    // Unix-socket permissions through chmod(path); fstat(socket_fd) does not
    // expose the pathname vnode identity, so validate the protected path.
    fs::set_permissions(endpoint, fs::Permissions::from_mode(0o600))?;
    let pathname = fs::symlink_metadata(endpoint)?;
    let invalid_reason = if !pathname.file_type().is_socket() {
        Some("managed Client path is not a socket")
    } else if pathname.uid() != geteuid().as_raw() {
        Some("managed Client socket has a different owner")
    } else if pathname.mode() & 0o777 != 0o600 {
        Some("managed Client socket permissions are not private")
    } else if pathname.nlink() != 1 {
        Some("managed Client socket has an unexpected link count")
    } else {
        None
    };
    if let Some(reason) = invalid_reason {
        return Err(io::Error::new(io::ErrorKind::InvalidData, reason));
    }
    Ok(listener)
}

fn start_listener(
    listener: UnixListener,
    expected_cohort: &str,
    terminals: Arc<LocalTerminalService>,
    agents: Arc<AgentService>,
) -> io::Result<thread::JoinHandle<io::Result<()>>> {
    terminals.bind_agent_names(&agents)?;
    let registry = Arc::new(Mutex::new(ClientConnectionRegistry::new()));
    let connection_slots = Arc::new(ConnectionSlots::new());
    let expected_cohort: Arc<str> = Arc::from(expected_cohort);
    let workbench = Arc::new(WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: true,
            create_terminal: true,
            manage_terminal_lifecycle: true,
        },
        Arc::clone(&terminals),
    ));
    thread::Builder::new()
        .name("orchbee-local-client-listener".to_owned())
        .spawn(move || {
            serve(
                listener,
                registry,
                connection_slots,
                expected_cohort,
                terminals,
                workbench,
                agents,
            )
        })
}

fn serve(
    listener: UnixListener,
    registry: Arc<Mutex<ClientConnectionRegistry>>,
    connection_slots: Arc<ConnectionSlots>,
    expected_cohort: Arc<str>,
    terminals: Arc<LocalTerminalService>,
    workbench: Arc<WorkbenchService>,
    agents: Arc<AgentService>,
) -> io::Result<()> {
    // The listener owns the canonical Agent runtime: when this loop ends,
    // the guard disposes it so already-admitted connections observe genuine
    // `runtime-unavailable` rather than fabricated state.
    let _runtime_owner = AgentRuntimeOwner::new(Arc::clone(&agents));
    loop {
        match listener.accept() {
            Ok((stream, _address)) => {
                let Some(connection_slot) = connection_slots.acquire() else {
                    continue;
                };
                let candidate_registry = Arc::clone(&registry);
                let candidate_cohort = Arc::clone(&expected_cohort);
                let candidate_terminals = Arc::clone(&terminals);
                let candidate_workbench = Arc::clone(&workbench);
                let candidate_agents = Arc::clone(&agents);
                let candidate = thread::Builder::new()
                    .name("orchbee-local-client".to_owned())
                    .spawn(move || {
                        handle_candidate(
                            stream,
                            &candidate_registry,
                            &candidate_cohort,
                            connection_slot,
                            &candidate_terminals,
                            &candidate_workbench,
                            &candidate_agents,
                        );
                    });
                // The unstarted closure drops the stream and slot, closing the candidate.
                if let Err(error) = candidate {
                    diagnostic::report(
                        &Diagnostic::new("accept", "spawn-handler").caused_by(&error),
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn handle_candidate(
    stream: UnixStream,
    registry: &Arc<Mutex<ClientConnectionRegistry>>,
    expected_cohort: &str,
    _connection_slot: ConnectionSlot,
    terminals: &LocalTerminalService,
    workbench: &WorkbenchService,
    agents: &AgentService,
) {
    handle_candidate_control(
        stream,
        registry,
        expected_cohort,
        _connection_slot,
        terminals,
        workbench,
        stop_control::StopControl::unmanaged(agents),
    );
}

fn handle_candidate_control(
    mut stream: UnixStream,
    registry: &Arc<Mutex<ClientConnectionRegistry>>,
    expected_cohort: &str,
    _connection_slot: ConnectionSlot,
    terminals: &LocalTerminalService,
    workbench: &WorkbenchService,
    control: stop_control::StopControl<'_>,
) {
    let agents = control.agents;
    if let Err(error) = stream
        .set_read_timeout(Some(HANDSHAKE_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)))
    {
        diagnostic::report(&Diagnostic::new("handshake", "transport-setup").caused_by(&error));
        return;
    }
    if require_same_effective_user(&stream).is_err() {
        diagnostic::report(&Diagnostic::new("handshake", "peer-credentials"));
        return;
    }

    let message = match read_message(&mut stream) {
        Ok(message) => message,
        Err(ReadMessageError::Protocol(ProtocolError::IncompatibleVersion { .. })) => {
            send_rejection(&mut stream, RejectionReason::IncompatibleProtocol);
            return;
        }
        Err(ReadMessageError::Io(error))
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            send_rejection(&mut stream, RejectionReason::HandshakeTimeout);
            return;
        }
        Err(ReadMessageError::Io(_) | ReadMessageError::Protocol(_)) => {
            send_rejection(&mut stream, RejectionReason::MalformedFrame);
            return;
        }
    };
    #[cfg(windows)]
    if matches!(message, LocalClientMessage::ServerMetadataRequest) {
        metadata::respond(&mut stream, expected_cohort, control.metadata_mode());
        return;
    }
    if let LocalClientMessage::SessionStopRequest(request) = message {
        if request.hello.cohort() != expected_cohort {
            send_rejection(&mut stream, RejectionReason::IncompatibleCohort);
            return;
        }
        let result = control.request(&request);
        if let Err(error) =
            write_message(&mut stream, &LocalClientMessage::SessionStopResult(result))
        {
            diagnostic::report(
                &Diagnostic::new("session-stop", "deliver-result").caused_by(&error),
            );
        }
        return;
    }
    let LocalClientMessage::ClientHello(hello) = message else {
        send_rejection(&mut stream, RejectionReason::WrongMessage);
        return;
    };
    if hello.cohort() != expected_cohort {
        send_rejection(&mut stream, RejectionReason::IncompatibleCohort);
        return;
    }
    let Some(admission_lease) = control.enter() else {
        send_rejection(&mut stream, RejectionReason::Stopping);
        return;
    };

    let admission = match registry.lock() {
        Ok(mut registry) => registry.admit(),
        Err(_poisoned) => {
            diagnostic::report(&Diagnostic::new("admission", "registry-poisoned"));
            return;
        }
    };
    let generation = match admission {
        ClientConnectionAdmission::Accepted { generation } => generation,
        ClientConnectionAdmission::CapacityExceeded => {
            send_rejection(&mut stream, RejectionReason::CapacityExceeded);
            return;
        }
        ClientConnectionAdmission::GenerationExhausted => {
            diagnostic::report(&Diagnostic::new("admission", "generation-exhausted"));
            return;
        }
    };
    let _connection_registration =
        ClientConnectionRegistration::new(Arc::clone(registry), generation);
    let workbench_connection = workbench.client_connection(generation);
    if write_message(
        &mut stream,
        &LocalClientMessage::ClientAccepted { generation },
    )
    .is_err()
    {
        return;
    }
    if stream.set_read_timeout(None).is_err() || stream.set_write_timeout(None).is_err() {
        return;
    }
    drop(admission_lease);

    let connections = std::cell::RefCell::new(std::collections::BTreeMap::new());
    let connection_result = application_connection::run_network(
        &stream,
        |lane, request| {
            connections
                .borrow_mut()
                .entry(lane)
                .or_insert_with(|| terminals.client_connection())
                .execute(request)
        },
        |lane, request| {
            workbench_connection.execute_with_terminal_lane(
                request,
                connections
                    .borrow_mut()
                    .entry(lane)
                    .or_insert_with(|| terminals.client_connection()),
                lane,
            )
        },
        |request| workbench_connection.execute(request),
        |request| match request {
            orchbee_agent_observation_protocol::AgentRequest::Changes(request) => agents
                .execute_changes_prepared(request, || terminals.prepare_agent_drain_read(agents))
                .map(orchbee_agent_observation_protocol::AgentResult::Changes),
            orchbee_agent_observation_protocol::AgentRequest::Read(request) => agents
                .execute_prepared(request, || terminals.prepare_agent_drain_read(agents))
                .map(orchbee_agent_observation_protocol::AgentResult::Read),
            orchbee_agent_observation_protocol::AgentRequest::Launch(request) => {
                Ok(orchbee_agent_observation_protocol::AgentResult::Launch(
                    agents.execute_launch_prepared(
                        request,
                        || terminals.prepare_agent_drain_read(agents),
                        || terminals.observed_commands(),
                    ),
                ))
            }
            orchbee_agent_observation_protocol::AgentRequest::Names(request) => {
                Ok(orchbee_agent_observation_protocol::AgentResult::Names(
                    terminals.execute_agent_names(agents, request),
                ))
            }
        },
        || control.enter(),
        |signal| {
            workbench_connection.watch_changes(signal);
            agents.watch_changes(signal);
        },
        |request| workbench_connection.cancel_subscription(request),
        |lane| {
            connections.borrow_mut().remove(&lane);
            workbench_connection.close_lane(lane);
        },
        || workbench_connection.maintain_subscription_deadline(),
    );
    report_connection_end(connection_result, generation);
}

/// Reports a Connection that ended on a fault; Session stop and a departed peer are ordinary ends.
fn report_connection_end(
    result: Result<(), ApplicationConnectionError>,
    generation: std::num::NonZeroU64,
) {
    let Err(error) = result else {
        return;
    };
    let failure = error.innermost();
    let departed = diagnostic::io_kind(failure).is_some_and(|kind| {
        matches!(
            kind,
            io::ErrorKind::UnexpectedEof
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::BrokenPipe
        )
    });
    if departed || matches!(failure, ApplicationConnectionError::Stopping) {
        return;
    }
    diagnostic::report(
        &Diagnostic::new("application-connection", failure.stage())
            .caused_by(failure)
            .connection(generation),
    );
}

fn send_rejection(stream: &mut UnixStream, reason: RejectionReason) {
    let _result = write_message(stream, &LocalClientMessage::Rejected { reason });
}

/// Disposes the canonical Agent runtime when the owning listener loop
/// ends; runtime disposal is the single unavailability authority.
struct AgentRuntimeOwner {
    agents: Arc<AgentService>,
}

impl AgentRuntimeOwner {
    const fn new(agents: Arc<AgentService>) -> Self {
        Self { agents }
    }
}

impl Drop for AgentRuntimeOwner {
    fn drop(&mut self) {
        self.agents.shutdown();
    }
}

struct ClientConnectionRegistration {
    registry: Arc<Mutex<ClientConnectionRegistry>>,
    generation: std::num::NonZeroU64,
}

impl ClientConnectionRegistration {
    const fn new(
        registry: Arc<Mutex<ClientConnectionRegistry>>,
        generation: std::num::NonZeroU64,
    ) -> Self {
        Self {
            registry,
            generation,
        }
    }
}

impl Drop for ClientConnectionRegistration {
    fn drop(&mut self) {
        let mut registry = match self.registry.lock() {
            Ok(registry) => registry,
            Err(poisoned) => poisoned.into_inner(),
        };
        let _was_active = registry.disconnect(self.generation);
    }
}

struct ConnectionSlots {
    in_use: Mutex<usize>,
}

impl ConnectionSlots {
    const fn new() -> Self {
        Self {
            in_use: Mutex::new(0),
        }
    }

    fn acquire(self: &Arc<Self>) -> Option<ConnectionSlot> {
        let mut in_use = self.in_use.lock().ok()?;
        if *in_use >= MAXIMUM_CLIENT_CONNECTIONS {
            return None;
        }
        *in_use += 1;
        Some(ConnectionSlot(Arc::clone(self)))
    }

    fn release(&self) {
        let mut in_use = match self.in_use.lock() {
            Ok(in_use) => in_use,
            Err(poisoned) => poisoned.into_inner(),
        };
        *in_use = in_use.saturating_sub(1);
    }
}

struct ConnectionSlot(Arc<ConnectionSlots>);

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(windows)]
fn bind_managed(endpoint: &Path) -> io::Result<UnixListener> {
    UnixListener::bind(endpoint)
}
