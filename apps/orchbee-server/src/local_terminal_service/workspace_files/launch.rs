//! Starting a Workspace files service. The Server never starts the process itself: it binds an
//! endpoint named for a fresh ticket, asks its Worker to start the service with that ticket, and
//! admits the one connection that presents it. The endpoint lives only while the Server waits, and
//! the service's file and Git work then travels on that connection alone.
use crate::terminal_coordinator::WorkerPlacement;
use crate::worker_obcn_carrier::{WorkerOperationRequest, WorkerOperationResult};
use crate::worker_registry::WorkerRegistry;
use orchbee_capability_node_protocol::{
    OperationId, WorkspaceFilesService, WorkspaceFilesServiceOutcome, WorkspaceFilesServiceRequest,
    WorkspaceFilesTicket,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use orchbee_workbench_protocol::WorkspaceFilesFailure as Failure;
use std::io::{self, Write};
use std::net::Shutdown;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How long a started service has to connect and present its ticket.
const CONNECT_TIME: Duration = Duration::from_secs(5);
/// How often the endpoint is checked for the service's connection while the Server waits.
const ACCEPT_INTERVAL: Duration = Duration::from_millis(10);
/// How long one line written to a service may wait for it to read; a service that reads nothing
/// for this long is stuck, and its connection is ended.
const WRITE_TIME: Duration = Duration::from_secs(10);

/// Starts services through this Session's Worker, beside the Session's runtime folder.
pub(super) struct Launcher {
    workers: Arc<Mutex<WorkerRegistry>>,
    runtime: OnceLock<PathBuf>,
}

impl Launcher {
    pub(super) fn new(workers: Arc<Mutex<WorkerRegistry>>) -> Self {
        Self {
            workers,
            runtime: OnceLock::new(),
        }
    }

    /// Names the runtime folder; until then, and in a Server without one, no service starts.
    pub(super) fn configure(&self, runtime: &Path) {
        let _first = self.runtime.set(runtime.to_owned());
    }

    /// Starts `service` and returns its admitted connection. This waits for the Worker's answer and
    /// the service's connection, so no caller holds a lock another request needs meanwhile.
    pub(super) fn launch(&self, service: WorkspaceFilesService) -> Result<Connection, Failure> {
        let runtime = self.runtime.get().ok_or(Failure::Unavailable)?;
        let (operation_id, ticket) = identities().ok_or(Failure::Unavailable)?;
        let endpoint = Endpoint::bind(runtime, ticket).map_err(|_| Failure::Unavailable)?;
        // The registry is held only to read the Worker's handle, never across the request.
        let handle = self
            .workers
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .request_handle()
            .filter(|handle| handle.is_active())
            .ok_or(Failure::Unavailable)?;
        let placement = WorkerPlacement::of(&handle).ok_or(Failure::Unavailable)?;
        let request = WorkspaceFilesServiceRequest {
            operation_id,
            worker_id: placement.worker_id,
            worker_generation: placement.generation,
            service,
            ticket,
        };
        match handle.request(WorkerOperationRequest::WorkspaceFilesService(request)) {
            Ok(WorkerOperationResult::WorkspaceFilesService(result))
                if result.outcome == WorkspaceFilesServiceOutcome::Started => {}
            _ => return Err(Failure::Unavailable),
        }
        endpoint
            .admit(ticket, Instant::now() + CONNECT_TIME)
            .map_err(|_| Failure::Unavailable)
    }
}

/// The request's identity and the endpoint's ticket, both fresh.
fn identities() -> Option<(OperationId, WorkspaceFilesTicket)> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    let (operation, ticket) = bytes.split_at(16);
    Some((
        OperationId::from_bytes(operation.try_into().ok()?).ok()?,
        WorkspaceFilesTicket::from_bytes(ticket.try_into().ok()?).ok()?,
    ))
}

/// The endpoint one service connects to. On Unix its socket file outlives the listener, so it is
/// removed when the endpoint is dropped.
struct Endpoint {
    listener: LocalListener,
    #[cfg(unix)]
    path: PathBuf,
}

impl Endpoint {
    fn bind(runtime: &Path, ticket: WorkspaceFilesTicket) -> io::Result<Self> {
        let path = ModeCheckedLocalSessionRendezvous::workspace_files_socket_path(
            runtime,
            ticket.as_bytes(),
        );
        let listener = LocalListener::bind(&path)?;
        #[cfg(unix)]
        let endpoint = {
            use std::os::unix::fs::PermissionsExt;
            let endpoint = Self { listener, path };
            std::fs::set_permissions(&endpoint.path, std::fs::Permissions::from_mode(0o600))?;
            endpoint
        };
        #[cfg(not(unix))]
        let endpoint = Self { listener };
        endpoint.listener.set_nonblocking(true)?;
        Ok(endpoint)
    }

    /// Admits the first connection, by `deadline`, that this user's process opened and that
    /// presents `ticket`; any other connection is closed and the wait goes on.
    fn admit(&self, ticket: WorkspaceFilesTicket, deadline: Instant) -> io::Result<Connection> {
        loop {
            match self.listener.accept() {
                Ok((stream, _address)) => {
                    if let Ok(connection) = admitted(stream, ticket, deadline) {
                        return Ok(connection);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            std::thread::sleep(ACCEPT_INTERVAL.min(left));
        }
    }
}

#[cfg(unix)]
impl Drop for Endpoint {
    fn drop(&mut self) {
        let _removed = std::fs::remove_file(&self.path);
    }
}

fn admitted(
    stream: LocalStream,
    ticket: WorkspaceFilesTicket,
    deadline: Instant,
) -> io::Result<Connection> {
    // macOS hands the listener's nonblocking flag to the sockets it accepts.
    stream.set_nonblocking(false)?;
    require_same_effective_user(&stream)?;
    let left = deadline.saturating_duration_since(Instant::now());
    stream.set_read_timeout(Some(left.max(Duration::from_millis(1))))?;
    let presented = orchbee_workspace_files::read_hello(&mut &stream)?;
    if presented != ticket.as_bytes() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(Some(WRITE_TIME))?;
    Ok(Connection { stream })
}

/// One admitted service connection. Dropping it shuts the connection down in both directions,
/// which ends the service and wakes the thread reading its lines.
pub(super) struct Connection {
    stream: LocalStream,
}

impl Connection {
    /// Another handle that reads the service's lines.
    pub(super) fn reader(&self) -> io::Result<LocalStream> {
        self.stream.try_clone()
    }

    /// Writes one line, which ends in a newline, whole.
    pub(super) fn write_line(&self, line: &[u8]) -> io::Result<()> {
        let mut stream = &self.stream;
        stream.write_all(line)?;
        stream.flush()
    }

    /// Shuts the connection down in both directions: the service ends, and the thread reading its
    /// lines sees the end.
    pub(super) fn close(&self) {
        let _shut = self.stream.shutdown(Shutdown::Both);
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
