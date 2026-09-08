use super::super::*;
mod tab_close;
use crate::{
    local_client::{
        LocalClientShutdown, LocalOperationClock,
        test_server::{
            FakeServer, PrewriteGate, connect_with_adjustable_clock,
            connect_with_gated_prewrite_clock, connect_with_switchable_ceiling_clock,
        },
        workbench::WorkbenchTabCreateRequest,
    },
    terminal_projection::TerminalProjection,
    terminal_session::{
        mailbox::{
            CausalSettlement, CommandMailbox, CreateMarker, EventError, MoveSettlement,
            MutationResult, PollKey, QueuedCommand, RefreshSettlement, SwitchMarker, Tagged,
            TakenEvent, UiReply, WorkerEvent,
        },
        state::{
            AttachmentKey, CarrierFailure, CleanupContinuation, CreateOperation,
            CreateOperationKey, CreateProjectionDecision, FrameIdentity, MoveDecision,
            MoveOperation, MoveResult, NativeSessionState, OPERATION_TOTAL, OperationDeadlines,
            PendingCreateCommit, PendingMoveCommit, ReadyDecision, RefreshDecision,
            SetupEventDisposition,
        },
    },
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControllerId, Geometry, HostEpoch as ObcsHostEpoch, RequestId,
    ServerMessage, SiteId as ObcsSiteId, TerminalId as ObcsTerminalId, TerminalViewerAction,
    TerminalViewerOutcome, ViewerId, WorkerId as ObcsWorkerId, read_client_message,
    write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, Envelope, HostEpoch, MessageRole, PayloadEncoding,
    ProjectionBlockReason, ProjectionBlocked, ProjectionDemandControl, TerminalTextFrame,
    decode_terminal_text_frame, encode_projection_demand_control, encode_transport_envelope,
    inspect_terminal_frame_route,
};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, SiteId, Snapshot,
    Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchLabel, WorkbenchPanelId, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalContextId,
    WorkbenchTerminalGeometry, WorkerId, read_workbench_request, write_workbench_response,
};
use std::{
    error::Error,
    io::{self, Read, Write},
    net::Shutdown,
    num::NonZeroU64,
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) type TestResult<Value = ()> = std::result::Result<Value, Box<dyn Error + Send + Sync>>;
pub(super) const IDENTITY_SEED: u128 = 0x100;
mod switch_membership;
pub(super) use switch_membership::drive_switch_snapshot;
const COHORT: &str = "native-scripted-test";
const WALL_GUARD: Duration = Duration::from_secs(2);
const VIEWER_ID: &str = "22222222222222222222222222222222";
const HOST_EPOCH: &str = "11111111111111111111111111111111";
const CONTROLLER_ID: &str = "33333333333333333333333333333333";
const FRAME: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
));

#[derive(Clone, Copy)]
pub(super) enum WireKind {
    Obcs,
    Workbench,
}

enum ObservedRequest {
    Obcs(ClientMessage),
    Workbench(WorkbenchRequest),
}

enum ScriptResponse {
    Obcs(ServerMessage),
    Workbench(WorkbenchResponse),
    MalformedWorkbench,
    Abort,
}

pub(super) struct ScriptedCarrier {
    server: Option<FakeServer>,
    requests: Receiver<ObservedRequest>,
    responses: SyncSender<ScriptResponse>,
    response_written: Receiver<()>,
}

impl ScriptedCarrier {
    fn start(steps: Vec<WireKind>) -> TestResult<Self> {
        let (request_sender, requests) = mpsc::sync_channel(0);
        let (responses, response_receiver) = mpsc::sync_channel(0);
        let (response_written_sender, response_written) = mpsc::sync_channel(0);
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            for step in steps {
                let request = match step {
                    WireKind::Obcs => ObservedRequest::Obcs(
                        read_client_message(&mut stream).map_err(io::Error::other)?,
                    ),
                    WireKind::Workbench => ObservedRequest::Workbench(
                        read_workbench_request(&mut stream).map_err(io::Error::other)?,
                    ),
                };
                request_sender.send(request).map_err(io::Error::other)?;
                match response_receiver.recv().map_err(io::Error::other)? {
                    ScriptResponse::Obcs(response) if matches!(step, WireKind::Obcs) => {
                        write_server_message(&mut stream, &response).map_err(io::Error::other)?;
                        response_written_sender.send(()).map_err(io::Error::other)?;
                    }
                    ScriptResponse::Workbench(response) if matches!(step, WireKind::Workbench) => {
                        write_workbench_response(&mut stream, &response)
                            .map_err(io::Error::other)?;
                        response_written_sender.send(()).map_err(io::Error::other)?;
                    }
                    ScriptResponse::MalformedWorkbench if matches!(step, WireKind::Workbench) => {
                        stream.write_all(&[b'X', b'X', b'X', b'X', 0, 1, 0, 4, 0, 0, 0, 0])?;
                        response_written_sender.send(()).map_err(io::Error::other)?;
                    }
                    ScriptResponse::Abort => {
                        let _closed = stream.shutdown(Shutdown::Both);
                        return Ok(());
                    }
                    ScriptResponse::Obcs(_)
                    | ScriptResponse::Workbench(_)
                    | ScriptResponse::MalformedWorkbench => {
                        return Err(io::Error::other(
                            "script response used the wrong wire family",
                        ));
                    }
                }
            }
            let mut trailing = Vec::new();
            stream.read_to_end(&mut trailing)?;
            if trailing.is_empty() {
                Ok(())
            } else {
                Err(io::Error::other(
                    "executor sent an unscripted trailing request",
                ))
            }
        })?;
        Ok(Self {
            server: Some(server),
            requests,
            responses,
            response_written,
        })
    }

    pub(super) fn take_obcs(&self) -> TestResult<ClientMessage> {
        match self.requests.recv_timeout(WALL_GUARD)? {
            ObservedRequest::Obcs(request) => Ok(request),
            ObservedRequest::Workbench(_) => Err(test_error("expected OBCS request")),
        }
    }

    pub(super) fn take_workbench(&self) -> TestResult<WorkbenchRequest> {
        match self.requests.recv_timeout(WALL_GUARD)? {
            ObservedRequest::Workbench(request) => Ok(request),
            ObservedRequest::Obcs(_) => Err(test_error("expected OBWP request")),
        }
    }

    pub(super) fn respond_obcs(&self, response: ServerMessage) -> TestResult {
        self.responses.send(ScriptResponse::Obcs(response))?;
        self.response_written.recv_timeout(WALL_GUARD)?;
        Ok(())
    }

    pub(super) fn respond_workbench(&self, response: WorkbenchResponse) -> TestResult {
        self.responses.send(ScriptResponse::Workbench(response))?;
        self.response_written.recv_timeout(WALL_GUARD)?;
        Ok(())
    }

    pub(super) fn respond_malformed_workbench(&self) -> TestResult {
        self.responses.send(ScriptResponse::MalformedWorkbench)?;
        self.response_written.recv_timeout(WALL_GUARD)?;
        Ok(())
    }

    pub(super) fn abort(&self) {
        let _sent = self.responses.try_send(ScriptResponse::Abort);
    }

    pub(super) fn disconnect(&self) -> TestResult {
        self.responses.send(ScriptResponse::Abort)?;
        Ok(())
    }

    fn finish(&mut self) -> TestResult {
        if let Some(server) = self.server.take() {
            server.finish()?;
        }
        Ok(())
    }
}

