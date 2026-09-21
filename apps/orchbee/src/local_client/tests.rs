use std::{
    io::{self, Read, Write},
    num::NonZeroU64,
    os::fd::{AsFd, BorrowedFd},
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    time::{Duration, Instant},
};

use nix::poll::PollFlags;
use orchbee_agent_observation_protocol::{
    AgentNamesListOutcome, AgentNamesListOutcomeEnvelope, AgentNamesResult, AgentReadRequest,
    AgentResult, AgentSafeInteger, AgentUnavailableReason, AgentWireError, ObapRequestId,
    read_agent_request, write_agent_result,
};
use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateRejectedReason, FRAME_VERSION as OBCS_FRAME_VERSION,
    ProtocolError as ObcsProtocolError, RequestId, ServerMessage, StartMode, StartOutcome,
    StartRejectedReason, TerminalCreateRequest, TerminalCreateResult, TerminalStartRequest,
    TerminalStartResult, encode_client_frame, encode_server_frame, read_client_message,
    write_server_message,
};
use orchbee_local_client_protocol::{
    ConnectClientError, LocalClientMessage, PROTOCOL_VERSION as OBLC_PROTOCOL_VERSION,
    ProtocolError as OblcProtocolError, RejectionReason, read_message as read_oblc_message,
    write_message as write_oblc_message,
};

use super::agent::{AgentReadError, AgentReadFailure};
use super::{
    ClientSessionConnection, DeadlineIo, IoChunkLimits, LocalClient, LocalClientConnectError,
    LocalOperationClock, MonotonicClock, OutcomeUnknownReason, ResultKind, SubmissionDeadline,
    SubmissionError, exchange_wake_fault, exchange_wake_ready,
};

const COHORT: &str = "test-cohort";
const FIRST_REQUEST_ID: &str = "11111111111111111111111111111111";
const SECOND_REQUEST_ID: &str = "22222222222222222222222222222222";
const TEST_REQUEST_TIMEOUT: Duration = Duration::from_millis(40);
const SERVER_GUARD_TIMEOUT: Duration = Duration::from_secs(1);

use super::test_server::{FakeServer, TestDirectory, connect_with_adjustable_clock};

