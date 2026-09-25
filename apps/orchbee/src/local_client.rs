#[cfg(any(target_os = "linux", windows))]
use std::path::PathBuf;
use std::{
    fmt,
    io::{self, Read, Write},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use orchbee_local_ipc::{AsLocalIo, LocalIo as BorrowedFd, PollFd, PollFlags, PollTimeout, poll};
#[cfg(unix)]
use std::os::fd::AsFd;

use orchbee_client_server_protocol::{
    ClientMessage, ProtocolError, ReadMessageError, RequestId, ServerMessage,
    TerminalControlAction, TerminalControlOutcome, TerminalControlRequest, TerminalControlResult,
    TerminalCreateRequest, TerminalCreateResult, TerminalLaunchReadRequest,
    TerminalLaunchReadResult, TerminalSendRequest, TerminalSendResult, TerminalStartRequest,
    TerminalStartResult, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    TerminalViewerResult, encode_client_frame, read_server_message,
};
use orchbee_local_client_protocol::{ClientSessionConnection, ConnectClientError};

const REQUEST_TOTAL_TIMEOUT: Duration = Duration::from_secs(50);

pub(crate) struct LocalClientShutdown(orchbee_local_ipc::LocalStream);
impl LocalClientShutdown {
    pub(crate) fn close(&self) -> io::Result<()> {
        self.0.shutdown(std::net::Shutdown::Both)
    }
}

#[derive(Debug)]
pub(crate) struct LocalClient {
    #[cfg(any(target_os = "linux", windows))]
    origin: LocalClientOrigin,
    connection: Option<ClientSessionConnection>,
    request_timeout: Duration,
    submission_deadline: Option<Instant>,
    clock: LocalOperationClock,
    io_chunk_limits: IoChunkLimits,
}

#[cfg(any(target_os = "linux", windows))]
#[derive(Clone, Debug)]
pub(crate) struct LocalClientOrigin {
    endpoint: PathBuf,
    cohort: String,
}

#[cfg(any(target_os = "linux", windows))]
impl LocalClientOrigin {
    pub(crate) fn reconnect_before(
        &self,
        deadline: Instant,
        clock: LocalOperationClock,
    ) -> Result<LocalClient, LocalClientConnectError> {
        let mut client = LocalClient::connect_before(&self.endpoint, &self.cohort, deadline)?;
        client.clock = clock;
        Ok(client)
    }

    /// `reconnect_before` with the owner's borrowed wake descriptor joined
    /// into the admission wait set — the narrow second-lane variant: it only
    /// forwards the private endpoint/cohort to the existing cancellable
    /// admission and reinstates the caller's clock, copying nothing else.
    pub(crate) fn reconnect_before_cancellable(
        &self,
        deadline: Instant,
        clock: LocalOperationClock,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<LocalClient, LocalClientConnectError> {
        let mut client =
            LocalClient::connect_before_cancellable(&self.endpoint, &self.cohort, deadline, wake)?;
        client.clock = clock;
        Ok(client)
    }
}

impl LocalClient {
    #[cfg(any(target_os = "linux", windows))]
    pub(crate) fn connect_before(
        endpoint: &Path,
        cohort: &str,
        deadline: Instant,
    ) -> Result<Self, LocalClientConnectError> {
        Self::connect_before_cancellable(endpoint, cohort, deadline, None)
    }

    /// `connect_before` with the signal owner's borrowed wake descriptor
    /// joined into the admission wait set: the protocol only polls it and
    /// reports provisional wake observations; the caller performs the sole
    /// arbitration read after the session is dropped.
    #[cfg(any(target_os = "linux", windows))]
    pub(crate) fn connect_before_cancellable(
        endpoint: &Path,
        cohort: &str,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self, LocalClientConnectError> {
        let connection =
            ClientSessionConnection::connect_before_cancellable(endpoint, cohort, deadline, wake)
                .map_err(LocalClientConnectError::Admission)?;
        Self::from_connection(
            connection,
            #[cfg(any(target_os = "linux", windows))]
            LocalClientOrigin {
                endpoint: endpoint.to_path_buf(),
                cohort: cohort.to_owned(),
            },
            REQUEST_TOTAL_TIMEOUT,
            Box::new(SystemMonotonicClock),
            IoChunkLimits::UNBOUNDED,
        )
    }

    pub(crate) fn connect(endpoint: &Path, cohort: &str) -> Result<Self, LocalClientConnectError> {
        Self::connect_with_request_timeout(endpoint, cohort, REQUEST_TOTAL_TIMEOUT)
    }

    fn connect_with_request_timeout(
        endpoint: &Path,
        cohort: &str,
        request_timeout: Duration,
    ) -> Result<Self, LocalClientConnectError> {
        Self::connect_with_submission_runtime(
            endpoint,
            cohort,
            request_timeout,
            Box::new(SystemMonotonicClock),
            IoChunkLimits::UNBOUNDED,
        )
    }

    fn connect_with_submission_runtime(
        endpoint: &Path,
        cohort: &str,
        request_timeout: Duration,
        clock: Box<dyn MonotonicClock>,
        io_chunk_limits: IoChunkLimits,
    ) -> Result<Self, LocalClientConnectError> {
        let connection = ClientSessionConnection::connect(endpoint, cohort)
            .map_err(LocalClientConnectError::Admission)?;
        Self::from_connection(
            connection,
            #[cfg(any(target_os = "linux", windows))]
            LocalClientOrigin {
                endpoint: endpoint.to_path_buf(),
                cohort: cohort.to_owned(),
            },
            request_timeout,
            clock,
            io_chunk_limits,
        )
    }

    fn from_connection(
        connection: ClientSessionConnection,
        #[cfg(any(target_os = "linux", windows))] origin: LocalClientOrigin,
        request_timeout: Duration,
        clock: Box<dyn MonotonicClock>,
        io_chunk_limits: IoChunkLimits,
    ) -> Result<Self, LocalClientConnectError> {
        connection
            .set_nonblocking(true)
            .map_err(LocalClientConnectError::RequestIoConfiguration)?;
        Ok(Self {
            #[cfg(any(target_os = "linux", windows))]
            origin,
            connection: Some(connection),
            request_timeout,
            submission_deadline: None,
            clock: LocalOperationClock(Arc::from(clock)),
            io_chunk_limits,
        })
    }

    pub(crate) fn submission_deadline(&self) -> Option<Instant> {
        self.submission_deadline
    }

    #[cfg(any(target_os = "linux", windows))]
    pub(crate) fn origin(&self) -> LocalClientOrigin {
        self.origin.clone()
    }

    pub(crate) fn operation_clock(&self) -> LocalOperationClock {
        self.clock.clone()
    }

    pub(crate) fn next_operation_deadline(&self) -> Instant {
        let deadline = self.clock.now() + self.request_timeout;
        self.submission_deadline
            .map_or(deadline, |ceiling| deadline.min(ceiling))
    }

    pub(crate) fn set_submission_deadline(&mut self, deadline: Option<Instant>) {
        self.submission_deadline = deadline;
    }

    pub(crate) fn shutdown_handle(&self) -> io::Result<LocalClientShutdown> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| io::Error::other("Client Session unavailable"))?;
        #[cfg(unix)]
        {
            Ok(LocalClientShutdown(std::os::unix::net::UnixStream::from(
                connection.as_fd().try_clone_to_owned()?,
            )))
        }
        #[cfg(windows)]
        {
            Ok(LocalClientShutdown(connection.shutdown_stream()?))
        }
    }

    #[must_use]
    pub(crate) fn generation(&self) -> Option<std::num::NonZeroU64> {
        self.connection
            .as_ref()
            .map(ClientSessionConnection::generation)
    }

    pub(crate) fn submit_terminal_open(
        &mut self,
        request: orchbee_client_server_protocol::TerminalOpenRequest,
    ) -> Result<orchbee_client_server_protocol::TerminalOpenResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let result = self.exchange(ClientMessage::TerminalOpen(request), &request_id)?;
        let ServerMessage::TerminalOpenResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::Open,
                    received: result_kind(&result),
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_terminal_create(
        &mut self,
        request: TerminalCreateRequest,
    ) -> Result<TerminalCreateResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let result = self.exchange(ClientMessage::TerminalCreate(request), &request_id)?;
        let ServerMessage::TerminalCreateResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::Create,
                    received: result_kind(&result),
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_terminal_start(
        &mut self,
        request: TerminalStartRequest,
    ) -> Result<TerminalStartResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let result = self.exchange(ClientMessage::TerminalStart(request), &request_id)?;
        let ServerMessage::TerminalStartResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::Start,
                    received: result_kind(&result),
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_terminal_viewer(
        &mut self,
        request: TerminalViewerRequest,
    ) -> Result<TerminalViewerResult, SubmissionError> {
        self.submit_terminal_viewer_with_deadline(request, None)
    }

    pub(crate) fn submit_terminal_viewer_before(
        &mut self,
        request: TerminalViewerRequest,
        deadline: Instant,
    ) -> Result<TerminalViewerResult, SubmissionError> {
        self.submit_terminal_viewer_with_deadline(request, Some(deadline))
    }

    fn submit_terminal_viewer_with_deadline(
        &mut self,
        request: TerminalViewerRequest,
        deadline: Option<Instant>,
    ) -> Result<TerminalViewerResult, SubmissionError> {
        self.submit_terminal_viewer_cancellable(request, deadline, None)
    }

    pub(crate) fn submit_terminal_viewer_cancellable(
        &mut self,
        request: TerminalViewerRequest,
        deadline: Option<Instant>,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<TerminalViewerResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let action = request.action().clone();
        let message = ClientMessage::TerminalViewer(request);
        let result = self.exchange_with_wake(message, &request_id, deadline, wake)?;
        let ServerMessage::TerminalViewerResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::Viewer,
                    received: result_kind(&result),
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        let compatible = matches!(
            (&action, result.outcome()),
            (
                _,
                TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. }
            ) | (
                TerminalViewerAction::Attach { .. } | TerminalViewerAction::AttachBinding { .. },
                TerminalViewerOutcome::Attached { .. }
            ) | (
                TerminalViewerAction::Poll { .. },
                TerminalViewerOutcome::Empty {} | TerminalViewerOutcome::Record(_)
            ) | (
                TerminalViewerAction::FrameApplied { .. }
                    | TerminalViewerAction::NeedSnapshot { .. },
                TerminalViewerOutcome::FeedbackAccepted {}
            ) | (
                TerminalViewerAction::ReadProcessInfo { .. },
                TerminalViewerOutcome::Record(_)
            ) | (
                TerminalViewerAction::ReadRows { .. }
                    | TerminalViewerAction::ReadViewportRows { .. },
                TerminalViewerOutcome::Record(_) | TerminalViewerOutcome::RowsRejected { .. }
            ) | (
                TerminalViewerAction::SetProjectionDemand { .. }
                    | TerminalViewerAction::SetHistoryDemand { .. }
                    | TerminalViewerAction::RowsApplied { .. }
                    | TerminalViewerAction::HistoryObserved { .. },
                TerminalViewerOutcome::FeedbackAccepted {}
            ) | (
                TerminalViewerAction::Detach { .. },
                TerminalViewerOutcome::Detached {}
            )
        );
        if !compatible {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::InvalidResult(ProtocolError::InvalidPayload),
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_terminal_control(
        &mut self,
        request: TerminalControlRequest,
    ) -> Result<TerminalControlResult, SubmissionError> {
        self.submit_terminal_control_with_deadline(request, None)
    }

    pub(crate) fn submit_terminal_control_before(
        &mut self,
        request: TerminalControlRequest,
        deadline: Instant,
    ) -> Result<TerminalControlResult, SubmissionError> {
        self.submit_terminal_control_with_deadline(request, Some(deadline))
    }

    fn submit_terminal_control_with_deadline(
        &mut self,
        request: TerminalControlRequest,
        deadline: Option<Instant>,
    ) -> Result<TerminalControlResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let action = request.action().clone();
        let message = ClientMessage::TerminalControl(request);
        let result = match deadline {
            Some(deadline) => self.exchange_before(message, &request_id, deadline)?,
            None => self.exchange(message, &request_id)?,
        };
        let ServerMessage::TerminalControlResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::Control,
                    received: result_kind(&result),
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        let compatible = match (&action, result.outcome()) {
            (
                _,
                TerminalControlOutcome::Rejected { .. }
                | TerminalControlOutcome::Unavailable { .. }
                | TerminalControlOutcome::OutcomeUnknown {},
            )
            | (TerminalControlAction::Acquire { .. }, TerminalControlOutcome::Acquired { .. })
            | (TerminalControlAction::Input { .. }, TerminalControlOutcome::InputAccepted {}) => {
                true
            }
            (
                TerminalControlAction::Resize { geometry, .. },
                TerminalControlOutcome::Resized { geometry: actual },
            ) => geometry == actual,
            _ => false,
        };
        if !compatible {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::InvalidResult(ProtocolError::InvalidPayload),
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_terminal_send_before(
        &mut self,
        request: TerminalSendRequest,
        deadline: Instant,
    ) -> Result<TerminalSendResult, SubmissionError> {
        self.submit_terminal_send_with_deadline(request, Some(deadline))
    }

    fn submit_terminal_send_with_deadline(
        &mut self,
        request: TerminalSendRequest,
        deadline: Option<Instant>,
    ) -> Result<TerminalSendResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let message = ClientMessage::TerminalSend(request);
        let result = match deadline {
            Some(deadline) => self.exchange_before(message, &request_id, deadline)?,
            None => self.exchange(message, &request_id)?,
        };
        let ServerMessage::TerminalSendResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::Send,
                    received: result_kind(&result),
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        // Every typed outcome variant is a legitimate send answer; impossible
        // input/cleanup pairs were already refused at the wire boundary.
        Ok(result)
    }

    pub(crate) fn submit_terminal_launch_read_before(
        &mut self,
        request: TerminalLaunchReadRequest,
        deadline: Instant,
    ) -> Result<TerminalLaunchReadResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let message = ClientMessage::TerminalLaunchRead(request);
        let result = self.exchange_before(message, &request_id, deadline)?;
        let ServerMessage::TerminalLaunchReadResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::LaunchRead,
                    received: result_kind(&result),
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        // Every typed outcome variant is a legitimate launch-read answer; the
        // conditional-null invariants were already enforced at the wire boundary.
        Ok(result)
    }

    fn exchange(
        &mut self,
        request: ClientMessage,
        request_id: &RequestId,
    ) -> Result<ServerMessage, SubmissionError> {
        self.exchange_with_deadline(request, request_id, None)
    }

    fn exchange_before(
        &mut self,
        request: ClientMessage,
        request_id: &RequestId,
        deadline: Instant,
    ) -> Result<ServerMessage, SubmissionError> {
        self.exchange_with_deadline(request, request_id, Some(deadline))
    }

    fn exchange_with_deadline(
        &mut self,
        request: ClientMessage,
        request_id: &RequestId,
        caller_deadline: Option<Instant>,
    ) -> Result<ServerMessage, SubmissionError> {
        self.exchange_with_wake(request, request_id, caller_deadline, None)
    }

    fn exchange_with_wake(
        &mut self,
        request: ClientMessage,
        request_id: &RequestId,
        caller_deadline: Option<Instant>,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<ServerMessage, SubmissionError> {
        if self.connection.is_none() {
            return Err(SubmissionError::SessionUnavailable);
        }

        let frame = encode_client_frame(&request).map_err(SubmissionError::RequestEncoding)?;
        let deadline = match caller_deadline {
            Some(deadline) => self
                .submission_deadline_before(deadline)
                .map_err(SubmissionError::RequestNotSubmitted)?,
            None => {
                let mut deadline = SubmissionDeadline::start(&self.clock, self.request_timeout)
                    .map_err(SubmissionError::RequestNotSubmitted)?;
                if let Some(startup) = self.submission_deadline {
                    deadline.expires_at = deadline.expires_at.min(startup);
                }
                deadline
            }
        };
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(SubmissionError::SessionUnavailable);
            };
            exchange_once(
                connection,
                &self.clock,
                deadline,
                self.io_chunk_limits,
                wake,
                &frame,
                |io| read_server_message(io),
            )
        };

        match attempt {
            ExchangeAttempt::Result(result) => Ok(result),
            ExchangeAttempt::NotSubmitted(error) => {
                Err(SubmissionError::RequestNotSubmitted(error))
            }
            ExchangeAttempt::WriteFailed(error) => {
                self.retire_with_unknown(request_id.clone(), OutcomeUnknownReason::Write(error))
            }
            ExchangeAttempt::ReadFailed(error) => match error {
                ReadMessageError::Io(error) => {
                    self.retire_with_unknown(request_id.clone(), OutcomeUnknownReason::Read(error))
                }
                ReadMessageError::Protocol(error) => self.retire_with_unknown(
                    request_id.clone(),
                    OutcomeUnknownReason::InvalidResult(error),
                ),
            },
            ExchangeAttempt::ReadDeadline(error) => {
                self.retire_with_unknown(request_id.clone(), OutcomeUnknownReason::Read(error))
            }
        }
    }

    fn submission_deadline_before(&self, deadline: Instant) -> io::Result<SubmissionDeadline> {
        let mut bounded = SubmissionDeadline::start(&self.clock, self.request_timeout)?;
        bounded.expires_at = bounded.expires_at.min(deadline);
        if let Some(startup) = self.submission_deadline {
            bounded.expires_at = bounded.expires_at.min(startup);
        }
        Ok(bounded)
    }

    fn retire_with_unknown<Value>(
        &mut self,
        request_id: RequestId,
        reason: OutcomeUnknownReason,
    ) -> Result<Value, SubmissionError> {
        self.connection.take();
        Err(SubmissionError::OutcomeUnknown { request_id, reason })
    }
}

