use std::{
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener, net::UnixStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use orchbee_local_site_protocol::{
    LocalSiteMessage, ProtocolError, ReadMessageError, RejectionReason, read_message,
    require_same_effective_user, write_message,
};

use crate::worker_registry::{WorkerAdmission, WorkerRegistry};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn run(
    arguments: impl Iterator<Item = OsString>,
    expected_cohort: &str,
) -> io::Result<()> {
    let endpoint = parse_endpoint(arguments)?;
    let listener = UnixListener::bind(&endpoint)?;
    fs::set_permissions(&endpoint, fs::Permissions::from_mode(0o600))?;
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

fn parse_endpoint(mut arguments: impl Iterator<Item = OsString>) -> io::Result<PathBuf> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--local-endpoint")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private Server invocation",
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
