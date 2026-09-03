use std::{
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener, net::UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_site_protocol::{
    LocalSiteMessage, ProtocolError, ReadMessageError, RejectionReason, read_message, write_message,
};

use crate::{
    local_client_service,
    worker_registry::{WorkerAdmission, WorkerRegistry},
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn run(
    arguments: impl Iterator<Item = OsString>,
    expected_cohort: &str,
) -> io::Result<()> {
    let endpoints = parse_endpoints(arguments)?;
    let listener = UnixListener::bind(&endpoints.worker)?;
    fs::set_permissions(&endpoints.worker, fs::Permissions::from_mode(0o600))?;
    let _client_service = endpoints
        .client
        .as_deref()
        .map(|endpoint| local_client_service::start(endpoint, expected_cohort))
        .transpose()?;
    write_ready_marker()?;

    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    loop {
        match listener.accept() {
            Ok((stream, _address)) => handle_candidate(stream, &registry, expected_cohort),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

struct LocalEndpoints {
    client: Option<PathBuf>,
    worker: PathBuf,
}

fn parse_endpoints(mut arguments: impl Iterator<Item = OsString>) -> io::Result<LocalEndpoints> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--local-endpoint")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private Server invocation",
        ));
    }
    let worker = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing endpoint path"))?;
    validate_endpoint(&worker)?;

    let client = match arguments.next() {
        None => None,
        Some(flag) if flag == "--client-endpoint" => {
            let endpoint = arguments.next().map(PathBuf::from).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "missing Client endpoint path")
            })?;
            validate_endpoint(&endpoint)?;
            if endpoint == worker {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Worker and Client endpoints must differ",
                ));
            }
            Some(endpoint)
        }
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid private Server invocation",
            ));
        }
    };
    if arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private Server invocation",
        ));
    }
    Ok(LocalEndpoints { client, worker })
}

fn validate_endpoint(endpoint: &Path) -> io::Result<()> {
    if endpoint.is_absolute() && endpoint.file_name().is_some() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid endpoint path",
    ))
}

fn write_ready_marker() -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"ready\n")?;
    stdout.flush()
}

fn handle_candidate(
    mut stream: UnixStream,
    registry: &Arc<Mutex<WorkerRegistry>>,
    expected_cohort: &str,
) {
    if stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
        || require_same_effective_user(&stream).is_err()
    {
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
    let LocalSiteMessage::WorkerHello(hello) = message else {
        send_rejection(&mut stream, RejectionReason::WrongMessage);
        return;
    };
    if hello.cohort() != expected_cohort {
        send_rejection(&mut stream, RejectionReason::IncompatibleCohort);
        return;
    }

    let admission = match registry.lock() {
        Ok(mut registry) => registry.admit(hello.worker_id()),
        Err(_poisoned) => return,
    };
    let generation = match admission {
        WorkerAdmission::Accepted { generation } => generation,
        WorkerAdmission::Duplicate { .. } => {
            send_rejection(&mut stream, RejectionReason::DuplicateWorker);
            return;
        }
        WorkerAdmission::GenerationExhausted => return,
    };
    if write_message(
        &mut stream,
        &LocalSiteMessage::WorkerAccepted { generation },
    )
    .is_err()
        || stream.set_read_timeout(None).is_err()
        || stream.set_write_timeout(None).is_err()
    {
        disconnect(registry, generation);
        return;
    }

    let monitor_registry = Arc::clone(registry);
    if thread::Builder::new()
        .name("orchbee-local-worker".to_owned())
        .spawn(move || monitor_worker(stream, monitor_registry, generation))
        .is_err()
    {
        disconnect(registry, generation);
    }
}

fn send_rejection(stream: &mut UnixStream, reason: RejectionReason) {
    let _result = write_message(stream, &LocalSiteMessage::Rejected { reason });
}

fn monitor_worker(
    mut stream: UnixStream,
    registry: Arc<Mutex<WorkerRegistry>>,
    generation: std::num::NonZeroU64,
) {
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0 | 1) => break,
            Ok(_count) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_error) => break,
        }
    }
    disconnect(&registry, generation);
}

fn disconnect(registry: &Arc<Mutex<WorkerRegistry>>, generation: std::num::NonZeroU64) {
    if let Ok(mut registry) = registry.lock() {
        registry.disconnect(generation);
    }
}
