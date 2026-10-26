#![cfg(unix)]

use std::{
    error::Error,
    fs,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_worker_protocol::{
    LocalWorkerMessage, RejectionReason, WorkerHello, WorkerId, read_message, write_message,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ClientId, ConnectionControl, CorrelationId, Envelope, Hello,
    HelloAccepted, HostEpoch, MAXIMUM_TRANSPORT_RECORD_BYTES, MessageRole, PayloadEncoding,
    TRANSPORT_ENVELOPE_HEADER_BYTES, decode_connection_control, decode_transport_envelope,
    encode_connection_control, encode_transport_envelope,
};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(5);
const HOST_ENDPOINT_ID: &str = "1234567890abcdef1234567890abcdef";
const WORKER_CLIENT_ID_BYTES: [u8; 16] = *b"orchbee-worker-1";
const LOCAL_WORKER_ID: WorkerId = WorkerId::from_bytes(*b"orchbee-local-v1");
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[path = "process/terminal_operations.rs"]
mod terminal_operations;

#[test]
fn rejects_every_invalid_private_invocation_without_output() -> TestResult {
    let valid_server = "/tmp/orchbee-session.sock";
    let valid_runtime = "/tmp/orchbee-host";
    for arguments in [
        Vec::<&str>::new(),
        vec!["--version"],
        vec!["--local-endpoint"],
        vec!["--local-endpoint", valid_server],
        vec![
            "--local-endpoint",
            "relative.sock",
            "--terminal-host-runtime-directory",
            valid_runtime,
            "--terminal-host-endpoint-id",
            HOST_ENDPOINT_ID,
        ],
        vec![
            "--local-endpoint",
            valid_server,
            "--terminal-host-runtime-directory",
            "relative",
            "--terminal-host-endpoint-id",
            HOST_ENDPOINT_ID,
        ],
        vec![
            "--local-endpoint",
            valid_server,
            "--terminal-host-runtime-directory",
            valid_runtime,
            "--terminal-host-endpoint-id",
            "1234567890ABCDEF1234567890ABCDEF",
        ],
        vec![
            "--local-endpoint",
            valid_server,
            "--terminal-host-runtime-directory",
            valid_runtime,
            "--terminal-host-endpoint-id",
            HOST_ENDPOINT_ID,
            "extra",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_orchbee-worker"))
            .args(arguments)
            .output()?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    Ok(())
}

#[test]
fn server_rejection_prevents_any_terminal_host_contact() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let host_listener = fixture.bind_host_listener()?;
    let server_listener = fixture.server_listener.try_clone()?;
    let server = thread::spawn(move || -> io::Result<WorkerHello> {
        let mut stream = accept_with_deadline(&server_listener)?;
        require_same_effective_user(&stream)?;
        configure_peer_stream(&stream)?;
        let hello = read_worker_hello(&mut stream)?;
        write_message(
            &mut stream,
            &LocalWorkerMessage::Rejected {
                reason: RejectionReason::IncompatibleCohort,
            },
        )
        .map_err(protocol_io)?;
        Ok(hello)
    });

    let output = RunningWorker::start(&fixture)?.finish()?;
    let hello = join_peer(server, "fake Server")?;
    assert_worker_identity(&hello);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(
        matches!(host_listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock)
    );
    Ok(())
}

#[test]
fn terminal_host_loss_exits_worker_and_disconnects_server() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let host_listener = fixture.bind_host_listener()?;
    let server_listener = fixture.server_listener.try_clone()?;
    let server = thread::spawn(move || -> io::Result<WorkerHello> {
        let (mut stream, hello) = admit_worker(&server_listener)?;
        require_connection_closed(&mut stream)?;
        Ok(hello)
    });
    let (release_host, released_host) = mpsc::channel();
    let host = thread::spawn(move || -> io::Result<Hello> {
        let mut stream = accept_with_deadline(&host_listener)?;
        require_same_effective_user(&stream)?;
        configure_peer_stream(&stream)?;
        let (hello, correlation_id) = read_host_hello(&mut stream)?;
        write_hello_accepted(&mut stream, &hello, correlation_id)?;
        released_host
            .recv_timeout(PROCESS_TIMEOUT)
            .map_err(channel_timeout)?;
        Ok(hello)
    });

    let worker = RunningWorker::start(&fixture)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    release_host
        .send(())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let output = worker.finish()?;
    let server_hello = join_peer(server, "fake Server")?;
    let host_hello = join_peer(host, "fake Terminal Host")?;
    assert_worker_identity(&server_hello);
    assert_host_hello(&host_hello)?;
    assert_failed_after_exact_readiness(output);
    Ok(())
}

#[test]
fn server_loss_exits_worker_and_closes_terminal_host_broker() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let host_listener = fixture.bind_host_listener()?;
    let server_listener = fixture.server_listener.try_clone()?;
    let (release_server, released_server) = mpsc::channel();
    let server = thread::spawn(move || -> io::Result<WorkerHello> {
        let (stream, hello) = admit_worker(&server_listener)?;
        released_server
            .recv_timeout(PROCESS_TIMEOUT)
            .map_err(channel_timeout)?;
        drop(stream);
        Ok(hello)
    });
    let host = thread::spawn(move || -> io::Result<Hello> {
        let mut stream = accept_with_deadline(&host_listener)?;
        require_same_effective_user(&stream)?;
        configure_peer_stream(&stream)?;
        let (hello, correlation_id) = read_host_hello(&mut stream)?;
        write_hello_accepted(&mut stream, &hello, correlation_id)?;
        require_connection_closed(&mut stream)?;
        Ok(hello)
    });

    let worker = RunningWorker::start(&fixture)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    release_server
        .send(())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let output = worker.finish()?;
    let server_hello = join_peer(server, "fake Server")?;
    let host_hello = join_peer(host, "fake Terminal Host")?;
    assert_worker_identity(&server_hello);
    assert_host_hello(&host_hello)?;
    assert_failed_after_exact_readiness(output);
    Ok(())
}

