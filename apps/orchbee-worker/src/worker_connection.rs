use std::{
    error::Error,
    ffi::OsString,
    io::{self, Write},
    num::NonZeroU64,
    os::{fd::AsFd, unix::net::UnixStream},
    time::Duration,
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_site_protocol::{
    LocalSiteMessage, WorkerHello, WorkerId, read_message, write_message,
};

use crate::{host_connection::HostBrokerConnection, worker_options::WorkerOptions};

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
    let options = WorkerOptions::parse(arguments)?;
    let server = connect_to_server(&options.server_endpoint, cohort)?;
    let host = HostBrokerConnection::connect(&options.terminal_host_endpoint)?;
    write_ready_marker(&server, &host)?;
    wait_for_peer_loss(&server, &host)
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

fn wait_for_peer_loss(
    server: &ServerConnection,
    host: &HostBrokerConnection,
) -> Result<(), Box<dyn Error>> {
    loop {
        let events = {
            let mut descriptors = [
                PollFd::new(server.stream().as_fd(), PollFlags::POLLIN),
                PollFd::new(host.stream().as_fd(), PollFlags::POLLIN),
            ];
            match poll(&mut descriptors, PollTimeout::NONE) {
                Ok(_ready_count) => (descriptors[0].revents(), descriptors[1].revents()),
                Err(Errno::EINTR) => continue,
                Err(error) => return Err(error.into()),
            }
        };
        if events.0.is_none_or(|flags| !flags.is_empty())
            || events.1.is_none_or(|flags| !flags.is_empty())
        {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Worker peer connection ended or sent an unexpected post-handshake record",
            )
            .into());
        }
    }
}