#[derive(Debug)]
pub(crate) enum LocalClientConnectError {
    Admission(ConnectClientError),
    RequestIoConfiguration(io::Error),
}

impl fmt::Display for LocalClientConnectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(formatter, "local Client admission failed: {error}"),
            Self::RequestIoConfiguration(error) => {
                write!(formatter, "local Client request I/O setup failed: {error}")
            }
        }
    }
}

impl std::error::Error for LocalClientConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::RequestIoConfiguration(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum SubmissionError {
    SessionUnavailable,
    RequestEncoding(ProtocolError),
    RequestNotSubmitted(io::Error),
    OutcomeUnknown {
        request_id: RequestId,
        reason: OutcomeUnknownReason,
    },
}

impl fmt::Display for SubmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionUnavailable => formatter.write_str("local Client Session is unavailable"),
            Self::RequestEncoding(error) => {
                write!(
                    formatter,
                    "OBCS request encoding failed before submission: {error}"
                )
            }
            Self::RequestNotSubmitted(error) => write!(
                formatter,
                "OBCS request failed before any transport handoff: {error}"
            ),
            Self::OutcomeUnknown { request_id, reason } => {
                write!(
                    formatter,
                    "OBCS request {request_id} outcome is unknown: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for SubmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
            Self::OutcomeUnknown { reason, .. } => reason.source(),
            Self::SessionUnavailable => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum OutcomeUnknownReason {
    Write(io::Error),
    Read(io::Error),
    InvalidResult(ProtocolError),
    UnexpectedResult {
        expected: ResultKind,
        received: ResultKind,
    },
    CorrelationMismatch {
        expected: RequestId,
        received: RequestId,
    },
}

impl fmt::Display for OutcomeUnknownReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Read(error) => write!(formatter, "result read failed: {error}"),
            Self::InvalidResult(error) => write!(formatter, "invalid Server result: {error}"),
            Self::UnexpectedResult { expected, received } => {
                write!(formatter, "expected {expected} but received {received}")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                formatter,
                "expected request identity {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for OutcomeUnknownReason {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::Read(error) => Some(error),
            Self::InvalidResult(error) => Some(error),
            Self::UnexpectedResult { .. } | Self::CorrelationMismatch { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultKind {
    Open,
    Create,
    Start,
    Viewer,
    Control,
    Send,
    LaunchRead,
}

impl fmt::Display for ResultKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Open => "terminal.open.result",
            Self::Create => "terminal.create.result",
            Self::Start => "terminal.start.result",
            Self::Viewer => "terminal.viewer.result",
            Self::Control => "terminal.control.result",
            Self::Send => "terminal.send.result",
            Self::LaunchRead => "terminal.launch.read.result",
        })
    }
}

fn result_kind(result: &ServerMessage) -> ResultKind {
    match result {
        ServerMessage::TerminalOpenResult(_) => ResultKind::Open,
        ServerMessage::TerminalCreateResult(_) => ResultKind::Create,
        ServerMessage::TerminalStartResult(_) => ResultKind::Start,
        ServerMessage::TerminalViewerResult(_) => ResultKind::Viewer,
        ServerMessage::TerminalControlResult(_) => ResultKind::Control,
        ServerMessage::TerminalSendResult(_) => ResultKind::Send,
        ServerMessage::TerminalLaunchReadResult(_) => ResultKind::LaunchRead,
    }
}

#[derive(Clone, Copy, Debug)]
struct SubmissionDeadline {
    expires_at: Instant,
}

impl SubmissionDeadline {
    fn start(clock: &dyn MonotonicClock, timeout: Duration) -> io::Result<Self> {
        let expires_at = clock.now().checked_add(timeout).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "OBCS request deadline exceeds the monotonic clock range",
            )
        })?;
        Ok(Self { expires_at })
    }

    fn remaining(self, clock: &dyn MonotonicClock) -> io::Result<Duration> {
        match self.expires_at.checked_duration_since(clock.now()) {
            Some(remaining) if !remaining.is_zero() => Ok(remaining),
            Some(_) | None => Err(deadline_elapsed()),
        }
    }
}