pub(super) struct Harness {
    pub(super) mailbox: Arc<CommandMailbox>,
    pub(super) elapsed_millis: Arc<AtomicU64>,
    pub(super) carrier: ScriptedCarrier,
    pub(super) ui: UiDriver,
    ceiling_clock: Option<Arc<AtomicBool>>,
    prewrite_gate: Option<Arc<PrewriteGate>>,
    shutdown: LocalClientShutdown,
    worker_done: Receiver<()>,
    worker: Option<JoinHandle<NativeWorkerExit>>,
}

impl Harness {
    pub(super) fn start(steps: Vec<WireKind>, initial_geometry: Geometry) -> TestResult<Self> {
        Self::start_inner(steps, initial_geometry, None, None, IDENTITY_SEED)
    }

    pub(super) fn start_with_identity_seed(
        steps: Vec<WireKind>,
        initial_geometry: Geometry,
        identity_seed: u128,
    ) -> TestResult<Self> {
        Self::start_inner(steps, initial_geometry, None, None, identity_seed)
    }

    pub(super) fn start_with_ceiling_clock(
        steps: Vec<WireKind>,
        initial_geometry: Geometry,
    ) -> TestResult<Self> {
        Self::start_inner(
            steps,
            initial_geometry,
            Some(Arc::new(AtomicBool::new(false))),
            None,
            IDENTITY_SEED,
        )
    }

    pub(super) fn start_with_prewrite_gate(
        steps: Vec<WireKind>,
        initial_geometry: Geometry,
    ) -> TestResult<Self> {
        Self::start_inner(
            steps,
            initial_geometry,
            None,
            Some(Arc::new(PrewriteGate::default())),
            IDENTITY_SEED,
        )
    }

    fn start_inner(
        steps: Vec<WireKind>,
        initial_geometry: Geometry,
        ceiling_clock: Option<Arc<AtomicBool>>,
        prewrite_gate: Option<Arc<PrewriteGate>>,
        identity_seed: u128,
    ) -> TestResult<Self> {
        let carrier = ScriptedCarrier::start(steps)?;
        let elapsed_millis = Arc::new(AtomicU64::new(0));
        let server = carrier.server.as_ref().ok_or("fake Server")?;
        let mut client = match (&ceiling_clock, &prewrite_gate) {
            (Some(at_ceiling), None) => connect_with_switchable_ceiling_clock(
                server.endpoint(),
                COHORT,
                Arc::clone(&elapsed_millis),
                Arc::clone(at_ceiling),
            )?,
            (None, Some(prewrite_gate)) => connect_with_gated_prewrite_clock(
                server.endpoint(),
                COHORT,
                Arc::clone(&elapsed_millis),
                Arc::clone(prewrite_gate),
            )?,
            (None, None) => connect_with_adjustable_clock(
                server.endpoint(),
                COHORT,
                Arc::clone(&elapsed_millis),
            )?,
            (Some(_), Some(_)) => return Err(test_error("Harness clock modes are exclusive")),
        };
        let clock = client.operation_clock();
        let startup_deadline = clock
            .now()
            .checked_add(Duration::from_secs(50))
            .ok_or("startup deadline")?;
        client.set_submission_deadline(Some(startup_deadline));
        let shutdown = client.shutdown_handle()?;
        let mailbox = Arc::new(CommandMailbox::new(identity_seed, initial_geometry));
        let ui = UiDriver::new(Arc::clone(&mailbox), clock);
        let (worker_done_sender, worker_done) = mpsc::sync_channel(1);
        let worker_mailbox = Arc::clone(&mailbox);
        let worker = thread::Builder::new()
            .name("native-scripted-executor".into())
            .spawn(move || {
                let exit = super::run(client, initial_geometry, worker_mailbox);
                let _sent = worker_done_sender.send(());
                exit
            })?;
        Ok(Self {
            mailbox,
            elapsed_millis,
            carrier,
            ui,
            ceiling_clock,
            prewrite_gate,
            shutdown,
            worker_done,
            worker: Some(worker),
        })
    }

    pub(super) fn advance_clock_to_ceiling(&self) -> TestResult {
        let at_ceiling = self
            .ceiling_clock
            .as_ref()
            .ok_or_else(|| test_error("Harness does not use the switchable ceiling clock"))?;
        at_ceiling.store(true, Ordering::SeqCst);
        if self.ui.clock_now().checked_add(OPERATION_TOTAL).is_some() {
            return Err(test_error(
                "switchable clock did not reach the required operation ceiling",
            ));
        }
        Ok(())
    }

    pub(super) fn arm_prewrite_gate(&self) -> TestResult {
        self.prewrite_gate
            .as_ref()
            .ok_or_else(|| test_error("Harness does not use the prewrite gate"))?
            .arm()
            .map_err(Into::into)
    }

    pub(super) fn await_prewrite_blocked(&self) -> TestResult {
        if self
            .prewrite_gate
            .as_ref()
            .ok_or_else(|| test_error("Harness does not use the prewrite gate"))?
            .await_blocked_before(Instant::now() + WALL_GUARD)
        {
            Ok(())
        } else {
            Err(test_error("native worker did not reach the prewrite gate"))
        }
    }

    pub(super) fn release_prewrite_gate(&self) -> TestResult {
        let gate = self
            .prewrite_gate
            .as_ref()
            .ok_or_else(|| test_error("Harness does not use the prewrite gate"))?;
        gate.release();
        Ok(())
    }

    pub(super) fn finish(self) -> TestResult {
        self.finish_with_failure(CarrierFailure::Closed)
    }

