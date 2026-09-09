use std::{
    fmt,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::{fd::AsFd, unix::net::UnixStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, AuthorizedTerminalStart, CapabilityNodeMessage, ProtocolError,
    ReadMessageError, TerminalCwdValidationRequest, TerminalCwdValidationResult,
    TerminalLaunchResult, TerminalOperationAuthorization, TerminalStartResult, encode_message,
    read_message,
};
use orchbee_local_site_protocol::WorkerId;

mod lifecycle;
pub(crate) use lifecycle::PendingLifecycleResult;

const REQUEST_QUEUE_CAPACITY: usize = 1;
const OPERATION_IO_TIMEOUT: Duration = Duration::from_secs(2);
const OWNER_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkerOperationRequest {
    CwdValidation(TerminalCwdValidationRequest),
    Lifecycle(orchbee_capability_node_protocol::TerminalLifecycleRequest),
    Control(orchbee_capability_node_protocol::TerminalControlRequest),
    Launch(AuthorizedTerminalLaunch),
    Start(AuthorizedTerminalStart),
    Viewer(orchbee_capability_node_protocol::TerminalViewerRequest),
}

impl WorkerOperationRequest {
    const fn worker(
        &self,
    ) -> (
        orchbee_capability_node_protocol::WorkerId,
        orchbee_capability_node_protocol::WorkerGeneration,
    ) {
        match self {
            Self::CwdValidation(request) => (request.worker_id, request.worker_generation),
            Self::Lifecycle(request) => (
                request.authorization.worker_id,
                request.authorization.worker_generation,
            ),
            Self::Control(request) => (request.worker_id, request.worker_generation),
            Self::Launch(request) => (
                request.authorization().worker_id(),
                request.authorization().worker_generation(),
            ),
            Self::Start(request) => (
                request.authorization().worker_id(),
                request.authorization().worker_generation(),
            ),
            Self::Viewer(request) => (request.worker_id, request.worker_generation),
        }
    }

    fn message(&self) -> CapabilityNodeMessage {
        match self {
            Self::CwdValidation(request) => {
                CapabilityNodeMessage::TerminalCwdValidationRequest(request.clone())
            }
            Self::Lifecycle(request) => CapabilityNodeMessage::TerminalLifecycleRequest(*request),
            Self::Control(request) => {
                CapabilityNodeMessage::TerminalControlRequest(request.clone())
            }
            Self::Launch(request) => {
                CapabilityNodeMessage::AuthorizedTerminalLaunch(request.clone())
            }
            Self::Start(request) => CapabilityNodeMessage::AuthorizedTerminalStart(*request),
            Self::Viewer(request) => CapabilityNodeMessage::TerminalViewerRequest(*request),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkerOperationResult {
    CwdValidation(TerminalCwdValidationResult),
    Lifecycle(orchbee_capability_node_protocol::TerminalLifecycleResult),
    Control(orchbee_capability_node_protocol::TerminalControlResult),
    Launch(TerminalLaunchResult),
    Start(TerminalStartResult),
    Viewer(orchbee_capability_node_protocol::TerminalViewerResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerTransportFailure {
    TimedOut,
    Closed,
    UnexpectedPeerData,
    Io(io::ErrorKind),
}

impl fmt::Display for WorkerTransportFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimedOut => formatter.write_str("Worker carrier I/O timed out"),
            Self::Closed => formatter.write_str("Worker carrier connection closed"),
            Self::UnexpectedPeerData => {
                formatter.write_str("Worker sent data without an outstanding request")
            }
            Self::Io(kind) => write!(formatter, "Worker carrier I/O failed with {kind:?}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerResponseFailure {
    TimedOut,
    Disconnected,
    CarrierStopped,
    Io(io::ErrorKind),
    InvalidFrame(ProtocolError),
    WrongDirection,
    WrongResultKind,
    MismatchedCorrelation,
    MismatchedAuthorization,
    MismatchedTarget,
}

impl fmt::Display for WorkerResponseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimedOut => formatter.write_str("Worker result timed out"),
            Self::Disconnected => formatter.write_str("Worker disconnected before its result"),
            Self::CarrierStopped => {
                formatter.write_str("Worker carrier stopped before reporting completion")
            }
            Self::Io(kind) => write!(formatter, "Worker result read failed with {kind:?}"),
            Self::InvalidFrame(error) => write!(formatter, "invalid Worker result frame: {error}"),
            Self::WrongDirection => formatter.write_str("Worker sent a Server-to-Worker message"),
            Self::WrongResultKind => formatter.write_str("Worker sent the wrong result kind"),
            Self::MismatchedCorrelation => {
                formatter.write_str("Worker result operation correlation did not match")
            }
            Self::MismatchedAuthorization => {
                formatter.write_str("Worker result authorization echo did not match")
            }
            Self::MismatchedTarget => formatter.write_str("Worker result target did not match"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerRequestError {
    Unavailable,
    Backpressure,
    WrongWorker,
    WrongGeneration,
    InvalidRequest(ProtocolError),
    NotDelivered(WorkerTransportFailure),
    PostDeliveryOutcomeUnknown(WorkerResponseFailure),
}

impl fmt::Display for WorkerRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("Worker carrier is unavailable"),
            Self::Backpressure => formatter.write_str("Worker request queue is full"),
            Self::WrongWorker => formatter.write_str("request targets a different Worker"),
            Self::WrongGeneration => formatter.write_str("request targets a different generation"),
            Self::InvalidRequest(error) => write!(formatter, "invalid Worker request: {error}"),
            Self::NotDelivered(error) => {
                write!(formatter, "Worker request was not delivered: {error}")
            }
            Self::PostDeliveryOutcomeUnknown(error) => {
                write!(
                    formatter,
                    "Worker request completed its carrier write, but its operation outcome is unknown: {error}"
                )
            }
        }
    }
}

impl std::error::Error for WorkerRequestError {}

#[derive(Clone, Debug)]
pub(crate) struct WorkerRequestHandle {
    active: Arc<AtomicBool>,
    generation: NonZeroU64,
    sender: SyncSender<PendingRequest>,
    worker_id: WorkerId,
}

impl WorkerRequestHandle {
    pub(crate) fn validate_terminal_cwd(
        &self,
        request: TerminalCwdValidationRequest,
    ) -> Result<TerminalCwdValidationResult, WorkerRequestError> {
        let result = self.submit(WorkerOperationRequest::CwdValidation(request), false)?;
        let WorkerOperationResult::CwdValidation(result) = result else {
            unreachable!("cwd validation request returned another operation result");
        };
        Ok(result)
    }

    pub(crate) fn request(
        &self,
        request: WorkerOperationRequest,
    ) -> Result<WorkerOperationResult, WorkerRequestError> {
        self.submit(request, false)
    }

    pub(crate) fn cleanup_control(
        &self,
        request: orchbee_capability_node_protocol::TerminalControlRequest,
    ) -> Result<WorkerOperationResult, WorkerRequestError> {
        if !matches!(
            request.action,
            orchbee_capability_node_protocol::TerminalControlAction::Release
        ) {
            return Err(WorkerRequestError::InvalidRequest(
                ProtocolError::InvalidPayload,
            ));
        }
        self.submit(WorkerOperationRequest::Control(request), true)
    }

    fn submit(
        &self,
        request: WorkerOperationRequest,
        cleanup: bool,
    ) -> Result<WorkerOperationResult, WorkerRequestError> {
        if !self.is_active() {
            return Err(WorkerRequestError::Unavailable);
        }
        let (worker_id, generation) = request.worker();
        if worker_id.as_bytes() != self.worker_id.as_bytes() {
            return Err(WorkerRequestError::WrongWorker);
        }
        if generation.get() != self.generation.get() {
            return Err(WorkerRequestError::WrongGeneration);
        }
        let encoded =
            encode_message(&request.message()).map_err(WorkerRequestError::InvalidRequest)?;
        let (completion, result) = mpsc::sync_channel(1);
        let complete_write = Arc::new(AtomicBool::new(false));
        let pending = PendingRequest {
            completion: Some(completion),
            complete_write: Arc::clone(&complete_write),
            encoded,
            request,
        };
        let admitted = if cleanup {
            self.sender
                .send(pending)
                .map_err(|error| TrySendError::Disconnected(error.0))
        } else {
            self.sender.try_send(pending)
        };
        match admitted {
            Ok(()) => match result.recv() {
                Ok(result) => result,
                Err(_disconnected) if complete_write.load(Ordering::Acquire) => {
                    Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                        WorkerResponseFailure::CarrierStopped,
                    ))
                }
                Err(_disconnected) => Err(WorkerRequestError::Unavailable),
            },
            Err(TrySendError::Full(_pending)) if self.is_active() => {
                Err(WorkerRequestError::Backpressure)
            }
            Err(TrySendError::Full(_pending) | TrySendError::Disconnected(_pending)) => {
                Err(WorkerRequestError::Unavailable)
            }
        }
    }

    // The retiring Client handler retains its bounded Session slot until the
    // existing queue admits this one cleanup. No shared service lock is held.
    pub(crate) fn enqueue_detach(
        &self,
        request: orchbee_capability_node_protocol::TerminalViewerRequest,
    ) {
        if !self.is_active()
            || !matches!(
                request.action,
                orchbee_capability_node_protocol::TerminalViewerAction::Detach
            )
            || request.worker_id.as_bytes() != self.worker_id.as_bytes()
            || request.worker_generation.get() != self.generation.get()
        {
            return;
        }
        let request = WorkerOperationRequest::Viewer(request);
        let Ok(encoded) = encode_message(&request.message()) else {
            return;
        };
        let _admitted = self.sender.send(PendingRequest {
            completion: None,
            complete_write: Arc::new(AtomicBool::new(false)),
            encoded,
            request,
        });
    }

    pub(crate) fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    pub(crate) fn worker_id(&self) -> WorkerId {
        self.worker_id
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

struct PendingRequest {
    completion: Option<SyncSender<Result<WorkerOperationResult, WorkerRequestError>>>,
    complete_write: Arc<AtomicBool>,
    encoded: Vec<u8>,
    request: WorkerOperationRequest,
}

pub(crate) struct WorkerCarrier {
    wire_owner: WorkerWireOwner,
    shutdown: WorkerCarrierShutdown,
    request_handle: WorkerRequestHandle,
}

impl WorkerCarrier {
    pub(crate) fn generation(&self) -> NonZeroU64 {
        self.request_handle.generation()
    }

    pub(crate) fn worker_id(&self) -> WorkerId {
        self.request_handle.worker_id()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.request_handle.is_active()
    }

    #[cfg(test)]
    pub(crate) fn request_handle(&self) -> WorkerRequestHandle {
        self.request_handle.clone()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (WorkerRequestHandle, WorkerCarrierShutdown, WorkerWireOwner) {
        (self.request_handle, self.shutdown, self.wire_owner)
    }
}

#[derive(Debug)]
pub(crate) struct WorkerCarrierShutdown {
    active: Arc<AtomicBool>,
}

impl Drop for WorkerCarrierShutdown {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

pub(crate) struct WorkerWireOwner {
    active: Arc<AtomicBool>,
    in_flight: Option<PendingRequest>,
    limits: WorkerCarrierLimits,
    receiver: Receiver<PendingRequest>,
}

#[derive(Clone, Copy)]
struct WorkerCarrierLimits {
    io_timeout: Duration,
    response_timeout: Duration,
    poll_interval: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerWireExit {
    Shutdown,
    PeerClosed,
    UnexpectedPeerData,
    Transport(WorkerTransportFailure),
    RequestFailed,
}

pub(crate) fn create_worker_carrier(worker_id: WorkerId, generation: NonZeroU64) -> WorkerCarrier {
    create_worker_carrier_with_limits(
        worker_id,
        generation,
        WorkerCarrierLimits {
            io_timeout: OPERATION_IO_TIMEOUT,
            response_timeout: Duration::from_secs(20),
            poll_interval: OWNER_POLL_INTERVAL,
        },
    )
}

fn create_worker_carrier_with_limits(
    worker_id: WorkerId,
    generation: NonZeroU64,
    limits: WorkerCarrierLimits,
) -> WorkerCarrier {
    let active = Arc::new(AtomicBool::new(true));
    let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
    WorkerCarrier {
        wire_owner: WorkerWireOwner {
            active: Arc::clone(&active),
            in_flight: None,
            limits,
            receiver,
        },
        shutdown: WorkerCarrierShutdown {
            active: Arc::clone(&active),
        },
        request_handle: WorkerRequestHandle {
            active: Arc::clone(&active),
            generation,
            sender,
            worker_id,
        },
    }
}

impl WorkerWireOwner {
    pub(crate) fn run(mut self, mut stream: UnixStream) -> WorkerWireExit {
        match configure_idle_stream(&stream) {
            Ok(()) => self.run_configured(&mut stream),
            Err(failure) => WorkerWireExit::Transport(failure),
        }
    }

    #[cfg(test)]
    pub(crate) fn run_configured_for_test<Stream>(mut self, mut stream: Stream) -> WorkerWireExit
    where
        Stream: AsFd + Read + Write,
    {
        self.run_configured(&mut stream)
    }

    fn run_configured<Stream>(&mut self, stream: &mut Stream) -> WorkerWireExit
    where
        Stream: AsFd + Read + Write,
    {
        loop {
            if !self.active.load(Ordering::Acquire) {
                return WorkerWireExit::Shutdown;
            }
            match self.receiver.recv_timeout(self.limits.poll_interval) {
                Ok(pending) => {
                    debug_assert!(self.in_flight.is_none());
                    self.in_flight = Some(pending);
                    if !self.active.load(Ordering::Acquire) {
                        self.complete_in_flight(Err(WorkerRequestError::Unavailable));
                        return WorkerWireExit::Shutdown;
                    }
                    match probe_idle_stream(stream) {
                        Ok(IdleStreamState::Open) => {}
                        Ok(IdleStreamState::Closed) => {
                            self.complete_in_flight(Err(WorkerRequestError::NotDelivered(
                                WorkerTransportFailure::Closed,
                            )));
                            return WorkerWireExit::PeerClosed;
                        }
                        Ok(IdleStreamState::UnexpectedData) => {
                            self.complete_in_flight(Err(WorkerRequestError::NotDelivered(
                                WorkerTransportFailure::UnexpectedPeerData,
                            )));
                            return WorkerWireExit::UnexpectedPeerData;
                        }
                        Err(failure) => {
                            self.complete_in_flight(Err(WorkerRequestError::NotDelivered(failure)));
                            return WorkerWireExit::Transport(failure);
                        }
                    }

                    if !self.exchange_in_flight(stream) {
                        return WorkerWireExit::RequestFailed;
                    }
                }
                Err(RecvTimeoutError::Timeout) => match probe_idle_stream(stream) {
                    Ok(IdleStreamState::Open) => {}
                    Ok(IdleStreamState::Closed) => return WorkerWireExit::PeerClosed,
                    Ok(IdleStreamState::UnexpectedData) => {
                        return WorkerWireExit::UnexpectedPeerData;
                    }
                    Err(failure) => return WorkerWireExit::Transport(failure),
                },
                Err(RecvTimeoutError::Disconnected) => return WorkerWireExit::Shutdown,
            }
        }
    }

    fn exchange_in_flight<Stream>(&mut self, stream: &mut Stream) -> bool
    where
        Stream: AsFd + Read + Write,
    {
        let pending = self
            .in_flight
            .as_ref()
            .unwrap_or_else(|| unreachable!("wire exchange requires one in-flight request"));
        if let Err(failure) = write_complete_frame(stream, &pending.encoded, self.limits.io_timeout)
        {
            self.complete_in_flight(Err(WorkerRequestError::NotDelivered(failure)));
            return false;
        }
        pending.complete_write.store(true, Ordering::Release);

        let response_deadline = match Instant::now().checked_add(self.limits.response_timeout) {
            Some(deadline) => deadline,
            None => {
                self.complete_in_flight(Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                    WorkerResponseFailure::Io(io::ErrorKind::InvalidInput),
                )));
                return false;
            }
        };
        let response = match read_message(&mut DeadlineReader {
            deadline: response_deadline,
            stream,
        }) {
            Ok(message) => {
                let pending = self.in_flight.as_ref().unwrap_or_else(|| {
                    unreachable!("result validation requires one in-flight request")
                });
                validate_response(&pending.request, message)
            }
            Err(error) => Err(response_read_failure(error)),
        };
        let keep_alive = response.is_ok();
        let completion_delivered = self
            .complete_in_flight(response.map_err(WorkerRequestError::PostDeliveryOutcomeUnknown));
        keep_alive && completion_delivered
    }

    fn complete_in_flight(
        &mut self,
        result: Result<WorkerOperationResult, WorkerRequestError>,
    ) -> bool {
        let pending = self
            .in_flight
            .take()
            .unwrap_or_else(|| unreachable!("completion requires one in-flight request"));
        send_completion(pending, result)
    }

    fn drain_unavailable(&mut self) {
        while let Ok(pending) = self.receiver.try_recv() {
            let _completion_delivered =
                send_completion(pending, Err(WorkerRequestError::Unavailable));
        }
    }
}

impl Drop for WorkerWireOwner {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        if let Some(pending) = self.in_flight.take() {
            // Strict decoding cannot dispatch an incomplete frame. Once the complete-write marker
            // is published, however, every abandoned exchange is necessarily outcome-unknown.
            let result = if pending.complete_write.load(Ordering::Acquire) {
                Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                    WorkerResponseFailure::CarrierStopped,
                ))
            } else {
                Err(WorkerRequestError::Unavailable)
            };
            let _completion_delivered = send_completion(pending, result);
        }
        self.drain_unavailable();
    }
}

fn send_completion(
    pending: PendingRequest,
    result: Result<WorkerOperationResult, WorkerRequestError>,
) -> bool {
    pending
        .completion
        .is_none_or(|completion| completion.send(result).is_ok())
}

fn configure_idle_stream(stream: &UnixStream) -> Result<(), WorkerTransportFailure> {
    stream
        .set_nonblocking(true)
        .map_err(|error| classify_transport_error(&error))
}

fn write_complete_frame<Stream>(
    stream: &mut Stream,
    frame: &[u8],
    timeout: Duration,
) -> Result<(), WorkerTransportFailure>
where
    Stream: AsFd + Write,
{
    let Some(deadline) = Instant::now().checked_add(timeout) else {
        return Err(WorkerTransportFailure::Io(io::ErrorKind::InvalidInput));
    };
    let mut offset = 0;
    while offset < frame.len() {
        remaining_until(deadline).map_err(|error| classify_transport_error(&error))?;
        match stream.write(&frame[offset..]) {
            Ok(0) => return Err(WorkerTransportFailure::Closed),
            Ok(written) => offset += written,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                wait_for_io(stream, PollFlags::POLLOUT, deadline)
                    .map_err(|error| classify_transport_error(&error))?;
            }
            Err(error) => return Err(classify_transport_error(&error)),
        }
    }
    Ok(())
}

struct DeadlineReader<'a, Stream> {
    deadline: Instant,
    stream: &'a mut Stream,
}