fn deadline_elapsed() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "local Client request total deadline elapsed",
    )
}

trait MonotonicClock: fmt::Debug + Send + Sync {
    fn now(&self) -> Instant;

    #[cfg(test)]
    fn before_request_write(&self) {}
}

#[derive(Clone, Debug)]
pub(crate) struct LocalOperationClock(Arc<dyn MonotonicClock>);

impl LocalOperationClock {
    #[must_use]
    pub(crate) fn now(&self) -> Instant {
        self.0.now()
    }
}

impl MonotonicClock for LocalOperationClock {
    fn now(&self) -> Instant {
        self.0.now()
    }

    #[cfg(test)]
    fn before_request_write(&self) {
        self.0.before_request_write();
    }
}

#[derive(Debug)]
struct SystemMonotonicClock;

impl MonotonicClock for SystemMonotonicClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Clone, Copy, Debug)]
struct IoChunkLimits {
    maximum_read: usize,
    maximum_write: usize,
}

impl IoChunkLimits {
    const UNBOUNDED: Self = Self {
        maximum_read: usize::MAX,
        maximum_write: usize::MAX,
    };

    #[cfg(test)]
    const fn new(maximum_read: usize, maximum_write: usize) -> Self {
        Self {
            maximum_read,
            maximum_write,
        }
    }
}