    pub(super) fn take_obcs_with_advancing_clock(&self) -> TestResult<ClientMessage> {
        let running = Arc::new(AtomicBool::new(true));
        let task_running = Arc::clone(&running);
        let elapsed_millis = Arc::clone(&self.elapsed_millis);
        let mailbox = Arc::clone(&self.mailbox);
        let initial = elapsed_millis.load(Ordering::SeqCst);
        let wall_start = Instant::now();
        let ticker = thread::spawn(move || {
            while task_running.load(Ordering::SeqCst) {
                let wall_millis =
                    u64::try_from(wall_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                elapsed_millis.store(initial.saturating_add(wall_millis), Ordering::SeqCst);
                mailbox.wake_waiters();
                thread::yield_now();
            }
        });
        let result = self.carrier.take_obcs();
        running.store(false, Ordering::SeqCst);
        ticker
            .join()
            .map_err(|_panic| test_error("adjustable-clock driver panicked"))?;
        result
    }

    pub(super) fn finish_with_failure(mut self, failure: CarrierFailure) -> TestResult {
        let closed = self.mailbox.close(failure);
        if closed.failure() != failure {
            return Err(test_error(&format!(
                "mailbox closed as {:?}, expected {failure:?}",
                closed.failure()
            )));
        }
        let _closed = self.shutdown.close();
        if let Some(gate) = &self.prewrite_gate {
            gate.release();
        }
        self.carrier.abort();
        self.worker_done.recv_timeout(WALL_GUARD)?;
        if let Some(worker) = self.worker.take() {
            let _exit = worker
                .join()
                .map_err(|_panic| test_error("native executor panicked"))?;
        }
        if self.mailbox.is_closed() != Some(failure) {
            return Err(test_error("worker changed the mailbox first failure"));
        }
        self.carrier.finish()
    }

    pub(super) fn finish_after_worker_failure(self, expected: CarrierFailure) -> TestResult {
        let _pending_create = self.finish_after_worker_failure_with_pending_create(expected)?;
        Ok(())
    }

    pub(super) fn finish_after_worker_failure_with_pending_create(
        self,
        expected: CarrierFailure,
    ) -> TestResult<Option<PendingCreateCommit>> {
        self.finish_after_worker_failure_with_exit(expected)
            .map(NativeWorkerExit::into_pending_create)
    }

    pub(super) fn finish_after_worker_failure_with_pending_commits(
        self,
        expected: CarrierFailure,
    ) -> TestResult<(Option<PendingCreateCommit>, Option<PendingMoveCommit>)> {
        self.finish_after_worker_failure_with_exit(expected)
            .map(NativeWorkerExit::into_pending_commits)
    }

    fn finish_after_worker_failure_with_exit(
        mut self,
        expected: CarrierFailure,
    ) -> TestResult<NativeWorkerExit> {
        self.worker_done.recv_timeout(WALL_GUARD)?;
        let exit = self
            .worker
            .take()
            .ok_or_else(|| test_error("native executor was already joined"))?
            .join()
            .map_err(|_panic| test_error("native executor panicked"))?;
        if self.mailbox.is_closed() != Some(expected) {
            return Err(test_error(&format!(
                "worker closed mailbox as {:?}, expected {expected:?}",
                self.mailbox.is_closed()
            )));
        }
        let _closed = self.shutdown.close();
        if let Some(gate) = &self.prewrite_gate {
            gate.release();
        }
        self.carrier.abort();
        self.carrier.finish()?;
        Ok(exit)
    }

    pub(super) fn close_with_pending_move(self) -> TestResult<Option<PendingMoveCommit>> {
        self.mailbox.close(CarrierFailure::Closed);
        self.shutdown.close()?;
        self.carrier.abort();
        self.finish_after_worker_failure_with_pending_commits(CarrierFailure::Closed)
            .map(|(_create, moved)| moved)
    }
}

#[derive(Clone, Copy)]
pub(super) enum FrameCut {
    None,
    BeforeDraw(Geometry),
    AfterDraw(Geometry),
    AfterDrawSwitch(usize),
}

#[derive(Clone, Debug)]
pub(super) struct AdmittedSwitch {
    pub(super) key: AttachmentKey,
    pub(super) deadlines: OperationDeadlines,
}

/// Canonical installation has consumed the distinct successor; this key licenses setup.
pub(super) struct InstalledSwitch {
    pub(super) key: AttachmentKey,
    pub(super) deadlines: OperationDeadlines,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SwitchDetachContinuation {
    Attach(AttachTarget),
    SettledWithoutAttachment,
    Superseded { desired: AttachmentKey },
}

pub(super) enum UiEvent {
    SwitchSnapshot {
        work: AttachmentKey,
        decision: RefreshDecision,
    },
    Bootstrap {
        key: AttachmentKey,
    },
    Attached {
        key: AttachmentKey,
    },
    AttachedSuperseded {
        key: AttachmentKey,
        desired: AttachmentKey,
    },
    SetupConfigured {
        key: AttachmentKey,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    },
    SetupConfigurationSuperseded {
        key: AttachmentKey,
        desired: AttachmentKey,
    },
    EmptyPoll {
        key: AttachmentKey,
    },
    FrameDrawn {
        key: AttachmentKey,
        feedback: TerminalViewerAction,
    },
    NoFrameRedraw {
        key: AttachmentKey,
        feedback: Option<TerminalViewerAction>,
    },
    FeedbackAccepted {
        key: AttachmentKey,
    },
    FeedbackSuperseded {
        key: AttachmentKey,
        desired: AttachmentKey,
    },
    Ready(ReadyDecision),
    SwitchOldDetached {
        work: AttachmentKey,
        detached: AttachmentKey,
        continuation: SwitchDetachContinuation,
    },
    SwitchEntryExpired {
        key: AttachmentKey,
        continuation: CleanupContinuation,
    },
    SetupWithoutAttachment {
        key: AttachmentKey,
        continuation: CleanupContinuation,
    },
    CleanupSettled {
        key: AttachmentKey,
        continuation: CleanupContinuation,
    },
    CreatePrecommit {
        operation: CreateOperationKey,
    },
    CreateCommittedKnown {
        operation: CreateOperationKey,
    },
    CreateProjection {
        operation: CreateOperationKey,
        decision: CreateProjectionDecision,
    },
    Refresh {
        generation: u64,
        decision: RefreshDecision,
    },
}

pub(super) struct UiDriver {
    mailbox: Arc<CommandMailbox>,
    clock: LocalOperationClock,
    state: Option<NativeSessionState>,
    projection: Option<TerminalProjection>,
    drawn_frames: usize,
    no_frame_redraws: usize,
}

impl UiDriver {
    fn new(mailbox: Arc<CommandMailbox>, clock: LocalOperationClock) -> Self {
        Self {
            mailbox,
            clock,
            state: None,
            projection: None,
            drawn_frames: 0,
            no_frame_redraws: 0,
        }
    }

    pub(super) fn state(&self) -> &NativeSessionState {
        self.state.as_ref().expect("Bootstrap must install state")
    }

    fn state_mut(&mut self) -> &mut NativeSessionState {
        self.state.as_mut().expect("Bootstrap must install state")
    }

    pub(super) const fn drawn_frames(&self) -> usize {
        self.drawn_frames
    }

    pub(super) const fn no_frame_redraws(&self) -> usize {
        self.no_frame_redraws
    }

    pub(super) fn projection_is_some(&self) -> bool {
        self.projection.is_some()
    }

    pub(super) fn projection_frame(&self) -> Option<&TerminalTextFrame> {
        self.projection.as_ref().and_then(TerminalProjection::frame)
    }

    pub(super) fn clock_now(&self) -> Instant {
        self.clock.now()
    }

    pub(super) fn observe_geometry(&mut self, geometry: Geometry) -> TestResult {
        self.mailbox
            .observe_geometry(geometry)
            .map_err(|closed| test_error(&format!("mailbox closed: {:?}", closed.failure())))?;
        if let Some(state) = &mut self.state {
            state.observe_geometry(geometry);
        }
        Ok(())
    }

    pub(super) fn admit_switch(&mut self, index: usize) -> TestResult<AdmittedSwitch> {
        let sampled_at = self.clock.now();
        let deadlines = OperationDeadlines::from_sample(sampled_at)
            .ok_or_else(|| test_error("Switch deadline overflow"))?;
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        let generation = mailbox
            .try_enqueue_switch_for(
                state,
                |state, generation| {
                    let tab_id = state
                        .tab(index)
                        .ok_or("missing Switch target")?
                        .tab_id()
                        .clone();
                    let key = AttachmentKey::new(generation, tab_id);
                    let target = state
                        .target_for_switch(index, key, deadlines)
                        .ok_or("invalid Switch target")?;
                    if !state.begin_switch(target.clone()) {
                        return Err("Switch reducer rejected target");
                    }
                    Ok(SwitchMarker::new(target))
                },
                |state, result, facts| {
                    if result.is_ok() {
                        state.finish_switch_membership_admission(
                            facts.discarded(),
                            facts.inherited(),
                            facts.authorized_cleanup(),
                        );
                    }
                },
            )
            .map_err(|error| test_error(&format!("Switch admission failed: {error:?}")))?;
        let key = self.state().desired().unwrap().clone();
        if key.generation() != generation {
            return Err(test_error("Switch generation and desired key diverged"));
        }
        if self
            .projection
            .as_ref()
            .is_some_and(|projection| projection.frame().is_some())
        {
            // Admission hides the renderer through state.projection_key; keep exact anchor pixels.
            self.no_frame_redraws += 1;
        }
        Ok(AdmittedSwitch { key, deadlines })
    }

