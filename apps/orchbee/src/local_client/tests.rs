use std::{
    fs,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateRejectedReason, FRAME_VERSION as OBCS_FRAME_VERSION,
    ProtocolError as ObcsProtocolError, RequestId, ServerMessage, StartMode, StartOutcome,
    StartRejectedReason, TerminalCreateRequest, TerminalCreateResult, TerminalStartRequest,
    TerminalStartResult, read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{
    ConnectClientError, LocalClientMessage, PROTOCOL_VERSION as OBLC_PROTOCOL_VERSION,
    ProtocolError as OblcProtocolError, RejectionReason, read_message as read_oblc_message,
    write_message as write_oblc_message,
};

use super::{
    LocalClient, LocalClientConnectError, OutcomeUnknownReason, ResultKind, SubmissionError,
};

const COHORT: &str = "test-cohort";
const FIRST_REQUEST_ID: &str = "11111111111111111111111111111111";
const SECOND_REQUEST_ID: &str = "22222222222222222222222222222222";
const TEST_REQUEST_TIMEOUT: Duration = Duration::from_millis(40);
const SERVER_GUARD_TIMEOUT: Duration = Duration::from_secs(1);

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn accepts_one_session_and_submits_create_then_start_on_the_same_stream() -> TestResult {
    let create = deferred_create(FIRST_REQUEST_ID)?;
    let start = start(SECOND_REQUEST_ID, FIRST_REQUEST_ID)?;
    let expected_create = create.clone();
    let expected_start = start.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 7)?;

        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(expected_create),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(FIRST_REQUEST_ID)?),
        )
        .map_err(io::Error::other)?;

        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalStart(expected_start),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalStartResult(start_denied(SECOND_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    assert_eq!(client.generation().map(NonZeroU64::get), Some(7));
    assert_eq!(
        client.submit_terminal_create(create)?,
        create_denied(FIRST_REQUEST_ID)?
    );
    assert_eq!(
        client.submit_terminal_start(start)?,
        start_denied(SECOND_REQUEST_ID)?
    );
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn preserves_a_correlated_server_reported_unknown_outcome() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 8)?;
        let _request = read_client_message(&mut stream).map_err(io::Error::other)?;
        let request_id = RequestId::parse(FIRST_REQUEST_ID).map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request_id,
                CreateOutcome::OutcomeUnknown,
            )),
        )
        .map_err(io::Error::other)
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let result = client.submit_terminal_create(request)?;
    assert_eq!(result.outcome(), CreateOutcome::OutcomeUnknown);
    assert_eq!(client.generation().map(NonZeroU64::get), Some(8));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn reports_exact_oblc_rejection() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        write_oblc_message(
            &mut stream,
            &LocalClientMessage::Rejected {
                reason: RejectionReason::CapacityExceeded,
            },
        )
        .map_err(io::Error::other)
    })?;

    let error = LocalClient::connect(server.endpoint(), COHORT)
        .expect_err("the fake Server rejection must fail admission");
    assert!(matches!(
        error,
        LocalClientConnectError::Admission(ConnectClientError::Rejected(
            RejectionReason::CapacityExceeded
        ))
    ));
    server.finish()?;
    Ok(())
}

#[test]
fn rejects_an_incompatible_oblc_response_version() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        let generation = NonZeroU64::new(9)
            .ok_or_else(|| io::Error::other("test generation must be nonzero"))?;
        let mut frame = Vec::new();
        write_oblc_message(
            &mut frame,
            &LocalClientMessage::ClientAccepted { generation },
        )
        .map_err(io::Error::other)?;
        frame[4..6].copy_from_slice(&(OBLC_PROTOCOL_VERSION + 1).to_be_bytes());
        stream.write_all(&frame)
    })?;

    let error = LocalClient::connect(server.endpoint(), COHORT)
        .expect_err("the incompatible OBLC version must fail admission");
    assert!(matches!(
        error,
        LocalClientConnectError::Admission(ConnectClientError::Protocol(
            OblcProtocolError::IncompatibleVersion { received }
        )) if received == OBLC_PROTOCOL_VERSION + 1
    ));
    server.finish()?;
    Ok(())
}

#[test]
fn sends_the_exact_cohort_and_preserves_typed_cohort_rejection() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        write_oblc_message(
            &mut stream,
            &LocalClientMessage::Rejected {
                reason: RejectionReason::IncompatibleCohort,
            },
        )
        .map_err(io::Error::other)
    })?;

    let error = LocalClient::connect(server.endpoint(), COHORT)
        .expect_err("the incompatible cohort must fail admission");
    assert!(matches!(
        error,
        LocalClientConnectError::Admission(ConnectClientError::Rejected(
            RejectionReason::IncompatibleCohort
        ))
    ));
    server.finish()?;
    Ok(())
}