impl<Stream> Read for DeadlineReader<'_, Stream>
where
    Stream: AsFd + Read,
{
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            remaining_until(self.deadline)?;
            match self.stream.read(buffer) {
                Ok(read) => return Ok(read),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    wait_for_io(self.stream, PollFlags::POLLIN, self.deadline)?;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

fn wait_for_io(stream: &impl AsFd, events: PollFlags, deadline: Instant) -> io::Result<()> {
    loop {
        let remaining = remaining_until(deadline)?;
        let timeout = poll_timeout(remaining)?;
        let ready = {
            let mut descriptors = [PollFd::new(stream.as_fd(), events)];
            match poll(&mut descriptors, timeout) {
                Ok(ready) => ready,
                Err(Errno::EINTR) => continue,
                Err(error) => return Err(io::Error::from_raw_os_error(error as i32)),
            }
        };
        if ready > 0 {
            return Ok(());
        }
    }
}

fn poll_timeout(duration: Duration) -> io::Result<PollTimeout> {
    let rounded_milliseconds = duration.as_millis().saturating_add(u128::from(
        !duration.subsec_nanos().is_multiple_of(1_000_000),
    ));
    PollTimeout::try_from(rounded_milliseconds)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

fn remaining_until(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Worker carrier deadline elapsed"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdleStreamState {
    Open,
    Closed,
    UnexpectedData,
}

fn probe_idle_stream(stream: &mut impl Read) -> Result<IdleStreamState, WorkerTransportFailure> {
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Ok(IdleStreamState::Closed),
            Ok(_) => return Ok(IdleStreamState::UnexpectedData),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(IdleStreamState::Open);
            }
            Err(error) => return Err(classify_transport_error(&error)),
        }
    }
}