    pub(super) fn admit_create(&mut self) -> TestResult<CreateOperationKey> {
        let sampled_at = self.clock.now();
        let deadlines = OperationDeadlines::from_sample(sampled_at)
            .ok_or_else(|| test_error("Create deadline overflow"))?;
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        mailbox
            .try_enqueue_create_for(
                state,
                |state, generation, request_id| {
                    let cache = state.cache();
                    let site_id = cache.site_id().clone();
                    let expected_revision = cache.revision();
                    let terminal_context_id = cache.terminal_context_id().clone();
                    let label = WorkbenchLabel::parse(&(cache.tabs().len() + 1).to_string())
                        .map_err(|_error| "invalid Create label")?;
                    let geometry = state.latest_geometry();
                    let workbench_geometry = WorkbenchTerminalGeometry::new(
                        geometry.columns(),
                        geometry.rows(),
                        geometry.cell_pixel_width(),
                        geometry.cell_pixel_height(),
                    )
                    .map_err(|_error| "invalid Create geometry")?;
                    let command = WorkbenchCommand {
                        protocol_version: ProtocolVersion,
                        request_id: request_id.clone(),
                        site_id: site_id.clone(),
                        expected_revision,
                        action: WorkbenchAction::TabCreate {
                            terminal_context_id: terminal_context_id.clone(),
                            label: label.clone(),
                            geometry: workbench_geometry,
                        },
                    };
                    let request = WorkbenchTabCreateRequest::try_from(command)
                        .map_err(|_command| "opaque Create request rejected tab.create")?;
                    let operation = CreateOperation::new(
                        CreateOperationKey::new(request_id, generation),
                        site_id,
                        expected_revision,
                        terminal_context_id,
                        label,
                        geometry,
                        deadlines,
                    )
                    .with_origin(
                        if state.phase_kind()
                            == crate::terminal_session::state::SessionPhaseKind::Empty
                        {
                            crate::terminal_session::state::CreateOriginKind::Empty
                        } else {
                            crate::terminal_session::state::CreateOriginKind::Ready
                        },
                    );
                    if !state.begin_create(operation.clone()) {
                        return Err("Create reducer rejected admission");
                    }
                    Ok(CreateMarker::new(request, operation))
                },
                |_state, _result| {},
            )
            .map_err(|error| test_error(&format!("Create admission failed: {error:?}")))
    }

    pub(super) fn admit_refresh(&mut self) -> TestResult<u64> {
        let deadline = self
            .clock
            .now()
            .checked_add(OPERATION_TOTAL)
            .ok_or_else(|| test_error("Refresh deadline overflow"))?;
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        mailbox
            .try_enqueue_refresh_for(
                state,
                deadline,
                |state, generation| {
                    if state.start_pending_refresh(generation) {
                        Ok(())
                    } else {
                        Err("Refresh reducer rejected admission")
                    }
                },
                |_state, _result| {},
            )
            .map_err(|error| test_error(&format!("Refresh admission failed: {error:?}")))
    }

    // This seam checks worker transport; canonical Move settlement is covered by the state owner.
    pub(super) fn admit_move_transport(
        &mut self,
        direction: WorkbenchTabMoveDirection,
    ) -> TestResult<MoveOperation> {
        let clock = self.clock.clone();
        let deadline = clock
            .now()
            .checked_add(OPERATION_TOTAL)
            .ok_or_else(|| test_error("Move deadline overflow"))?;
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        mailbox
            .try_enqueue_move_for(
                state,
                deadline,
                &clock,
                |state, request_id| {
                    Ok::<_, &'static str>(MoveOperation::new(
                        request_id,
                        state.desired().unwrap().clone(),
                        state.cache().clone(),
                        direction,
                        deadline,
                    ))
                },
                |_state, _operation| Ok(()),
            )
            .map_err(|error| test_error(&format!("Move transport admission failed: {error:?}")))
    }

    pub(super) fn settle_move_transport(
        &mut self,
        taken: TakenEvent,
    ) -> TestResult<(MoveOperation, MoveResult)> {
        self.settle_move_transport_decision(taken, MoveDecision::Continue)
    }

    pub(super) fn settle_move_transport_decision(
        &mut self,
        taken: TakenEvent,
        decision: MoveDecision,
    ) -> TestResult<(MoveOperation, MoveResult)> {
        let (wait_key, event) = taken.into_parts();
        let WorkerEvent::MoveResult { operation, outcome } = event else {
            return Err(test_error("expected typed Move result"));
        };
        let wait = wait_key.ok_or_else(|| test_error("Move wait key absent"))?;
        require_applied(self.mailbox.deposit_ui_reply_before(
            &wait,
            &self.clock,
            |_transaction| {
                Ok(UiReply::Move(MoveSettlement::new(
                    decision,
                    operation.target().clone(),
                )))
            },
        ))?;
        Ok((operation, outcome))
    }

    pub(super) fn admit_input(&mut self, input: ControlInput) -> TestResult<u64> {
        let key = self.state().desired().unwrap().clone();
        self.mailbox
            .try_enqueue(QueuedCommand::Input(Tagged::new(key, input)))
            .map_err(|error| test_error(&format!("Input admission failed: {error:?}")))
    }

    pub(super) fn admit_resize(&mut self, geometry: Geometry) -> TestResult<u64> {
        self.mailbox
            .observe_geometry(geometry)
            .map_err(|closed| test_error(&format!("mailbox closed: {:?}", closed.failure())))?;
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        state.observe_geometry(geometry);
        mailbox
            .try_enqueue_with(|| {
                let key = state.desired().unwrap().clone();
                if !state.record_admitted_geometry(&key, geometry) {
                    return Err("Resize reducer rejected admission");
                }
                Ok(QueuedCommand::Resize(Tagged::new(key, geometry)))
            })
            .map_err(|error| test_error(&format!("Resize admission failed: {error:?}")))
    }

    pub(super) fn take_next(&self) -> TestResult<TakenEvent> {
        self.mailbox
            .await_event_before(&self.clock, Instant::now() + WALL_GUARD)
            .map_err(event_error)?
            .ok_or_else(|| test_error("timed out awaiting native worker event"))
    }

    pub(super) fn settle_taken(
        &mut self,
        taken: TakenEvent,
        frame_cut: FrameCut,
    ) -> TestResult<UiEvent> {
        self.settle(taken, frame_cut, None)
    }

    pub(super) fn settle_refresh_and_supersede(
        &mut self,
        taken: TakenEvent,
        index: usize,
    ) -> TestResult<(UiEvent, AdmittedSwitch)> {
        let mailbox = Arc::clone(&self.mailbox);
        let checkpoint = mailbox.pause_next_ui_reply_resume_for_test();
        if !mailbox.await_condition_waiter_before(Instant::now() + WALL_GUARD) {
            return Err(test_error(
                "worker did not enter its Refresh reply condition-variable wait",
            ));
        }
        let event = self.settle_taken(taken, FrameCut::None)?;
        if !checkpoint.await_reached_before(Instant::now() + WALL_GUARD) {
            return Err(test_error(
                "worker did not pause after receiving the Refresh reply",
            ));
        }
        let superseding = self.admit_switch(index)?;
        drop(checkpoint);
        Ok((event, superseding))
    }