#[test]
fn validates_the_cohort_before_connecting() -> TestResult {
    let directory = TestDirectory::new()?;
    let endpoint = directory.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    listener.set_nonblocking(true)?;

    let error = LocalClient::connect(&endpoint, "")
        .expect_err("an invalid cohort must fail before the connection effect");
    assert!(matches!(
        error,
        LocalClientConnectError::Admission(ConnectClientError::Protocol(
            OblcProtocolError::InvalidCohort
        ))
    ));
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock
    ));
    Ok(())
}

#[test]
fn supports_partial_io_across_oblc_and_obcs() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let expected_request = request.clone();
    let server = FakeServer::start(move |stream| {
        let mut stream = ChunkedStream::new(stream, 1, 1);
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 11)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(expected_request),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(FIRST_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    assert_eq!(
        client.submit_terminal_create(request)?,
        create_denied(FIRST_REQUEST_ID)?
    );
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn retires_the_session_when_create_receives_a_start_result() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 13)?;
        let _request = read_client_message(&mut stream).map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalStartResult(start_denied(FIRST_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let error = client
        .submit_terminal_create(request)
        .expect_err("the wrong result kind must not be accepted");
    assert!(matches!(
        error,
        SubmissionError::OutcomeUnknown {
            request_id,
            reason: OutcomeUnknownReason::UnexpectedResult {
                expected: ResultKind::TerminalCreate,
                received: ResultKind::TerminalStart,
            },
        } if request_id.as_str() == FIRST_REQUEST_ID
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(())
}

#[test]
fn retires_the_session_when_result_correlation_is_wrong() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 17)?;
        let _request = read_client_message(&mut stream).map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(SECOND_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let error = client
        .submit_terminal_create(request)
        .expect_err("the wrong correlation identity must not be accepted");
    assert!(matches!(
        error,
        SubmissionError::OutcomeUnknown {
            request_id,
            reason: OutcomeUnknownReason::CorrelationMismatch { expected, received },
        } if request_id.as_str() == FIRST_REQUEST_ID
            && expected.as_str() == FIRST_REQUEST_ID
            && received.as_str() == SECOND_REQUEST_ID
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(())
}

#[test]
fn rejects_an_incompatible_obcs_result_version_as_unknown() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 19)?;
        let _request = read_client_message(&mut stream).map_err(io::Error::other)?;
        let mut frame = Vec::new();
        write_server_message(
            &mut frame,
            &ServerMessage::TerminalCreateResult(create_denied(FIRST_REQUEST_ID)?),
        )
        .map_err(io::Error::other)?;
        frame[4..6].copy_from_slice(&(OBCS_FRAME_VERSION + 1).to_be_bytes());
        stream.write_all(&frame)
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let error = client
        .submit_terminal_create(request)
        .expect_err("the incompatible OBCS version must not be accepted");
    assert!(matches!(
        error,
        SubmissionError::OutcomeUnknown {
            request_id,
            reason: OutcomeUnknownReason::InvalidResult(
                ObcsProtocolError::IncompatibleFrameVersion { received }
            ),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && received == OBCS_FRAME_VERSION + 1
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(())
}

#[test]
fn request_timeout_is_unknown_retires_the_session_and_never_resubmits() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let expected_request = request.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 23)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(expected_request),
        )?;
        stream.set_read_timeout(Some(SERVER_GUARD_TIMEOUT))?;
        let mut extra = [0_u8; 1];
        match stream.read(&mut extra) {
            Ok(0) => Ok(()),
            Ok(_) => Err(io::Error::other("Client submitted a second request")),
            Err(error) => Err(io::Error::other(format!(
                "Client did not retire the ambiguous Session: {error}"
            ))),
        }
    })?;

    let mut client =
        LocalClient::connect_with_request_timeout(server.endpoint(), COHORT, TEST_REQUEST_TIMEOUT)?;
    let error = client
        .submit_terminal_create(request)
        .expect_err("a result timeout must be an unknown outcome");
    assert!(matches!(
        error,
        SubmissionError::OutcomeUnknown {
            request_id,
            reason: OutcomeUnknownReason::Read(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && matches!(error.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)
    ));
    assert_eq!(client.generation(), None);
    assert!(matches!(
        client.submit_terminal_create(deferred_create(SECOND_REQUEST_ID)?),
        Err(SubmissionError::SessionUnavailable)
    ));
    server.finish()?;
    Ok(())
}

#[test]
fn eof_after_one_request_is_an_unknown_outcome_without_retry() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let expected_request = request.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 29)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(expected_request),
        )
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let error = client
        .submit_terminal_create(request)
        .expect_err("EOF after submission must be an unknown outcome");
    assert!(matches!(
        error,
        SubmissionError::OutcomeUnknown {
            request_id,
            reason: OutcomeUnknownReason::Read(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && matches!(
                error.kind(),
                io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
            )
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(())
}

fn deferred_create(request_id: &str) -> io::Result<TerminalCreateRequest> {
    let request_id = RequestId::parse(request_id).map_err(io::Error::other)?;
    Ok(TerminalCreateRequest::new(
        request_id,
        None,
        None,
        StartMode::Deferred,
    ))
}

fn start(request_id: &str, create_request_id: &str) -> io::Result<TerminalStartRequest> {
    let request_id = RequestId::parse(request_id).map_err(io::Error::other)?;
    let create_request_id = RequestId::parse(create_request_id).map_err(io::Error::other)?;
    let geometry =
        orchbee_client_server_protocol::Geometry::new(80, 24, 9, 18).map_err(io::Error::other)?;
    TerminalStartRequest::new(request_id, create_request_id, geometry).map_err(io::Error::other)
}

fn create_denied(request_id: &str) -> io::Result<TerminalCreateResult> {
    let request_id = RequestId::parse(request_id).map_err(io::Error::other)?;
    Ok(TerminalCreateResult::new(
        request_id,
        CreateOutcome::Rejected {
            reason: CreateRejectedReason::NotAuthorized,
        },
    ))
}

fn start_denied(request_id: &str) -> io::Result<TerminalStartResult> {
    let request_id = RequestId::parse(request_id).map_err(io::Error::other)?;
    Ok(TerminalStartResult::new(
        request_id,
        StartOutcome::Rejected {
            reason: StartRejectedReason::NotAuthorized,
        },
    ))
}

fn expect_hello(stream: &mut impl Read, expected_cohort: &str) -> io::Result<()> {
    let message = read_oblc_message(stream).map_err(io::Error::other)?;
    let LocalClientMessage::ClientHello(hello) = message else {
        return Err(io::Error::other("fake Server expected ClientHello"));
    };
    if hello.cohort() != expected_cohort {
        return Err(io::Error::other(format!(
            "fake Server received cohort {}; expected {expected_cohort}",
            hello.cohort()
        )));
    }
    Ok(())
}

fn admit(stream: &mut impl Write, generation: u64) -> io::Result<()> {
    let generation = NonZeroU64::new(generation)
        .ok_or_else(|| io::Error::other("test generation must be nonzero"))?;
    write_oblc_message(stream, &LocalClientMessage::ClientAccepted { generation })
        .map_err(io::Error::other)
}

fn require_message(actual: ClientMessage, expected: ClientMessage) -> io::Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "fake Server received {actual:?}; expected {expected:?}"
        )))
    }
}