mod workbench;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[cfg(target_os = "linux")]
fn deadline_admission_hands_the_same_session_to_obcs() -> TestResult {
    let create = deferred_create(FIRST_REQUEST_ID)?;
    let expected_create = create.clone();
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
        .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect_before(
        server.endpoint(),
        COHORT,
        Instant::now() + SERVER_GUARD_TIMEOUT,
    )?;
    assert_eq!(client.generation().map(NonZeroU64::get), Some(7));
    assert_eq!(
        client.submit_terminal_create(create)?,
        create_denied(FIRST_REQUEST_ID)?
    );
    drop(client);
    server.finish()?;
    Ok(())
}

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
                expected: ResultKind::Create,
                received: ResultKind::Start,
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
fn slow_trickle_read_exhausts_one_total_deadline_despite_progress() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let expected_request = request.clone();
    let response = encode_server_frame(&ServerMessage::TerminalCreateResult(create_denied(
        FIRST_REQUEST_ID,
    )?))?;
    let (progress_sender, progress_receiver) = sync_channel(0);
    let (written_sender, written_receiver) = sync_channel(0);
    let sent_bytes = Arc::new(AtomicU64::new(0));
    let server_sent_bytes = sent_bytes.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 24)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(expected_request),
        )?;

        for byte in response {
            if progress_receiver.recv().is_err() {
                return Ok(());
            }
            let write_result = stream.write_all(&[byte]);
            if write_result.is_ok() {
                server_sent_bytes.fetch_add(1, Ordering::Relaxed);
            }
            let _acknowledged = written_sender.send(());
            write_result?;
        }
        Err(io::Error::other(
            "fake Server delivered the complete result before deadline expiry",
        ))
    })?;

    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::with_progress_signal(
        Duration::from_millis(10),
        observations.clone(),
        ProgressSignal {
            sender: progress_sender,
            written: Mutex::new(written_receiver),
            first_observation: 4,
            period: 2,
        },
    );
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        Duration::from_millis(80),
        Box::new(clock),
        IoChunkLimits::UNBOUNDED,
    )?;
    let error = client
        .submit_terminal_create(request)
        .expect_err("partial read progress must not renew the total request deadline");
    assert!(matches!(
        error,
        SubmissionError::OutcomeUnknown {
            request_id,
            reason: OutcomeUnknownReason::Read(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    assert!(matches!(
        client.submit_terminal_create(deferred_create(SECOND_REQUEST_ID)?),
        Err(SubmissionError::SessionUnavailable)
    ));
    drop(client);
    server.finish()?;
    assert!(observations.load(Ordering::Relaxed) >= 9);
    assert_eq!(sent_bytes.load(Ordering::Relaxed), 3);
    Ok(())
}

#[test]
fn slow_trickle_write_exhausts_one_total_deadline_after_partial_handoff() -> TestResult {
    let request = deferred_create(FIRST_REQUEST_ID)?;
    let expected_frame = encode_client_frame(&ClientMessage::TerminalCreate(request.clone()))?;
    let expected_prefix = expected_frame[..4].to_vec();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 25)?;

        let mut received = Vec::new();
        stream.read_to_end(&mut received)?;
        if received != expected_prefix {
            return Err(io::Error::other(format!(
                "fake Server received partial request bytes {received:?}; expected one OBCS prefix"
            )));
        }
        Ok(())
    })?;

    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        Duration::from_millis(90),
        Box::new(clock),
        IoChunkLimits::new(usize::MAX, 1),
    )?;
    let error = client
        .submit_terminal_create(request)
        .expect_err("partial write progress must not renew the total request deadline");
    assert!(matches!(
        error,
        SubmissionError::OutcomeUnknown {
            request_id,
            reason: OutcomeUnknownReason::Write(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    assert!(matches!(
        client.submit_terminal_create(deferred_create(SECOND_REQUEST_ID)?),
        Err(SubmissionError::SessionUnavailable)
    ));
    drop(client);
    server.finish()?;
    assert!(observations.load(Ordering::Relaxed) >= 10);
    Ok(())
}

#[test]
fn deadline_expiry_before_the_first_write_is_a_definite_failure() -> TestResult {
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 26)?;
        let mut received = Vec::new();
        stream.read_to_end(&mut received)?;
        if received.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "fake Server received bytes before the pre-write failure: {received:?}"
            )))
        }
    })?;

    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        Duration::from_millis(5),
        Box::new(clock),
        IoChunkLimits::UNBOUNDED,
    )?;
    let error = client
        .submit_terminal_create(deferred_create(FIRST_REQUEST_ID)?)
        .expect_err("deadline expiry before the first write must be definite");
    assert!(matches!(
        error,
        SubmissionError::RequestNotSubmitted(error)
            if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation().map(NonZeroU64::get), Some(26));
    drop(client);
    server.finish()?;
    assert_eq!(observations.load(Ordering::Relaxed), 2);
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
        None,
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

struct ChunkedStream {
    stream: UnixStream,
    maximum_read_chunk: usize,
    maximum_write_chunk: usize,
}

#[derive(Debug)]
struct SteppingMonotonicClock {
    origin: Instant,
    step: Duration,
    observations: Arc<AtomicU64>,
    progress_signal: Option<ProgressSignal>,
}

impl SteppingMonotonicClock {
    fn new(step: Duration, observations: Arc<AtomicU64>) -> Self {
        Self {
            origin: Instant::now(),
            step,
            observations,
            progress_signal: None,
        }
    }

    fn with_progress_signal(
        step: Duration,
        observations: Arc<AtomicU64>,
        progress_signal: ProgressSignal,
    ) -> Self {
        Self {
            origin: Instant::now(),
            step,
            observations,
            progress_signal: Some(progress_signal),
        }
    }
}

impl MonotonicClock for SteppingMonotonicClock {
    fn now(&self) -> Instant {
        let observation = self.observations.fetch_add(1, Ordering::Relaxed);
        if let Some(signal) = &self.progress_signal
            && observation >= signal.first_observation
            && (observation - signal.first_observation).is_multiple_of(signal.period)
            && signal.sender.send(()).is_ok()
        {
            let _written = signal
                .written
                .lock()
                .expect("progress receiver mutex must remain usable")
                .recv();
        }
        let multiplier = u32::try_from(observation).unwrap_or(u32::MAX);
        self.origin
            .checked_add(self.step.saturating_mul(multiplier))
            .unwrap_or(self.origin)
    }
}

#[derive(Debug)]
struct ProgressSignal {
    sender: SyncSender<()>,
    written: Mutex<Receiver<()>>,
    first_observation: u64,
    period: u64,
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

#[test]
fn viewer_reads_large_records_and_retires_on_an_incompatible_result() -> TestResult {
    use orchbee_client_server_protocol::{
        TerminalViewerAction as Action, TerminalViewerOutcome as Outcome, TerminalViewerRequest,
        TerminalViewerResult, ViewerId,
    };
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 9)?;
        for outcome in [Outcome::Record(vec![0xa5; 100_000]), Outcome::Detached {}] {
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected viewer request"));
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
        }
        let mut byte = [0];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "invalid result must retire without resubmission"
        );
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let request = TerminalViewerRequest::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        Action::Poll {
            attachment_id: ViewerId::parse(FIRST_REQUEST_ID)?,
        },
    );
    assert_eq!(
        client.submit_terminal_viewer(request.clone())?.outcome(),
        &Outcome::Record(vec![0xa5; 100_000])
    );
    assert!(matches!(
        client.submit_terminal_viewer(request.clone()),
        Err(SubmissionError::OutcomeUnknown {
            reason: OutcomeUnknownReason::InvalidResult(_),
            ..
        })
    ));
    assert!(matches!(
        client.submit_terminal_viewer(request),
        Err(SubmissionError::SessionUnavailable)
    ));
    server.finish()?;
    Ok(())
}