    pub(super) fn settle_cleanup_before_and_advance_to(
        &mut self,
        taken: TakenEvent,
        elapsed_millis: &AtomicU64,
        next_millis: u64,
    ) -> TestResult<UiEvent> {
        self.settle(taken, FrameCut::None, Some((elapsed_millis, next_millis)))
    }

    pub(super) fn settle_next(&mut self, frame_cut: FrameCut) -> TestResult<UiEvent> {
        let taken = self.take_next()?;
        self.settle(taken, frame_cut, None)
    }

    fn settle(
        &mut self,
        taken: TakenEvent,
        frame_cut: FrameCut,
        cleanup_deposit_advance: Option<(&AtomicU64, u64)>,
    ) -> TestResult<UiEvent> {
        let (wait_key, event) = taken.into_parts();
        match event {
            WorkerEvent::SwitchSnapshotResult { marker, outcome } => self.settle_switch_snapshot(
                wait_key.ok_or_else(|| test_error("Switch Snapshot wait"))?,
                marker,
                outcome,
            ),
            WorkerEvent::SwitchForwardExpired { permit } => self.settle_switch_forward_expired(
                wait_key.ok_or_else(|| test_error("Switch expiry wait"))?,
                permit.into_target(),
            ),
            WorkerEvent::MembershipCleanupSkipped { cleanup } => self
                .settle_switch_membership_cleanup(
                    wait_key.ok_or_else(|| test_error("Switch skipped wait"))?,
                    cleanup,
                    false,
                ),
            WorkerEvent::MembershipDetached { cleanup, detached } => {
                if matches!(
                    cleanup.result(),
                    crate::terminal_session::state::MembershipResultKey::Switch(_)
                ) {
                    if cleanup.old() != &detached {
                        return Err(test_error("Switch detached key mismatch"));
                    }
                    return self.settle_switch_membership_cleanup(
                        wait_key.ok_or_else(|| test_error("Switch cleanup wait"))?,
                        cleanup,
                        true,
                    );
                }
                let wait_key = wait_key.ok_or_else(|| test_error("membership wait key"))?;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                let prepared = state
                    .prepare_cleaned_membership_candidate(&cleanup, &detached)
                    .map_err(|_| test_error("exact membership cleanup"))?;
                let result_key = cleanup.result().clone();
                let mut decision = None;
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |transaction| {
                        let settled = if transaction.observed_at()
                            >= cleanup.deadlines().forward_cutoff()
                        {
                            state.retire(CarrierFailure::Deadline);
                            RefreshDecision::Retire
                        } else {
                            let install = prepared.selected_tab_id().map_or(
                                crate::terminal_session::mailbox::DesiredInstall::Empty,
                                |tab| {
                                    if let Some(committed) = state.create_membership_commit()
                                        && committed.committed_key().tab_id() == tab {
                                        crate::terminal_session::mailbox::DesiredInstall::ReuseReserved(committed.committed_key().clone())
                                    } else {
                                        crate::terminal_session::mailbox::DesiredInstall::Fresh(tab.clone())
                                    }
                                },
                            );
                            let desired = transaction.prepare_desired_install(install)?;
                            let state_plan = state.prepare_cleaned_membership_install(
                                &cleanup,
                                prepared,
                                desired.desired().cloned(),
                            )?;
                            let installed = state_plan.commit();
                            desired.commit();
                            use crate::terminal_session::state::MembershipInstallDecision;
                            match installed {
                                MembershipInstallDecision::Attach(target) => {
                                    RefreshDecision::AttachRecovered(target)
                                }
                                MembershipInstallDecision::Empty
                                | MembershipInstallDecision::SelectedWithoutAttachment => {
                                    RefreshDecision::SettledWithoutAttachment
                                }
                                MembershipInstallDecision::Preserved => RefreshDecision::Continue,
                            }
                        };
                        decision = Some(settled.clone());
                        Ok(UiReply::Membership(RefreshSettlement::new(
                            settled,
                            state.desired().cloned(),
                        )))
                    },
                ))?;
                self.projection = None;
                let decision = decision.ok_or_else(|| test_error("membership decision"))?;
                match result_key {
                    crate::terminal_session::state::MembershipResultKey::Refresh(generation) => {
                        Ok(UiEvent::Refresh {
                            generation,
                            decision,
                        })
                    }
                    crate::terminal_session::state::MembershipResultKey::Create(operation) => {
                        Ok(UiEvent::CreateProjection {
                            operation,
                            decision: match decision {
                                RefreshDecision::AttachRecovered(target) => {
                                    CreateProjectionDecision::Attach(target)
                                }
                                RefreshDecision::SettledWithoutAttachment => {
                                    CreateProjectionDecision::SettledWithoutAttachment
                                }
                                RefreshDecision::Retire => CreateProjectionDecision::Retire,
                                _ => return Err(test_error("unexpected Create cleanup decision")),
                            },
                        })
                    }
                    crate::terminal_session::state::MembershipResultKey::Switch(_) => {
                        Err(test_error("Switch membership cleanup is staged"))
                    }
                    crate::terminal_session::state::MembershipResultKey::Close(_) => {
                        Err(test_error("use the exact Close transport consumer"))
                    }
                }
            }
            WorkerEvent::CloseReconciliationSnapshot { .. }
            | WorkerEvent::MembershipGone { .. }
            | WorkerEvent::CloseResult { .. } => {
                Err(test_error("use the exact Close transport consumer"))
            }
            WorkerEvent::MoveResult { .. } => Err(test_error("Move consumer is staged")),
            WorkerEvent::Bootstrap { cache } => {
                let wait_key = wait_key.ok_or_else(|| test_error("Bootstrap wait key"))?;
                let mut installed = None;
                let result = self.mailbox.deposit_bootstrap_reply_before(
                    &wait_key,
                    &self.clock,
                    |generation, geometry| {
                        let (state, target) =
                            NativeSessionState::starting(cache, generation, geometry)
                                .ok_or(CarrierFailure::AuthorityDivergence)?;
                        installed = Some(state);
                        Ok(target)
                    },
                );
                require_applied(result)?;
                self.state = installed;
                Ok(UiEvent::Bootstrap {
                    key: self.state().desired().unwrap().clone(),
                })
            }
            WorkerEvent::Attached { key, projection } => {
                let wait_key = wait_key.ok_or_else(|| test_error("Attached wait key"))?;
                let mut disposition = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled = state
                            .settle_attached_event(&key)
                            .ok_or(CarrierFailure::Protocol)?;
                        disposition = Some(settled.clone());
                        Ok(UiReply::AttachedSettled(settled))
                    },
                ))?;
                match disposition
                    .ok_or_else(|| test_error("Attached disposition was not deposited"))?
                {
                    SetupEventDisposition::Applied => {
                        self.projection = Some(projection);
                        Ok(UiEvent::Attached { key })
                    }
                    SetupEventDisposition::Superseded { desired } => {
                        Ok(UiEvent::AttachedSuperseded { key, desired })
                    }
                }
            }
            WorkerEvent::SetupConfigured {
                key,
                mode,
                applied_geometry,
            } => {
                let wait_key = wait_key.ok_or_else(|| test_error("SetupConfigured wait key"))?;
                let mut disposition = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled = state
                            .settle_setup_configuration_event(&key, mode.clone(), applied_geometry)
                            .ok_or(CarrierFailure::Protocol)?;
                        disposition = Some(settled.clone());
                        Ok(UiReply::SetupConfigurationSettled(settled))
                    },
                ))?;
                match disposition
                    .ok_or_else(|| test_error("setup disposition was not deposited"))?
                {
                    SetupEventDisposition::Applied => {
                        if applied_geometry.is_some()
                            && let Some(projection) = &mut self.projection
                            && projection.frame().is_some()
                        {
                            projection.clear_frame();
                            self.no_frame_redraws += 1;
                        }
                        Ok(UiEvent::SetupConfigured {
                            key,
                            mode,
                            applied_geometry,
                        })
                    }
                    SetupEventDisposition::Superseded { desired } => {
                        Ok(UiEvent::SetupConfigurationSuperseded { key, desired })
                    }
                }
            }
            WorkerEvent::PollResult { poll_key, record } => {
                if wait_key.is_some() {
                    return Err(test_error("PollResult must use the causal slot"));
                }
                self.settle_poll(poll_key, record, frame_cut)
            }
            WorkerEvent::FeedbackAccepted { key, frame } => {
                let wait_key = wait_key.ok_or_else(|| test_error("FeedbackAccepted wait key"))?;
                let mut disposition = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled = state
                            .settle_feedback_event(&key, frame)
                            .ok_or(CarrierFailure::Protocol)?;
                        disposition = Some(settled.clone());
                        Ok(UiReply::FeedbackSettled(settled))
                    },
                ))?;
                match disposition
                    .ok_or_else(|| test_error("feedback disposition was not deposited"))?
                {
                    SetupEventDisposition::Applied => Ok(UiEvent::FeedbackAccepted { key }),
                    SetupEventDisposition::Superseded { desired } => {
                        Ok(UiEvent::FeedbackSuperseded { key, desired })
                    }
                }
            }
            WorkerEvent::ReadyCandidate {
                key,
                mode,
                last_accepted_geometry,
                frame,
            } => {
                let wait_key = wait_key.ok_or_else(|| test_error("ReadyCandidate wait key"))?;
                let mut decision = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled = state.settle_ready_candidate(
                            &key,
                            frame,
                            mode,
                            last_accepted_geometry,
                        )?;
                        decision = Some(settled);
                        Ok(UiReply::Ready(settled))
                    },
                ))?;
                Ok(UiEvent::Ready(decision.ok_or_else(|| {
                    test_error("Ready decision was not deposited")
                })?))
            }
            WorkerEvent::SwitchEntryExpired {
                key,
                target_index,
                target_provenance,
            } => {
                let wait_key = wait_key.ok_or_else(|| test_error("SwitchEntryExpired wait key"))?;
                let mut continuation = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |transaction| {
                        let settled = state
                            .settle_switch_entry_expired(
                                &key,
                                target_index,
                                &target_provenance,
                                transaction.observed_at(),
                            )
                            .ok_or(CarrierFailure::Protocol)?;
                        continuation = Some(settled.clone());
                        Ok(UiReply::SwitchEntryExpiredApplied(settled))
                    },
                ))?;
                Ok(UiEvent::SwitchEntryExpired {
                    key,
                    continuation: continuation.ok_or_else(|| {
                        test_error("Switch entry expiry continuation was not deposited")
                    })?,
                })
            }
            WorkerEvent::SetupWithoutAttachment {
                key,
                target_index,
                target_provenance,
            } => {
                let wait_key =
                    wait_key.ok_or_else(|| test_error("SetupWithoutAttachment wait key"))?;
                let mut continuation = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled = state
                            .settle_unattached_setup(&key, target_index, &target_provenance)
                            .ok_or(CarrierFailure::Protocol)?;
                        continuation = Some(settled.clone());
                        Ok(UiReply::SetupWithoutAttachmentApplied(settled))
                    },
                ))?;
                self.projection = None;
                Ok(UiEvent::SetupWithoutAttachment {
                    key,
                    continuation: continuation.ok_or_else(|| {
                        test_error("unattached setup continuation was not deposited")
                    })?,
                })
            }
            WorkerEvent::CleanupSettled {
                key,
                target_index,
                target_provenance,
            } => {
                let wait_key = wait_key.ok_or_else(|| test_error("CleanupSettled wait key"))?;
                let mut continuation = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled = state
                            .settle_cleanup(&key, target_index, &target_provenance)
                            .ok_or(CarrierFailure::Protocol)?;
                        continuation = Some(settled.clone());
                        if let Some((elapsed_millis, next_millis)) = cleanup_deposit_advance {
                            elapsed_millis.store(next_millis, Ordering::SeqCst);
                        }
                        Ok(UiReply::CleanupApplied(settled))
                    },
                ))?;
                self.projection = None;
                Ok(UiEvent::CleanupSettled {
                    key,
                    continuation: continuation
                        .ok_or_else(|| test_error("cleanup continuation was not deposited"))?,
                })
            }
            WorkerEvent::CreatePrecommit {
                operation,
                outcome,
                applied_geometry,
            } => {
                let wait_key = wait_key.ok_or_else(|| test_error("CreatePrecommit wait key"))?;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |transaction| {
                        let _definite_outcome = outcome;
                        if !state.restore_precommit(&operation, applied_geometry) {
                            return Err(CarrierFailure::Protocol);
                        }
                        let latest = transaction.latest_geometry();
                        if state.latest_geometry() != latest {
                            return Err(CarrierFailure::Protocol);
                        }
                        if applied_geometry.is_some_and(|applied| applied != latest) {
                            let key = state.desired().unwrap().clone();
                            transaction.enqueue_catch_up_resize(key.clone(), latest)?;
                            if !state.record_admitted_geometry(&key, latest) {
                                return Err(CarrierFailure::Protocol);
                            }
                        }
                        Ok(UiReply::PrecommitRestored)
                    },
                ))?;
                Ok(UiEvent::CreatePrecommit { operation })
            }
            WorkerEvent::CreateCommittedKnown { operation, commit } => {
                let wait_key =
                    wait_key.ok_or_else(|| test_error("CreateCommittedKnown wait key"))?;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        if !state.retain_create_commit(&operation, commit) {
                            return Err(CarrierFailure::Protocol);
                        }
                        Ok(UiReply::CommitRetained)
                    },
                ))?;
                Ok(UiEvent::CreateCommittedKnown { operation })
            }
            WorkerEvent::CreateSnapshotResult { operation, outcome } => {
                let wait_key =
                    wait_key.ok_or_else(|| test_error("CreateSnapshotResult wait key"))?;
                let mut decision = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                let prepared = state.prepare_create_snapshot(&operation, outcome);
                if let crate::terminal_session::state::PreparedCreateSnapshot::Available {
                    baseline,
                    committed,
                    projection,
                } = prepared
                {
                    let prepared = state
                        .prepare_create_membership_candidate(&baseline, &committed, projection)
                        .map_err(|_| test_error("prepared Create candidate"))?;
                    require_applied(mailbox.deposit_ui_reply_before(&wait_key, &clock, |transaction| {
                        let deadlines = committed.operation().deadlines();
                        let settled = if state.attached_key().is_some() {
                            CreateProjectionDecision::Cleanup(state.stage_create_membership_cleanup(&operation, prepared, deadlines)?)
                        } else {
                            if transaction.observed_at() >= deadlines.forward_cutoff() { return Err(CarrierFailure::Deadline); }
                            let install = match prepared.selected_tab_id() {
                                Some(tab) if tab == committed.committed_key().tab_id() => crate::terminal_session::mailbox::DesiredInstall::ReuseReserved(committed.committed_key().clone()),
                                Some(tab) => crate::terminal_session::mailbox::DesiredInstall::Fresh(tab.clone()),
                                None => crate::terminal_session::mailbox::DesiredInstall::Empty,
                            };
                            let desired = transaction.prepare_desired_install(install)?;
                            let plan = state.prepare_create_membership_install(&operation, prepared, desired.desired().cloned(), deadlines)?;
                            let installed = plan.commit();
                            desired.commit();
                            match installed {
                                crate::terminal_session::state::MembershipInstallDecision::Attach(target) => CreateProjectionDecision::Attach(target),
                                crate::terminal_session::state::MembershipInstallDecision::Empty
                                | crate::terminal_session::state::MembershipInstallDecision::SelectedWithoutAttachment => CreateProjectionDecision::SettledWithoutAttachment,
                                crate::terminal_session::state::MembershipInstallDecision::Preserved => unreachable!("Create changes selection"),
                            }
                        };
                        decision = Some(settled.clone());
                        Ok(UiReply::CreateProjection(settled))
                    }))?;
                    return Ok(UiEvent::CreateProjection {
                        operation,
                        decision: decision
                            .ok_or_else(|| test_error("Create candidate decision"))?,
                    });
                }
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled = state.apply_prepared_create_snapshot(prepared);
                        decision = Some(settled.clone());
                        Ok(UiReply::CreateProjection(settled))
                    },
                ))?;
                Ok(UiEvent::CreateProjection {
                    operation,
                    decision: decision.ok_or_else(|| {
                        test_error("Create projection decision was not deposited")
                    })?,
                })
            }
            WorkerEvent::RefreshSnapshotResult {
                generation,
                outcome,
            } => {
                let wait_key =
                    wait_key.ok_or_else(|| test_error("RefreshSnapshotResult wait key"))?;
                let prepared = self.state().prepare_refresh(generation, outcome);
                let recovery_deadlines = prepared
                    .needs_recovery_deadlines()
                    .then(|| OperationDeadlines::from_sample(self.clock.now()))
                    .flatten();
                let mut decision = None;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                require_applied(mailbox.deposit_ui_reply_before(
                    &wait_key,
                    &clock,
                    |_transaction| {
                        let settled =
                            if let crate::terminal_session::state::PreparedRefresh::CreateMembership { generation, operation, prepared } = prepared {
                                RefreshDecision::Cleanup(state.stage_create_refresh_cleanup(generation, &operation, prepared, recovery_deadlines.ok_or(CarrierFailure::Deadline)?)?)
                            } else if let crate::terminal_session::state::PreparedRefresh::Membership {
                                generation,
                                prepared,
                            } = prepared
                            {
                                RefreshDecision::Cleanup(state.stage_refresh_membership_cleanup(
                                    generation,
                                    prepared,
                                    recovery_deadlines.ok_or(CarrierFailure::Deadline)?,
                                )?)
                            } else {
                                state.apply_prepared_refresh(prepared, recovery_deadlines)
                            };
                        let desired = state.desired().cloned();
                        decision = Some(settled.clone());
                        Ok(UiReply::Refresh(RefreshSettlement::new(settled, desired)))
                    },
                ))?;
                Ok(UiEvent::Refresh {
                    generation,
                    decision: decision
                        .ok_or_else(|| test_error("Refresh decision was not deposited"))?,
                })
            }
        }
    }

    fn settle_poll(
        &mut self,
        poll_key: PollKey,
        record: Option<Vec<u8>>,
        frame_cut: FrameCut,
    ) -> TestResult<UiEvent> {
        let key = poll_key.attachment().clone();
        if self.state().projection_key() != Some(&key) {
            match self
                .mailbox
                .deposit_causal(&poll_key, CausalSettlement::NoFeedback)
                .map_err(|closed| test_error(&format!("mailbox closed: {:?}", closed.failure())))?
            {
                MutationResult::Applied | MutationResult::Stale => {}
            }
            return Ok(UiEvent::EmptyPoll { key });
        }
        let Some(record) = record else {
            let result = self
                .mailbox
                .complete_empty_poll(&poll_key)
                .map_err(|closed| test_error(&format!("mailbox closed: {:?}", closed.failure())))?;
            if result != MutationResult::Applied {
                return Err(test_error("current Empty Poll settlement was stale"));
            }
            return Ok(UiEvent::EmptyPoll { key });
        };
        let expected_geometry = self.state().expected_frame_geometry(&key);
        let outcome = self
            .projection
            .as_mut()
            .ok_or_else(|| test_error("PollResult without projection"))?
            .receive_for_geometry(&record, expected_geometry)
            .map_err(|()| test_error("terminal projection rejected routed record"))?;
        match outcome {
            Some(feedback @ TerminalViewerAction::FrameApplied { .. }) => {
                if self
                    .projection
                    .as_ref()
                    .and_then(TerminalProjection::frame)
                    .is_none()
                {
                    return Err(test_error("FrameApplied without a drawable frame"));
                }
                if let FrameCut::BeforeDraw(geometry) = frame_cut {
                    self.observe_geometry(geometry)?;
                }
                let frame = FrameIdentity::new(poll_key.serial());
                if !self.state_mut().stage_drawn_frame(&key, frame) {
                    return Err(test_error("actual draw checkpoint rejected current frame"));
                }
                self.drawn_frames += 1;
                if let FrameCut::AfterDraw(geometry) = frame_cut {
                    self.observe_geometry(geometry)?;
                }
                if let FrameCut::AfterDrawSwitch(index) = frame_cut {
                    self.admit_switch(index)?;
                }
                let mutation = self
                    .mailbox
                    .deposit_causal(&poll_key, CausalSettlement::Feedback(feedback.clone()))
                    .map_err(|closed| {
                        test_error(&format!("mailbox closed: {:?}", closed.failure()))
                    })?;
                if !matches!(frame_cut, FrameCut::AfterDrawSwitch(_)) {
                    require_mutation(Ok(mutation))?;
                } else if mutation != MutationResult::Stale {
                    return Err(test_error(
                        "Switch admitted after draw must stale the unsent feedback",
                    ));
                }
                Ok(UiEvent::FrameDrawn { key, feedback })
            }
            Some(feedback @ TerminalViewerAction::NeedSnapshot { .. }) => {
                if !self.state_mut().invalidate_projection_frame(&key) {
                    return Err(test_error(
                        "NeedSnapshot no-frame redraw rejected current key",
                    ));
                }
                self.no_frame_redraws += 1;
                require_mutation(
                    self.mailbox
                        .deposit_causal(&poll_key, CausalSettlement::Feedback(feedback.clone())),
                )?;
                Ok(UiEvent::NoFrameRedraw {
                    key,
                    feedback: Some(feedback),
                })
            }
            None => {
                if !self.state_mut().invalidate_projection_frame(&key) {
                    return Err(test_error("blocked no-frame redraw rejected current key"));
                }
                self.no_frame_redraws += 1;
                require_mutation(
                    self.mailbox
                        .deposit_causal(&poll_key, CausalSettlement::NoFeedback),
                )?;
                Ok(UiEvent::NoFrameRedraw {
                    key,
                    feedback: None,
                })
            }
            Some(TerminalViewerAction::Attach { .. })
            | Some(TerminalViewerAction::AttachBinding { .. })
            | Some(TerminalViewerAction::Poll {})
            | Some(TerminalViewerAction::Detach {}) => {
                Err(test_error("projection returned a non-feedback action"))
            }
        }
    }
}