#[test]
fn rejects_wrong_terminal_host_correlation_without_readiness() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let host_listener = fixture.bind_host_listener()?;
    let server_listener = fixture.server_listener.try_clone()?;
    let server = thread::spawn(move || -> io::Result<WorkerHello> {
        let (mut stream, hello) = admit_worker(&server_listener)?;
        require_connection_closed(&mut stream)?;
        Ok(hello)
    });
    let host = thread::spawn(move || -> io::Result<Hello> {
        let mut stream = accept_with_deadline(&host_listener)?;
        require_same_effective_user(&stream)?;
        configure_peer_stream(&stream)?;
        let (hello, _correlation_id) = read_host_hello(&mut stream)?;
        let wrong_correlation = CorrelationId::from_bytes([0x55; 16]).map_err(protocol_io)?;
        write_hello_accepted(&mut stream, &hello, wrong_correlation)?;
        require_connection_closed(&mut stream)?;
        Ok(hello)
    });

    let output = RunningWorker::start(&fixture)?.finish()?;
    let server_hello = join_peer(server, "fake Server")?;
    let host_hello = join_peer(host, "fake Terminal Host")?;
    assert_worker_identity(&server_hello);
    assert_host_hello(&host_hello)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn real_worker_and_zig_host_complete_the_production_handshake_and_cleanup() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let mut host = RunningHost::start(&fixture)?;
    wait_for_socket(&fixture.host_endpoint())?;
    let server_listener = fixture.server_listener.try_clone()?;
    let (release_server, released_server) = mpsc::channel();
    let server = thread::spawn(move || -> io::Result<WorkerHello> {
        let (stream, hello) = admit_worker(&server_listener)?;
        released_server
            .recv_timeout(PROCESS_TIMEOUT)
            .map_err(channel_timeout)?;
        drop(stream);
        Ok(hello)
    });

    let worker = RunningWorker::start(&fixture)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    release_server
        .send(())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let worker_output = worker.finish()?;
    let server_hello = join_peer(server, "fake Server")?;
    let host_output = host.finish()?;

    assert_worker_identity(&server_hello);
    assert_failed_after_exact_readiness(worker_output);
    assert!(host_output.status.success());
    assert!(host_output.stdout.is_empty());
    assert!(host_output.stderr.is_empty());
    assert!(!fixture.host_endpoint().exists());
    Ok(())
}

