//! Owns admitted Client protocol exchanges, request correlation, absolute submission budgets,
//! and explicit pre-submission versus unknown-delivery failure classification. It does not
//! activate a Session, authorize Server effects, or treat local cancellation as retraction of
//! a delivered operation.

pub(crate) mod cancellation;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
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
    TerminalControlRequest, TerminalControlResult, TerminalLaunchReadRequest,
    TerminalLaunchReadResult, TerminalSendRequest, TerminalSendResult, TerminalViewerRequest,
    TerminalViewerResult, encode_client_frame, read_server_message,
};
use orchbee_local_client_protocol::{ClientConnection, ConnectClientError};

const REQUEST_TOTAL_TIMEOUT: Duration = Duration::from_secs(50);

pub(crate) enum LocalClientShutdown {
    Direct(orchbee_local_ipc::LocalStream),
    Multiplex(Arc<multiplex::Connection>),
    Lane(Arc<multiplex::Lane>, orchbee_local_ipc::LocalStream),
}
impl LocalClientShutdown {
    pub(crate) fn close(&self) -> io::Result<()> {
        match self {
            Self::Direct(stream) => stream.shutdown(std::net::Shutdown::Both),
            Self::Multiplex(connection) => connection.shutdown(),
            Self::Lane(lane, stream) => {
                lane.close();
                stream.shutdown(std::net::Shutdown::Both)
            }
        }
    }
}