fn validate_response(
    request: &WorkerOperationRequest,
    response: CapabilityNodeMessage,
) -> Result<WorkerOperationResult, WorkerResponseFailure> {
    match (request, response) {
        (
            WorkerOperationRequest::CwdValidation(expected),
            CapabilityNodeMessage::TerminalCwdValidationResult(actual),
        ) => {
            if actual.request.operation_id != expected.operation_id {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            if actual.request.worker_id != expected.worker_id
                || actual.request.worker_generation != expected.worker_generation
                || actual.request.cwd != expected.cwd
            {
                return Err(WorkerResponseFailure::MismatchedTarget);
            }
            Ok(WorkerOperationResult::CwdValidation(actual))
        }
        (_, CapabilityNodeMessage::TerminalCwdValidationRequest(_)) => {
            Err(WorkerResponseFailure::WrongDirection)
        }
        (_, CapabilityNodeMessage::TerminalCwdValidationResult(_)) => {
            Err(WorkerResponseFailure::WrongResultKind)
        }
        (
            WorkerOperationRequest::CwdValidation(_),
            CapabilityNodeMessage::TerminalLifecycleResult(_)
            | CapabilityNodeMessage::TerminalControlResult(_)
            | CapabilityNodeMessage::TerminalViewerResult(_)
            | CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_),
        ) => Err(WorkerResponseFailure::WrongResultKind),
        (
            WorkerOperationRequest::Lifecycle(expected),
            CapabilityNodeMessage::TerminalLifecycleResult(actual),
        ) => {
            let echo = actual.request();
            if echo.request_id != expected.request_id
                || echo.close_operation_id != expected.close_operation_id
                || echo.attempt_generation != expected.attempt_generation
                || echo.step_ordinal != expected.step_ordinal
            {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            if echo.authorization != expected.authorization {
                return Err(WorkerResponseFailure::MismatchedAuthorization);
            }
            if echo.target != expected.target {
                return Err(WorkerResponseFailure::MismatchedTarget);
            }
            if echo.action != expected.action {
                return Err(WorkerResponseFailure::WrongResultKind);
            }
            Ok(WorkerOperationResult::Lifecycle(actual))
        }
        (_, CapabilityNodeMessage::TerminalLifecycleRequest(_)) => {
            Err(WorkerResponseFailure::WrongDirection)
        }
        (_, CapabilityNodeMessage::TerminalLifecycleResult(_))
        | (
            WorkerOperationRequest::Lifecycle(_),
            CapabilityNodeMessage::TerminalControlResult(_)
            | CapabilityNodeMessage::TerminalViewerResult(_)
            | CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_),
        ) => Err(WorkerResponseFailure::WrongResultKind),
        (
            WorkerOperationRequest::Control(expected),
            CapabilityNodeMessage::TerminalControlResult(actual),
        ) => {
            use orchbee_capability_node_protocol::{
                TerminalControlAction as Action, TerminalControlOutcome as Outcome,
            };
            if actual.request_id != expected.request_id {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            if actual.viewer_id != expected.viewer_id
                || actual.controller_id != expected.controller_id
            {
                return Err(WorkerResponseFailure::MismatchedTarget);
            }
            let compatible = match (&expected.action, actual.outcome) {
                (_, Outcome::Rejected(_))
                | (Action::Acquire, Outcome::Acquired)
                | (Action::Release, Outcome::Released)
                | (Action::Input { .. }, Outcome::InputAccepted) => true,
                (Action::Resize { geometry, .. }, Outcome::Resized(actual)) => *geometry == actual,
                _ => false,
            };
            if !compatible {
                return Err(WorkerResponseFailure::WrongResultKind);
            }
            Ok(WorkerOperationResult::Control(actual))
        }
        (_, CapabilityNodeMessage::TerminalControlRequest(_)) => {
            Err(WorkerResponseFailure::WrongDirection)
        }
        (_, CapabilityNodeMessage::TerminalControlResult(_))
        | (
            WorkerOperationRequest::Control(_),
            CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_)
            | CapabilityNodeMessage::TerminalViewerResult(_),
        ) => Err(WorkerResponseFailure::WrongResultKind),
        (
            WorkerOperationRequest::Viewer(expected),
            CapabilityNodeMessage::TerminalViewerResult(actual),
        ) => {
            use orchbee_capability_node_protocol::{
                TerminalViewerAction as Action, TerminalViewerOutcome as Outcome,
            };
            if actual.request_id != expected.request_id {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            if actual.attachment_id != expected.attachment_id {
                return Err(WorkerResponseFailure::MismatchedTarget);
            }
            if !matches!(
                (&expected.action, &actual.outcome),
                (Action::Attach { .. }, Outcome::Attached)
                    | (Action::Poll, Outcome::Empty | Outcome::Record(_))
                    | (
                        Action::FrameApplied { .. } | Action::NeedSnapshot { .. },
                        Outcome::FeedbackAccepted
                    )
                    | (Action::Detach, Outcome::Detached)
                    | (_, Outcome::Rejected(_))
            ) {
                return Err(WorkerResponseFailure::WrongResultKind);
            }
            Ok(WorkerOperationResult::Viewer(actual))
        }
        (
            WorkerOperationRequest::Launch(expected),
            CapabilityNodeMessage::TerminalLaunchResult(actual),
        ) => {
            validate_authorization(expected.authorization(), actual.authorization())?;
            Ok(WorkerOperationResult::Launch(actual))
        }
        (
            WorkerOperationRequest::Start(expected),
            CapabilityNodeMessage::TerminalStartResult(actual),
        ) => {
            validate_authorization(expected.authorization(), actual.authorization())?;
            if actual.target() != expected.identity() {
                return Err(WorkerResponseFailure::MismatchedTarget);
            }
            Ok(WorkerOperationResult::Start(actual))
        }
        (
            _,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(_)
            | CapabilityNodeMessage::AuthorizedTerminalStart(_)
            | CapabilityNodeMessage::TerminalViewerRequest(_),
        ) => Err(WorkerResponseFailure::WrongDirection),
        (
            WorkerOperationRequest::Viewer(_),
            CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_),
        )
        | (_, CapabilityNodeMessage::TerminalViewerResult(_))
        | (WorkerOperationRequest::Launch(_), CapabilityNodeMessage::TerminalStartResult(_))
        | (WorkerOperationRequest::Start(_), CapabilityNodeMessage::TerminalLaunchResult(_)) => {
            Err(WorkerResponseFailure::WrongResultKind)
        }
    }
}