#[derive(Debug)]
enum ExchangeAttempt<ResultType, ReadError> {
    Result(ResultType),
    NotSubmitted(io::Error),
    WriteFailed(io::Error),
    ReadFailed(ReadError),
    ReadDeadline(io::Error),
}

fn exchange_once<ResultType, ReadError>(
    connection: &mut ClientSessionConnection,
    clock: &dyn MonotonicClock,
    deadline: SubmissionDeadline,
    io_chunk_limits: IoChunkLimits,
    wake: Option<BorrowedFd<'_>>,
    frame: &[u8],
    read_result: impl FnOnce(&mut DeadlineIo<'_>) -> Result<ResultType, ReadError>,
) -> ExchangeAttempt<ResultType, ReadError> {
    let mut io = DeadlineIo {
        connection,
        clock,
        deadline,
        io_chunk_limits,
        wake,
        write_began: false,
    };

    #[cfg(test)]
    clock.before_request_write();
    if let Err(error) = io.write_all(frame) {
        return if io.write_began {
            ExchangeAttempt::WriteFailed(error)
        } else {
            ExchangeAttempt::NotSubmitted(error)
        };
    }
    if let Err(error) = deadline.remaining(clock) {
        return ExchangeAttempt::WriteFailed(error);
    }

    match read_result(&mut io) {
        Ok(result) => match deadline.remaining(clock) {
            Ok(_remaining) => ExchangeAttempt::Result(result),
            Err(error) => ExchangeAttempt::ReadDeadline(error),
        },
        Err(error) => ExchangeAttempt::ReadFailed(error),
    }
}

struct DeadlineIo<'a> {
    connection: &'a mut ClientSessionConnection,
    clock: &'a dyn MonotonicClock,
    deadline: SubmissionDeadline,
    io_chunk_limits: IoChunkLimits,
    /// The signal owner's borrowed wake descriptor — only ever polled, never
    /// owned, read, drained, or closed here.
    wake: Option<BorrowedFd<'a>>,
    write_began: bool,
}

