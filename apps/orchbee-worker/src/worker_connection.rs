use std::{
    cell::RefCell,
    error::Error,
    ffi::OsString,
    io::{self, Write},
    num::NonZeroU64,
    os::unix::net::UnixStream,
    rc::Rc,
    time::Duration,
};

use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_site_protocol::{
    LocalSiteMessage, WorkerHello, WorkerId, read_message, write_message,
};

use crate::{
    agent_drain::AgentDrain, host_connection::HostBrokerConnection, terminal_operations,
    worker_options::WorkerOptions,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
const LOCAL_WORKER_ID: WorkerId = WorkerId::from_bytes(*b"orchbee-local-v1");

struct ServerConnection {
    generation: NonZeroU64,
    stream: UnixStream,
}

impl ServerConnection {
    const fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    const fn stream(&self) -> &UnixStream {
        &self.stream
    }
}

pub(crate) fn run(
    arguments: impl Iterator<Item = OsString>,
    cohort: &str,
) -> Result<(), Box<dyn Error>> {
    let mut arguments = arguments.peekable();
    #[cfg(target_os = "linux")]
    if arguments
        .peek()
        .is_some_and(|argument| argument == "--local-site")
    {
        arguments.next();
        let runtime = arguments
            .next()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing Site runtime"))?;
        if arguments.next().is_some() {
            return Err(io::Error::from(io::ErrorKind::InvalidInput).into());
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let cohort = orchbee_local_site_runtime::NativeCohort::current(deadline)?;
        let site =
            orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous::from_runtime_directory(
                &runtime,
            )?;
        if !site.activation_started()? || !site.worker_socket_exists()? {
            return Err(io::Error::other("Site startup is unavailable").into());
        }
        let server = connect_to_server(site.worker_socket_path(), cohort.identity())?;
        return terminal_operations::run_lazy(
            server.stream(),
            LOCAL_WORKER_ID,
            server.generation(),
            &cohort,
            &runtime,
        );
    }
    let options = WorkerOptions::parse(arguments)?;
    let server = connect_to_server(&options.server_endpoint, cohort)?;
    // The Agent Drain queue belongs to this admitted Server connection and is
    // created before the Host broker so both paths share one owner.
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let mut host =
        HostBrokerConnection::connect(&options.terminal_host_endpoint, Rc::clone(&drain))?;
    write_ready_marker(&server, &host)?;
    terminal_operations::run(
        server.stream(),
        &mut host,
        LOCAL_WORKER_ID,
        server.generation(),
        &drain,
    )
}

fn connect_to_server(
    endpoint: &std::path::Path,
    cohort: &str,
) -> Result<ServerConnection, Box<dyn Error>> {
    let mut stream = UnixStream::connect(endpoint)?;
    require_same_effective_user(&stream)?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;

    let hello = WorkerHello::new(cohort, LOCAL_WORKER_ID)?;
    write_message(&mut stream, &LocalSiteMessage::WorkerHello(hello))?;
    let LocalSiteMessage::WorkerAccepted { generation } = read_message(&mut stream)? else {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Worker was rejected").into());
    };
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    Ok(ServerConnection { generation, stream })
}

fn write_ready_marker(server: &ServerConnection, host: &HostBrokerConnection) -> io::Result<()> {
    debug_assert!(server.generation().get() > 0);
    debug_assert!(host.host_epoch().as_bytes().iter().any(|byte| *byte != 0));
    let negotiated_limits = host.negotiated_limits();
    debug_assert!(negotiated_limits.frame_bytes > 0);
    debug_assert!(negotiated_limits.input_bytes > 0);
    debug_assert!(negotiated_limits.row_window_bytes > 0);
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"ready\n")?;
    stdout.flush()
}