fn admit(stream: &mut UnixStream) -> io::Result<()> {
    if !matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ) {
        return Err(io::Error::other("expected ClientHello"));
    }
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}

fn require_applied(result: std::result::Result<MutationResult, EventError>) -> TestResult {
    match result.map_err(event_error)? {
        MutationResult::Applied => Ok(()),
        MutationResult::Stale => Err(test_error("current UI reply was stale")),
    }
}

fn require_mutation(
    result: std::result::Result<MutationResult, crate::terminal_session::mailbox::MailboxClosed>,
) -> TestResult {
    match result.map_err(|closed| test_error(&format!("mailbox closed: {:?}", closed.failure())))? {
        MutationResult::Applied => Ok(()),
        MutationResult::Stale => Err(test_error("current causal settlement was stale")),
    }
}

fn event_error(error: EventError) -> Box<dyn Error + Send + Sync> {
    test_error(&format!("mailbox event failed: {error:?}"))
}

pub(super) fn test_error(message: &str) -> Box<dyn Error + Send + Sync> {
    Box::new(io::Error::other(message.to_owned()))
}

pub(super) fn fixture_geometry() -> TestResult<Geometry> {
    let frame = decode_terminal_text_frame(FRAME)?;
    Ok(Geometry::new(
        frame.geometry.columns,
        frame.geometry.rows,
        frame.geometry.cell_pixel_width,
        frame.geometry.cell_pixel_height,
    )?)
}

