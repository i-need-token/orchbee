use std::{
    fmt,
    io::{self, Read, Write},
    os::fd::AsFd,
    path::Path,
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use orchbee_client_server_protocol::{
    ClientMessage, ProtocolError, ReadMessageError, RequestId, ServerMessage,
    TerminalCreateRequest, TerminalCreateResult, TerminalStartRequest, TerminalStartResult,
    encode_client_frame, read_server_message,
};
use orchbee_local_client_protocol::{ClientSessionConnection, ConnectClientError};

const REQUEST_TOTAL_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) struct LocalClient {
    connection: Option<ClientSessionConnection>,
    request_timeout: Duration,
    clock: Box<dyn MonotonicClock>,
    io_chunk_limits: IoChunkLimits,
}

impl LocalClient {
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
        connection
            .set_nonblocking(true)
            .map_err(LocalClientConnectError::RequestIoConfiguration)?;
        Ok(Self {
            connection: Some(connection),
            request_timeout,
            clock,
            io_chunk_limits,
        })
    }

    #[must_use]
    pub(crate) fn generation(&self) -> Option<std::num::NonZeroU64> {
        self.connection
            .as_ref()
            .map(ClientSessionConnection::generation)
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
                    expected: ResultKind::TerminalCreate,
                    received: ResultKind::TerminalStart,
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
                    expected: ResultKind::TerminalStart,
                    received: ResultKind::TerminalCreate,
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

    fn exchange(
        &mut self,
        request: ClientMessage,
        request_id: &RequestId,
    ) -> Result<ServerMessage, SubmissionError> {
        if self.connection.is_none() {
            return Err(SubmissionError::SessionUnavailable);
        }

        let frame = encode_client_frame(&request).map_err(SubmissionError::RequestEncoding)?;
        let deadline = SubmissionDeadline::start(self.clock.as_ref(), self.request_timeout)
            .map_err(SubmissionError::RequestNotSubmitted)?;
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(SubmissionError::SessionUnavailable);
            };
            exchange_once(
                connection,
                self.clock.as_ref(),
                deadline,
                self.io_chunk_limits,
                &frame,
            )
        };

        match attempt {
            ExchangeAttempt::Result(result) => Ok(result),
            ExchangeAttempt::NotSubmitted(error) => {
                Err(SubmissionError::RequestNotSubmitted(error))
            }
            ExchangeAttempt::OutcomeUnknown(reason) => {
                self.retire_with_unknown(request_id.clone(), reason)
            }
        }
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
    TerminalCreate,
    TerminalStart,
}

impl fmt::Display for ResultKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TerminalCreate => "terminal.create.result",
            Self::TerminalStart => "terminal.start.result",
        })
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
        "OBCS request total deadline elapsed",
    )
}

trait MonotonicClock: fmt::Debug + Send {
    fn now(&self) -> Instant;
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
enum ExchangeAttempt {
    Result(ServerMessage),
    NotSubmitted(io::Error),
    OutcomeUnknown(OutcomeUnknownReason),
}

fn exchange_once(
    connection: &mut ClientSessionConnection,
    clock: &dyn MonotonicClock,
    deadline: SubmissionDeadline,
    io_chunk_limits: IoChunkLimits,
    frame: &[u8],
) -> ExchangeAttempt {
    let mut io = DeadlineIo {
        connection,
        clock,
        deadline,
        io_chunk_limits,
        write_began: false,
    };

    if let Err(error) = io.write_all(frame) {
        return if io.write_began {
            ExchangeAttempt::OutcomeUnknown(OutcomeUnknownReason::Write(error))
        } else {
            ExchangeAttempt::NotSubmitted(error)
        };
    }
    if let Err(error) = deadline.remaining(clock) {
        return ExchangeAttempt::OutcomeUnknown(OutcomeUnknownReason::Write(error));
    }

    match read_server_message(&mut io) {
        Ok(result) => match deadline.remaining(clock) {
            Ok(_remaining) => ExchangeAttempt::Result(result),
            Err(error) => ExchangeAttempt::OutcomeUnknown(OutcomeUnknownReason::Read(error)),
        },
        Err(ReadMessageError::Io(error)) => {
            ExchangeAttempt::OutcomeUnknown(OutcomeUnknownReason::Read(error))
        }
        Err(ReadMessageError::Protocol(error)) => {
            ExchangeAttempt::OutcomeUnknown(OutcomeUnknownReason::InvalidResult(error))
        }
    }
}

struct DeadlineIo<'a> {
    connection: &'a mut ClientSessionConnection,
    clock: &'a dyn MonotonicClock,
    deadline: SubmissionDeadline,
    io_chunk_limits: IoChunkLimits,
    write_began: bool,
}

impl DeadlineIo<'_> {
    fn wait_until_ready(&self, interest: PollFlags) -> io::Result<()> {
        loop {
            let remaining = self.deadline.remaining(self.clock)?;
            let timeout = poll_timeout(remaining);
            let mut descriptors = [PollFd::new(self.connection.as_fd(), interest)];
            match poll(&mut descriptors, timeout) {
                Ok(0) => {}
                Ok(_ready) => {
                    self.deadline.remaining(self.clock)?;
                    return Ok(());
                }
                Err(Errno::EINTR) => {}
                Err(error) => return Err(error.into()),
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

#[cfg(test)]
mod tests;