pub(crate) mod multiplex;
pub(crate) mod subscription;
#[derive(Debug)]
enum ClientTransport {
    Direct(ClientConnection),
    Lane(ClientLane),
}
#[derive(Debug)]
struct ClientLane {
    connection: Arc<multiplex::Connection>,
    cancel: orchbee_local_ipc::LocalStream,
    shutdown: orchbee_local_ipc::LocalStream,
    owner: Arc<multiplex::Lane>,
    reply_generation: Option<std::num::NonZeroU64>,
}
impl ClientLane {
    fn new(connection: Arc<multiplex::Connection>) -> io::Result<Self> {
        let (cancel, shutdown) = orchbee_local_ipc::LocalStream::pair()?;
        Ok(Self {
            owner: connection.lane()?,
            reply_generation: None,
            connection,
            cancel,
            shutdown,
        })
    }
}
impl ClientTransport {
    fn generation(&self) -> std::num::NonZeroU64 {
        match self {
            Self::Direct(connection) => connection.generation(),
            Self::Lane(lane) => lane.connection.generation(),
        }
    }
    fn reply_generation(&self) -> Option<std::num::NonZeroU64> {
        match self {
            Self::Direct(connection) => Some(connection.generation()),
            Self::Lane(lane) => lane.reply_generation,
        }
    }
    fn as_local_io(&self) -> BorrowedFd<'_> {
        match self {
            Self::Direct(connection) => connection.as_local_io(),
            Self::Lane(_) => unreachable!("lane I/O is owned by the connection actor"),
        }
    }
}
impl Read for ClientTransport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Direct(connection) => connection.read(buffer),
            Self::Lane(_) => Err(io::ErrorKind::Unsupported.into()),
        }
    }
}
impl Write for ClientTransport {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self {
            Self::Direct(connection) => connection.write(buffer),
            Self::Lane(_) => Err(io::ErrorKind::Unsupported.into()),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Direct(connection) => connection.flush(),
            Self::Lane(_) => Ok(()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct LocalClient {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    origin: LocalClientOrigin,
    connection: Option<ClientTransport>,
    request_timeout: Duration,
    submission_deadline: Option<Instant>,
    clock: LocalOperationClock,
    io_chunk_limits: IoChunkLimits,
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[derive(Clone, Debug)]
pub(crate) struct LocalClientOrigin {
    endpoint: PathBuf,
    cohort: String,
    local: bool,
    multiplex: Option<Arc<multiplex::Connection>>,
    #[cfg(windows)]
    tunnel: Option<Arc<crate::machine_tunnel::Tunnel>>,
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
impl LocalClientOrigin {
    /// Locality belongs to the logical origin; OS transport ownership is separate.
    pub(crate) fn is_local(&self) -> bool {
        self.local
    }

    pub(crate) fn lane_before(
        &self,
        deadline: Instant,
        clock: LocalOperationClock,
    ) -> Result<LocalClient, LocalClientConnectError> {
        self.lane_before_cancellable(deadline, clock, None)
    }

    /// Creates a cancellable logical lane on a promoted TUI connection. Direct CLI origins
    /// retain their bounded admission adapter; TUI reconnect belongs exclusively to the actor.
    pub(crate) fn lane_before_cancellable(
        &self,
        deadline: Instant,
        clock: LocalOperationClock,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<LocalClient, LocalClientConnectError> {
        if let Some(connection) = &self.multiplex {
            if clock.now() >= deadline {
                return Err(LocalClientConnectError::RequestIoConfiguration(
                    io::ErrorKind::TimedOut.into(),
                ));
            }
            return Ok(LocalClient {
                origin: self.clone(),
                connection: Some(ClientTransport::Lane(
                    ClientLane::new(Arc::clone(connection))
                        .map_err(LocalClientConnectError::RequestIoConfiguration)?,
                )),
                request_timeout: REQUEST_TOTAL_TIMEOUT,
                submission_deadline: None,
                clock,
                io_chunk_limits: IoChunkLimits::UNBOUNDED,
            });
        }
        let mut client =
            LocalClient::connect_before_cancellable(&self.endpoint, &self.cohort, deadline, wake)?;
        client.origin = self.clone();
        client.clock = clock;
        Ok(client)
    }
}

impl LocalClient {
    /// Transfers the bootstrap stream once. Every derived lane shares this connection owner.
    pub(crate) fn into_multiplexed(mut self) -> io::Result<Self> {
        if let Some(transport) = self.connection.take() {
            let ClientTransport::Direct(connection) = transport else {
                self.connection = Some(transport);
                return Ok(self);
            };
            let connection = multiplex::Connection::start(connection, self.origin.clone())?;
            self.origin.multiplex = Some(Arc::clone(&connection));
            self.connection = Some(ClientTransport::Lane(ClientLane::new(connection)?));
        }
        Ok(self)
    }
    /// Retains the transport owner for lanes derived from this origin.
    #[cfg(windows)]
    pub(crate) fn with_tunnel(mut self, tunnel: Arc<crate::machine_tunnel::Tunnel>) -> Self {
        self.origin.local = false;
        self.origin.tunnel = Some(tunnel);
        self
    }
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
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
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) fn connect_before_cancellable(
        endpoint: &Path,
        cohort: &str,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self, LocalClientConnectError> {
        let connection =
            ClientConnection::connect_before_cancellable(endpoint, cohort, deadline, wake)
                .map_err(LocalClientConnectError::Admission)?;
        Self::from_connection(
            connection,
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            LocalClientOrigin {
                endpoint: endpoint.to_path_buf(),
                cohort: cohort.to_owned(),
                local: true,
                multiplex: None,
                #[cfg(windows)]
                tunnel: None,
            },
            REQUEST_TOTAL_TIMEOUT,
            Box::new(SystemMonotonicClock),
            IoChunkLimits::UNBOUNDED,
        )
    }

    fn from_connection(
        connection: ClientConnection,
        #[cfg(any(target_os = "linux", target_os = "macos", windows))] origin: LocalClientOrigin,
        request_timeout: Duration,
        clock: Box<dyn MonotonicClock>,
        io_chunk_limits: IoChunkLimits,
    ) -> Result<Self, LocalClientConnectError> {
        connection
            .set_nonblocking(true)
            .map_err(LocalClientConnectError::RequestIoConfiguration)?;
        Ok(Self {
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            origin,
            connection: Some(ClientTransport::Direct(connection)),
            request_timeout,
            submission_deadline: None,
            clock: LocalOperationClock(Arc::from(clock)),
            io_chunk_limits,
        })
    }

    pub(crate) fn submission_deadline(&self) -> Option<Instant> {
        self.submission_deadline
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
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

    pub(crate) fn connection_shutdown_handle(&self) -> io::Result<LocalClientShutdown> {
        match &self.connection {
            Some(ClientTransport::Lane(lane)) => {
                Ok(LocalClientShutdown::Multiplex(Arc::clone(&lane.connection)))
            }
            _ => self.shutdown_handle(),
        }
    }

    pub(crate) fn shutdown_handle(&self) -> io::Result<LocalClientShutdown> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| io::Error::other("Client Connection unavailable"))?;
        let ClientTransport::Direct(connection) = connection else {
            let ClientTransport::Lane(connection) = connection else {
                unreachable!()
            };
            return Ok(LocalClientShutdown::Lane(
                Arc::clone(&connection.owner),
                connection.shutdown.try_clone()?,
            ));
        };
        #[cfg(unix)]
        {
            Ok(LocalClientShutdown::Direct(
                std::os::unix::net::UnixStream::from(connection.as_fd().try_clone_to_owned()?),
            ))
        }
        #[cfg(windows)]
        {
            Ok(LocalClientShutdown::Direct(connection.shutdown_stream()?))
        }
    }

    #[must_use]
    pub(crate) fn generation(&self) -> Option<std::num::NonZeroU64> {
        self.connection.as_ref().map(ClientTransport::generation)
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

        if !action.admits(result.outcome()) {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::InvalidResult(ProtocolError::InvalidPayload),
            );
        }
        Ok(result)
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
        self.submit_terminal_control_cancellable(request, deadline, None)
    }

    pub(crate) fn submit_terminal_control_cancellable(
        &mut self,
        request: TerminalControlRequest,
        deadline: Option<Instant>,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<TerminalControlResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let action = request.action().clone();
        let message = ClientMessage::TerminalControl(request);
        let result = self.exchange_with_wake(message, &request_id, deadline, wake)?;
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
        if !action.admits(result.outcome()) {
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
        self.submit_terminal_send_cancellable(request, deadline, None)
    }

    pub(crate) fn submit_terminal_send_cancellable(
        &mut self,
        request: TerminalSendRequest,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<TerminalSendResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let message = ClientMessage::TerminalSend(request);
        let result = self.exchange_with_wake(message, &request_id, Some(deadline), wake)?;
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
        self.submit_terminal_launch_read_cancellable(request, deadline, None)
    }

    pub(crate) fn submit_terminal_launch_read_cancellable(
        &mut self,
        request: TerminalLaunchReadRequest,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<TerminalLaunchReadResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let message = ClientMessage::TerminalLaunchRead(request);
        let result = self.exchange_with_wake(message, &request_id, Some(deadline), wake)?;
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

    fn exchange_with_wake(
        &mut self,
        request: ClientMessage,
        request_id: &RequestId,
        caller_deadline: Option<Instant>,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<ServerMessage, SubmissionError> {
        if self.connection.is_none() {
            return Err(SubmissionError::ConnectionUnavailable);
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
                return Err(SubmissionError::ConnectionUnavailable);
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
    ConnectionUnavailable,
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
            Self::ConnectionUnavailable => {
                formatter.write_str("local Client Connection is unavailable")
            }
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
            Self::ConnectionUnavailable => None,
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
    connection: &mut ClientTransport,
    clock: &dyn MonotonicClock,
    deadline: SubmissionDeadline,
    io_chunk_limits: IoChunkLimits,
    wake: Option<BorrowedFd<'_>>,
    frame: &[u8],
    read_result: impl FnOnce(&mut DeadlineIo<'_>) -> Result<ResultType, ReadError>,
) -> ExchangeAttempt<ResultType, ReadError> {
    let buffered = if let ClientTransport::Lane(lane) = connection {
        // The queued-to-handoff CAS owns definite refusal versus possible delivery. Cancellation
        // never replays a request, and only the connection actor can write the physical stream.
        match lane.connection.exchange(
            lane.owner.id(),
            frame,
            deadline.expires_at,
            clock,
            wake,
            Some(lane.cancel.as_local_io()),
        ) {
            Ok((bytes, generation)) => {
                lane.reply_generation = Some(generation);
                Some(io::Cursor::new(bytes))
            }
            Err((false, error)) => return ExchangeAttempt::NotSubmitted(error),
            Err((true, error)) => return ExchangeAttempt::WriteFailed(error),
        }
    } else {
        None
    };
    let mut io = DeadlineIo {
        connection,
        clock,
        deadline,
        io_chunk_limits,
        wake,
        write_began: false,
        buffered,
    };

    #[cfg(test)]
    clock.before_request_write();
    if io.buffered.is_none()
        && let Err(error) = io.write_all(frame)
    {
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
    connection: &'a mut ClientTransport,
    buffered: Option<io::Cursor<Vec<u8>>>,
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
        if let Some(buffered) = &mut self.buffered {
            return buffered.read(buffer);
        }
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
pub(crate) mod tests;
