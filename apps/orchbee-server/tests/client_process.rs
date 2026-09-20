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

use orchbee_local_client_protocol::{
    ClientHello, ClientSessionConnection, ConnectClientError, LocalClientMessage,
    PROTOCOL_VERSION as CLIENT_PROTOCOL_VERSION, RejectionReason as ClientRejectionReason,
    read_message as read_client_message, write_message as write_client_message,
};
use orchbee_local_site_protocol::{
    LocalSiteMessage, RejectionReason as WorkerRejectionReason, WorkerHello, WorkerId,
    read_message as read_worker_message, write_message as write_worker_message,
};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_WORKER: WorkerId = WorkerId::from_bytes([1; 16]);
const SECOND_WORKER: WorkerId = WorkerId::from_bytes([2; 16]);
const READY_MARKER: [u8; 6] = *b"ready\n";
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[path = "client_process/terminal_operations.rs"]
mod terminal_operations;

#[path = "client_process/obcs.rs"]
mod obcs;

#[path = "client_process/tab_create.rs"]
mod tab_create;

#[path = "client_process/workbench.rs"]
mod workbench;

#[path = "client_process/workspace_directory.rs"]
mod workspace_directory;

#[test]
fn rejects_invalid_client_endpoint_invocations_without_output() -> TestResult {
    let directory = TestDirectory::new("invalid-client-endpoint")?;
    let worker_endpoint = directory.path().join("worker.sock");
    for arguments in [
        vec![
            "--local-endpoint".into(),
            worker_endpoint.as_os_str().to_owned(),
            "--client-endpoint".into(),
        ],
        vec![
            "--local-endpoint".into(),
            worker_endpoint.as_os_str().to_owned(),
            "--client-endpoint".into(),
            "relative.sock".into(),
        ],
        vec![
            "--local-endpoint".into(),
            worker_endpoint.as_os_str().to_owned(),
            "--client-endpoint".into(),
            worker_endpoint.as_os_str().to_owned(),
        ],
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
fn binds_separate_private_client_and_worker_endpoints_before_readiness() -> TestResult {
    let server = RunningServer::start()?;
    for endpoint in [server.worker_endpoint(), server.client_endpoint()] {
        let mode = fs::metadata(endpoint)?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    Ok(())
}

#[test]
fn concurrent_clients_are_independent_and_disconnect_preserves_worker() -> TestResult {
    let mut server = RunningServer::start()?;
    let worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let first = connect_client(server.client_endpoint())?;
    let mut second = connect_client(server.client_endpoint())?;
    assert!(second.generation() > first.generation());

    drop(first);
    let mut replacement = connect_client(server.client_endpoint())?;
    assert!(replacement.generation() > second.generation());
    assert_eq!(
        exchange_worker(server.worker_endpoint(), SECOND_WORKER)?,
        LocalSiteMessage::Rejected {
            reason: WorkerRejectionReason::DuplicateWorker,
        }
    );

    server.stop()?;
    second.wait_for_server_loss()?;
    replacement.wait_for_server_loss()?;
    drop(worker);
    Ok(())
}

#[test]
fn active_session_capacity_is_typed_and_released_after_an_obcs_fault() -> TestResult {
    let server = RunningServer::start()?;
    let mut sessions = Vec::new();
    for _session in 0..32 {
        sessions.push(connect_bounded_client(server.client_endpoint())?);
    }
    assert!(matches!(
        ClientSessionConnection::connect(server.client_endpoint(), env!("CARGO_PKG_VERSION")),
        Err(ConnectClientError::Rejected(
            ClientRejectionReason::CapacityExceeded
        ))
    ));

    let (mut faulted, disconnected_generation) = sessions.remove(0);
    faulted.write_all(b"XBCS\x00\x01\x00\x00")?;
    expect_connection_closed(&mut faulted)?;
    let replacement = wait_for_client_admission(server.client_endpoint())?;
    assert!(replacement.generation() > disconnected_generation);
    assert_eq!(sessions.len(), 31);
    Ok(())
}

#[test]
fn slow_duplicate_and_invalid_clients_do_not_consume_other_sessions() -> TestResult {
    let server = RunningServer::start()?;

    let mut slow = connect_candidate(server.client_endpoint())?;
    slow.write_all(b"OBLC")?;
    let ordinary = connect_client(server.client_endpoint())?;
    assert_eq!(
        read_client_message(&mut slow)?,
        LocalClientMessage::Rejected {
            reason: ClientRejectionReason::HandshakeTimeout,
        }
    );

    let mut worker_on_client = connect_candidate(server.client_endpoint())?;
    write_worker_message(
        &mut worker_on_client,
        &LocalSiteMessage::WorkerHello(WorkerHello::new(env!("CARGO_PKG_VERSION"), FIRST_WORKER)?),
    )?;
    assert_eq!(
        read_client_message(&mut worker_on_client)?,
        LocalClientMessage::Rejected {
            reason: ClientRejectionReason::MalformedFrame,
        }
    );

    let mut client_on_worker = connect_candidate(server.worker_endpoint())?;
    write_client_message(
        &mut client_on_worker,
        &LocalClientMessage::ClientHello(ClientHello::new(env!("CARGO_PKG_VERSION"))?),
    )?;
    assert_eq!(
        read_worker_message(&mut client_on_worker)?,
        LocalSiteMessage::Rejected {
            reason: WorkerRejectionReason::MalformedFrame,
        }
    );

    assert_eq!(
        exchange_client(
            server.client_endpoint(),
            LocalClientMessage::ClientHello(ClientHello::new("different-cohort")?),
        )?,
        LocalClientMessage::Rejected {
            reason: ClientRejectionReason::IncompatibleCohort,
        }
    );
    let generation = NonZeroU64::new(9).ok_or_else(|| io::Error::other("invalid generation"))?;
    assert_eq!(
        exchange_client(
            server.client_endpoint(),
            LocalClientMessage::ClientAccepted { generation },
        )?,
        LocalClientMessage::Rejected {
            reason: ClientRejectionReason::WrongMessage,
        }
    );
    assert_eq!(
        exchange_client_raw(
            server.client_endpoint(),
            &client_frame(CLIENT_PROTOCOL_VERSION + 1, &[1]),
        )?,
        LocalClientMessage::Rejected {
            reason: ClientRejectionReason::IncompatibleProtocol,
        }
    );
    let mut malformed = client_frame(CLIENT_PROTOCOL_VERSION, &[]);
    malformed[0] = b'X';
    assert_eq!(
        exchange_client_raw(server.client_endpoint(), &malformed)?,
        LocalClientMessage::Rejected {
            reason: ClientRejectionReason::MalformedFrame,
        }
    );

    let mut duplicate = connect_candidate(server.client_endpoint())?;
    let hello = LocalClientMessage::ClientHello(ClientHello::new(env!("CARGO_PKG_VERSION"))?);
    write_client_message(&mut duplicate, &hello)?;
    assert!(matches!(
        read_client_message(&mut duplicate)?,
        LocalClientMessage::ClientAccepted { .. }
    ));
    write_client_message(&mut duplicate, &hello)?;
    expect_connection_closed(&mut duplicate)?;

    let another = connect_client(server.client_endpoint())?;
    assert!(another.generation() > ordinary.generation());
    Ok(())
}

fn connect_client(endpoint: &Path) -> TestResult<ClientSessionConnection> {
    ClientSessionConnection::connect(endpoint, env!("CARGO_PKG_VERSION"))
        .map_err(|error| Box::new(error) as Box<dyn Error>)
}

fn connect_bounded_client(endpoint: &Path) -> TestResult<(UnixStream, NonZeroU64)> {
    let mut stream = connect_candidate(endpoint)?;
    write_client_message(
        &mut stream,
        &LocalClientMessage::ClientHello(ClientHello::new(env!("CARGO_PKG_VERSION"))?),
    )?;
    match read_client_message(&mut stream)? {
        LocalClientMessage::ClientAccepted { generation } => Ok((stream, generation)),
        response => Err(io::Error::other(format!(
            "expected Client admission, received {response:?}"
        ))
        .into()),
    }
}

fn wait_for_client_admission(endpoint: &Path) -> TestResult<ClientSessionConnection> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match ClientSessionConnection::connect(endpoint, env!("CARGO_PKG_VERSION")) {
            Ok(connection) => return Ok(connection),
            Err(ConnectClientError::Rejected(ClientRejectionReason::CapacityExceeded))
                if Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn exchange_client(endpoint: &Path, message: LocalClientMessage) -> TestResult<LocalClientMessage> {
    let mut stream = connect_candidate(endpoint)?;
    write_client_message(&mut stream, &message)?;
    Ok(read_client_message(&mut stream)?)
}

fn exchange_client_raw(endpoint: &Path, bytes: &[u8]) -> TestResult<LocalClientMessage> {
    let mut stream = connect_candidate(endpoint)?;
    stream.write_all(bytes)?;
    Ok(read_client_message(&mut stream)?)
}

fn connect_candidate(endpoint: &Path) -> io::Result<UnixStream> {
    let stream = UnixStream::connect(endpoint)?;
    stream.set_read_timeout(Some(PROCESS_TIMEOUT))?;
    stream.set_write_timeout(Some(PROCESS_TIMEOUT))?;
    Ok(stream)
}

fn expect_connection_closed(stream: &mut UnixStream) -> io::Result<()> {
    let mut byte = [0_u8; 1];
    match stream.read(&mut byte) {
        Ok(0) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
            ) =>
        {
            Ok(())
        }
        outcome => Err(io::Error::other(format!(
            "Client connection remained open: {outcome:?}"
        ))),
    }
}

fn client_frame(version: u16, payload: &[u8]) -> Vec<u8> {
    let payload_length = u16::try_from(payload.len()).unwrap_or(u16::MAX);
    let mut encoded = Vec::with_capacity(8 + payload.len());
    encoded.extend_from_slice(b"OBLC");
    encoded.extend_from_slice(&version.to_be_bytes());
    encoded.extend_from_slice(&payload_length.to_be_bytes());
    encoded.extend_from_slice(payload);
    encoded
}

fn connect_worker(endpoint: &Path, worker_id: WorkerId) -> TestResult<UnixStream> {
    let mut stream = connect_candidate(endpoint)?;
    write_worker_message(
        &mut stream,
        &LocalSiteMessage::WorkerHello(WorkerHello::new(env!("CARGO_PKG_VERSION"), worker_id)?),
    )?;
    match read_worker_message(&mut stream)? {
        LocalSiteMessage::WorkerAccepted { .. } => Ok(stream),
        response => Err(io::Error::other(format!(
            "expected Worker admission, received {response:?}"
        ))
        .into()),
    }
}

fn exchange_worker(endpoint: &Path, worker_id: WorkerId) -> TestResult<LocalSiteMessage> {
    let mut stream = connect_candidate(endpoint)?;
    write_worker_message(
        &mut stream,
        &LocalSiteMessage::WorkerHello(WorkerHello::new(env!("CARGO_PKG_VERSION"), worker_id)?),
    )?;
    Ok(read_worker_message(&mut stream)?)
}

struct RunningServer {
    child: Option<Child>,
    _directory: TestDirectory,
    client_endpoint: PathBuf,
    worker_endpoint: PathBuf,
}

impl RunningServer {
    fn start() -> TestResult<Self> {
        let directory = TestDirectory::new("client-server")?;
        let worker_endpoint = directory.path().join("worker.sock");
        let client_endpoint = directory.path().join("client.sock");
        let mut child = Command::new(env!("CARGO_BIN_EXE_orchbee-server"))
            .arg("--local-endpoint")
            .arg(&worker_endpoint)
            .arg("--client-endpoint")
            .arg(&client_endpoint)
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
            let _result = sender.send(result);
        });

        let mut running = Self {
            child: Some(child),
            _directory: directory,
            client_endpoint,
            worker_endpoint,
        };
        let marker = match receiver.recv_timeout(PROCESS_TIMEOUT) {
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

    fn worker_endpoint(&self) -> &Path {
        &self.worker_endpoint
    }

    fn client_endpoint(&self) -> &Path {
        &self.client_endpoint
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
        let _result = self.stop();
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
        let _result = fs::remove_dir_all(&self.0);
    }
}