fn admit_worker(listener: &UnixListener) -> io::Result<(UnixStream, WorkerHello)> {
    let mut stream = accept_with_deadline(listener)?;
    require_same_effective_user(&stream)?;
    configure_peer_stream(&stream)?;
    let hello = read_worker_hello(&mut stream)?;
    let generation =
        NonZeroU64::new(7).ok_or_else(|| io::Error::other("test generation must be nonzero"))?;
    write_message(
        &mut stream,
        &LocalWorkerMessage::WorkerAccepted { generation },
    )
    .map_err(protocol_io)?;
    Ok((stream, hello))
}

fn read_worker_hello(stream: &mut UnixStream) -> io::Result<WorkerHello> {
    let LocalWorkerMessage::WorkerHello(hello) = read_message(stream).map_err(protocol_io)? else {
        return Err(io::Error::other(
            "Worker sent the wrong Server handshake message",
        ));
    };
    Ok(hello)
}

fn assert_worker_identity(hello: &WorkerHello) {
    assert_eq!(hello.cohort(), env!("CARGO_PKG_VERSION"));
    assert_eq!(hello.worker_id(), LOCAL_WORKER_ID);
}

fn read_host_hello(stream: &mut UnixStream) -> io::Result<(Hello, CorrelationId)> {
    let record = read_terminal_record(stream)?;
    let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
    let correlation_id = envelope
        .correlation_id
        .ok_or_else(|| io::Error::other("Worker Hello omitted its correlation identity"))?;
    if envelope.role != MessageRole::Request
        || envelope.host_epoch.is_some()
        || envelope.optional_message
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
    {
        return Err(io::Error::other("Worker sent an invalid Hello envelope"));
    }
    let ConnectionControl::Hello(hello) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
            .map_err(protocol_io)?
    else {
        return Err(io::Error::other(
            "Worker sent the wrong Terminal Host handshake message",
        ));
    };
    Ok((hello, correlation_id))
}

fn assert_host_hello(hello: &Hello) -> TestResult {
    assert_eq!(
        hello.client_id,
        ClientId::from_bytes(WORKER_CLIENT_ID_BYTES)?
    );
    assert_eq!(hello.protocol, CURRENT_PROTOCOL_VERSION);
    assert!(hello.frame_bytes > 0);
    assert!(hello.input_bytes > 0);
    assert!(hello.row_window_bytes > 0);
    assert!(hello.supported_optional_features.is_empty());
    Ok(())
}

fn write_hello_accepted(
    stream: &mut UnixStream,
    hello: &Hello,
    correlation_id: CorrelationId,
) -> io::Result<()> {
    let host_epoch = HostEpoch::from_bytes([0x44; 16]).map_err(protocol_io)?;
    let control = encode_connection_control(&ConnectionControl::HelloAccepted(HelloAccepted {
        frame_bytes: hello.frame_bytes,
        host_epoch,
        host_version: "0.1.0".to_owned(),
        input_bytes: hello.input_bytes,
        protocol: CURRENT_PROTOCOL_VERSION,
        row_window_bytes: hello.row_window_bytes,
        supported_optional_features: Vec::new(),
    }))
    .map_err(protocol_io)?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation_id),
        host_epoch: Some(host_epoch),
        kind: control.kind,
        optional_message: false,
        payload: &control.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: control.role,
    })
    .map_err(protocol_io)?;
    stream.write_all(&record)
}

fn read_terminal_record(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
    stream.read_exact(&mut header)?;
    let total_length = usize::try_from(u32::from_le_bytes([
        header[8], header[9], header[10], header[11],
    ]))
    .map_err(protocol_io)?;
    if !(TRANSPORT_ENVELOPE_HEADER_BYTES..=MAXIMUM_TRANSPORT_RECORD_BYTES).contains(&total_length) {
        return Err(io::Error::other(
            "Worker sent an invalid Terminal Protocol record length",
        ));
    }
    let mut record = vec![0_u8; total_length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    stream.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}

fn require_connection_closed(stream: &mut UnixStream) -> io::Result<()> {
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
            "peer connection remained usable or carried data: {outcome:?}"
        ))),
    }
}