#[test]
fn control_accepts_exact_unknown_and_retires_on_a_wrong_resize_geometry() -> TestResult {
    use orchbee_client_server_protocol::{
        ControllerId, Geometry, TerminalControlAction as Action, TerminalControlOutcome as Outcome,
        TerminalControlRequest, TerminalControlResult, ViewerId,
    };
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 10)?;
        for outcome in [
            Outcome::OutcomeUnknown {},
            Outcome::Resized {
                geometry: Geometry::new(81, 24, 8, 16).map_err(io::Error::other)?,
            },
        ] {
            let ClientMessage::TerminalControl(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected control"));
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                    request.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
        }
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte)?, 0);
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let action = Action::Resize {
        attachment_id: ViewerId::parse(FIRST_REQUEST_ID)?,
        controller_id: ControllerId::parse(FIRST_REQUEST_ID)?,
        sequence: NonZeroU64::MIN,
        geometry: Geometry::new(80, 24, 8, 16)?,
    };
    assert_eq!(
        client
            .submit_terminal_control(TerminalControlRequest::new(
                RequestId::parse(FIRST_REQUEST_ID)?,
                action
            ))?
            .outcome(),
        &Outcome::OutcomeUnknown {}
    );
    assert!(matches!(
        client.submit_terminal_control(TerminalControlRequest::new(
            RequestId::parse(SECOND_REQUEST_ID)?,
            Action::Resize {
                attachment_id: ViewerId::parse(SECOND_REQUEST_ID)?,
                controller_id: ControllerId::parse(SECOND_REQUEST_ID)?,
                sequence: NonZeroU64::MIN,
                geometry: Geometry::new(80, 24, 8, 16)?,
            }
        )),
        Err(SubmissionError::OutcomeUnknown { .. })
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(())
}

#[test]
fn initial_startup_budget_clamps_requests_until_explicit_readiness() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 1)?;
        let request = read_client_message(&mut stream).map_err(io::Error::other)?;
        require_message(
            request,
            ClientMessage::TerminalCreate(deferred_create(FIRST_REQUEST_ID)?),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(FIRST_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    client.set_submission_deadline(Some(Instant::now()));
    assert!(
        matches!(client.submit_terminal_create(deferred_create(FIRST_REQUEST_ID)?),
        Err(SubmissionError::RequestNotSubmitted(error)) if error.kind() == io::ErrorKind::TimedOut)
    );
    assert!(client.generation().is_some());
    client.set_submission_deadline(None);
    assert_eq!(
        client.submit_terminal_create(deferred_create(FIRST_REQUEST_ID)?)?,
        create_denied(FIRST_REQUEST_ID)?
    );
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn caller_deadline_viewer_and_control_forms_fail_before_write_and_keep_the_carrier() -> TestResult {
    use orchbee_client_server_protocol::{
        TerminalControlAction, TerminalControlRequest, TerminalViewerAction, TerminalViewerRequest,
        ViewerId,
    };

    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 37)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(deferred_create(SECOND_REQUEST_ID)?),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(SECOND_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;

    client.set_submission_deadline(Some(client.operation_clock().now()));
    assert!(matches!(
        client.submit_terminal_viewer_before(
            TerminalViewerRequest::new(
                RequestId::parse(FIRST_REQUEST_ID)?,
                TerminalViewerAction::Poll {
                    attachment_id: ViewerId::parse(FIRST_REQUEST_ID)?,
                },
            ),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        ),
        Err(SubmissionError::RequestNotSubmitted(error))
            if error.kind() == io::ErrorKind::TimedOut
    ));
    client.set_submission_deadline(None);

    let deadline = client.operation_clock().now();
    assert!(matches!(
        client.submit_terminal_control_before(
            TerminalControlRequest::new(
                RequestId::parse(FIRST_REQUEST_ID)?,
                TerminalControlAction::Acquire {
                    attachment_id: ViewerId::parse(FIRST_REQUEST_ID)?,
                },
            ),
            deadline,
        ),
        Err(SubmissionError::RequestNotSubmitted(error))
            if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation().map(NonZeroU64::get), Some(37));

    assert_eq!(
        client.submit_terminal_create(deferred_create(SECOND_REQUEST_ID)?)?,
        create_denied(SECOND_REQUEST_ID)?
    );
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn caller_deadline_viewer_and_control_forms_keep_the_shorter_exchange_budget() -> TestResult {
    use orchbee_client_server_protocol::{
        ControlRejectedReason, TerminalControlAction, TerminalControlOutcome,
        TerminalControlRequest, TerminalControlResult, TerminalViewerAction, TerminalViewerOutcome,
        TerminalViewerRequest, TerminalViewerResult, ViewerId, ViewerRejectedReason,
    };

    #[derive(Clone, Copy)]
    enum RequestKind {
        Viewer,
        Control,
    }

    for kind in [RequestKind::Viewer, RequestKind::Control] {
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 49)?;
            let request = read_client_message(&mut stream).map_err(io::Error::other)?;
            let response = match (kind, request) {
                (RequestKind::Viewer, ClientMessage::TerminalViewer(request)) => {
                    ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        TerminalViewerOutcome::Rejected {
                            reason: ViewerRejectedReason::TerminalUnavailable,
                        },
                    ))
                }
                (RequestKind::Control, ClientMessage::TerminalControl(request)) => {
                    ServerMessage::TerminalControlResult(TerminalControlResult::new(
                        request.request_id().clone(),
                        TerminalControlOutcome::Rejected {
                            reason: ControlRejectedReason::ViewerUnavailable,
                        },
                    ))
                }
                _ => return Err(io::Error::other("unexpected caller-deadline request kind")),
            };
            write_server_message(&mut stream, &response).map_err(io::Error::other)?;
            let mut byte = [0_u8; 1];
            assert_eq!(stream.read(&mut byte)?, 0, "expired carrier must retire");
            Ok(())
        })?;
        let observations = Arc::new(AtomicU64::new(0));
        let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
        let caller_deadline = clock.origin + SERVER_GUARD_TIMEOUT;
        let mut client = LocalClient::connect_with_submission_runtime(
            server.endpoint(),
            COHORT,
            Duration::from_millis(80),
            Box::new(clock),
            IoChunkLimits::UNBOUNDED,
        )?;
        let error = match kind {
            RequestKind::Viewer => client
                .submit_terminal_viewer_before(
                    TerminalViewerRequest::new(
                        RequestId::parse(FIRST_REQUEST_ID)?,
                        TerminalViewerAction::Poll {
                            attachment_id: ViewerId::parse(FIRST_REQUEST_ID)?,
                        },
                    ),
                    caller_deadline,
                )
                .expect_err("ordinary viewer budget must clamp a later caller deadline"),
            RequestKind::Control => client
                .submit_terminal_control_before(
                    TerminalControlRequest::new(
                        RequestId::parse(FIRST_REQUEST_ID)?,
                        TerminalControlAction::Acquire {
                            attachment_id: ViewerId::parse(FIRST_REQUEST_ID)?,
                        },
                    ),
                    caller_deadline,
                )
                .expect_err("ordinary control budget must clamp a later caller deadline"),
        };
        assert!(matches!(
            error,
            SubmissionError::OutcomeUnknown {
                request_id,
                reason: OutcomeUnknownReason::Read(error),
            } if request_id.as_str() == FIRST_REQUEST_ID
                && error.kind() == io::ErrorKind::TimedOut
        ));
        assert_eq!(client.generation(), None);
        server.finish()?;
        assert_eq!(observations.load(Ordering::Relaxed), 9);
    }
    Ok(())
}

