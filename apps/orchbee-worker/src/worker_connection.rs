use std::{
    ffi::OsString,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};

use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_site_protocol::{
    LocalSiteMessage, WorkerHello, WorkerId, read_message, write_message,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
const LOCAL_WORKER_ID: WorkerId = WorkerId::from_bytes(*b"orchbee-local-v1");

pub(crate) fn run(
    arguments: impl Iterator<Item = OsString>,
    cohort: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = parse_endpoint(arguments)?;
    let mut stream = UnixStream::connect(endpoint)?;
    require_same_effective_user(&stream)?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;

    let hello = WorkerHello::new(cohort, LOCAL_WORKER_ID)?;
    write_message(&mut stream, &LocalSiteMessage::WorkerHello(hello))?;
    let LocalSiteMessage::WorkerAccepted { .. } = read_message(&mut stream)? else {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Worker was rejected").into());
    };
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    write_ready_marker()?;
    wait_for_server_loss(&mut stream)
}

fn parse_endpoint(mut arguments: impl Iterator<Item = OsString>) -> io::Result<PathBuf> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--local-endpoint")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private Worker invocation",
        ));
    }
    let endpoint = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing endpoint path"))?;
    if arguments.next().is_some() || !endpoint.is_absolute() || endpoint.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid endpoint path",
        ));
    }
    Ok(endpoint)
}

fn write_ready_marker() -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"ready\n")?;
    stdout.flush()
}

fn wait_for_server_loss(stream: &mut UnixStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Server disconnected",
                )
                .into());
            }
            Ok(1) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected post-handshake message",
                )
                .into());
            }
            Ok(_count) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
}