struct FakeServer {
    _directory: TestDirectory,
    endpoint: PathBuf,
    thread: thread::JoinHandle<io::Result<()>>,
}

impl FakeServer {
    fn start(
        handler: impl FnOnce(UnixStream) -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        let directory = TestDirectory::new()?;
        let endpoint = directory.path.join("client.sock");
        let listener = UnixListener::bind(&endpoint)?;
        let thread = thread::spawn(move || {
            let (stream, _address) = listener.accept()?;
            stream.set_read_timeout(Some(SERVER_GUARD_TIMEOUT))?;
            stream.set_write_timeout(Some(SERVER_GUARD_TIMEOUT))?;
            handler(stream)
        });
        Ok(Self {
            _directory: directory,
            endpoint,
            thread,
        })
    }

    fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    fn finish(self) -> io::Result<()> {
        self.thread
            .join()
            .map_err(|_panic| io::Error::other("fake Server panicked"))?
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchbee-local-client-{}-{fixture_id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique local Client test directory",
        ))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.path);
    }
}

struct ChunkedStream {
    stream: UnixStream,
    maximum_read_chunk: usize,
    maximum_write_chunk: usize,
}

impl ChunkedStream {
    const fn new(
        stream: UnixStream,
        maximum_read_chunk: usize,
        maximum_write_chunk: usize,
    ) -> Self {
        Self {
            stream,
            maximum_read_chunk,
            maximum_write_chunk,
        }
    }
}

impl Read for ChunkedStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let chunk_length = buffer.len().min(self.maximum_read_chunk);
        self.stream.read(&mut buffer[..chunk_length])
    }
}

impl Write for ChunkedStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let chunk_length = buffer.len().min(self.maximum_write_chunk);
        self.stream.write(&buffer[..chunk_length])
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