pub(super) fn full_frame_record(
    sequence: u64,
    revision: u64,
    geometry: Geometry,
) -> TestResult<Vec<u8>> {
    let mut frame = FRAME.to_vec();
    frame[48..56].copy_from_slice(&sequence.to_le_bytes());
    frame[56..64].copy_from_slice(&u64::MAX.to_le_bytes());
    frame[64..72].copy_from_slice(&revision.to_le_bytes());
    frame[136..138].copy_from_slice(&geometry.cell_pixel_width().to_le_bytes());
    frame[138..140].copy_from_slice(&geometry.cell_pixel_height().to_le_bytes());
    let route = inspect_terminal_frame_route(&frame)?;
    Ok(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: &frame,
    })?)
}

pub(super) fn blocked_record(revision: u64) -> TestResult<Vec<u8>> {
    let blocked = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id: AttachmentId::parse(VIEWER_ID)?,
            reason: ProjectionBlockReason::ProjectionTooLarge,
            revision,
        },
    ))?;
    Ok(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: blocked.role,
        kind: blocked.kind,
        payload_encoding: PayloadEncoding::ControlJson,
        host_epoch: Some(HostEpoch::parse(HOST_EPOCH)?),
        correlation_id: None,
        optional_message: false,
        payload: &blocked.payload,
    })?)
}

