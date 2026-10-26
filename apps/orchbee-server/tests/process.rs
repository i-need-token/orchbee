#![cfg(unix)]

use std::{
    error::Error,
    fs,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_local_worker_protocol::{
    LocalWorkerMessage, PROTOCOL_VERSION, RejectionReason, WorkerHello, WorkerId, read_message,
    write_message,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_WORKER: WorkerId = WorkerId::from_bytes([1; 16]);
const SECOND_WORKER: WorkerId = WorkerId::from_bytes([2; 16]);
const READY_MARKER: [u8; 6] = *b"ready\n";
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn rejects_every_invalid_private_invocation_without_output() -> TestResult {
    for arguments in [
        Vec::<&str>::new(),
        vec!["--version"],
        vec!["--local-endpoint"],
        vec!["--local-endpoint", "relative.sock"],
        vec!["--local-endpoint", "/tmp/orchbee.sock", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_orchbee-server"))
            .args(arguments)
            .output()?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    Ok(())
}

#[test]
fn binds_one_private_listener_with_exact_readiness_and_permissions() -> TestResult {
    let server = RunningServer::start()?;
    let mode = fs::metadata(server.endpoint())?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    let collision = Command::new(env!("CARGO_BIN_EXE_orchbee-server"))
        .arg("--local-endpoint")
        .arg(server.endpoint())
        .output()?;
    assert_eq!(collision.status.code(), Some(1));
    assert!(collision.stdout.is_empty());
    assert!(collision.stderr.is_empty());
    Ok(())
}

#[test]
fn admits_one_worker_replaces_it_after_disconnect_and_closes_it_on_server_loss() -> TestResult {
    let mut server = RunningServer::start()?;
    let (first_stream, first_generation) =
        connect_worker(server.endpoint(), env!("CARGO_PKG_VERSION"), FIRST_WORKER)?;

    assert_eq!(
        exchange(
            server.endpoint(),
            LocalWorkerMessage::WorkerHello(WorkerHello::new(
                env!("CARGO_PKG_VERSION"),
                SECOND_WORKER,
            )?),
        )?,
        LocalWorkerMessage::Rejected {
            reason: RejectionReason::DuplicateWorker,
        }
    );

    drop(first_stream);
    let (mut replacement_stream, replacement_generation) =
        wait_for_worker_admission(server.endpoint(), env!("CARGO_PKG_VERSION"), SECOND_WORKER)?;
    assert!(replacement_generation > first_generation);

    server
        .stop()
        .map_err(|error| io::Error::new(error.kind(), format!("failed to stop Server: {error}")))?;
    let mut byte = [0_u8; 1];
    match replacement_stream.read(&mut byte) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
            ) => {}
        outcome => {
            return Err(io::Error::other(format!(
                "Worker connection remained usable after Server loss: {outcome:?}"
            ))
            .into());
        }
    }
    Ok(())
}

#[test]
fn rejects_bounded_invalid_candidates_without_consuming_worker_authority() -> TestResult {
    let server = RunningServer::start()?;
    assert_eq!(
        exchange(
            server.endpoint(),
            LocalWorkerMessage::WorkerHello(WorkerHello::new("different-cohort", FIRST_WORKER)?),
        )?,
        LocalWorkerMessage::Rejected {
            reason: RejectionReason::IncompatibleCohort,
        }
    );

    let generation =
        NonZeroU64::new(9).ok_or_else(|| io::Error::other("test generation must be nonzero"))?;
    assert_eq!(
        exchange(
            server.endpoint(),
            LocalWorkerMessage::WorkerAccepted { generation },
        )?,
        LocalWorkerMessage::Rejected {
            reason: RejectionReason::WrongMessage,
        }
    );

    assert_eq!(
        exchange_raw(server.endpoint(), &frame(PROTOCOL_VERSION + 1, &[1]),)?,
        LocalWorkerMessage::Rejected {
            reason: RejectionReason::IncompatibleProtocol,
        }
    );

    let mut malformed = frame(PROTOCOL_VERSION, &[]);
    malformed[0] = b'X';
    assert_eq!(
        exchange_raw(server.endpoint(), &malformed)?,
        LocalWorkerMessage::Rejected {
            reason: RejectionReason::MalformedFrame,
        }
    );

    let mut silent_candidate = connect_candidate(server.endpoint())?;
    assert_eq!(
        read_message(&mut silent_candidate)?,
        LocalWorkerMessage::Rejected {
            reason: RejectionReason::HandshakeTimeout,
        }
    );

    let (_worker, admitted_generation) =
        connect_worker(server.endpoint(), env!("CARGO_PKG_VERSION"), FIRST_WORKER)?;
    assert_eq!(admitted_generation, NonZeroU64::MIN);
    Ok(())
}

fn connect_worker(
    endpoint: &Path,
    cohort: &str,
    worker_id: WorkerId,
) -> TestResult<(UnixStream, NonZeroU64)> {
    let mut stream = connect_candidate(endpoint)?;
    write_message(
        &mut stream,
        &LocalWorkerMessage::WorkerHello(WorkerHello::new(cohort, worker_id)?),
    )?;
    match read_message(&mut stream)? {
        LocalWorkerMessage::WorkerAccepted { generation } => Ok((stream, generation)),
        response => Err(io::Error::other(format!(
            "expected Worker admission, received {response:?}"
        ))
        .into()),
    }
}

fn wait_for_worker_admission(
    endpoint: &Path,
    cohort: &str,
    worker_id: WorkerId,
) -> TestResult<(UnixStream, NonZeroU64)> {
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    loop {
        let mut stream = connect_candidate(endpoint)?;
        write_message(
            &mut stream,
            &LocalWorkerMessage::WorkerHello(WorkerHello::new(cohort, worker_id)?),
        )?;
        match read_message(&mut stream)? {
            LocalWorkerMessage::WorkerAccepted { generation } => return Ok((stream, generation)),
            LocalWorkerMessage::Rejected {
                reason: RejectionReason::DuplicateWorker,
            } if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            response => {
                return Err(io::Error::other(format!(
                    "replacement Worker was not admitted: {response:?}"
                ))
                .into());
            }
        }
    }
}

fn exchange(endpoint: &Path, message: LocalWorkerMessage) -> TestResult<LocalWorkerMessage> {
    let mut stream = connect_candidate(endpoint)?;
    write_message(&mut stream, &message)?;
    Ok(read_message(&mut stream)?)
}

fn exchange_raw(endpoint: &Path, bytes: &[u8]) -> TestResult<LocalWorkerMessage> {
    let mut stream = connect_candidate(endpoint)?;
    stream.write_all(bytes)?;
    Ok(read_message(&mut stream)?)
}

fn connect_candidate(endpoint: &Path) -> io::Result<UnixStream> {
    let stream = UnixStream::connect(endpoint)?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
    Ok(stream)
}

fn frame(version: u16, payload: &[u8]) -> Vec<u8> {
    let payload_length = u16::try_from(payload.len()).unwrap_or(u16::MAX);
    let mut encoded = Vec::with_capacity(8 + payload.len());
    encoded.extend_from_slice(b"OBLW");
    encoded.extend_from_slice(&version.to_be_bytes());
    encoded.extend_from_slice(&payload_length.to_be_bytes());
    encoded.extend_from_slice(payload);
    encoded
}

struct RunningServer {
    child: Option<Child>,
    _directory: TestDirectory,
    endpoint: PathBuf,
}

impl RunningServer {
    fn start() -> TestResult<Self> {
        let directory = TestDirectory::new("server")?;
        let endpoint = directory.path().join("site.sock");
        let mut child = Command::new(env!("CARGO_BIN_EXE_orchbee-server"))
            .arg("--local-endpoint")
            .arg(&endpoint)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Server stdout pipe is unavailable"))?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut marker = [0_u8; READY_MARKER.len()];
            let result = stdout
                .read_exact(&mut marker)
                .map(|()| marker)
                .map_err(|error| error.to_string());
            let _ignored = sender.send(result);
        });

        let mut running = Self {
            child: Some(child),
            _directory: directory,
            endpoint,
        };
        let marker = match receiver.recv_timeout(HANDSHAKE_TIMEOUT) {
            Ok(Ok(marker)) => marker,
            Ok(Err(message)) => {
                running.stop()?;
                return Err(io::Error::other(message).into());
            }
            Err(error) => {
                running.stop()?;
                return Err(io::Error::other(format!(
                    "timed out waiting for Server readiness: {error}"
                ))
                .into());
            }
        };
        if marker != READY_MARKER {
            running.stop()?;
            return Err(io::Error::other(format!(
                "unexpected Server readiness marker: {marker:?}"
            ))
            .into());
        }
        Ok(running)
    }

    fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    fn stop(&mut self) -> io::Result<()> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        if child.try_wait()?.is_none() {
            child.kill()?;
        }
        let _status = child.wait()?;
        Ok(())
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        let _ignored = self.stop();
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchbee-{label}-{}-{fixture_id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique process-test directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}