/// Provisional wake observation private to the Client exchange path: the
/// borrowed descriptor reported readable data. The armed signal owner still
/// performs the sole arbitration read — this marker is never a verdict.
#[derive(Debug)]
struct ExchangeWakeReady;

impl fmt::Display for ExchangeWakeReady {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the borrowed wake descriptor is readable")
    }
}

impl std::error::Error for ExchangeWakeReady {}

/// The borrowed wake descriptor reported a fault flag with no readable data
/// left — the owner's arbitration read could never observe a capture, so
/// the observation is a source fault, not readiness.
#[derive(Debug)]
struct ExchangeWakeFault(PollFlags);

impl fmt::Display for ExchangeWakeFault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the borrowed wake descriptor faulted: {:?}",
            self.0
        )
    }
}

impl std::error::Error for ExchangeWakeFault {}

/// Whether an exchange I/O error carries the private wake-ready marker —
/// the armed owner's discriminator for "descriptor readable, no signal
/// read yet" once the span ends clear.
pub(crate) fn exchange_wake_ready(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|inner| inner.is::<ExchangeWakeReady>())
}

/// Whether an exchange I/O error carries the private wake-fault marker.
pub(crate) fn exchange_wake_fault(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|inner| inner.is::<ExchangeWakeFault>())
}