#[test]
fn operation_clock_clones_share_the_injected_monotonic_source() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 38)?;
        let mut received = Vec::new();
        stream.read_to_end(&mut received)?;
        assert!(received.is_empty());
        Ok(())
    })?;
    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let origin = clock.origin;
    let client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        TEST_REQUEST_TIMEOUT,
        Box::new(clock),
        IoChunkLimits::UNBOUNDED,
    )?;
    let first = client.operation_clock();
    let second = first.clone();
    assert_eq!(first.now(), origin);
    assert_eq!(second.now(), origin + Duration::from_millis(10));
    assert_eq!(
        client.next_operation_deadline(),
        origin + Duration::from_millis(20) + TEST_REQUEST_TIMEOUT
    );
    assert_eq!(observations.load(Ordering::Relaxed), 3);
    assert_eq!(client.generation().map(NonZeroU64::get), Some(38));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn adjustable_clock_exposes_distinct_forward_and_cleanup_cutoffs() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 39)?;
        let mut received = Vec::new();
        stream.read_to_end(&mut received)?;
        assert!(received.is_empty());
        Ok(())
    })?;
    let elapsed_millis = Arc::new(AtomicU64::new(0));
    let client = connect_with_adjustable_clock(server.endpoint(), COHORT, elapsed_millis.clone())?;
    let clock = client.operation_clock();
    let origin = clock.now();

    elapsed_millis.store(45_000, Ordering::SeqCst);
    assert_eq!(clock.now(), origin + Duration::from_secs(45));
    elapsed_millis.store(50_000, Ordering::SeqCst);
    assert_eq!(clock.now(), origin + Duration::from_secs(50));

    assert_eq!(client.generation().map(NonZeroU64::get), Some(39));
    drop(client);
    server.finish()?;
    Ok(())
}