fn configure_peer_stream(stream: &UnixStream) -> io::Result<()> {
    stream.set_read_timeout(Some(PROCESS_TIMEOUT))?;
    stream.set_write_timeout(Some(PROCESS_TIMEOUT))
}

fn accept_with_deadline(listener: &UnixListener) -> io::Result<UnixStream> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _address)) => {
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for Worker connection",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn wait_for_socket(endpoint: &Path) -> io::Result<()> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match fs::metadata(endpoint) {
            Ok(metadata) if metadata.file_type().is_socket() => return Ok(()),
            Ok(_metadata) => {
                return Err(io::Error::other(
                    "Terminal Host endpoint exists but is not a Unix socket",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
}

fn assert_failed_after_exact_readiness(output: ProcessOutput) {
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"ready\n");
    assert!(output.stderr.is_empty());
}

fn join_peer<T>(thread: JoinHandle<io::Result<T>>, name: &str) -> TestResult<T> {
    thread
        .join()
        .map_err(|_panic| io::Error::other(format!("{name} thread panicked")))?
        .map_err(Into::into)
}

fn channel_timeout(error: mpsc::RecvTimeoutError) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!("timed out waiting for test boundary transition: {error}"),
    )
}

fn protocol_io(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

struct RuntimeFixture {
    _directory: TestDirectory,
    server_endpoint: PathBuf,
    server_listener: UnixListener,
}

impl RuntimeFixture {
    fn new() -> io::Result<Self> {
        let directory = TestDirectory::new()?;
        let server_endpoint = directory.path().join("session.sock");
        let server_listener = bind_listener(&server_endpoint)?;
        Ok(Self {
            _directory: directory,
            server_endpoint,
            server_listener,
        })
    }

    fn bind_host_listener(&self) -> io::Result<UnixListener> {
        bind_listener(&self.host_endpoint())
    }

    fn host_endpoint(&self) -> PathBuf {
        self._directory.path().join(format!("{HOST_ENDPOINT_ID}.s"))
    }
}

fn bind_listener(endpoint: &Path) -> io::Result<UnixListener> {
    let listener = UnixListener::bind(endpoint)?;
    fs::set_permissions(endpoint, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

struct RunningWorker(CapturedProcess);

impl RunningWorker {
    fn start(fixture: &RuntimeFixture) -> io::Result<Self> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_orchbee-worker"));
        command
            .env("SHELL", "/bin/sh")
            .env("HOME", "/not-the-worker-account-home")
            .current_dir(fixture._directory.path())
            .arg("--local-endpoint")
            .arg(&fixture.server_endpoint)
            .arg("--terminal-host-runtime-directory")
            .arg(fixture._directory.path())
            .arg("--terminal-host-endpoint-id")
            .arg(HOST_ENDPOINT_ID);
        CapturedProcess::spawn(&mut command, true).map(Self)
    }

    fn await_readiness(&self) -> io::Result<Option<[u8; 6]>> {
        self.0.await_readiness()
    }

    fn finish(self) -> io::Result<ProcessOutput> {
        self.0.finish()
    }
}

struct RunningHost(CapturedProcess);

impl RunningHost {
    fn start(fixture: &RuntimeFixture) -> io::Result<Self> {
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../terminal-host/zig-out/bin/orchbee-terminal-host");
        if !binary.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "real Host process witness requires `pnpm --filter @orchbee/terminal-host build`: {}",
                    binary.display()
                ),
            ));
        }
        let mut command = Command::new(binary);
        command
            .arg("serve")
            .arg("--runtime-directory")
            .arg(fixture._directory.path())
            .arg("--endpoint-id")
            .arg(HOST_ENDPOINT_ID);
        CapturedProcess::spawn(&mut command, false).map(Self)
    }

    fn finish(&mut self) -> io::Result<ProcessOutput> {
        self.0.finish_mut()
    }
}

struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

type ReadinessResult = Result<Option<[u8; 6]>, String>;

struct CapturedProcess {
    child: Option<Child>,
    readiness: Option<Receiver<ReadinessResult>>,
    stderr: Option<JoinHandle<io::Result<Vec<u8>>>>,
    stdout: Option<JoinHandle<io::Result<Vec<u8>>>>,
}

impl CapturedProcess {
    fn spawn(command: &mut Command, observe_readiness: bool) -> io::Result<Self> {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("child stdout pipe is unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("child stderr pipe is unavailable"))?;
        let (readiness_sender, readiness) = mpsc::channel();
        let sender = observe_readiness.then_some(readiness_sender);
        Ok(Self {
            child: Some(child),
            readiness: observe_readiness.then_some(readiness),
            stdout: Some(thread::spawn(move || capture_stdout(stdout, sender))),
            stderr: Some(thread::spawn(move || capture_stream(stderr))),
        })
    }

    fn await_readiness(&self) -> io::Result<Option<[u8; 6]>> {
        self.readiness
            .as_ref()
            .ok_or_else(|| io::Error::other("process has no readiness observer"))?
            .recv_timeout(PROCESS_TIMEOUT)
            .map_err(channel_timeout)?
            .map_err(io::Error::other)
    }

    fn finish(mut self) -> io::Result<ProcessOutput> {
        self.finish_mut()
    }

    fn finish_mut(&mut self) -> io::Result<ProcessOutput> {
        let mut child = self
            .child
            .take()
            .ok_or_else(|| io::Error::other("child process was already consumed"))?;
        let status = wait_for_exit(&mut child)?;
        let stdout = join_capture(self.stdout.take(), "stdout")?;
        let stderr = join_capture(self.stderr.take(), "stderr")?;
        Ok(ProcessOutput {
            status,
            stdout,
            stderr,
        })
    }
}

impl Drop for CapturedProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ignored = child.kill();
            let _ignored = child.wait();
        }
        if let Some(stdout) = self.stdout.take() {
            let _ignored = stdout.join();
        }
        if let Some(stderr) = self.stderr.take() {
            let _ignored = stderr.join();
        }
    }
}

fn capture_stdout(
    mut stream: impl Read,
    readiness: Option<mpsc::Sender<ReadinessResult>>,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut readiness = readiness;
    let mut buffer = [0_u8; 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                if let Some(sender) = readiness.take() {
                    let _ignored = sender.send(Ok(None));
                }
                return Ok(output);
            }
            Ok(count) => {
                output.extend_from_slice(&buffer[..count]);
                if output.len() >= 6
                    && let Some(sender) = readiness.take()
                {
                    let marker = output[..6]
                        .try_into()
                        .map_err(|_error| io::Error::other("invalid readiness marker length"))?;
                    let _ignored = sender.send(Ok(Some(marker)));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                if let Some(sender) = readiness.take() {
                    let _ignored = sender.send(Err(error.to_string()));
                }
                return Err(error);
            }
        }
    }
}

fn capture_stream(mut stream: impl Read) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    stream.read_to_end(&mut output)?;
    Ok(output)
}

fn join_capture(
    capture: Option<JoinHandle<io::Result<Vec<u8>>>>,
    name: &str,
) -> io::Result<Vec<u8>> {
    capture
        .ok_or_else(|| io::Error::other(format!("child {name} capture was already consumed")))?
        .join()
        .map_err(|_panic| io::Error::other(format!("child {name} capture thread panicked")))?
}

fn wait_for_exit(child: &mut Child) -> io::Result<ExitStatus> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ignored = child.kill();
            let _ignored = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "child process did not exit before the test deadline",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            // Darwin's Unix-domain socket path is only 103 bytes. Keep the
            // fixture root short enough for the Host's 34-byte socket name.
            let path = Path::new("/tmp").join(format!("obw-{}-{fixture_id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
                    return Ok(Self(path));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique Worker-test directory",
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