pub(super) fn snapshot(tab_count: usize, revision: u64) -> Snapshot {
    Snapshot {
        navigation: Navigation {
            projects: Vec::new(),
            standalone_terminal_context_id: workbench_context_id(200).unwrap(),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        site_id: workbench_site_id(100).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs: (0..tab_count).map(tab).collect(),
            terminal_context_id: workbench_context_id(200).unwrap(),
        }],
    }
}

fn tab(index: usize) -> Tab {
    let base = 1_000 + u128::try_from(index).unwrap() * 10;
    let label = (index + 1).to_string();
    Tab {
        group_id: None,
        label: label.clone(),
        pinned: false,
        root: PanelNode::Panel {
            child_presentations: Vec::new(),
            panel_id: WorkbenchPanelId::parse(&id(base + 1)).unwrap(),
            primary_presentation: Presentation {
                binding: Some(PresentationBinding {
                    terminal_id: TerminalId::parse(&id(base + 4)).unwrap(),
                    worker_id: WorkerId::parse(&id(base + 3)).unwrap(),
                }),
                label,
                presentation_id: TerminalPresentationId::parse(&id(base + 2)).unwrap(),
            },
        },
        tab_id: WorkbenchTabId::parse(&id(base)).unwrap(),
    }
}

pub(super) fn attach_action(index: usize) -> TestResult<TerminalViewerAction> {
    let base = 1_000 + u128::try_from(index)? * 10;
    Ok(TerminalViewerAction::AttachBinding {
        site_id: ObcsSiteId::parse(&id(100))?,
        worker_id: ObcsWorkerId::parse(&id(base + 3))?,
        terminal_id: ObcsTerminalId::parse(&id(base + 4))?,
    })
}

pub(super) fn attached_outcome() -> TestResult<TerminalViewerOutcome> {
    Ok(TerminalViewerOutcome::Attached {
        attachment_id: ViewerId::parse(VIEWER_ID)?,
        host_epoch: ObcsHostEpoch::parse(HOST_EPOCH)?,
    })
}

pub(super) fn controller_id() -> TestResult<ControllerId> {
    Ok(ControllerId::parse(CONTROLLER_ID)?)
}

pub(super) fn obcs_request_id(value: u128) -> TestResult<RequestId> {
    Ok(RequestId::parse(&id(value))?)
}

pub(super) fn workbench_tab_id(value: u128) -> TestResult<WorkbenchTabId> {
    Ok(WorkbenchTabId::parse(&id(value))?)
}

fn workbench_site_id(value: u128) -> TestResult<SiteId> {
    Ok(SiteId::parse(&id(value))?)
}

fn workbench_context_id(value: u128) -> TestResult<WorkbenchTerminalContextId> {
    Ok(WorkbenchTerminalContextId::parse(&id(value))?)
}

fn id(value: u128) -> String {
    format!("{value:032x}")
}

pub(super) fn assert_strictly_sequential_ids(ids: &[String], seed: u128) -> TestResult {
    for (offset, actual) in ids.iter().enumerate() {
        let expected = id(seed + u128::try_from(offset)? + 1);
        if actual != &expected {
            return Err(test_error(&format!(
                "request identity {offset} was {actual}, expected {expected}"
            )));
        }
    }
    Ok(())
}

pub(super) fn drive_open_snapshot_bootstrap(
    harness: &mut Harness,
    tab_count: usize,
) -> TestResult<AttachmentKey> {
    let ClientMessage::TerminalOpen(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("startup must begin with terminal.open"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalOpenResult(
            orchbee_client_server_protocol::TerminalOpenResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::OpenOutcome::Opened {
                    terminal_create_request_id: obcs_request_id(900)?,
                },
            ),
        ))?;
    let WorkbenchRequest::SnapshotRead(request) = harness.carrier.take_workbench()? else {
        return Err(test_error("terminal.open must be followed by SnapshotRead"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Snapshot(
            orchbee_workbench_protocol::WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(tab_count, 1),
                },
            },
        ))?;
    match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::Bootstrap { key } => Ok(key),
        _ => Err(test_error("snapshot must publish Bootstrap")),
    }
}

pub(super) fn drive_attach(harness: &mut Harness, index: usize) -> TestResult<AttachmentKey> {
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("Bootstrap must license AttachBinding"));
    };
    assert_eq!(request.action(), &attach_action(index)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            orchbee_client_server_protocol::TerminalViewerResult::new(
                request.request_id().clone(),
                attached_outcome()?,
            ),
        ))?;
    match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::Attached { key } => Ok(key),
        _ => Err(test_error("Attached result must install projection")),
    }
}