// ---- borrowed wake descriptor (Agent cancellation owner) ------------------

/// A `DeadlineIo` over the admitted connection plus a fresh deadline; the
/// borrowed wake descriptor only joins the wait set.
fn deadline_io<'a>(
    connection: &'a mut ClientSessionConnection,
    clock: &'a LocalOperationClock,
    deadline: SubmissionDeadline,
    wake: Option<BorrowedFd<'a>>,
) -> DeadlineIo<'a> {
    DeadlineIo {
        connection,
        clock,
        deadline,
        io_chunk_limits: IoChunkLimits::UNBOUNDED,
        wake,
        write_began: false,
    }
}

/// A peer that completes OBLC admission then holds the session open until
/// the client drops it — no OBAP/OBCS bytes ever arrive.
fn hold_after_admission(stream: &mut UnixStream) -> io::Result<()> {
    expect_hello(stream, COHORT)?;
    admit(stream, 7)?;
    let mut sink = [0_u8; 8];
    stream.read(&mut sink).map(|_| ())
}

#[test]
fn wake_readiness_interrupts_the_exchange_wait() -> TestResult {
    let server = FakeServer::start(|mut stream| hold_after_admission(&mut stream))?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    wake_write.write_all(b"x")?;
    let clock = client.operation_clock();
    let deadline = SubmissionDeadline::start(&clock, Duration::from_secs(5))?;
    let io = deadline_io(
        client.connection.as_mut().expect("admitted connection"),
        &clock,
        deadline,
        Some(wake_read.as_fd()),
    );
    let error = io
        .wait_until_ready(PollFlags::POLLIN)
        .expect_err("wake readiness interrupts the wait");
    assert!(exchange_wake_ready(&error), "readiness marker: {error:?}");
    assert!(!exchange_wake_fault(&error), "not a fault: {error:?}");
    // The byte is never drained — the owner's arbitration read still finds it.
    let mut byte = [0_u8; 1];
    (&wake_read).read_exact(&mut byte)?;
    assert_eq!(&byte, b"x");
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn wake_readiness_wins_over_simultaneous_socket_data() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 7)?;
        stream.write_all(b"noise")?;
        let mut sink = [0_u8; 8];
        // The client drops with the noise byte unread, so this read may fail
        // with ECONNRESET — either outcome means the peer is gone.
        let _ = stream.read(&mut sink);
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    wake_write.write_all(b"x")?;
    let clock = client.operation_clock();
    let deadline = SubmissionDeadline::start(&clock, Duration::from_secs(5))?;
    let io = deadline_io(
        client.connection.as_mut().expect("admitted connection"),
        &clock,
        deadline,
        Some(wake_read.as_fd()),
    );
    let error = io
        .wait_until_ready(PollFlags::POLLIN)
        .expect_err("wake readiness wins over socket readiness");
    assert!(exchange_wake_ready(&error), "readiness marker: {error:?}");
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn wake_fault_is_distinct_from_readiness() -> TestResult {
    let server = FakeServer::start(|mut stream| hold_after_admission(&mut stream))?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    // A pipe, not a socket: with nothing written and the write end closed, the
    // read end reports POLLHUP without POLLIN — a stream socket would instead
    // report the peer's EOF as readable POLLIN.
    let (wake_read, wake_write) = nix::unistd::pipe()?;
    drop(wake_write);
    let clock = client.operation_clock();
    let deadline = SubmissionDeadline::start(&clock, Duration::from_secs(5))?;
    let io = deadline_io(
        client.connection.as_mut().expect("admitted connection"),
        &clock,
        deadline,
        Some(wake_read.as_fd()),
    );
    let error = io
        .wait_until_ready(PollFlags::POLLIN)
        .expect_err("a broken wake source reports a fault");
    assert!(exchange_wake_fault(&error), "fault marker: {error:?}");
    assert!(!exchange_wake_ready(&error), "not readiness: {error:?}");
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn wake_readiness_wins_over_simultaneous_hangup() -> TestResult {
    let server = FakeServer::start(|mut stream| hold_after_admission(&mut stream))?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    wake_write.write_all(b"x")?;
    drop(wake_write);
    let clock = client.operation_clock();
    let deadline = SubmissionDeadline::start(&clock, Duration::from_secs(5))?;
    let io = deadline_io(
        client.connection.as_mut().expect("admitted connection"),
        &clock,
        deadline,
        Some(wake_read.as_fd()),
    );
    let error = io
        .wait_until_ready(PollFlags::POLLIN)
        .expect_err("readable data still wins over POLLHUP");
    assert!(exchange_wake_ready(&error), "readiness marker: {error:?}");
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn absent_wake_descriptor_preserves_the_plain_wait() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 7)?;
        stream.write_all(b"noise")?;
        let mut sink = [0_u8; 8];
        // The client drops with the noise byte unread, so this read may fail
        // with ECONNRESET — either outcome means the peer is gone.
        let _ = stream.read(&mut sink);
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let clock = client.operation_clock();
    let deadline = SubmissionDeadline::start(&clock, Duration::from_secs(5))?;
    let io = deadline_io(
        client.connection.as_mut().expect("admitted connection"),
        &clock,
        deadline,
        None,
    );
    io.wait_until_ready(PollFlags::POLLIN)
        .expect("socket readiness alone answers the wait");
    drop(client);
    server.finish()?;
    Ok(())
}