fn validate_authorization(
    expected: TerminalOperationAuthorization,
    actual: TerminalOperationAuthorization,
) -> Result<(), WorkerResponseFailure> {
    if actual.operation_id() != expected.operation_id() {
        return Err(WorkerResponseFailure::MismatchedCorrelation);
    }
    if actual != expected {
        return Err(WorkerResponseFailure::MismatchedAuthorization);
    }
    Ok(())
}

fn response_read_failure(error: ReadMessageError) -> WorkerResponseFailure {
    match error {
        ReadMessageError::Protocol(error) => WorkerResponseFailure::InvalidFrame(error),
        ReadMessageError::Io(error)
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            WorkerResponseFailure::TimedOut
        }
        ReadMessageError::Io(error)
            if matches!(
                error.kind(),
                io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::NotConnected
            ) =>
        {
            WorkerResponseFailure::Disconnected
        }
        ReadMessageError::Io(error) => WorkerResponseFailure::Io(error.kind()),
    }
}

fn classify_transport_error(error: &io::Error) -> WorkerTransportFailure {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        WorkerTransportFailure::TimedOut
    } else if matches!(
        error.kind(),
        io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::NotConnected
    ) {
        WorkerTransportFailure::Closed
    } else {
        WorkerTransportFailure::Io(error.kind())
    }
}

#[cfg(test)]
mod tests;