/// Test-only marker constructors: the query classifiers are provable only
/// against the real private marker payloads.
#[cfg(test)]
pub(crate) fn wake_ready_io() -> io::Error {
    io::Error::other(ExchangeWakeReady)
}

#[cfg(test)]
pub(crate) fn wake_fault_io() -> io::Error {
    io::Error::other(ExchangeWakeFault(PollFlags::POLLHUP))
}

impl DeadlineIo<'_> {
    fn wait_until_ready(&self, interest: PollFlags) -> io::Result<()> {
        loop {
            let remaining = self.deadline.remaining(self.clock)?;
            let timeout = poll_timeout(remaining);
            let Some(wake) = self.wake else {
                let mut descriptors = [PollFd::new(self.connection.as_local_io(), interest)];
                match poll(&mut descriptors, timeout) {
                    Ok(0) => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Ok(_ready) => {
                        self.deadline.remaining(self.clock)?;
                        return Ok(());
                    }
                    Err(error) => return Err(error),
                }
                continue;
            };
            // The borrowed wake descriptor joins the same deadline-bounded
            // wait set; its readiness is decided before simultaneous socket
            // readiness so the owner observes it first.
            let mut descriptors = [
                PollFd::new(self.connection.as_local_io(), interest),
                PollFd::new(wake, PollFlags::POLLIN),
            ];
            match poll(&mut descriptors, timeout) {
                Ok(0) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Ok(_ready) => {
                    let observed = descriptors[1].revents().unwrap_or(PollFlags::empty());
                    // Readable data still wins over simultaneous fault flags:
                    // the owner's arbitration read can still consume it.
                    if observed.contains(PollFlags::POLLIN) {
                        return Err(io::Error::other(ExchangeWakeReady));
                    }
                    if observed
                        .intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL)
                    {
                        return Err(io::Error::other(ExchangeWakeFault(observed)));
                    }
                    if descriptors[0]
                        .revents()
                        .unwrap_or(PollFlags::empty())
                        .is_empty()
                    {
                        continue;
                    }
                    self.deadline.remaining(self.clock)?;
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }
    }
}

fn poll_timeout(remaining: Duration) -> PollTimeout {
    let rounded_milliseconds = remaining.as_millis().saturating_add(u128::from(
        !remaining.subsec_nanos().is_multiple_of(1_000_000),
    ));
    PollTimeout::try_from(rounded_milliseconds).unwrap_or(PollTimeout::MAX)
}

impl Read for DeadlineIo<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.wait_until_ready(PollFlags::POLLIN)?;
        let chunk_length = buffer.len().min(self.io_chunk_limits.maximum_read);
        self.connection.read(&mut buffer[..chunk_length])
    }
}

impl Write for DeadlineIo<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.wait_until_ready(PollFlags::POLLOUT)?;
        let chunk_length = buffer.len().min(self.io_chunk_limits.maximum_write);
        self.write_began = true;
        self.connection.write(&buffer[..chunk_length])
    }

    fn flush(&mut self) -> io::Result<()> {
        self.wait_until_ready(PollFlags::POLLOUT)?;
        self.connection.flush()
    }
}

pub(crate) mod agent;
pub(crate) mod workbench;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_server;
