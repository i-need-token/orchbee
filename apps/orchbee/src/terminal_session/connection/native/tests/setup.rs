use super::super::*;
use orchbee_local_ipc::LocalStream as UnixStream;
mod tab_close;
mod workspace_setup;
use crate::{
    local_client::{
        LocalClientShutdown, LocalOperationClock,
        tests::server::{
            FakeServer, PrewriteGate, connect_with_adjustable_clock,
            connect_with_gated_prewrite_clock, connect_with_switchable_ceiling_clock,
        },
        workbench::WorkbenchTabCreateRequest,
    },
    terminal_projection::TerminalProjection,
    terminal_session::{
        mailbox::{
            CausalSettlement, CommandMailbox, CreateMarker, EventError, MoveSettlement,
            MutationResult, PanelSettlement, PollKey, QueuedCommand, RefreshSettlement,
            SwitchMarker, Tagged, TakenEvent, UiReply, WorkerEvent,
        },
        state::{
            AttachmentKey, CarrierFailure, CleanupContinuation, CreateOperation,
            CreateOperationKey, CreateProjectionDecision, FrameIdentity, MoveDecision,
            MoveOperation, MoveResult, NativeSessionState, OPERATION_TOTAL, OperationDeadlines,
            PaneKey, PaneRef, PanelDecision, PanelOperation, PanelRequestKind, PanelResult,
            PendingCreateCommit, PendingMoveCommit, PendingPanelCommit, ReadyDecision,
            RefreshDecision, SetupEventDisposition,
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
    Navigation, PanelNode, PanelOrientation, PanelSlot, Presentation, PresentationBinding,
    ProtocolVersion, PublicationReadIntent, SiteId, Snapshot, SnapshotReadIntent, Tab,
    TerminalContext, TerminalId, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchPublicationDocument,
    WorkbenchPublicationOutcome, WorkbenchPublicationResult, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchSplitId, WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalContextId,
    WorkbenchTerminalGeometry, WorkerId, read_workbench_request, write_workbench_response,
};
use std::{
    error::Error,
    io::{self, Read, Write},
    net::Shutdown,
    num::NonZeroU64,
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
pub(super) const WALL_GUARD: Duration = Duration::from_secs(2);
const VIEWER_ID: &str = "22222222222222222222222222222222";
/// A second physical viewer identity: a legitimate Server mints a distinct
/// attachment per pane, so multi-pane tests must not reuse `VIEWER_ID`.
pub(super) const PEER_VIEWER_ID: &str = "44444444444444444444444444444444";
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

/// A Workbench read observed on the wire. Refresh-path reads arrive as `publication.read` once
/// the executor holds a baseline; bootstrap reads stay `snapshot.read`.
pub(super) enum ObservedRead {
    Snapshot(SnapshotReadIntent),
    Publication(PublicationReadIntent),
}

impl ObservedRead {
    pub(super) fn request_id(&self) -> &WorkbenchRequestId {
        match self {
            Self::Snapshot(intent) => &intent.request_id,
            Self::Publication(intent) => &intent.request_id,
        }
    }

    pub(super) fn known_revision(&self) -> Option<WorkbenchRevision> {
        match self {
            Self::Snapshot(_) => None,
            Self::Publication(intent) => Some(intent.known_revision),
        }
    }
}

/// Maps the latest committed snapshot outcome onto a `publication.read` reply the way the real
/// journal does: `current` when the reader already holds that revision, otherwise `published`
/// carrying the full snapshot as the boundary publication.
fn publication_outcome_for(
    outcome: WorkbenchSnapshotOutcome,
    known_revision: WorkbenchRevision,
) -> WorkbenchPublicationOutcome {
    match outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => {
            if snapshot.revision > known_revision {
                WorkbenchPublicationOutcome::Published {
                    publications: vec![WorkbenchPublicationDocument::Snapshot(snapshot)],
                }
            } else {
                WorkbenchPublicationOutcome::Current {
                    revision: snapshot.revision,
                }
            }
        }
        WorkbenchSnapshotOutcome::Denied {} => WorkbenchPublicationOutcome::Denied {},
        WorkbenchSnapshotOutcome::Unavailable { reason } => {
            WorkbenchPublicationOutcome::Unavailable { reason }
        }
    }
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
            let inner = |stream: &mut orchbee_local_ipc::LocalStream| -> io::Result<()> {
                admit(&mut *stream)?;
                for step in steps {
                    let request = loop {
                        let mut magic = [0; 4];
                        (*stream).read_exact(&mut magic)?;
                        let mut reader = io::Cursor::new(magic).chain(&mut *stream);
                        match step {
                            WireKind::Obcs => {
                                if magic == *b"OBWP" {
                                    match read_workbench_request(&mut reader)
                                        .map_err(io::Error::other)?
                                    {
                                        // A publication probe may interleave with scripted OBCS
                                        // traffic; answer it like an idle journal without
                                        // consuming the scripted step.
                                        WorkbenchRequest::PublicationRead(intent) => {
                                            write_workbench_response(
                                                &mut *stream,
                                                &WorkbenchResponse::Publication(
                                                    WorkbenchPublicationResult {
                                                        protocol_version: ProtocolVersion,
                                                        request_id: intent.request_id,
                                                        outcome:
                                                            WorkbenchPublicationOutcome::Current {
                                                                revision: intent.known_revision,
                                                            },
                                                    },
                                                ),
                                            )
                                            .map_err(io::Error::other)?;
                                            continue;
                                        }
                                        _ => {
                                            return Err(io::Error::other(
                                                "unscripted Workbench request during an OBCS step",
                                            ));
                                        }
                                    }
                                }
                                break ObservedRequest::Obcs(
                                    read_client_message(&mut reader).map_err(io::Error::other)?,
                                );
                            }
                            WireKind::Workbench => {
                                break ObservedRequest::Workbench(
                                    read_workbench_request(&mut reader)
                                        .map_err(io::Error::other)?,
                                );
                            }
                        }
                    };
                    request_sender.send(request).map_err(io::Error::other)?;
                    match response_receiver.recv().map_err(io::Error::other)? {
                        ScriptResponse::Obcs(response) if matches!(step, WireKind::Obcs) => {
                            write_server_message(&mut *stream, &response)
                                .map_err(io::Error::other)?;
                            response_written_sender.send(()).map_err(io::Error::other)?;
                        }
                        ScriptResponse::Workbench(response)
                            if matches!(step, WireKind::Workbench) =>
                        {
                            write_workbench_response(&mut *stream, &response)
                                .map_err(io::Error::other)?;
                            response_written_sender.send(()).map_err(io::Error::other)?;
                        }
                        ScriptResponse::MalformedWorkbench
                            if matches!(step, WireKind::Workbench) =>
                        {
                            (*stream)
                                .write_all(&[b'X', b'X', b'X', b'X', 0, 1, 0, 4, 0, 0, 0, 0])?;
                            response_written_sender.send(()).map_err(io::Error::other)?;
                        }
                        ScriptResponse::Abort => {
                            let _closed = (*stream).shutdown(Shutdown::Both);
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
                // After the script ends the session may still emit publication probes from idle
                // polls; answer them like an idle journal and fail on anything else.
                loop {
                    let mut magic = [0; 4];
                    match (*stream).read_exact(&mut magic) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                        Err(error) => return Err(error),
                    }
                    if magic == *b"OBWP" {
                        let mut reader = io::Cursor::new(magic).chain(&mut *stream);
                        if let WorkbenchRequest::PublicationRead(intent) =
                            read_workbench_request(&mut reader).map_err(io::Error::other)?
                        {
                            write_workbench_response(
                                &mut *stream,
                                &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                                    protocol_version: ProtocolVersion,
                                    request_id: intent.request_id,
                                    outcome: WorkbenchPublicationOutcome::Current {
                                        revision: intent.known_revision,
                                    },
                                }),
                            )
                            .map_err(io::Error::other)?;
                            continue;
                        }
                    }
                    return Err(io::Error::other(
                        "executor sent an unscripted trailing request",
                    ));
                }
            };
            inner(&mut stream)
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

    /// Takes the next scripted request when it is a Workbench read, whichever read kind the
    /// executor picked for its current baseline state.
    pub(super) fn take_read(&self) -> TestResult<ObservedRead> {
        match self.take_workbench()? {
            WorkbenchRequest::SnapshotRead(intent) => Ok(ObservedRead::Snapshot(intent)),
            WorkbenchRequest::PublicationRead(intent) => Ok(ObservedRead::Publication(intent)),
            _ => Err(test_error("expected a Workbench read request")),
        }
    }

    /// Answers an observed read with the latest snapshot outcome; `publication.read` requests are
    /// served `current`/`published` through the journal-style mapping.
    pub(super) fn respond_read(
        &self,
        read: &ObservedRead,
        outcome: WorkbenchSnapshotOutcome,
    ) -> TestResult {
        match read {
            ObservedRead::Snapshot(intent) => {
                self.respond_workbench(WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: intent.request_id.clone(),
                    outcome,
                }))
            }
            ObservedRead::Publication(intent) => {
                self.respond_workbench(WorkbenchResponse::Publication(WorkbenchPublicationResult {
                    protocol_version: ProtocolVersion,
                    request_id: intent.request_id.clone(),
                    outcome: publication_outcome_for(outcome, intent.known_revision),
                }))
            }
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
        Self::start_inner(steps, initial_geometry, None, None, IDENTITY_SEED, None)
    }

    pub(super) fn start_with_identity_seed(
        steps: Vec<WireKind>,
        initial_geometry: Geometry,
        identity_seed: u128,
    ) -> TestResult<Self> {
        Self::start_inner(steps, initial_geometry, None, None, identity_seed, None)
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
            None,
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
            None,
        )
    }

    #[cfg(windows)]
    pub(super) fn start_at(
        steps: Vec<WireKind>,
        initial_geometry: Geometry,
        target: crate::terminal_session::NativeStartupTarget,
    ) -> TestResult<Self> {
        Self::start_inner(
            steps,
            initial_geometry,
            None,
            None,
            IDENTITY_SEED,
            Some(target),
        )
    }

    fn start_inner(
        steps: Vec<WireKind>,
        initial_geometry: Geometry,
        ceiling_clock: Option<Arc<AtomicBool>>,
        prewrite_gate: Option<Arc<PrewriteGate>>,
        identity_seed: u128,
        target: Option<crate::terminal_session::NativeStartupTarget>,
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
                let exit = if target.is_some() {
                    super::run_at(client, initial_geometry, worker_mailbox, target)
                } else {
                    super::run(client, initial_geometry, worker_mailbox)
                };
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

    pub(super) fn close_with_pending_panel(self) -> TestResult<Option<PendingPanelCommit>> {
        self.mailbox.close(CarrierFailure::Closed);
        self.shutdown.close()?;
        self.carrier.abort();
        self.finish_after_worker_failure_with_exit(CarrierFailure::Closed)
            .map(|exit| exit.into_pending_results().panel)
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

#[derive(Debug)]
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
    Move(MoveDecision),
    Panel(PanelDecision),
    Membership(RefreshDecision),
    PaneDetached,
    PaneFailed,
}

pub(super) struct UiDriver {
    mailbox: Arc<CommandMailbox>,
    clock: LocalOperationClock,
    state: Option<NativeSessionState>,
    projection: Option<TerminalProjection>,
    /// Which live incarnation `projection` belongs to. `NativeUi` keeps one
    /// projection per pane (`self.projections`, keyed by `PaneRef`), so a
    /// background pane's setup poll draws into its own renderer; the driver
    /// keeps the most recently installed one and routes polls to it by exact
    /// `PaneRef`, not by focus or anchor.
    projection_pane: Option<PaneRef>,
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
            projection_pane: None,
            drawn_frames: 0,
            no_frame_redraws: 0,
        }
    }

    pub(super) fn state(&self) -> &NativeSessionState {
        self.state.as_ref().expect("Bootstrap must install state")
    }

    /// The live incarnation of the pane carrying `anchor` — tests name panes
    /// by their attachment key while commands require the exact `PaneRef`.
    pub(super) fn pane_ref(
        &self,
        anchor: &AttachmentKey,
    ) -> TestResult<crate::terminal_session::state::PaneRef> {
        self.state()
            .pane_ref_for_anchor(anchor)
            .ok_or_else(|| test_error("no live pane for attachment anchor"))
    }

    pub(super) fn state_mut(&mut self) -> &mut NativeSessionState {
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
        self.admit_switch_target(index, None)
    }

    pub(super) fn admit_presentation(
        &mut self,
        index: usize,
        panel: WorkbenchPanelId,
        presentation: TerminalPresentationId,
    ) -> TestResult<AdmittedSwitch> {
        self.admit_switch_target(index, Some((panel, presentation)))
    }

    fn admit_switch_target(
        &mut self,
        index: usize,
        presentation: Option<(WorkbenchPanelId, TerminalPresentationId)>,
    ) -> TestResult<AdmittedSwitch> {
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
                    let target = match &presentation {
                        Some((panel, member)) => state
                            .target_for_presentation(index, panel, member, generation, deadlines),
                        None => state.target_for_switch(index, key, deadlines),
                    }
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
        let key = self.state().desired().unwrap().anchor().clone();
        if key.generation() != generation {
            return Err(test_error("Switch generation and desired key diverged"));
        }
        if self
            .projection
            .as_ref()
            .is_some_and(|projection| projection.frame().is_some())
        {
            // Admission hides the renderer through state.projection_key(); keep exact anchor pixels.
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
                            placement: None,
                            automatic_label: false,
                            terminal_context_id: terminal_context_id.clone(),
                            label: label.clone(),
                            geometry: workbench_geometry,
                            launch: None,
                            profile: None,
                            options: Default::default(),
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

    /// Mirrors the native UI's stable-tick capture: the mailbox ingress latch becomes
    /// canonical debt, and one pending demand drains into a refresh flight. Busy/Full
    /// refusal is a local no-op so the debt survives for a later settle point.
    pub(super) fn drain_publication_hint(&mut self) -> TestResult {
        let mailbox = Arc::clone(&self.mailbox);
        let deadline = self
            .clock
            .now()
            .checked_add(OPERATION_TOTAL)
            .ok_or_else(|| test_error("Refresh deadline overflow"))?;
        let state = self.state_mut();
        mailbox
            .capture_publication_hint_for(state, |state| state.note_publication_hint())
            .map_err(|closed| test_error(&format!("mailbox closed: {:?}", closed.failure())))?;
        if !state.publication_debt_pending() {
            return Ok(());
        }
        match mailbox.try_enqueue_refresh_for(
            state,
            deadline,
            |state, generation| {
                state
                    .start_publication_drain(generation)
                    .then_some(())
                    .ok_or("publication drain busy")
            },
            |_state, _result| {},
        ) {
            Ok(_) => Ok(()),
            Err(
                crate::terminal_session::mailbox::AdmissionError::Busy
                | crate::terminal_session::mailbox::AdmissionError::Full
                | crate::terminal_session::mailbox::AdmissionError::Build(_),
            ) => Ok(()),
            Err(error) => Err(test_error(&format!("publication drain failed: {error:?}"))),
        }
    }

    pub(super) fn admit_context_selection(&mut self) -> TestResult<u64> {
        let clock = self.clock.clone();
        let deadlines = OperationDeadlines::from_sample(clock.now()).unwrap();
        let mailbox = Arc::clone(&self.mailbox);
        mailbox
            .try_enqueue_context_selection_for(
                self.state_mut(),
                deadlines,
                &clock,
                |state, generation| {
                    state
                        .start_context_selection(generation, true, deadlines)
                        .then_some(())
                        .ok_or("context admission")
                },
                |_state, _result| {},
            )
            .map_err(|error| test_error(&format!("context selection admission failed: {error:?}")))?
            .ok_or_else(|| test_error("context selection expired at admission"))
    }

    // This seam checks worker transport; canonical Move settlement is covered by the state owner.
    pub(super) fn admit_move_transport(
        &mut self,
        direction: WorkbenchTabMoveDirection,
    ) -> TestResult<MoveOperation> {
        self.admit_move_using(direction, false)
    }

    pub(super) fn admit_move(
        &mut self,
        direction: WorkbenchTabMoveDirection,
    ) -> TestResult<MoveOperation> {
        self.admit_move_using(direction, true)
    }

    fn admit_move_using(
        &mut self,
        direction: WorkbenchTabMoveDirection,
        canonical: bool,
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
                        state.desired().unwrap().anchor().clone(),
                        state.cache().clone(),
                        direction,
                        deadline,
                    ))
                },
                |state, operation| {
                    if !canonical || state.begin_move(operation.clone()) {
                        Ok(())
                    } else {
                        Err("canonical Move admission failed")
                    }
                },
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
                    self.state().desired().cloned(),
                )))
            },
        ))?;
        Ok((operation, outcome))
    }

    // This seam checks worker transport; canonical Panel settlement is covered by the
    // state owner.
    pub(super) fn admit_panel_transport(
        &mut self,
        kind: PanelRequestKind,
    ) -> TestResult<PanelOperation> {
        self.admit_panel_using(kind, false)
    }

    pub(super) fn admit_panel(&mut self, kind: PanelRequestKind) -> TestResult<PanelOperation> {
        self.admit_panel_using(kind, true)
    }

    fn admit_panel_using(
        &mut self,
        kind: PanelRequestKind,
        canonical: bool,
    ) -> TestResult<PanelOperation> {
        let clock = self.clock.clone();
        let deadline = clock
            .now()
            .checked_add(OPERATION_TOTAL)
            .ok_or_else(|| test_error("Panel deadline overflow"))?;
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        mailbox
            .try_enqueue_panel_for(
                state,
                deadline,
                &clock,
                |state, request_id| {
                    state
                        .plan_panel(request_id, kind, deadline)
                        .ok_or("panel plan refused")
                },
                |state, operation| {
                    if !canonical || state.begin_panel(operation.clone()) {
                        Ok(())
                    } else {
                        Err("canonical Panel admission failed")
                    }
                },
            )
            .map_err(|error| test_error(&format!("Panel transport admission failed: {error:?}")))
    }

    pub(super) fn settle_panel_transport(
        &mut self,
        taken: TakenEvent,
    ) -> TestResult<(PanelOperation, PanelResult)> {
        self.settle_panel_transport_decision(taken, PanelDecision::Continue)
    }

    pub(super) fn settle_panel_transport_decision(
        &mut self,
        taken: TakenEvent,
        decision: PanelDecision,
    ) -> TestResult<(PanelOperation, PanelResult)> {
        let (wait_key, event) = taken.into_parts();
        let WorkerEvent::PanelResult { operation, outcome } = event else {
            return Err(test_error("expected typed Panel result"));
        };
        let wait = wait_key.ok_or_else(|| test_error("Panel wait key absent"))?;
        require_applied(self.mailbox.deposit_ui_reply_before(
            &wait,
            &self.clock,
            |_transaction| {
                Ok(UiReply::Panel(PanelSettlement::new(
                    decision,
                    self.state().desired().cloned(),
                )))
            },
        ))?;
        Ok((operation, outcome))
    }

    /// Mirrors `NativeUi::observe_panes`: the leaf-geometry report syncs the
    /// pane set, queues visible controlling-pane resizes, and schedules the reconcile
    /// snapshot the executor converges its attachments against.
    pub(super) fn observe_panes(
        &mut self,
        observed: &[(WorkbenchPanelId, usize, Geometry)],
    ) -> TestResult {
        let clock = self.clock.clone();
        let sampled_at = clock.now();
        let resizes = {
            let state = self.state_mut();
            state.sync_pane_set(observed, sampled_at);
            if state.phase_kind() == crate::terminal_session::state::SessionPhaseKind::Starting {
                Vec::new()
            } else {
                state.admit_observed_geometries(observed)
            }
        };
        let mailbox = Arc::clone(&self.mailbox);
        for (key, geometry) in resizes {
            let state = self.state_mut();
            if state.close_suppresses_attachment(key.pane().anchor()) {
                continue;
            }
            match mailbox.try_enqueue(QueuedCommand::Resize(Tagged::new(key.clone(), geometry))) {
                Ok(_serial) => {
                    if !state.record_admitted_geometry(&key, geometry) {
                        return Err(test_error("resize admission bookkeeping rejected"));
                    }
                }
                Err(
                    crate::terminal_session::mailbox::AdmissionError::Full
                    | crate::terminal_session::mailbox::AdmissionError::Busy,
                ) => {}
                Err(error) => {
                    return Err(test_error(&format!("resize admission failed: {error:?}")));
                }
            }
        }
        if self.state().phase_kind() == crate::terminal_session::state::SessionPhaseKind::Starting {
            return Ok(());
        }
        let Some((anchor, targets, deadlines)) = self.state_mut().reconcile_targets(clock.now())
        else {
            return Ok(());
        };
        match mailbox.try_enqueue_reconcile(anchor, targets, deadlines) {
            Ok(_)
            | Err(
                crate::terminal_session::mailbox::AdmissionError::Full
                | crate::terminal_session::mailbox::AdmissionError::Busy,
            ) => Ok(()),
            Err(error) => Err(test_error(&format!(
                "reconcile admission failed: {error:?}"
            ))),
        }
    }

    /// Mirrors `NativeUi::toggle_zoom`: the mailbox admission serializes the
    /// flag flip against in-flight installs exactly like the production
    /// action, and the same `bool` reports whether the drawn set changed.
    pub(super) fn toggle_zoom(&mut self) -> bool {
        let mailbox = Arc::clone(&self.mailbox);
        mailbox.apply_local_for(self.state_mut(), NativeSessionState::toggle_zoom)
    }

    /// Mirrors `NativeUi::move_panel_focus_to`: a purely local focus move plans
    /// through the live record, installs the same desired/mode swap, and
    /// republishes the mailbox's desired key so focus-scoped admission (and
    /// the executor's focus re-check) follows the move.
    pub(super) fn move_panel_focus_to_pane(
        &mut self,
        panel: &WorkbenchPanelId,
    ) -> TestResult<PaneKey> {
        let Some(pane) = self.state().plan_panel_focus_local_at(panel) else {
            return Err(test_error("local focus plan rejected the target panel"));
        };
        if !self.state_mut().move_panel_focus_local(pane.clone()) {
            return Err(test_error("local focus move was refused"));
        }
        self.mailbox
            .publish_desired(pane.clone())
            .map_err(|closed| test_error(&format!("mailbox closed: {:?}", closed.failure())))?;
        Ok(pane)
    }

    pub(super) fn admit_input(&mut self, input: ControlInput) -> TestResult<u64> {
        let key = self
            .state()
            .focus_pane_ref()
            .ok_or_else(|| test_error("Input admission needs a live focus pane"))?;
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
                let key = state
                    .focus_pane_ref()
                    .ok_or("Resize admission needs a live focus pane")?;
                if !state.record_admitted_geometry_t(&key, geometry) {
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

    /// Deposits the `SetupConfigured` reply for a taken event and returns the raw admission
    /// result instead of mapping errors through `event_error`: a boundary test parks this
    /// call at the mailbox deposit-sample gate so the worker's forward expiry can run
    /// first, after which the resumed same-tick deposit must observe `MutationResult::Stale`
    /// rather than `EventError::ForwardExpired`.
    pub(super) fn deposit_setup_configuration(
        &mut self,
        taken: TakenEvent,
    ) -> TestResult<std::result::Result<MutationResult, EventError>> {
        let (wait_key, event) = taken.into_parts();
        let WorkerEvent::SetupConfigured {
            key,
            mode,
            applied_geometry,
        } = event
        else {
            return Err(test_error("expected a taken SetupConfigured event"));
        };
        let wait_key = wait_key.ok_or_else(|| test_error("SetupConfigured wait key"))?;
        let mailbox = Arc::clone(&self.mailbox);
        let clock = self.clock.clone();
        let state = self.state_mut();
        Ok(
            mailbox.deposit_ui_reply_before(&wait_key, &clock, |_transaction| {
                Ok(UiReply::SetupConfigurationSettled(
                    state
                        .settle_setup_configuration_event_t(&key, mode.clone(), applied_geometry)
                        .ok_or(CarrierFailure::Protocol)?,
                ))
            }),
        )
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
        // Notice events are canonical pane-fact publications: apply them to
        // state like the real UI, then continue to the next causal event.
        loop {
            let taken = self.take_next()?;
            let settled = self.settle(taken, frame_cut, None)?;
            if matches!(settled, UiEvent::PaneDetached | UiEvent::PaneFailed) {
                continue;
            }
            return Ok(settled);
        }
    }

    fn settle(
        &mut self,
        taken: TakenEvent,
        frame_cut: FrameCut,
        cleanup_deposit_advance: Option<(&AtomicU64, u64)>,
    ) -> TestResult<UiEvent> {
        let (wait_key, event) = taken.into_parts();
        match event {
            WorkerEvent::PaneDetached { key } => {
                if let Some(state) = self.state.as_mut() {
                    assert!(state.settle_pane_detached(&key));
                }
                Ok(UiEvent::PaneDetached)
            }
            WorkerEvent::PaneFailed { key, fault } => {
                if let Some(state) = self.state.as_mut() {
                    assert!(state.settle_pane_fault(&key, fault));
                }
                Ok(UiEvent::PaneFailed)
            }
            #[cfg(windows)]
            WorkerEvent::WorktreePrepared { .. } => {
                Err(test_error("requires the actual NativeUi consumer"))
            }
            WorkerEvent::QuickWorkspacePrepared { .. }
            | WorkerEvent::WorkspaceCloseResult { .. }
            | WorkerEvent::ControlTakeoverResult { .. }
            | WorkerEvent::WorkspaceSetupResult { .. }
            | WorkerEvent::RenameResult { .. }
            | WorkerEvent::ViewportOutcome { .. }
            | WorkerEvent::SelectionOutcome { .. } => {
                Err(test_error("requires the actual NativeUi consumer"))
            }
            WorkerEvent::SwitchSnapshotResult { marker, outcome } => self.settle_switch_snapshot(
                wait_key.ok_or_else(|| test_error("Switch Snapshot wait"))?,
                marker.anchor().clone(),
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
                                        crate::terminal_session::mailbox::DesiredInstall::ReuseReserved {
                                            key: committed.committed_key().clone(),
                                            panel: committed
                                                .expectation()
                                                .panel_id()
                                                .clone(),
                                        }
                                    } else {
                                        match prepared.focus_panel_id() {
                                            Some(panel) => crate::terminal_session::mailbox::DesiredInstall::Fresh {
                                                tab_id: tab.clone(),
                                                panel: panel.clone(),
                                            },
                                            None => crate::terminal_session::mailbox::DesiredInstall::Empty,
                                        }
                                    }
                                },
                            );
                            let desired = transaction.prepare_desired_install(install)?;
                            let state_plan = state.prepare_cleaned_membership_install(
                                &cleanup,
                                prepared,
                                desired.desired().map(|pane| pane.anchor().clone()),
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
                self.projection_pane = None;
                let decision = decision.ok_or_else(|| test_error("membership decision"))?;
                match result_key {
                    crate::terminal_session::state::MembershipResultKey::Rename(_) => Err(
                        test_error("Rename membership requires the actual NativeUi consumer"),
                    ),
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
                    crate::terminal_session::state::MembershipResultKey::Move(_)
                    | crate::terminal_session::state::MembershipResultKey::Panel(_) => {
                        Ok(UiEvent::Membership(decision))
                    }
                }
            }
            WorkerEvent::CloseReconciliationSnapshot { .. }
            | WorkerEvent::MembershipGone { .. }
            | WorkerEvent::CloseResult { .. } => {
                Err(test_error("use the exact Close transport consumer"))
            }
            WorkerEvent::MoveResult { operation, outcome } => {
                let wait = wait_key.ok_or_else(|| test_error("Move wait key"))?;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                let prepared = state.prepare_move(&operation, outcome);
                let membership = prepared.membership_candidate().is_some();
                let deadlines = membership
                    .then(|| OperationDeadlines::from_sample(clock.now()))
                    .flatten();
                let mut decision = None;
                require_applied(mailbox.deposit_ui_reply_before(&wait, &clock, |transaction| {
                    let settled = if membership {
                        let (operation, prepared) = state.retain_prepared_move_membership(prepared)?;
                        let deadlines = deadlines.ok_or(CarrierFailure::Deadline)?;
                        if transaction.observed_at() >= deadlines.forward_cutoff() { return Err(CarrierFailure::Deadline); }
                        if state.has_attachments() {
                            MoveDecision::Cleanup(state.stage_move_membership_cleanup(operation.request_id(), prepared, deadlines)?)
                        } else {
                            let install = prepared.selected_tab_id().map_or(crate::terminal_session::mailbox::DesiredInstall::Empty,
                                |tab| match prepared.focus_panel_id() {
                                    Some(panel) => crate::terminal_session::mailbox::DesiredInstall::Fresh {
                                        tab_id: tab.clone(),
                                        panel: panel.clone(),
                                    },
                                    None => crate::terminal_session::mailbox::DesiredInstall::Empty,
                                });
                            let desired = transaction.prepare_desired_install(install)?;
                            let plan = state.prepare_move_membership_install(operation.request_id(), prepared, desired.desired().map(|pane| pane.anchor().clone()), Some(deadlines))?;
                            let installed = plan.commit();
                            desired.commit();
                            match installed {
                                crate::terminal_session::state::MembershipInstallDecision::Attach(target) => MoveDecision::AttachRecovered(target),
                                crate::terminal_session::state::MembershipInstallDecision::Preserved => MoveDecision::Continue,
                                _ => MoveDecision::SettledWithoutAttachment,
                            }
                        }
                    } else { state.apply_prepared_move(prepared) };
                    decision = Some(settled.clone());
                    Ok(UiReply::Move(MoveSettlement::new(settled, state.desired().cloned())))
                }))?;
                Ok(UiEvent::Move(
                    decision.ok_or_else(|| test_error("Move decision"))?,
                ))
            }
            WorkerEvent::PanelResult { operation, outcome } => {
                let wait = wait_key.ok_or_else(|| test_error("Panel wait key"))?;
                let mailbox = Arc::clone(&self.mailbox);
                let clock = self.clock.clone();
                let state = self.state_mut();
                let prepared = state.prepare_panel(&operation, outcome);
                let membership = prepared.membership_candidate().is_some();
                let deadlines = membership
                    .then(|| OperationDeadlines::from_sample(clock.now()))
                    .flatten();
                let mut decision = None;
                require_applied(mailbox.deposit_ui_reply_before(&wait, &clock, |transaction| {
                    let settled = if membership {
                        let (operation, prepared) = state.retain_prepared_panel_membership(prepared)?;
                        let deadlines = deadlines.ok_or(CarrierFailure::Deadline)?;
                        if transaction.observed_at() >= deadlines.forward_cutoff() { return Err(CarrierFailure::Deadline); }
                        if state.has_attachments() {
                            PanelDecision::Cleanup(state.stage_panel_membership_cleanup(operation.request_id(), prepared, deadlines)?)
                        } else {
                            let install = prepared.selected_tab_id().map_or(crate::terminal_session::mailbox::DesiredInstall::Empty,
                                |tab| match prepared.focus_panel_id() {
                                    Some(panel) => crate::terminal_session::mailbox::DesiredInstall::Fresh {
                                        tab_id: tab.clone(),
                                        panel: panel.clone(),
                                    },
                                    None => crate::terminal_session::mailbox::DesiredInstall::Empty,
                                });
                            let desired = transaction.prepare_desired_install(install)?;
                            let plan = state.prepare_panel_membership_install(operation.request_id(), prepared, desired.desired().map(|pane| pane.anchor().clone()), Some(deadlines))?;
                            let installed = plan.commit();
                            desired.commit();
                            match installed {
                                crate::terminal_session::state::MembershipInstallDecision::Attach(target) => PanelDecision::AttachRecovered(target),
                                crate::terminal_session::state::MembershipInstallDecision::Preserved => PanelDecision::Continue,
                                _ => PanelDecision::SettledWithoutAttachment,
                            }
                        }
                    } else { state.apply_prepared_panel(prepared) };
                    decision = Some(settled.clone());
                    Ok(UiReply::Panel(PanelSettlement::new(settled, state.desired().cloned())))
                }))?;
                Ok(UiEvent::Panel(
                    decision.ok_or_else(|| test_error("Panel decision"))?,
                ))
            }
            WorkerEvent::Bootstrap { cache, selection } => {
                let wait_key = wait_key.ok_or_else(|| test_error("Bootstrap wait key"))?;
                let mut installed = None;
                let result = self.mailbox.deposit_bootstrap_reply_before(
                    &wait_key,
                    &self.clock,
                    |generation, geometry| {
                        let (state, target) = NativeSessionState::bootstrap_at(
                            cache,
                            generation,
                            geometry,
                            selection.as_ref(),
                        )
                        .ok_or(CarrierFailure::AuthorityDivergence)?;
                        installed = Some(state);
                        Ok(target)
                    },
                );
                require_applied(result)?;
                self.state = installed;
                Ok(UiEvent::Bootstrap {
                    key: self.state().desired().unwrap().anchor().clone(),
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
                            .settle_attached_event_t(&key)
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
                        self.projection_pane = Some(key.clone());
                        Ok(UiEvent::Attached {
                            key: key.anchor().clone(),
                        })
                    }
                    SetupEventDisposition::Superseded { desired } => {
                        Ok(UiEvent::AttachedSuperseded {
                            key: key.anchor().clone(),
                            desired: desired.anchor().clone(),
                        })
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
                            .settle_setup_configuration_event_t(
                                &key,
                                mode.clone(),
                                applied_geometry,
                            )
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
                            key: key.anchor().clone(),
                            mode,
                            applied_geometry,
                        })
                    }
                    SetupEventDisposition::Superseded { desired } => {
                        Ok(UiEvent::SetupConfigurationSuperseded {
                            key: key.anchor().clone(),
                            desired: desired.anchor().clone(),
                        })
                    }
                }
            }
            WorkerEvent::PollResult { poll_key, record } => {
                if wait_key.is_some() {
                    return Err(test_error("PollResult must use the causal slot"));
                }
                self.drain_publication_hint()?;
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
                            .settle_feedback_event_t(&key, frame)
                            .ok_or(CarrierFailure::Protocol)?;
                        disposition = Some(settled.clone());
                        Ok(UiReply::FeedbackSettled(settled))
                    },
                ))?;
                match disposition
                    .ok_or_else(|| test_error("feedback disposition was not deposited"))?
                {
                    SetupEventDisposition::Applied => Ok(UiEvent::FeedbackAccepted {
                        key: key.anchor().clone(),
                    }),
                    SetupEventDisposition::Superseded { desired } => {
                        Ok(UiEvent::FeedbackSuperseded {
                            key: key.anchor().clone(),
                            desired: desired.anchor().clone(),
                        })
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
                        let settled = state.settle_ready_candidate_t(
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
                            .settle_switch_entry_expired_t(
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
                    key: key.anchor().clone(),
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
                            .settle_unattached_setup_t(&key, target_index, &target_provenance)
                            .ok_or(CarrierFailure::Protocol)?;
                        continuation = Some(settled.clone());
                        Ok(UiReply::SetupWithoutAttachmentApplied(settled))
                    },
                ))?;
                self.projection = None;
                self.projection_pane = None;
                Ok(UiEvent::SetupWithoutAttachment {
                    key: key.anchor().clone(),
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
                            .settle_cleanup_t(&key, target_index, &target_provenance)
                            .ok_or(CarrierFailure::Protocol)?;
                        continuation = Some(settled.clone());
                        if let Some((elapsed_millis, next_millis)) = cleanup_deposit_advance {
                            elapsed_millis.store(next_millis, Ordering::SeqCst);
                        }
                        Ok(UiReply::CleanupApplied(settled))
                    },
                ))?;
                self.projection = None;
                self.projection_pane = None;
                Ok(UiEvent::CleanupSettled {
                    key: key.anchor().clone(),
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
                            let key = state.focus_pane_ref().ok_or(CarrierFailure::Protocol)?;
                            transaction.enqueue_catch_up_resize(key.clone(), latest)?;
                            if !state.record_admitted_geometry_t(&key, latest) {
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
                        let settled = if state.has_attachments() {
                            CreateProjectionDecision::Cleanup(state.stage_create_membership_cleanup(&operation, prepared, deadlines)?)
                        } else {
                            if transaction.observed_at() >= deadlines.forward_cutoff() { return Err(CarrierFailure::Deadline); }
                            let install = match prepared.selected_tab_id() {
                                Some(tab) if tab == committed.committed_key().tab_id() => crate::terminal_session::mailbox::DesiredInstall::ReuseReserved {
                                    key: committed.committed_key().clone(),
                                    panel: committed.expectation().panel_id().clone(),
                                },
                                Some(tab) => match prepared.focus_panel_id() {
                                    Some(panel) => crate::terminal_session::mailbox::DesiredInstall::Fresh {
                                        tab_id: tab.clone(),
                                        panel: panel.clone(),
                                    },
                                    None => crate::terminal_session::mailbox::DesiredInstall::Empty,
                                },
                                None => crate::terminal_session::mailbox::DesiredInstall::Empty,
                            };
                            let desired = transaction.prepare_desired_install(install)?;
                            let plan = state.prepare_create_membership_install(&operation, prepared, desired.desired().map(|pane| pane.anchor().clone()), deadlines)?;
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
                let recovery_deadlines = self.state().context_selection_deadlines().or_else(|| {
                    prepared
                        .needs_recovery_deadlines()
                        .then(|| OperationDeadlines::from_sample(self.clock.now()))
                        .flatten()
                });
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
                                if state.has_attachments() && !prepared.preserves_selection() {
                                    RefreshDecision::Cleanup(state.stage_refresh_membership_cleanup(generation, prepared, recovery_deadlines.ok_or(CarrierFailure::Deadline)?)?)
                                } else {
                                    use crate::terminal_session::{mailbox::DesiredInstall, state::MembershipInstallDecision};
                                    let install = if prepared.preserves_selection() { DesiredInstall::Preserve }
                                        else if let Some(tab) = prepared.selected_tab_id() {
                                            let panel = prepared.focus_panel_id().ok_or(CarrierFailure::Protocol)?;
                                            DesiredInstall::Fresh { tab_id: tab.clone(), panel: panel.clone() }
                                        } else { DesiredInstall::Empty };
                                    let desired = _transaction.prepare_desired_install(install)?;
                                    let plan = state.prepare_refresh_membership_install(generation, prepared, desired.desired().map(|key| key.anchor().clone()), recovery_deadlines)?;
                                    let installed = plan.commit();
                                    desired.commit();
                                    match installed {
                                        MembershipInstallDecision::Attach(target) => RefreshDecision::AttachRecovered(target),
                                        MembershipInstallDecision::Preserved => RefreshDecision::Continue,
                                        MembershipInstallDecision::Empty | MembershipInstallDecision::SelectedWithoutAttachment => RefreshDecision::SettledWithoutAttachment,
                                    }
                                }
                            } else {
                                state.apply_prepared_refresh(prepared, _transaction.observed_at())
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
        if self.projection_pane.as_ref() != Some(poll_key.pane_ref()) {
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
        let expected_geometry = self.state().expected_frame_geometry_t(poll_key.pane_ref());
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
                if !self
                    .state_mut()
                    .stage_drawn_frame_t(poll_key.pane_ref(), frame)
                {
                    // Mirrors `NativeUi::after_draw`: a frame that never staged
                    // into a live, non-departing leaf settles as `NoFeedback`,
                    // never a draw; a superseded causal slot may already be
                    // stale.
                    match self
                        .mailbox
                        .deposit_causal(&poll_key, CausalSettlement::NoFeedback)
                        .map_err(|closed| {
                            test_error(&format!("mailbox closed: {:?}", closed.failure()))
                        })? {
                        MutationResult::Applied | MutationResult::Stale => {}
                    }
                    return Ok(UiEvent::EmptyPoll { key });
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
                if !self
                    .state_mut()
                    .invalidate_projection_frame_t(poll_key.pane_ref())
                {
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
                if !self
                    .state_mut()
                    .invalidate_projection_frame_t(poll_key.pane_ref())
                {
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
            // Mirrors `NativeUi::apply_poll_result`: a history event settles
            // its own causal acknowledgement without staging a frame or
            // touching the projection status.
            Some(feedback @ TerminalViewerAction::HistoryObserved { .. }) => {
                require_mutation(
                    self.mailbox
                        .deposit_causal(&poll_key, CausalSettlement::Feedback(feedback.clone())),
                )?;
                Ok(UiEvent::NoFrameRedraw {
                    key,
                    feedback: Some(feedback),
                })
            }
            Some(TerminalViewerAction::Attach { .. })
            | Some(TerminalViewerAction::AttachBinding { .. })
            | Some(TerminalViewerAction::Poll { .. })
            | Some(TerminalViewerAction::ReadRows { .. })
            | Some(TerminalViewerAction::ReadProcessInfo { .. })
            | Some(TerminalViewerAction::SetProjectionDemand { .. })
            | Some(TerminalViewerAction::SetHistoryDemand { .. })
            | Some(TerminalViewerAction::ReadViewportRows { .. })
            | Some(TerminalViewerAction::RowsApplied { .. })
            | Some(TerminalViewerAction::SelectionGesture { .. })
            | Some(TerminalViewerAction::SelectionExport { .. })
            | Some(TerminalViewerAction::SelectionExportChunkApplied { .. })
            | Some(TerminalViewerAction::SelectionExportCancel { .. })
            | Some(TerminalViewerAction::Detach { .. }) => {
                Err(test_error("projection returned a non-feedback action"))
            }
        }
    }
}

pub(super) fn admit(stream: &mut UnixStream) -> io::Result<()> {
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
    full_frame_record_for(VIEWER_ID, sequence, revision, geometry)
}

/// `full_frame_record` for the attachment `viewer_id` identifies: the OBTF
/// routing prefix carries the attachment at bytes 32..48, and the receiving
/// projection rejects any other identity.
pub(super) fn full_frame_record_for(
    viewer_id: &str,
    sequence: u64,
    revision: u64,
    geometry: Geometry,
) -> TestResult<Vec<u8>> {
    let attachment = AttachmentId::parse(viewer_id)?;
    let mut frame = FRAME.to_vec();
    frame[32..48].copy_from_slice(attachment.as_bytes());
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
            directories: Vec::new(),
            workspaces: Vec::new(),
            workspace_order: Vec::new(),
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
        automatic_label: false,
        group_id: None,
        label: label.clone(),
        pinned: false,
        root: panel_node(base + 1, base + 2, base + 3, base + 4, &label),
        tab_id: WorkbenchTabId::parse(&id(base)).unwrap(),
    }
}

pub(super) fn panel_node(
    panel: u128,
    presentation: u128,
    worker: u128,
    terminal: u128,
    label: &str,
) -> PanelNode {
    PanelNode::Panel {
        right_click: Default::default(),
        child_presentations: Vec::new(),
        panel_id: WorkbenchPanelId::parse(&id(panel)).unwrap(),
        primary_presentation: Presentation {
            custom_label: None,
            binding: Some(PresentationBinding {
                terminal_id: TerminalId::parse(&id(terminal)).unwrap(),
                worker_id: WorkerId::parse(&id(worker)).unwrap(),
            }),
            label: label.to_owned(),
            presentation_id: TerminalPresentationId::parse(&id(presentation)).unwrap(),
        },
    }
}

pub(super) fn split_node(
    first: PanelNode,
    first_slot: u128,
    second: PanelNode,
    second_slot: u128,
    orientation: PanelOrientation,
    split: u128,
) -> PanelNode {
    PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: first,
                slot_id: WorkbenchPanelSlotId::parse(&id(first_slot)).unwrap(),
            },
            PanelSlot {
                node: second,
                slot_id: WorkbenchPanelSlotId::parse(&id(second_slot)).unwrap(),
            },
        ]),
        orientation,
        split_id: WorkbenchSplitId::parse(&id(split)).unwrap(),
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
    attached_outcome_for(VIEWER_ID)
}

pub(super) fn attached_outcome_for(viewer_id: &str) -> TestResult<TerminalViewerOutcome> {
    Ok(TerminalViewerOutcome::Attached {
        attachment_id: ViewerId::parse(viewer_id)?,
        host_epoch: ObcsHostEpoch::parse(HOST_EPOCH)?,
        host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(HOST_EPOCH)?,
    })
}

pub(super) fn attached_viewer_id() -> TestResult<ViewerId> {
    attached_viewer_id_for(VIEWER_ID)
}

pub(super) fn attached_viewer_id_for(viewer_id: &str) -> TestResult<ViewerId> {
    let TerminalViewerOutcome::Attached { attachment_id, .. } = attached_outcome_for(viewer_id)?
    else {
        return Err(test_error("attached fixture did not contain a ViewerId"));
    };
    Ok(attachment_id)
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

/// The fixture panel identity used when a `DesiredInstall` needs one.
fn panel() -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&id(0xF1)).unwrap()
}

/// The fabricated pane key the mailbox fixtures admit for `anchor`.
fn pane_key(anchor: &AttachmentKey) -> crate::terminal_session::state::PaneKey {
    crate::terminal_session::state::PaneKey::new(anchor.clone(), panel())
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
    drive_open_snapshot(harness, snapshot(tab_count, 1))
}

pub(super) fn drive_open_snapshot(
    harness: &mut Harness,
    source: Snapshot,
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
    let request = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Available { snapshot: source },
    )?;
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