/// A wake observation before the first request byte keeps the request
/// unsubmitted: the marker surfaces at the `RequestNotSubmitted` position.
#[test]
fn agent_read_wake_ready_before_write_is_request_not_submitted() -> TestResult {
    let server = FakeServer::start(|mut stream| hold_after_admission(&mut stream))?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    wake_write.write_all(b"x")?;
    let request = AgentReadRequest::List {
        request_id: ObapRequestId::parse(FIRST_REQUEST_ID).expect("request id"),
        include_ended: false,
        limit: AgentSafeInteger::new(50),
        cursor: None,
    };
    match client.submit_agent_read_before_cancellable(
        request,
        Instant::now() + Duration::from_secs(5),
        Some(wake_read.as_fd()),
    ) {
        Err(AgentReadError::RequestNotSubmitted(error)) => {
            assert!(exchange_wake_ready(&error), "readiness marker: {error:?}")
        }
        other => panic!("expected RequestNotSubmitted wake marker, got {other:?}"),
    }
    drop(client);
    server.finish()?;
    Ok(())
}

/// A wake observation inside the response wait surfaces inside
/// `AgentReadError::Failed` through `AgentWireError::Io` — the marker
/// traverses every error layer without being read or retried.
#[test]
fn agent_read_wake_ready_mid_response_surfaces_through_io() -> TestResult {
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 7)?;
        read_agent_request(&mut stream).map_err(io::Error::other)?;
        // The request is fully on the wire; now the owner-side byte arrives
        // inside the client's response wait.
        wake_write.write_all(b"x")?;
        let mut sink = [0_u8; 8];
        stream.read(&mut sink).map(|_| ())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let request = AgentReadRequest::List {
        request_id: ObapRequestId::parse(FIRST_REQUEST_ID).expect("request id"),
        include_ended: false,
        limit: AgentSafeInteger::new(50),
        cursor: None,
    };
    match client.submit_agent_read_before_cancellable(
        request,
        Instant::now() + Duration::from_secs(5),
        Some(wake_read.as_fd()),
    ) {
        Err(AgentReadError::Failed { reason, .. }) => match reason {
            AgentReadFailure::Response(AgentWireError::Io(error)) => {
                assert!(exchange_wake_ready(&error), "readiness marker: {error:?}")
            }
            other => panic!("expected Response(Io) wake marker, got {other:?}"),
        },
        other => panic!("expected Failed(Response(Io)), got {other:?}"),
    }
    drop(client);
    server.finish()?;
    Ok(())
}

