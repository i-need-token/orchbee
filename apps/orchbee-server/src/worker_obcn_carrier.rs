use orchbee_local_ipc::AsLocalIo;
use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    collections::VecDeque,
    fmt,
    io::{self, Read, Write},
    num::NonZeroU64,
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, AuthorizedTerminalStart, CapabilityNodeMessage, ProtocolError,
    ReadMessageError, TerminalCwdValidationRequest, TerminalCwdValidationResult,
    TerminalLaunchResult, TerminalOperationAuthorization, TerminalStartResult, WorktreeJobRequest,
    WorktreeJobResult, encode_message, read_message,
};
use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_local_site_protocol::WorkerId;

mod lifecycle;
pub(crate) use lifecycle::PendingLifecycleResult;

const HIGH_LANE_CAPACITY: usize = 8;
const LOW_LANE_CAPACITY: usize = 8;
const HIGH_LANE_BURST_LIMIT: usize = 4;
const OPERATION_IO_TIMEOUT: Duration = Duration::from_secs(2);
const OWNER_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkerOperationRequest {
    #[cfg(any(windows, test))]
    Shutdown(orchbee_capability_node_protocol::WorkerShutdownRequest),
    CwdValidation(TerminalCwdValidationRequest),
    Lifecycle(orchbee_capability_node_protocol::TerminalLifecycleRequest),
    Control(orchbee_capability_node_protocol::TerminalControlRequest),
    Launch(AuthorizedTerminalLaunch),
    Start(AuthorizedTerminalStart),
    Viewer(orchbee_capability_node_protocol::TerminalViewerRequest),
    Send(orchbee_capability_node_protocol::TerminalSendRequest),
    WorktreeJob(WorktreeJobRequest),
    PluginJob(orchbee_capability_node_protocol::PluginJobRequest),
    WorktreeMutation(orchbee_capability_node_protocol::WorktreeMutationRequest),
    // The Server drain pump admits drains through admit_agent_drain.
    AgentDrain(orchbee_capability_node_protocol::AgentDrainRequest),
    Titles(orchbee_capability_node_protocol::TerminalTitlesRequest),
}

impl WorkerOperationRequest {
    const fn worker(
        &self,
    ) -> (
        orchbee_capability_node_protocol::WorkerId,
        orchbee_capability_node_protocol::WorkerGeneration,
    ) {
        match self {
            #[cfg(any(windows, test))]
            Self::Shutdown(request) => (request.worker_id, request.worker_generation),
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
            Self::Send(request) => (request.worker_id, request.worker_generation),
            Self::WorktreeJob(request) => (request.worker_id, request.worker_generation),
            Self::PluginJob(request) => (request.worker_id, request.worker_generation),
            Self::WorktreeMutation(request) => (request.worker_id, request.worker_generation),
            Self::AgentDrain(request) => (request.worker_id, request.worker_generation),
            Self::Titles(request) => (request.worker_id, request.worker_generation),
        }
    }

    const fn lane(&self) -> RequestLane {
        match self {
            Self::AgentDrain(_) | Self::Titles(_) => RequestLane::Low,
            Self::Viewer(_) => RequestLane::Low,
            #[cfg(any(windows, test))]
            Self::Shutdown(_) => RequestLane::High,
            Self::CwdValidation(_)
            | Self::Lifecycle(_)
            | Self::Control(_)
            | Self::Launch(_)
            | Self::Start(_)
            | Self::Send(_)
            | Self::PluginJob(_)
            | Self::WorktreeJob(_)
            | Self::WorktreeMutation(_) => RequestLane::High,
        }
    }