/// A names-family result inside a read exchange is an unexpected response:
/// requestId correlation is verified first, then the session retires with
/// `UnexpectedResponse` and the connection is taken — the request is never
/// replayed on a session whose answer shape is already wrong.
#[test]
fn agent_read_names_family_result_retires_the_session() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 7)?;
        let request = read_agent_request(&mut stream).map_err(io::Error::other)?;
        write_agent_result(
            &mut stream,
            &AgentResult::Names(AgentNamesResult::NamesList {
                request_id: request.request_id().clone(),
                result: AgentNamesListOutcomeEnvelope {
                    outcome: AgentNamesListOutcome::Unavailable {
                        reason: AgentUnavailableReason::RuntimeUnavailable,
                    },
                },
            }),
        )
        .map_err(io::Error::other)?;
        // The client retires the session on the family mismatch; the peer
        // observes the drop as EOF.
        let mut sink = [0_u8; 8];
        stream.read(&mut sink).map(|_| ())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let request = AgentReadRequest::List {
        request_id: ObapRequestId::parse(FIRST_REQUEST_ID).expect("request id"),
        include_ended: false,
        limit: AgentSafeInteger::new(50),
        cursor: None,
    };
    match client.submit_agent_read_before_cancellable(
        request,
        Instant::now() + Duration::from_secs(5),
        None,
    ) {
        Err(AgentReadError::Failed { request_id, reason }) => {
            assert_eq!(request_id.as_str(), FIRST_REQUEST_ID);
            assert!(
                matches!(reason, AgentReadFailure::UnexpectedResponse),
                "expected UnexpectedResponse, got {reason:?}"
            );
        }
        other => panic!("expected Failed(UnexpectedResponse), got {other:?}"),
    }
    assert!(
        client.connection.is_none(),
        "the session connection is taken"
    );
    drop(client);
    server.finish()?;
    Ok(())
}