    fn message(&self) -> CapabilityNodeMessage {
        match self {
            #[cfg(any(windows, test))]
            Self::Shutdown(request) => CapabilityNodeMessage::WorkerShutdownRequest(*request),
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
            Self::Send(request) => CapabilityNodeMessage::TerminalSendRequest(request.clone()),
            Self::PluginJob(request) => CapabilityNodeMessage::PluginJobRequest(request.clone()),
            Self::WorktreeMutation(request) => {
                CapabilityNodeMessage::WorktreeMutationRequest(request.clone())
            }
            Self::WorktreeJob(request) => {
                CapabilityNodeMessage::WorktreeJobRequest(request.clone())
            }
            Self::AgentDrain(request) => CapabilityNodeMessage::AgentDrainRequest(*request),
            Self::Titles(request) => CapabilityNodeMessage::TerminalTitlesRequest(request.clone()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkerOperationResult {
    #[cfg(any(windows, test))]
    Shutdown(orchbee_capability_node_protocol::WorkerShutdownResult),
    CwdValidation(TerminalCwdValidationResult),
    Lifecycle(orchbee_capability_node_protocol::TerminalLifecycleResult),
    Control(orchbee_capability_node_protocol::TerminalControlResult),
    Launch(TerminalLaunchResult),
    Start(TerminalStartResult),
    Viewer(orchbee_capability_node_protocol::TerminalViewerResult),
    Send(orchbee_capability_node_protocol::TerminalSendResult),
    WorktreeJob(WorktreeJobResult),
    PluginJob(orchbee_capability_node_protocol::PluginJobResult),
    WorktreeMutation(orchbee_capability_node_protocol::WorktreeMutationResult),
    // Constructed by the Server drain pump's reply path.
    AgentDrain(orchbee_capability_node_protocol::AgentDrainReply),
    Titles(orchbee_capability_node_protocol::TerminalTitlesResult),
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

#[derive(Debug)]
pub(crate) struct WorkerRequestHandle {
    active: Arc<AtomicBool>,
    generation: NonZeroU64,
    queue: Arc<CarrierQueue>,
    worker_id: WorkerId,
}

impl Clone for WorkerRequestHandle {
    fn clone(&self) -> Self {
        self.queue.handle_opened();
        Self {
            active: Arc::clone(&self.active),
            generation: self.generation,
            queue: Arc::clone(&self.queue),
            worker_id: self.worker_id,
        }
    }
}

impl Drop for WorkerRequestHandle {
    fn drop(&mut self) {
        self.queue.handle_closed();
    }
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

    // Test flush markers take the same blocking high-lane admission as cleanup
    // producers so their response proves every earlier high-lane or waiter
    // predecessor left the queue; earlier low-lane requests may still be queued
    // when the marker dispatches — the D=4 rotation can interleave them.
    #[cfg(test)]
    pub(crate) fn blocking_admit_for_test(
        &self,
        request: WorkerOperationRequest,
    ) -> Result<WorkerOperationResult, WorkerRequestError> {
        self.submit(request, true)
    }

    fn submit(
        &self,
        request: WorkerOperationRequest,
        cleanup: bool,
    ) -> Result<WorkerOperationResult, WorkerRequestError> {
        self.admit(request, cleanup)?.recv()
    }

    // The Server drain reader's admission seam: enqueue the typed
    // AgentDrain request on the LOW lane and hand back its exchange so the
    // caller can wait with its own bounded deadline instead of blocking
    // like submit.
    pub(crate) fn admit_agent_drain(
        &self,
        request: orchbee_capability_node_protocol::AgentDrainRequest,
    ) -> Result<PendingExchange, WorkerRequestError> {
        self.admit(WorkerOperationRequest::AgentDrain(request), false)
    }

    /// Read-only discovery jobs retain their physical TTL and receipt in the
    /// Worker. A bounded Server reader may abandon this exchange; the wire owner
    /// still validates and consumes its late reply. Admission never waits.
    pub(crate) fn admit_worktree_job(
        &self,
        request: WorktreeJobRequest,
    ) -> Result<PendingExchange, WorkerRequestError> {
        self.admit(WorkerOperationRequest::WorktreeJob(request), false)
    }

    /// Admit one request through the existing worker/generation fence and lane
    /// rules, returning the exchange that receives its result. Completion
    /// ownership is fixed by the admitted request kind.
    fn admit(
        &self,
        request: WorkerOperationRequest,
        cleanup: bool,
    ) -> Result<PendingExchange, WorkerRequestError> {
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
        let lane = request.lane();
        let (completion, result) = mpsc::sync_channel(1);
        let complete_write = Arc::new(AtomicBool::new(false));
        let pending = PendingRequest {
            completion: CompletionDelivery::for_request(&request, completion),
            complete_write: Arc::clone(&complete_write),
            encoded,
            request,
        };
        let admitted = match pending.request {
            #[cfg(any(windows, test))]
            WorkerOperationRequest::Shutdown(_) => self.queue.admit_shutdown(pending),
            _ => {
                if cleanup {
                    match self.queue.block_admit_high(pending) {
                        true => AdmitOutcome::Admitted,
                        false => AdmitOutcome::Closed,
                    }
                } else {
                    self.queue.try_admit(lane, pending)
                }
            }
        };
        match admitted {
            AdmitOutcome::Admitted => Ok(PendingExchange {
                result,
                complete_write,
            }),
            AdmitOutcome::Full if self.is_active() => Err(WorkerRequestError::Backpressure),
            AdmitOutcome::Full | AdmitOutcome::Closed => Err(WorkerRequestError::Unavailable),
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
        let _admitted = self.queue.block_admit_high(PendingRequest {
            completion: CompletionDelivery::Detached,
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

/// Who owns the completion delivery for one admitted request. A Required
/// result must reach a live receiver or the carrier dies; a DiscardableRead
/// result may be consumed and dropped when its bounded Server reader abandoned
/// the wait, keeping healthy carrier traffic alive. Discovery physical jobs and
/// their TTL remain Worker-owned. Detached requests carry
/// no receiver at all.
#[derive(Debug)]
enum CompletionDelivery {
    Required(SyncSender<Result<WorkerOperationResult, WorkerRequestError>>),
    DiscardableRead(SyncSender<Result<WorkerOperationResult, WorkerRequestError>>),
    Detached,
}

impl CompletionDelivery {
    /// Completion ownership is derived from the admitted request kind, never
    /// chosen by the caller: typed AgentDrain and read-only WorktreeJob requests
    /// get discardable completion; effects still require a live receiver.
    fn for_request(
        request: &WorkerOperationRequest,
        completion: SyncSender<Result<WorkerOperationResult, WorkerRequestError>>,
    ) -> Self {
        match request {
            WorkerOperationRequest::AgentDrain(_) | WorkerOperationRequest::WorktreeJob(_) => {
                Self::DiscardableRead(completion)
            }
            _ => Self::Required(completion),
        }
    }
}

#[derive(Debug)]
struct PendingRequest {
    completion: CompletionDelivery,
    complete_write: Arc<AtomicBool>,
    encoded: Vec<u8>,
    request: WorkerOperationRequest,
}

/// One admitted exchange handed back to the requester: exactly one result
/// receiver plus the shared complete-write marker used to classify a lost
/// completion channel.
#[derive(Debug)]
pub(crate) struct PendingExchange {
    result: Receiver<Result<WorkerOperationResult, WorkerRequestError>>,
    complete_write: Arc<AtomicBool>,
}

impl PendingExchange {
    /// Blocking wait used by `submit`: preserves the existing result mapping
    /// where a lost channel before a complete write is Unavailable and a lost
    /// channel after it leaves the post-delivery outcome unknown.
    fn recv(self) -> Result<WorkerOperationResult, WorkerRequestError> {
        match self.result.recv() {
            Ok(result) => result,
            Err(_disconnected) if self.complete_write.load(Ordering::Acquire) => {
                Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                    WorkerResponseFailure::CarrierStopped,
                ))
            }
            Err(_disconnected) => Err(WorkerRequestError::Unavailable),
        }
    }

    /// Bounded wait against one absolute deadline. An elapsed wait leaves the
    /// attempt alive — the exchange may be waited again; only a lost channel
    /// is terminal and is classified by the complete-write marker exactly like
    /// the blocking wait.
    pub(crate) fn recv_deadline(&self, deadline: Instant) -> ExchangeWait {
        match self
            .result
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(result) => ExchangeWait::Ready(Box::new(result)),
            Err(mpsc::RecvTimeoutError::Timeout) => ExchangeWait::Elapsed,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                ExchangeWait::Lost(if self.complete_write.load(Ordering::Acquire) {
                    WorkerRequestError::PostDeliveryOutcomeUnknown(
                        WorkerResponseFailure::CarrierStopped,
                    )
                } else {
                    WorkerRequestError::Unavailable
                })
            }
        }
    }
}

/// Outcome of one bounded wait on a pending exchange.
#[derive(Debug)]
pub(crate) enum ExchangeWait {
    Ready(Box<Result<WorkerOperationResult, WorkerRequestError>>),
    Elapsed,
    /// The typed cause of the loss; the pump folds every loss to
    /// `PreparedUnavailable`, so the payload is carried unread.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "typed loss cause; pump folds every loss to PreparedUnavailable"
        )
    )]
    Lost(WorkerRequestError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestLane {
    High,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmitOutcome {
    Admitted,
    Full,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TakeNext {
    Closed,
    Dispatched,
    Idle,
}

// A blocked cleanup producer deposits its bounded payload here and waits until
// the wire owner transfers it into a freed high-lane slot under the queue lock.
// Deposit order is FIFO and waiters always precede later try-admissions.
#[derive(Debug)]
struct HighWaiter {
    admitted: Arc<AtomicBool>,
    request: PendingRequest,
}

#[derive(Debug)]
struct QueueState {
    closed: bool,
    stopping: bool,
    consecutive_high: usize,
    high: VecDeque<PendingRequest>,
    high_waiters: VecDeque<HighWaiter>,
    low: VecDeque<PendingRequest>,
    open_handles: usize,
}

impl QueueState {
    // Dispatch frees the popped lane slot immediately; the oldest high waiter
    // then occupies that freed tail slot before any new try-admission can.
    fn dispatch(&mut self) -> Option<PendingRequest> {
        let (popped, lane) =
            if self.consecutive_high < HIGH_LANE_BURST_LIMIT && !self.high.is_empty() {
                (self.high.pop_front(), RequestLane::High)
            } else if !self.low.is_empty() {
                (self.low.pop_front(), RequestLane::Low)
            } else {
                (self.high.pop_front(), RequestLane::High)
            };
        let pending = popped?;
        match lane {
            RequestLane::High => {
                self.consecutive_high += 1;
                if let Some(waiter) = self.high_waiters.pop_front() {
                    self.high.push_back(waiter.request);
                    waiter.admitted.store(true, Ordering::Release);
                }
            }
            RequestLane::Low => self.consecutive_high = 0,
        }
        Some(pending)
    }
}

#[derive(Debug)]
struct CarrierQueue {
    changed: Condvar,
    state: Mutex<QueueState>,
}

impl CarrierQueue {
    fn new() -> Self {
        Self {
            changed: Condvar::new(),
            state: Mutex::new(QueueState {
                closed: false,
                stopping: false,
                consecutive_high: 0,
                high: VecDeque::new(),
                high_waiters: VecDeque::new(),
                low: VecDeque::new(),
                open_handles: 1,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, QueueState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn handle_opened(&self) {
        self.lock().open_handles += 1;
    }

    fn handle_closed(&self) {
        let mut state = self.lock();
        debug_assert!(
            state.open_handles > 0,
            "carrier handle count cannot underflow"
        );
        state.open_handles = state.open_handles.saturating_sub(1);
        if state.open_handles == 0 {
            self.changed.notify_all();
        }
    }

    fn try_admit(&self, lane: RequestLane, request: PendingRequest) -> AdmitOutcome {
        let mut state = self.lock();
        if state.closed || state.stopping {
            return AdmitOutcome::Closed;
        }
        let full = match lane {
            RequestLane::High => {
                !state.high_waiters.is_empty() || state.high.len() >= HIGH_LANE_CAPACITY
            }
            RequestLane::Low => state.low.len() >= LOW_LANE_CAPACITY,
        };
        if full {
            return AdmitOutcome::Full;
        }
        match lane {
            RequestLane::High => state.high.push_back(request),
            RequestLane::Low => state.low.push_back(request),
        }
        self.changed.notify_all();
        AdmitOutcome::Admitted
    }

    fn block_admit_high(&self, request: PendingRequest) -> bool {
        let mut state = self.lock();
        if state.closed || state.stopping {
            return false;
        }
        if state.high_waiters.is_empty() && state.high.len() < HIGH_LANE_CAPACITY {
            state.high.push_back(request);
            self.changed.notify_all();
            return true;
        }
        let admitted = Arc::new(AtomicBool::new(false));
        state.high_waiters.push_back(HighWaiter {
            admitted: Arc::clone(&admitted),
            request,
        });
        loop {
            if admitted.load(Ordering::Acquire) {
                return true;
            }
            if state.closed || state.stopping {
                return false;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    fn take_next(&self, timeout: Duration, dispatched: &mut Option<PendingRequest>) -> TakeNext {
        let deadline = Instant::now().checked_add(timeout);
        let mut state = self.lock();
        loop {
            if let Some(pending) = state.dispatch() {
                self.changed.notify_all();
                *dispatched = Some(pending);
                return TakeNext::Dispatched;
            }
            if state.closed || state.open_handles == 0 {
                return TakeNext::Closed;
            }
            let Some(remaining) = deadline
                .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
                .filter(|remaining| !remaining.is_zero())
            else {
                return TakeNext::Idle;
            };
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }

    #[cfg(any(windows, test))]
    fn admit_shutdown(&self, request: PendingRequest) -> AdmitOutcome {
        let pending = {
            let mut state = self.lock();
            if state.closed {
                return AdmitOutcome::Closed;
            }
            if state
                .high
                .iter()
                .any(|pending| matches!(pending.request, WorkerOperationRequest::Shutdown(_)))
            {
                return AdmitOutcome::Full;
            }
            state.stopping = true;
            let mut pending: Vec<_> = state.high.drain(..).collect();
            pending.extend(state.low.drain(..));
            pending.extend(state.high_waiters.drain(..).map(|waiter| waiter.request));
            state.high.push_back(request);
            self.changed.notify_all();
            pending
        };
        for request in pending {
            let _delivered = send_completion(request, Err(WorkerRequestError::Unavailable));
        }
        AdmitOutcome::Admitted
    }

    fn close_and_drain(&self) -> Vec<PendingRequest> {
        let mut state = self.lock();
        state.closed = true;
        let mut drained =
            Vec::with_capacity(state.high.len() + state.low.len() + state.high_waiters.len());
        drained.extend(state.high.drain(..));
        drained.extend(state.low.drain(..));
        drained.extend(state.high_waiters.drain(..).map(|waiter| waiter.request));
        self.changed.notify_all();
        drained
    }

    #[cfg(test)]
    fn dispatch_next(&self) -> Option<PendingRequest> {
        let dispatched = self.lock().dispatch();
        if dispatched.is_some() {
            self.changed.notify_all();
        }
        dispatched
    }
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
    queue: Arc<CarrierQueue>,
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
    let queue = Arc::new(CarrierQueue::new());
    WorkerCarrier {
        wire_owner: WorkerWireOwner {
            active: Arc::clone(&active),
            in_flight: None,
            limits,
            queue: Arc::clone(&queue),
        },
        shutdown: WorkerCarrierShutdown {
            active: Arc::clone(&active),
        },
        request_handle: WorkerRequestHandle {
            active: Arc::clone(&active),
            generation,
            queue,
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
        Stream: AsLocalIo + Read + Write,
    {
        self.run_configured(&mut stream)
    }

    fn run_configured<Stream>(&mut self, stream: &mut Stream) -> WorkerWireExit
    where
        Stream: AsLocalIo + Read + Write,
    {
        loop {
            if !self.active.load(Ordering::Acquire) {
                return WorkerWireExit::Shutdown;
            }
            debug_assert!(self.in_flight.is_none());
            match self
                .queue
                .take_next(self.limits.poll_interval, &mut self.in_flight)
            {
                TakeNext::Dispatched => {
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
                TakeNext::Idle => match probe_idle_stream(stream) {
                    Ok(IdleStreamState::Open) => {}
                    Ok(IdleStreamState::Closed) => return WorkerWireExit::PeerClosed,
                    Ok(IdleStreamState::UnexpectedData) => {
                        return WorkerWireExit::UnexpectedPeerData;
                    }
                    Err(failure) => return WorkerWireExit::Transport(failure),
                },
                TakeNext::Closed => return WorkerWireExit::Shutdown,
            }
        }
    }

    fn exchange_in_flight<Stream>(&mut self, stream: &mut Stream) -> bool
    where
        Stream: AsLocalIo + Read + Write,
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
        for pending in self.queue.close_and_drain() {
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
    match pending.completion {
        CompletionDelivery::Detached => true,
        CompletionDelivery::Required(completion) => completion.send(result).is_ok(),
        // An abandoned observation must not kill healthy carrier traffic: the
        // reply is still consumed off the wire, then discarded when its
        // receiver is gone.
        CompletionDelivery::DiscardableRead(completion) => {
            let _delivered = completion.send(result);
            true
        }
    }
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
    Stream: AsLocalIo + Write,
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
    Stream: AsLocalIo + Read,
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

fn wait_for_io(stream: &impl AsLocalIo, events: PollFlags, deadline: Instant) -> io::Result<()> {
    loop {
        let remaining = remaining_until(deadline)?;
        let timeout = poll_timeout(remaining)?;
        let ready = {
            let mut descriptors = [PollFd::new(stream.as_local_io(), events)];
            match poll(&mut descriptors, timeout) {
                Ok(ready) => ready,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
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
        #[cfg(any(windows, test))]
        (
            WorkerOperationRequest::Shutdown(expected),
            CapabilityNodeMessage::WorkerShutdownResult(actual),
        ) => {
            if actual.request.operation_id != expected.operation_id {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            if actual.request != *expected {
                return Err(WorkerResponseFailure::MismatchedAuthorization);
            }
            Ok(WorkerOperationResult::Shutdown(actual))
        }
        (_, CapabilityNodeMessage::WorkerShutdownRequest(_)) => {
            Err(WorkerResponseFailure::WrongDirection)
        }
        (_, CapabilityNodeMessage::WorkerShutdownResult(_)) => {
            Err(WorkerResponseFailure::WrongResultKind)
        }
        #[cfg(any(windows, test))]
        (WorkerOperationRequest::Shutdown(_), _) => Err(WorkerResponseFailure::WrongResultKind),
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
            | CapabilityNodeMessage::TerminalTitlesResult(_)
            | CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_)
            | CapabilityNodeMessage::TerminalSendResult(_),
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
            | CapabilityNodeMessage::TerminalTitlesResult(_)
            | CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_)
            | CapabilityNodeMessage::TerminalSendResult(_),
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
                | (Action::Acquire | Action::Takeover { .. }, Outcome::Acquired)
                | (Action::Release, Outcome::Released)
                | (Action::Input { .. }, Outcome::InputAccepted) => true,
                (Action::Resize { geometry, .. }, Outcome::Resized(actual)) => *geometry == actual,
                (Action::Paste { content, .. }, Outcome::Pasted { accepted_bytes }) => content
                    .as_bytes()
                    .len()
                    .checked_mul(2)
                    .and_then(|bound| bound.checked_add(12))
                    .is_some_and(|bound| {
                        usize::try_from(accepted_bytes.get()).is_ok_and(|v| v <= bound)
                    }),
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
            | CapabilityNodeMessage::TerminalViewerResult(_)
            | CapabilityNodeMessage::TerminalTitlesResult(_)
            | CapabilityNodeMessage::TerminalSendResult(_),
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
                    | (Action::ReadProcessInfo, Outcome::Record(_))
                    | (
                        Action::FrameApplied { .. } | Action::NeedSnapshot { .. },
                        Outcome::FeedbackAccepted
                    )
                    | (
                        Action::SetProjectionDemand { .. }
                            | Action::SetHistoryDemand { .. }
                            | Action::RowsApplied { .. }
                            | Action::HistoryObserved { .. },
                        Outcome::FeedbackAccepted
                    )
                    | (
                        Action::ReadRows { .. } | Action::ReadViewportRows { .. },
                        Outcome::Record(_) | Outcome::RowsRejected(_)
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
            WorkerOperationRequest::Send(expected),
            CapabilityNodeMessage::TerminalSendResult(actual),
        ) => {
            if actual.request_id() != expected.request_id {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            Ok(WorkerOperationResult::Send(actual))
        }
        (
            WorkerOperationRequest::Titles(expected),
            CapabilityNodeMessage::TerminalTitlesResult(actual),
        ) => {
            if !actual.answers(expected) {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            Ok(WorkerOperationResult::Titles(actual))
        }
        (
            WorkerOperationRequest::AgentDrain(expected),
            CapabilityNodeMessage::AgentDrainReply(actual),
        ) => {
            if actual.token_echo() != expected.token
                || actual.acknowledged_echo() != expected.acknowledged
            {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            Ok(WorkerOperationResult::AgentDrain(actual))
        }
        (
            WorkerOperationRequest::PluginJob(expected),
            CapabilityNodeMessage::PluginJobResult(actual),
        ) => {
            if !actual.answers(expected) {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            Ok(WorkerOperationResult::PluginJob(actual))
        }
        (
            WorkerOperationRequest::WorktreeMutation(expected),
            CapabilityNodeMessage::WorktreeMutationResult(actual),
        ) => {
            use orchbee_capability_node_protocol::{
                WorktreeMutationJobAction, WorktreeMutationOutcome,
            };
            if actual.operation_id != expected.operation_id {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            let expected_job = match &expected.action {
                // Start mints the Job identity: the echoed Job ID is the
                // request's own operation identity.
                WorktreeMutationJobAction::Start(_) => expected.operation_id,
                WorktreeMutationJobAction::Poll { job_id }
                | WorktreeMutationJobAction::Cancel { job_id } => *job_id,
            };
            if actual.job_id != expected_job {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            if !matches!(
                (&expected.action, &actual.outcome),
                (
                    WorktreeMutationJobAction::Start(_),
                    WorktreeMutationOutcome::Start(_)
                ) | (
                    WorktreeMutationJobAction::Poll { .. },
                    WorktreeMutationOutcome::Poll(_)
                ) | (
                    WorktreeMutationJobAction::Cancel { .. },
                    WorktreeMutationOutcome::Cancel(_)
                )
            ) {
                return Err(WorkerResponseFailure::WrongResultKind);
            }
            Ok(WorkerOperationResult::WorktreeMutation(actual))
        }
        (
            _,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(_)
            | CapabilityNodeMessage::AuthorizedTerminalStart(_)
            | CapabilityNodeMessage::TerminalViewerRequest(_)
            | CapabilityNodeMessage::TerminalSendRequest(_)
            | CapabilityNodeMessage::AgentDrainRequest(_)
            | CapabilityNodeMessage::TerminalTitlesRequest(_)
            | CapabilityNodeMessage::WorktreeMutationRequest(_)
            | CapabilityNodeMessage::PluginJobRequest(_),
        ) => Err(WorkerResponseFailure::WrongDirection),
        (
            WorkerOperationRequest::Viewer(_),
            CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_),
        )
        | (_, CapabilityNodeMessage::TerminalViewerResult(_))
        | (WorkerOperationRequest::Launch(_), CapabilityNodeMessage::TerminalStartResult(_))
        | (WorkerOperationRequest::Start(_), CapabilityNodeMessage::TerminalLaunchResult(_))
        | (
            WorkerOperationRequest::Send(_),
            CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_),
        )
        | (_, CapabilityNodeMessage::AgentDrainReply(_))
        | (_, CapabilityNodeMessage::TerminalTitlesResult(_))
        | (_, CapabilityNodeMessage::TerminalSendResult(_))
        | (_, CapabilityNodeMessage::PluginJobResult(_))
        | (_, CapabilityNodeMessage::WorktreeMutationResult(_)) => {
            Err(WorkerResponseFailure::WrongResultKind)
        }
        (
            WorkerOperationRequest::WorktreeJob(expected),
            CapabilityNodeMessage::WorktreeJobResult(actual),
        ) => {
            use orchbee_capability_node_protocol::{WorktreeJobAction, WorktreeJobOutcome};
            if actual.operation_id != expected.operation_id {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            let expected_job = match &expected.action {
                // Start mints the Job identity: the echoed Job ID is the
                // request's own operation identity.
                WorktreeJobAction::Start { .. }
                | WorktreeJobAction::StartSource { .. }
                | WorktreeJobAction::StartStatus { .. } => expected.operation_id,
                WorktreeJobAction::Poll { job_id } | WorktreeJobAction::Cancel { job_id } => {
                    *job_id
                }
            };
            if actual.job_id != expected_job {
                return Err(WorkerResponseFailure::MismatchedCorrelation);
            }
            if !matches!(
                (&expected.action, &actual.outcome),
                (
                    WorktreeJobAction::Start { .. }
                        | WorktreeJobAction::StartSource { .. }
                        | WorktreeJobAction::StartStatus { .. },
                    WorktreeJobOutcome::Start(_)
                ) | (WorktreeJobAction::Poll { .. }, WorktreeJobOutcome::Poll(_))
                    | (
                        WorktreeJobAction::Cancel { .. },
                        WorktreeJobOutcome::Cancel(_)
                    )
            ) {
                return Err(WorkerResponseFailure::WrongResultKind);
            }
            Ok(WorkerOperationResult::WorktreeJob(actual))
        }
        (_, CapabilityNodeMessage::WorktreeJobRequest(_)) => {
            Err(WorkerResponseFailure::WrongDirection)
        }
        (_, CapabilityNodeMessage::WorktreeJobResult(_))
        | (WorkerOperationRequest::Titles(_), _)
        | (WorkerOperationRequest::WorktreeJob(_), _)
        | (WorkerOperationRequest::PluginJob(_), _)
        | (WorkerOperationRequest::WorktreeMutation(_), _) => {
            Err(WorkerResponseFailure::WrongResultKind)
        }
        (
            WorkerOperationRequest::AgentDrain(_),
            CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_),
        ) => Err(WorkerResponseFailure::WrongResultKind),
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
