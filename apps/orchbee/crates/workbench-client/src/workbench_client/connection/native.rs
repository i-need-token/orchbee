//! Owns the background Client executor's serialized command, attachment, projection, and
//! cleanup exchanges with correlated UI handoffs and bounded deadlines. It reports exact
//! evidence without mutating canonical UI state directly or replaying uncertain effects.

use super::{ClientConnectionExit, attach_binding};
use crate::local_client::workbench::{
    PublicationReadError, PublicationReadFailure, SnapshotReadError, SnapshotReadFailure,
    WorkbenchTabCreateNotSubmitted, WorkbenchTabCreateOutcomeUnknownReason,
    WorkbenchTabCreateRequest, WorkbenchTabCreateSubmission,
};
use crate::local_client::{LocalClient, LocalOperationClock, SubmissionError};
use crate::startup_directory::ClientStartupTarget;
use crate::terminal_projection::TerminalProjection;
use crate::terminal_viewport::{
    ViewportReadRejection, ViewportStep, ViewportWindowFailure, decode_viewport_window,
};
use crate::workbench_client::mailbox::{
    AdmissionError, BarrierKind, CausalSettlement, CommandLease, CommandMailbox,
    CreatePrecommitOutcome, EventDeadline, EventError, MailboxClosed, MutationResult, PollKey,
    QueuedCommand, ReconcileMarker, SetupPollKey, Tagged, UiReply, ViewportOutcome, WorkerEvent,
};
use crate::workbench_projection::{project_native_context_tabs, project_preferred_workspace};
use crate::workbench_state::{
    AttachTarget, BootstrapTarget, CarrierFailure, CleanupContinuation, ControllerMode,
    CreateOperation, CreateProjectionDecision, CreateTerminalProvenance, FrameIdentity,
    OperationDeadlines, PaneFault, PaneKey, PaneRef, PendingCloseResult, PendingCreateCommit,
    PendingMoveCommit, ReadyDecision, ReconcileStep, ReconcileTarget, RefreshDecision,
    SetupEventDisposition,
};
use orchbee_client_server_protocol::{
    ControlRejectedReason, ControlUnavailableReason, Geometry, HistoryDemand, ProjectionDemand,
    TerminalControlAction, TerminalControlOutcome, TerminalControlRequest, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerRequest, ViewerRejectedReason, ViewerUnavailableReason,
};
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use orchbee_workbench_protocol::{
    DeltaRejectionReason, ProtocolVersion, PublicationReadIntent, Snapshot, SnapshotReadIntent,
    WorkbenchPublicationOutcome, WorkbenchRequestId, WorkbenchSnapshotOutcome,
    apply_workbench_publication,
};
use std::{
    collections::VecDeque,
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant},
};

mod close_reconciliation;
mod membership;
mod panel;
mod status_recovery;
mod switch_membership;
mod tab_close;
pub(in crate::workbench_client) use status_recovery::{RecoveryShutdown, run_status_recovery};
mod control_takeover;
mod current_directory;
mod quick_workspace;
mod startup_directory;
mod tab_move;
mod tab_rename;
mod workspace_close;
mod workspace_setup;
mod worktree_launch;

type Result<Value> = std::result::Result<Value, CarrierFailure>;
type ReadyResult<Value> = std::result::Result<Value, ReadyFlowError>;

enum ReadyFlowError {
    Carrier(CarrierFailure),
    ForwardDrainExpired,
}

impl From<CarrierFailure> for ReadyFlowError {
    fn from(failure: CarrierFailure) -> Self {
        Self::Carrier(failure)
    }
}

#[cfg(test)]
fn run(
    client: LocalClient,
    initial_geometry: Geometry,
    mailbox: Arc<CommandMailbox>,
) -> ClientConnectionExit {
    run_at(client, initial_geometry, mailbox, None)
}

pub(super) fn run_at(
    client: LocalClient,
    initial_geometry: Geometry,
    mailbox: Arc<CommandMailbox>,
    context: Option<ClientStartupTarget>,
) -> ClientConnectionExit {
    let mut exit = WorkerExit::new(Arc::clone(&mailbox));
    let mut executor = ClientExecutor::new(client, mailbox);
    executor.initial_context = context;
    let result = executor.run(initial_geometry);
    let failure = result.err();
    if let Some(failure) = failure {
        exit.failure = failure;
    }
    let returned = ClientConnectionExit::new(
        executor.pending_create.take(),
        executor.pending_move.take(),
        executor.pending_panel.take(),
        executor.pending_close.take(),
        executor.pending_rename.take(),
    )
    .with_setup(executor.pending_setup.take())
    .with_workspace_close(executor.pending_workspace_close.take())
    .with_takeover(executor.pending_takeover.take());
    returned.with_unavailable_poll(executor.unavailable_poll.take())
}

struct WorkerExit {
    mailbox: Arc<CommandMailbox>,
    failure: CarrierFailure,
}

impl WorkerExit {
    fn new(mailbox: Arc<CommandMailbox>) -> Self {
        Self {
            mailbox,
            failure: CarrierFailure::WorkerExited,
        }
    }
}

impl Drop for WorkerExit {
    fn drop(&mut self) {
        self.mailbox.close(self.failure);
    }
}

struct ClientExecutor {
    initial_context: Option<ClientStartupTarget>,
    status_only: bool,
    membership_recovery: bool,
    unavailable_poll: Option<crate::workbench_state::retired_membership::UnavailablePoll>,
    client: LocalClient,
    clock: LocalOperationClock,
    mailbox: Arc<CommandMailbox>,
    /// Live viewer attachments keyed by exact pane incarnation. The shared
    /// server-side HostViewers budget (8 slots) bounds this collection; the
    /// reconcile command is the only writer that installs new entries.
    attachments: Vec<AttachedTerminal>,
    workbench_baseline: Option<Snapshot>,
    publications: Option<crate::local_client::multiplex::Subscription>,
    _viewer_ready: Option<crate::local_client::multiplex::ViewerReadyWatch>,
    pending_create: Option<PendingCreateCommit>,
    pending_move: Option<PendingMoveCommit>,
    pending_panel: Option<crate::workbench_state::PendingPanelCommit>,
    pending_close: Option<PendingCloseResult>,
    pending_rename: Option<crate::workbench_state::tab_rename::RenameWorkerEvidence>,
    pending_setup: Option<crate::workbench_state::workspace_setup::SetupWorkerEvidence>,
    pending_workspace_close:
        Option<crate::workbench_state::workspace_close::WorkspaceCloseEvidence>,
    pending_takeover: Option<crate::workbench_state::control_takeover::TakeoverEvidence>,
}

/// The wall-clock backoff before a reconcile that yielded on `ServerBusy`
/// re-enters the queue — the same slice the in-lease retry wait used, now
/// spent off the lease so queued live work runs first.
const RECONCILE_BUSY_RETRY: Duration = Duration::from_millis(10);

fn settled_record(record: &[u8]) -> bool {
    orchbee_terminal_protocol::decode_transport_envelope(record)
        .is_ok_and(|envelope| matches!(envelope.kind, 15 | 19))
}

#[derive(Clone)]
struct SetupTarget {
    key: PaneRef,
    index: usize,
    provenance: CreateTerminalProvenance,
    attach: TerminalViewerAction,
    deadlines: Option<OperationDeadlines>,
    /// The geometry this pane converges to during setup. The focus pane uses
    /// the mailbox's latest observed geometry; reconcile targets carry their
    /// own recorded geometry.
    geometry: Option<Geometry>,
    /// Reconcile targets settle attach failures as pane-local `PaneFailed`
    /// notices; operation targets use the focus-scoped `SetupWithoutAttachment`
    /// settlement, which only the Switching phase can consume.
    fault_on_failure: bool,
    expected_host: Option<Box<orchbee_agent_observation_protocol::AgentLaunchBinding>>,
}

impl SetupTarget {
    fn bootstrap(target: BootstrapTarget) -> Result<Self> {
        Ok(Self {
            key: target.pane_ref(),
            index: target.selected().index(),
            provenance: CreateTerminalProvenance::BindingOnly,
            attach: attach_binding(
                target.session_id(),
                target.binding().ok_or(CarrierFailure::Protocol)?,
            )
            .map_err(|_status| CarrierFailure::Protocol)?,
            deadlines: None,
            geometry: None,
            fault_on_failure: false,
            expected_host: None,
        })
    }

    fn operation(target: AttachTarget) -> Result<Self> {
        Ok(Self {
            key: target.pane_ref(),
            index: target.selected().index(),
            provenance: target.provenance().clone(),
            attach: attach_binding(
                target.session_id(),
                target.binding().ok_or(CarrierFailure::Protocol)?,
            )
            .map_err(|_status| CarrierFailure::Protocol)?,
            deadlines: Some(target.deadlines()),
            geometry: None,
            fault_on_failure: false,
            expected_host: target.expected_host,
        })
    }

    /// A reconcile-minted attach target: the pane's own binding and recorded
    /// geometry, with the reconcile's single absolute deadline.
    fn reconcile(target: &ReconcileTarget, deadlines: OperationDeadlines) -> Result<Self> {
        Ok(Self {
            key: target.pane_ref().clone(),
            index: target.index(),
            provenance: target.provenance().clone(),
            attach: attach_binding(target.session_id(), target.binding())
                .map_err(|_status| CarrierFailure::Protocol)?,
            deadlines: Some(deadlines),
            geometry: Some(target.geometry()),
            fault_on_failure: true,
            expected_host: None,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ControlFrame {
    Request,
    Waiting,
}

struct AttachedTerminal {
    control_observation_due: Option<Instant>,
    control_frame: Option<ControlFrame>,
    key: PaneRef,
    attachment_id: orchbee_client_server_protocol::ViewerId,
    /// `Some` while the setup pipeline is unfinished — including a reconcile
    /// setup that yielded on `ServerBusy`, which leaves the half-installed
    /// viewer parked for the next reconcile pass to dispose and re-attach.
    setup: Option<SetupTarget>,
    /// Whether the `Attached` event settled `Applied` — the record learned
    /// the attachment. Disposing an announced half-setup owes the pane a
    /// `ViewerLost` notice; an unannounced one is silent.
    announced: bool,
    mode: Option<ControllerMode>,
    next_sequence: u64,
    accepted_geometry: Option<Geometry>,
    /// Outer-terminal colors this controller incarnation last reported.
    reported_host_colors: Option<crate::client_view::TerminalQueryColors>,
}

#[derive(Clone, Copy)]
enum SetupOwner<'lease> {
    Startup {
        cutoff: Instant,
    },
    Lease(&'lease CommandLease),
    /// Reconcile-owned setup: identical lease fencing to `Lease`, except a
    /// `ServerBusy` retry wait yields `Deadline` instead of holding the
    /// command lease — `run_reconcile` requeues the marker deferred so queued
    /// live work proceeds while the busy pane backs off.
    Reconcile(&'lease CommandLease),
}

/// The outcome of one bounded reconcile step. `Done` consumes the step;
/// `Retry` rotates it behind the remaining plan for a deferred retry; and
/// `Expired` ends the episode — the shared deadline is spent, so the marker
/// is dropped without requeue or a fresh budget.
enum ReconcileProgress {
    Done,
    Retry,
    Expired,
}

impl SetupOwner<'_> {
    fn forward_cutoff(self, target: &SetupTarget) -> Result<Instant> {
        match self {
            Self::Startup { cutoff } => Ok(cutoff),
            Self::Lease(_) | Self::Reconcile(_) => target
                .deadlines
                .map(OperationDeadlines::forward_cutoff)
                .ok_or(CarrierFailure::Protocol),
        }
    }
}

enum SetupCompletion {
    Ready,
    StableWithoutViewer,
    Superseded,
}

#[derive(Clone, Copy)]
enum SetupEntryExpiry {
    Retire,
    SettleSwitch,
}

enum InitialAttachResult {
    Outcome(TerminalViewerOutcome),
    Superseded,
    EntryExpired,
}

/// What the Server settled for one viewport demand leg.
#[derive(Clone, Copy, Eq, PartialEq)]
enum DemandLeg {
    Applied,
    /// The Host refused the demand; the Server keeps the viewer slot.
    HostRefused,
    /// Any other refusal or unavailability.
    Refused,
}

impl ClientExecutor {
    fn new(client: LocalClient, mailbox: Arc<CommandMailbox>) -> Self {
        let clock = client.operation_clock();
        let publications = client
            .origin()
            .subscribe(
                &orchbee_workbench_protocol::WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse("fffffffffffffffffffffffffffffffe")
                        .expect("nonzero subscription identity"),
                }),
                orchbee_workbench_protocol::StreamMode::State,
            )
            .ok();
        let hinted = Arc::downgrade(&mailbox);
        let viewer_ready = client.origin().watch_viewer_ready(move |viewer| {
            if let Some(mailbox) = hinted.upgrade() {
                mailbox.note_viewer_ready(viewer.value());
            }
        });
        Self {
            client,
            _viewer_ready: viewer_ready,
            initial_context: None,
            status_only: false,
            membership_recovery: false,
            unavailable_poll: None,
            clock,
            mailbox,
            attachments: Vec::new(),
            workbench_baseline: None,
            publications,
            pending_create: None,
            pending_move: None,
            pending_panel: None,
            pending_close: None,
            pending_rename: None,
            pending_setup: None,
            pending_workspace_close: None,
            pending_takeover: None,
        }
    }

    fn run(&mut self, initial_geometry: Geometry) -> Result<()> {
        let startup_deadline = self
            .client
            .submission_deadline()
            .ok_or(CarrierFailure::Protocol)?;
        let Some(bootstrap) = self.bootstrap(initial_geometry, startup_deadline)? else {
            self.client.set_submission_deadline(None);
            return self.run_ready_queue();
        };
        if bootstrap.binding().is_none() {
            self.client.set_submission_deadline(None);
            return self.run_ready_queue();
        }
        let target = SetupTarget::bootstrap(bootstrap)?;
        if !matches!(
            self.setup_target(
                target,
                SetupOwner::Startup {
                    cutoff: startup_deadline,
                },
                SetupEntryExpiry::Retire,
            )?,
            SetupCompletion::Ready | SetupCompletion::StableWithoutViewer
        ) {
            return Err(CarrierFailure::Protocol);
        }
        self.client.set_submission_deadline(None);
        self.run_ready_queue()
    }

    fn bootstrap(
        &mut self,
        initial_geometry: Geometry,
        startup_deadline: Instant,
    ) -> Result<Option<BootstrapTarget>> {
        self.ensure_before(startup_deadline)?;
        let snapshot_request = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let WorkbenchSnapshotOutcome::Available { snapshot } =
            self.read_workbench_before(snapshot_request, startup_deadline)?
        else {
            return Err(CarrierFailure::Transport);
        };
        let (snapshot, created) =
            self.directory_startup(snapshot, initial_geometry, startup_deadline)?;

        #[allow(unused_mut)]
        let mut selection = None;
        let cache = if let Some((cache, member)) = created {
            selection = Some(member);
            Ok(cache)
        } else if let Some(target) = self
            .initial_context
            .as_ref()
            .and_then(ClientStartupTarget::context)
        {
            if target.session_id != snapshot.session_id {
                return Err(CarrierFailure::AuthorityDivergence);
            }
            project_native_context_tabs(&snapshot, &target.location)
        } else if let Some(cache) = self.remembered_startup(&snapshot, &mut selection)? {
            Ok(cache)
        } else {
            project_preferred_workspace(&snapshot)
        }
        .map_err(|_error| CarrierFailure::AuthorityDivergence)?;
        if let Some(ClientStartupTarget::Agent(target)) = &self.initial_context {
            // This read shares the startup deadline and precedes any attachment.
            crate::machine_agents::verify(
                &mut self.client,
                &snapshot,
                target,
                startup_deadline,
                crate::local_client::cancellation::Cancellation::new(None),
            )
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
            selection = Some(crate::workbench_projection::ClientMemberTarget {
                tab: target.tab.clone(),
                panel: target.panel.clone(),
                presentation: target.member.clone(),
            });
        }
        self.publish_and_wait(
            WorkerEvent::Bootstrap { cache, selection },
            EventDeadline::Startup(startup_deadline),
            startup_deadline,
        )?
        .into_bootstrap()
    }

    fn remembered_startup(
        &self,
        snapshot: &orchbee_workbench_protocol::Snapshot,
        selection: &mut Option<crate::workbench_projection::ClientMemberTarget>,
    ) -> Result<Option<crate::workbench_projection::ClientContextTabs>> {
        if let Some(ClientStartupTarget::LocalDirectory {
            position, fallback, ..
        }) = &self.initial_context
        {
            let Some(position) = position else {
                let location = crate::workbench_projection::preferred_workspace_location(snapshot)
                    .map_err(|_| CarrierFailure::AuthorityDivergence)?;
                return project_native_context_tabs(snapshot, &location)
                    .map(Some)
                    .map_err(|_| CarrierFailure::AuthorityDivergence);
            };
            let (cache, member, changed) = crate::startup_preference::resolve(snapshot, position)
                .map_err(|_| CarrierFailure::AuthorityDivergence)?;
            *selection = member;
            fallback.store(changed, std::sync::atomic::Ordering::Release);
            return Ok(Some(cache));
        }

        Ok(None)
    }

    fn setup_target(
        &mut self,
        target: SetupTarget,
        owner: SetupOwner<'_>,
        entry_expiry: SetupEntryExpiry,
    ) -> Result<SetupCompletion> {
        // Exactly one live attachment per pane incarnation; other panes coexist.
        if self.attached_ref(&target.key).is_ok() {
            return Err(CarrierFailure::Protocol);
        }
        let forward_cutoff = owner.forward_cutoff(&target)?;
        if !self.setup_is_current(owner, &target.key)? {
            return Ok(SetupCompletion::Superseded);
        }
        if !target.provenance.attachable() {
            return self.settle_without_attachment(&target, forward_cutoff);
        }

        let outcome = match self.viewer_for_initial_attach(
            owner,
            &target.key,
            target.attach.clone(),
            forward_cutoff,
            entry_expiry,
        )? {
            InitialAttachResult::Outcome(outcome) => outcome,
            InitialAttachResult::Superseded => return Ok(SetupCompletion::Superseded),
            InitialAttachResult::EntryExpired => {
                return self.settle_switch_entry_expired_setup(owner, &target);
            }
        };
        let TerminalViewerOutcome::Attached {
            attachment_id,
            host_epoch,
            host_terminal_id,
            ..
        } = outcome
        else {
            return match outcome {
                TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {
                    if !self.setup_is_current(owner, &target.key)? {
                        Ok(SetupCompletion::Superseded)
                    } else if target.fault_on_failure {
                        self.settle_attach_fault(&target, &outcome, forward_cutoff)
                    } else {
                        self.settle_without_attachment(&target, forward_cutoff)
                    }
                }
                TerminalViewerOutcome::Empty {}
                | TerminalViewerOutcome::FeedbackAccepted {}
                | TerminalViewerOutcome::Detached {}
                | TerminalViewerOutcome::Record(_)
                | TerminalViewerOutcome::RowsRejected { .. } => Err(CarrierFailure::Protocol),
                TerminalViewerOutcome::Attached { .. } => unreachable!("matched attached outcome"),
            };
        };
        self.attachments.push(AttachedTerminal {
            control_observation_due: None,
            control_frame: None,
            key: target.key.clone(),
            attachment_id: attachment_id.clone(),
            setup: Some(target.clone()),
            announced: false,
            mode: None,
            next_sequence: 0,
            accepted_geometry: None,
            reported_host_colors: None,
        });
        if target.expected_host.as_ref().is_some_and(|expected| {
            host_epoch.as_str() != expected.host_epoch.as_str()
                || host_terminal_id.as_str() != expected.host_terminal_id.as_str()
        }) {
            // No poll or projection was admitted for this unexpected physical terminal.
            return self.cleanup_after_setup_failure(owner, &target);
        }
        if matches!(owner, SetupOwner::Startup { .. })
            && let Some(ClientStartupTarget::Agent(expected)) = &self.initial_context
            && (host_epoch.as_str() != expected.physical.host_epoch.as_str()
                || host_terminal_id.as_str() != expected.physical.host_terminal_id.as_str())
        {
            self.cleanup_startup_setup(owner, &target.key)?;
            return Err(CarrierFailure::AuthorityDivergence);
        }
        let setup = self.finish_attached_setup(
            &target,
            owner,
            forward_cutoff,
            attachment_id.as_str(),
            host_epoch.as_str(),
            host_terminal_id,
        );
        match setup {
            Ok(completion) => Ok(completion),
            Err(failure)
                if self.client.generation().is_none() || self.mailbox.is_closed().is_some() =>
            {
                Err(failure)
            }
            Err(failure)
                if !matches!(
                    failure,
                    CarrierFailure::Deadline | CarrierFailure::Transport
                ) =>
            {
                Err(failure)
            }
            Err(failure) if target.deadlines.is_none() => {
                self.cleanup_startup_setup(owner, &target.key)?;
                Err(failure)
            }
            Err(_failure) => self.cleanup_after_setup_failure(owner, &target),
        }
    }

    fn finish_attached_setup(
        &mut self,
        target: &SetupTarget,
        owner: SetupOwner<'_>,
        forward_cutoff: Instant,
        attachment_id: &str,
        host_epoch: &str,
        host_terminal_id: orchbee_client_server_protocol::HostTerminalId,
    ) -> Result<SetupCompletion> {
        self.finish_attached_setup_inner(
            target,
            owner,
            forward_cutoff,
            attachment_id,
            host_epoch,
            host_terminal_id,
        )
    }

    fn finish_attached_setup_inner(
        &mut self,
        target: &SetupTarget,
        owner: SetupOwner<'_>,
        forward_cutoff: Instant,
        attachment_id: &str,
        host_epoch: &str,
        host_terminal_id: orchbee_client_server_protocol::HostTerminalId,
    ) -> Result<SetupCompletion> {
        let projection = TerminalProjection::new(
            AttachmentId::parse(attachment_id).map_err(|_error| CarrierFailure::Protocol)?,
            HostEpoch::parse(host_epoch).map_err(|_error| CarrierFailure::Protocol)?,
        )
        .with_host_terminal(host_terminal_id);
        if !self.setup_is_current(owner, &target.key)? {
            return Err(CarrierFailure::Deadline);
        }
        let installed = self.publish_and_wait(
            WorkerEvent::Attached {
                key: target.key.clone(),
                projection,
            },
            EventDeadline::Forward(forward_cutoff),
            forward_cutoff,
        )?;
        match installed {
            UiReply::AttachedSettled(SetupEventDisposition::Applied) => {}
            UiReply::AttachedSettled(SetupEventDisposition::Superseded { .. }) => {
                return Err(CarrierFailure::Deadline);
            }
            _ => return Err(CarrierFailure::Protocol),
        }
        self.attached_mut(&target.key)?.announced = true;
        if !self.setup_is_current(owner, &target.key)? {
            return Err(CarrierFailure::Deadline);
        }

        // Settled Host terminals retain readable viewers but refuse controller
        // attachment. That refusal must not retire the Client's navigation.
        let acquire = self
            .control_for_setup(
                owner,
                &target.key,
                TerminalControlAction::Acquire {
                    attachment_id: self.viewer_id_for(&target.key)?,
                },
                forward_cutoff,
            )?
            .ok_or(CarrierFailure::Deadline)?;
        let mode = match acquire {
            TerminalControlOutcome::Observed { .. } => return Err(CarrierFailure::Protocol),
            TerminalControlOutcome::Acquired { controller_id } => {
                ControllerMode::Controlling(controller_id)
            }
            TerminalControlOutcome::Rejected {
                reason: ControlRejectedReason::ControllerBusy | ControlRejectedReason::HostRejected,
            }
            | TerminalControlOutcome::Unavailable {
                reason: ControlUnavailableReason::TerminalRuntimeUnavailable,
            } => ControllerMode::ReadOnly,
            TerminalControlOutcome::Rejected { .. }
            | TerminalControlOutcome::Unavailable { .. } => return Err(CarrierFailure::Deadline),
            TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Pasted { .. }
            | TerminalControlOutcome::HostColorsAccepted {}
            | TerminalControlOutcome::Cleared { .. }
            | TerminalControlOutcome::ResetAccepted { .. } => return Err(CarrierFailure::Protocol),
            TerminalControlOutcome::OutcomeUnknown {} => {
                return Err(CarrierFailure::OutcomeUnknown);
            }
        };
        if !self.setup_is_current(owner, &target.key)? {
            return Err(CarrierFailure::Deadline);
        }
        self.set_attached_mode(&target.key, mode.clone())?;
        if let ControllerMode::Controlling(controller_id) = &mode {
            self.report_host_colors_for_setup(
                owner,
                &target.key,
                controller_id.clone(),
                forward_cutoff,
            )?;
            let geometry = match target.geometry {
                Some(geometry) => geometry,
                None => self.mailbox.latest_geometry().map_err(closed)?,
            };
            if !self.resize_for_setup(
                owner,
                &target.key,
                controller_id.clone(),
                geometry,
                forward_cutoff,
            )? {
                return Err(CarrierFailure::Deadline);
            }
        } else {
            self.publish_setup_configuration(&target.key, forward_cutoff)?;
        }

        loop {
            if !self.setup_is_current(owner, &target.key)? {
                return Err(CarrierFailure::Deadline);
            }
            let poll_key = self
                .allocate_setup_poll(owner, &target.key)?
                .ok_or(CarrierFailure::Deadline)?;
            let outcome = self
                .viewer_for_setup(
                    owner,
                    &target.key,
                    TerminalViewerAction::Poll {
                        attachment_id: self.viewer_id_for(&target.key)?,
                    },
                    forward_cutoff,
                )?
                .ok_or(CarrierFailure::Deadline)?;
            let record = match outcome {
                TerminalViewerOutcome::Empty {} => None,
                TerminalViewerOutcome::Record(record) => Some(record),
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::FeedbackAccepted {}
                | TerminalViewerOutcome::Detached {}
                | TerminalViewerOutcome::RowsRejected { .. } => {
                    return Err(CarrierFailure::Protocol);
                }
                TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {
                    return Err(CarrierFailure::Deadline);
                }
            };
            if record.as_deref().is_some_and(settled_record) {
                self.set_attached_mode(&target.key, ControllerMode::ReadOnly)?;
                self.attached_mut(&target.key)?.accepted_geometry = None;
                self.publish_setup_configuration(&target.key, forward_cutoff)?;
            }
            self.mailbox
                .publish_poll_result_before(
                    poll_key.clone(),
                    record,
                    false,
                    forward_cutoff,
                    &self.clock,
                )
                .map_err(event)?;
            let causal = self
                .mailbox
                .take_causal_before(&poll_key, forward_cutoff, &self.clock)
                .map_err(event)?
                .ok_or(CarrierFailure::Deadline)?;
            let CausalSettlement::Feedback(feedback) = causal else {
                continue;
            };
            if !self.setup_is_current(owner, &target.key)? {
                return Err(CarrierFailure::Deadline);
            }
            let frame_applied = matches!(feedback, TerminalViewerAction::FrameApplied { .. });
            let result = self
                .viewer_for_setup(owner, &target.key, feedback, forward_cutoff)?
                .ok_or(CarrierFailure::Deadline)?;
            match result {
                TerminalViewerOutcome::FeedbackAccepted {} => {}
                TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {
                    return Err(CarrierFailure::Deadline);
                }
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::Detached {}
                | TerminalViewerOutcome::Empty {}
                | TerminalViewerOutcome::Record(_)
                | TerminalViewerOutcome::RowsRejected { .. } => {
                    return Err(CarrierFailure::Protocol);
                }
            }
            if !self.setup_is_current(owner, &target.key)? {
                return Err(CarrierFailure::Deadline);
            }
            if !frame_applied {
                continue;
            }
            let frame = FrameIdentity::new(poll_key.serial());
            match self.publish_and_wait(
                WorkerEvent::FeedbackAccepted {
                    key: target.key.clone(),
                    frame,
                },
                EventDeadline::Forward(forward_cutoff),
                forward_cutoff,
            )? {
                UiReply::FeedbackSettled(SetupEventDisposition::Applied) => {}
                UiReply::FeedbackSettled(SetupEventDisposition::Superseded { .. }) => {
                    return Err(CarrierFailure::Deadline);
                }
                _ => return Err(CarrierFailure::Protocol),
            }
            if !self.owns_global_ready(owner, &target.key)? {
                // A background pane's staged frame already converges its
                // record; the Ready phase belongs to the focus pane alone.
                self.attached_mut(&target.key)?.setup = None;
                return Ok(SetupCompletion::Ready);
            }
            let (mode, accepted_geometry) = self.attached_setup_facts(&target.key)?;
            let reply = self.publish_and_wait(
                WorkerEvent::ReadyCandidate {
                    key: target.key.clone(),
                    mode: mode.clone(),
                    last_accepted_geometry: accepted_geometry,
                    frame,
                },
                EventDeadline::Forward(forward_cutoff),
                forward_cutoff,
            )?;
            match reply {
                UiReply::Ready(ReadyDecision::Ready) => {
                    // Setup ordinals belong to its frozen cache. Ready work retains only identity.
                    self.attached_mut(&target.key)?.setup = None;
                    return Ok(SetupCompletion::Ready);
                }
                UiReply::Ready(ReadyDecision::Superseded) => {
                    return Err(CarrierFailure::Deadline);
                }
                UiReply::Ready(ReadyDecision::CatchUpResize(geometry)) => {
                    let ControllerMode::Controlling(controller_id) = mode else {
                        return Err(CarrierFailure::Protocol);
                    };
                    if !self.resize_for_setup(
                        owner,
                        &target.key,
                        controller_id,
                        geometry,
                        forward_cutoff,
                    )? {
                        return Err(CarrierFailure::Deadline);
                    }
                }
                _ => return Err(CarrierFailure::Protocol),
            }
        }
    }

    fn run_ready_queue(&mut self) -> Result<()> {
        loop {
            let mut lease = self.mailbox.recv().map_err(closed)?;
            if self.membership_recovery && !matches!(lease.record(), QueuedCommand::Refresh(_)) {
                return Err(CarrierFailure::Protocol);
            }
            if self.status_only
                && !matches!(lease.record(), QueuedCommand::ReconcileClose(marker) if marker.membership_recovery())
                && !matches!(lease.record(), QueuedCommand::Close(marker)
                if matches!(marker.operation().kind(), crate::workbench_state::CloseOperationKind::Read { .. }
                    | crate::workbench_state::CloseOperationKind::Resume { .. }))
            {
                return Err(CarrierFailure::Protocol);
            }
            let create = match lease.record_mut() {
                QueuedCommand::Create(marker) => {
                    let request = marker.take_request().ok_or(CarrierFailure::Protocol)?;
                    let operation = marker.operation().clone();
                    Some((request, operation))
                }
                _ => None,
            };
            let started = Instant::now();
            let command = lease.record().kind();
            let bytes = match lease.record() {
                QueuedCommand::Input(tagged) => tagged.value().as_bytes().len(),
                _ => 0,
            };
            let step = if let Some((request, operation)) = create {
                self.run_create(&lease, request, operation)
            } else {
                match lease.record() {
                    QueuedCommand::Input(tagged) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_input(&lease, tagged, deadline))
                    }
                    QueuedCommand::Resize(tagged) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_resize(&lease, tagged, deadline))
                    }
                    QueuedCommand::Poll(_) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_poll(&lease, deadline))
                    }
                    QueuedCommand::Viewport(tagged) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_viewport(&lease, tagged, deadline))
                    }
                    QueuedCommand::Selection(tagged) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_selection(&lease, tagged, deadline))
                    }
                    QueuedCommand::ClearTerminal(tagged) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_terminal_clear(&lease, tagged, deadline))
                    }
                    QueuedCommand::Switch(marker) => {
                        self.run_switch(&lease, marker.target().clone())
                    }
                    QueuedCommand::Create(_) => Err(CarrierFailure::Protocol),
                    QueuedCommand::Refresh(marker) => self.run_refresh(&lease, marker.clone()),
                    QueuedCommand::Move(marker) => {
                        self.run_move(&lease, marker.operation().clone())
                    }
                    QueuedCommand::Panel(marker) => {
                        self.run_panel(&lease, marker.operation().clone())
                    }
                    QueuedCommand::WorkspaceSetup(marker) => {
                        self.run_workspace_setup(&lease, marker.0.clone())
                    }
                    QueuedCommand::Rename(marker) => {
                        self.run_rename(&lease, marker.operation().clone())
                    }
                    QueuedCommand::Close(marker) => {
                        self.run_close(&lease, marker.operation().clone())
                    }
                    QueuedCommand::ReconcileClose(marker) => {
                        self.run_close_reconciliation(&lease, marker)
                    }
                    QueuedCommand::ReconcilePanes(marker) => self.run_reconcile(&lease, marker),
                    QueuedCommand::WorkspaceCloseRead(read) => {
                        self.run_workspace_close_read(&lease, read)
                    }
                }
            };
            let elapsed = started.elapsed();
            if elapsed >= crate::workbench_client::diagnostic_events::SLOW_STEP {
                crate::workbench_client::diagnostic_events::step_slow(
                    command,
                    bytes,
                    self.mailbox.queued_input_bytes(),
                    elapsed,
                    step.is_ok(),
                );
            }
            step?;
            self.mailbox.finish_lease(lease).map_err(closed)?;
        }
    }

    /// Input serves the pane focused at admission; the pane must still hold a
    /// controlling lease. A focus-scoped command that arrives for a degraded
    /// or detached pane is dropped — pane-local failure never kills the
    /// session carrying unrelated panes.
    fn run_input(
        &mut self,
        lease: &CommandLease,
        tagged: &Tagged<orchbee_client_server_protocol::ControlInput>,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let (key, input) = (tagged.key(), tagged.value().clone());
        self.execution_desired(lease)?;
        // A queued input binds the exact attachment incarnation: once the pane
        // detached and re-bound, the fence drops the stale command instead of
        // replaying it against the new attachment.
        if self.attached_ref(key).is_err() {
            return Ok(());
        }
        let Some(controller) = self.controller_for(key)? else {
            return Ok(());
        };
        self.sync_host_colors_for_lease(lease, key, controller.clone(), deadline)?;
        let sequence = self.next_control_sequence(key)?;
        let outcome = self.control_for_lease(
            lease,
            key,
            TerminalControlAction::Input {
                attachment_id: self.viewer_id_for(key)?,
                controller_id: controller,
                sequence,
                bytes: input,
            },
            deadline,
        )?;
        match outcome {
            TerminalControlOutcome::Observed { .. } => Err(CarrierFailure::Protocol.into()),
            TerminalControlOutcome::InputAccepted {} => Ok(()),
            TerminalControlOutcome::OutcomeUnknown {} => {
                Err(ReadyFlowError::Carrier(CarrierFailure::OutcomeUnknown))
            }
            TerminalControlOutcome::Rejected {
                reason:
                    ControlRejectedReason::NotController
                    | ControlRejectedReason::HostInputUnavailable
                    | ControlRejectedReason::HostRejected
                    | ControlRejectedReason::ViewerUnavailable,
            } => self.settle_pane_control_lost(lease, key, deadline),
            TerminalControlOutcome::Rejected { .. } => {
                Err(ReadyFlowError::Carrier(CarrierFailure::Protocol))
            }
            TerminalControlOutcome::Unavailable {
                reason: ControlUnavailableReason::TerminalRuntimeUnavailable,
            } => self.settle_pane_control_lost(lease, key, deadline),
            TerminalControlOutcome::Acquired { .. }
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Pasted { .. }
            | TerminalControlOutcome::HostColorsAccepted {}
            | TerminalControlOutcome::Cleared { .. }
            | TerminalControlOutcome::ResetAccepted { .. }
            | TerminalControlOutcome::Unavailable { .. } => {
                Err(ReadyFlowError::Carrier(CarrierFailure::Protocol))
            }
        }
    }

    fn run_resize(
        &mut self,
        lease: &CommandLease,
        tagged: &Tagged<Geometry>,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let key = tagged.key().clone();
        let geometry = *tagged.value();
        self.execution_desired(lease)?;
        // Same incarnation fence as input: a queued resize that outlived its
        // attachment is dropped, never replayed to a re-bound viewer.
        if self.attached_ref(&key).is_err() {
            return Ok(());
        }
        let Some(controller) = self.controller_for(&key)? else {
            return Ok(());
        };
        self.sync_host_colors_for_lease(lease, &key, controller.clone(), deadline)?;
        let sequence = self.next_control_sequence(&key)?;
        let outcome = self.control_for_lease(
            lease,
            &key,
            TerminalControlAction::Resize {
                attachment_id: self.viewer_id_for(&key)?,
                controller_id: controller,
                sequence,
                geometry,
            },
            deadline,
        )?;
        match outcome {
            TerminalControlOutcome::Observed { .. } => return Err(CarrierFailure::Protocol.into()),
            TerminalControlOutcome::Resized { geometry: actual } if actual == geometry => {}
            TerminalControlOutcome::OutcomeUnknown {} => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::OutcomeUnknown));
            }
            TerminalControlOutcome::Rejected {
                reason:
                    ControlRejectedReason::NotController
                    | ControlRejectedReason::HostRejected
                    | ControlRejectedReason::ViewerUnavailable,
            } => {
                return self.settle_pane_control_lost(lease, &key, deadline);
            }
            TerminalControlOutcome::Rejected { .. } => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
            TerminalControlOutcome::Unavailable {
                reason: ControlUnavailableReason::TerminalRuntimeUnavailable,
            } => {
                return self.settle_pane_control_lost(lease, &key, deadline);
            }
            TerminalControlOutcome::Acquired { .. }
            | TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Pasted { .. }
            | TerminalControlOutcome::HostColorsAccepted {}
            | TerminalControlOutcome::Cleared { .. }
            | TerminalControlOutcome::ResetAccepted { .. }
            | TerminalControlOutcome::Unavailable { .. } => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
        }
        self.set_accepted_geometry(&key, geometry)?;
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.publish_ready_setup_configuration(lease, &key, deadline, cutoff)
    }

    /// One retained-history step for a pane incarnation. Every leg runs
    /// under the ready-lease fences — the pane must still be the canonical
    /// record with a live attachment — and each step's outcome publishes as
    /// a fire-and-forget `ViewportOutcome` notice. The viewport core in the UI
    /// owns the state machine; this executor only guarantees the action ran
    /// against a still-admitted attachment and reports what the host said.
    fn run_viewport(
        &mut self,
        lease: &CommandLease,
        tagged: &Tagged<ViewportStep>,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let key = tagged.key().clone();
        let step = *tagged.value();
        if self.attached_ref(&key).is_err() || !self.pane_admitted(lease, &key)? {
            return Ok(());
        }
        if let Some(controller) = self.controller_for(&key)? {
            self.sync_host_colors_for_lease(lease, &key, controller, deadline)?;
        }
        let attachment_id = self.viewer_id_for(&key)?;
        let outcome = match step {
            ViewportStep::Enter | ViewportStep::EnterTail => {
                let demand = if step == ViewportStep::EnterTail {
                    ProjectionDemand::Active
                } else {
                    ProjectionDemand::Paused
                };
                let Some(outcome) =
                    self.viewport_enter(lease, &key, &attachment_id, demand, deadline)?
                else {
                    return Ok(());
                };
                outcome
            }
            ViewportStep::Read {
                origin,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
            } => {
                let Some((_, outcome)) = self.viewer_for_ready_lease(
                    lease,
                    &key,
                    TerminalViewerAction::ReadViewportRows {
                        detail: orchbee_client_server_protocol::ViewerRowDetail::Render,
                        attachment_id,
                        origin,
                        row_count,
                        revision,
                        generation,
                        request_sequence,
                        maximum_bytes,
                    },
                    deadline,
                )?
                else {
                    return Ok(());
                };
                match outcome {
                    TerminalViewerOutcome::Record(record) => {
                        match decode_viewport_window(&record, maximum_bytes) {
                            Ok(window) => ViewportOutcome::Window {
                                request_sequence,
                                window: Box::new(window),
                            },
                            Err(ViewportWindowFailure::Invalid) => {
                                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
                            }
                            Err(ViewportWindowFailure::MemoryLimit) => ViewportOutcome::ReadRejected {
                                request_sequence,
                                rejection: ViewportReadRejection::Typed(
                                    orchbee_client_server_protocol::ViewerRowsRejectedReason::ResourceExhausted,
                                ),
                            },
                        }
                    }
                    TerminalViewerOutcome::RowsRejected { reason } => {
                        ViewportOutcome::ReadRejected {
                            request_sequence,
                            rejection: ViewportReadRejection::Typed(reason),
                        }
                    }
                    TerminalViewerOutcome::Rejected { .. } => ViewportOutcome::ReadRejected {
                        request_sequence,
                        rejection: ViewportReadRejection::Host,
                    },
                    TerminalViewerOutcome::Unavailable {
                        reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                    } => return self.dispose_lost_viewer(lease, &key, deadline),
                    TerminalViewerOutcome::Unavailable { .. } => {
                        return self.settle_pane_viewer_lost(lease, &key, deadline);
                    }
                    TerminalViewerOutcome::Attached { .. }
                    | TerminalViewerOutcome::FeedbackAccepted {}
                    | TerminalViewerOutcome::Detached {}
                    | TerminalViewerOutcome::Empty {} => {
                        return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
                    }
                }
            }
            ViewportStep::Ack {
                request_sequence,
                generation,
                revision,
            } => {
                let Some((_, outcome)) = self.viewer_for_ready_lease(
                    lease,
                    &key,
                    TerminalViewerAction::RowsApplied {
                        attachment_id,
                        request_sequence,
                        generation,
                        revision,
                    },
                    deadline,
                )?
                else {
                    return Ok(());
                };
                match outcome {
                    TerminalViewerOutcome::FeedbackAccepted {} => ViewportOutcome::Ack {
                        request_sequence,
                        accepted: true,
                    },
                    TerminalViewerOutcome::RowsRejected { .. }
                    | TerminalViewerOutcome::Rejected { .. } => ViewportOutcome::Ack {
                        request_sequence,
                        accepted: false,
                    },
                    TerminalViewerOutcome::Unavailable {
                        reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                    } => return self.dispose_lost_viewer(lease, &key, deadline),
                    TerminalViewerOutcome::Unavailable { .. } => {
                        return self.settle_pane_viewer_lost(lease, &key, deadline);
                    }
                    TerminalViewerOutcome::Attached { .. }
                    | TerminalViewerOutcome::Detached {}
                    | TerminalViewerOutcome::Empty {}
                    | TerminalViewerOutcome::Record(_) => {
                        return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
                    }
                }
            }
            ViewportStep::Recover => {
                let Some(outcome) = self.viewport_recover(lease, &key, &attachment_id, deadline)?
                else {
                    return Ok(());
                };
                outcome
            }
        };
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.mailbox
            .publish_notice_before(
                WorkerEvent::ViewportOutcome {
                    key: key.clone(),
                    outcome,
                },
                cutoff,
                &self.clock,
            )
            .map_err(|error| self.ready_event_error(lease, &key, deadline, error))?;
        Ok(())
    }

    /// Clear uses the same viewer-only, one-shot request as the CLI; it sends no program input.
    fn run_terminal_clear(
        &mut self,
        lease: &CommandLease,
        tagged: &Tagged<()>,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let key = tagged.key();
        if self.attached_ref(key).is_err() || !self.pane_admitted(lease, key)? {
            return Ok(());
        }
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.ensure_ready_before(lease, key, deadline, cutoff)?;
        let action = TerminalControlAction::Clear {
            attachment_id: self.viewer_id_for(key)?,
        };
        let outcome = self.control_once_ready(lease, key, action, deadline, cutoff)?;
        if !matches!(
            outcome,
            TerminalControlOutcome::Cleared { .. }
                | TerminalControlOutcome::Rejected { .. }
                | TerminalControlOutcome::Unavailable { .. }
                | TerminalControlOutcome::OutcomeUnknown {}
        ) {
            return Err(CarrierFailure::Protocol.into());
        }
        self.mailbox
            .publish_notice_before(
                WorkerEvent::TerminalClearOutcome {
                    key: key.clone(),
                    outcome,
                },
                cutoff,
                &self.clock,
            )
            .map_err(|error| self.ready_event_error(lease, key, deadline, error))?;
        Ok(())
    }

    fn run_selection(
        &mut self,
        lease: &CommandLease,
        tagged: &Tagged<crate::workbench_client::terminal_selection::Command>,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let key = tagged.key().clone();
        if self.attached_ref(&key).is_err() || !self.pane_admitted(lease, &key)? {
            return Ok(());
        }
        let command = *tagged.value();
        let action = command.viewer_action(self.viewer_id_for(&key)?);
        let Some((_, outcome)) = self.viewer_for_ready_lease(lease, &key, action, deadline)? else {
            return Ok(());
        };
        if matches!(
            outcome,
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
            }
        ) {
            return self.dispose_lost_viewer(lease, &key, deadline);
        }
        if matches!(outcome, TerminalViewerOutcome::Unavailable { .. }) {
            return self.settle_pane_viewer_lost(lease, &key, deadline);
        }
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.mailbox
            .publish_notice_before(
                WorkerEvent::SelectionOutcome {
                    key: key.clone(),
                    command,
                    outcome,
                },
                cutoff,
                &self.clock,
            )
            .map_err(|error| self.ready_event_error(lease, &key, deadline, error))?;
        Ok(())
    }

    /// One demand-setting leg: what the Server settled, or `None` when the
    /// pane left the admitted set mid-flight or its runtime was lost and the
    /// caller must drop the whole step.
    fn viewport_leg(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        action: TerminalViewerAction,
        deadline: Instant,
    ) -> ReadyResult<Option<DemandLeg>> {
        let Some((_, outcome)) = self.viewer_for_ready_lease(lease, key, action, deadline)? else {
            return Ok(None);
        };
        match outcome {
            TerminalViewerOutcome::FeedbackAccepted {} => Ok(Some(DemandLeg::Applied)),
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
            } => {
                self.dispose_lost_viewer(lease, key, deadline)?;
                Ok(None)
            }
            TerminalViewerOutcome::Rejected {
                reason: ViewerRejectedReason::HostRejected,
            } => Ok(Some(DemandLeg::HostRefused)),
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                Ok(Some(DemandLeg::Refused))
            }
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Detached {}
            | TerminalViewerOutcome::Empty {}
            | TerminalViewerOutcome::Record(_)
            | TerminalViewerOutcome::RowsRejected { .. } => {
                Err(ReadyFlowError::Carrier(CarrierFailure::Protocol))
            }
        }
    }

    /// Entry's demand pair: `ViewingHistory` then the requested projection demand.
    /// Manual history pauses frames; automatic tail must keep them active. A refused first
    /// leg settles `applied:false` without the second; a failed second leg
    /// runs the single reset leg (`FollowingBottom`), and a reset that fails
    /// settles as a refused exit leg instead of guessing.
    fn viewport_enter(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        attachment_id: &orchbee_client_server_protocol::ViewerId,
        projection_demand: ProjectionDemand,
        deadline: Instant,
    ) -> ReadyResult<Option<ViewportOutcome>> {
        let applied = self.viewport_leg(
            lease,
            key,
            TerminalViewerAction::SetHistoryDemand {
                attachment_id: attachment_id.clone(),
                demand: HistoryDemand::ViewingHistory,
            },
            deadline,
        )?;
        let outcome = match applied {
            None => return Ok(None),
            Some(DemandLeg::HostRefused | DemandLeg::Refused) => {
                ViewportOutcome::Enter { applied: false }
            }
            Some(DemandLeg::Applied) => match self.viewport_leg(
                lease,
                key,
                TerminalViewerAction::SetProjectionDemand {
                    attachment_id: attachment_id.clone(),
                    demand: projection_demand,
                },
                deadline,
            )? {
                None => return Ok(None),
                Some(DemandLeg::Applied) => ViewportOutcome::Enter { applied: true },
                Some(DemandLeg::HostRefused | DemandLeg::Refused) => match self.viewport_leg(
                    lease,
                    key,
                    TerminalViewerAction::SetHistoryDemand {
                        attachment_id: attachment_id.clone(),
                        demand: HistoryDemand::FollowingBottom,
                    },
                    deadline,
                )? {
                    Some(DemandLeg::Applied) => ViewportOutcome::Enter { applied: false },
                    Some(refusal) => {
                        self.settle_refused_exit_leg(lease, key, refusal, deadline)?;
                        return Ok(None);
                    }
                    None => return Ok(None),
                },
            },
        };
        Ok(Some(outcome))
    }

    /// `Recover`'s demand pair: `FollowingBottom` then `Active`. Any leg
    /// failure settles as a refused exit leg — recovery is the exit ramp
    /// itself and has nothing left to fall back to.
    fn viewport_recover(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        attachment_id: &orchbee_client_server_protocol::ViewerId,
        deadline: Instant,
    ) -> ReadyResult<Option<ViewportOutcome>> {
        for demand in [
            TerminalViewerAction::SetHistoryDemand {
                attachment_id: attachment_id.clone(),
                demand: HistoryDemand::FollowingBottom,
            },
            TerminalViewerAction::SetProjectionDemand {
                attachment_id: attachment_id.clone(),
                demand: ProjectionDemand::Active,
            },
        ] {
            match self.viewport_leg(lease, key, demand, deadline)? {
                Some(DemandLeg::Applied) => {}
                Some(refusal) => {
                    self.settle_refused_exit_leg(lease, key, refusal, deadline)?;
                    return Ok(None);
                }
                None => return Ok(None),
            }
        }
        Ok(Some(ViewportOutcome::Recovered))
    }

    /// A refused exit leg leaves the viewer outside live following. A Host
    /// refusal keeps the Server slot under a demand the Client can no longer
    /// name, so the viewer is released first; once the Server proves the
    /// slot gone, the pane takes the typed loss path that parks a focus. Any
    /// other refusal, or a release the Server does not prove, keeps the
    /// non-typed loss boundary.
    fn settle_refused_exit_leg(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        refusal: DemandLeg,
        deadline: Instant,
    ) -> ReadyResult<()> {
        if refusal == DemandLeg::HostRefused {
            let Some((_, outcome)) = self.viewer_for_ready_lease(
                lease,
                key,
                TerminalViewerAction::Detach {
                    attachment_id: self.viewer_id_for(key)?,
                },
                deadline,
            )?
            else {
                return Ok(());
            };
            match outcome {
                TerminalViewerOutcome::Detached {}
                | TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                } => return self.dispose_lost_viewer(lease, key, deadline),
                TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {}
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::Empty {}
                | TerminalViewerOutcome::FeedbackAccepted {}
                | TerminalViewerOutcome::Record(_)
                | TerminalViewerOutcome::RowsRejected { .. } => {
                    return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
                }
            }
        }
        self.settle_pane_viewer_lost(lease, key, deadline)
    }

    /// Polls run per pane incarnation: any exact member of the reconciled set
    /// polls continuously, focused or not. Typed runtime loss disposes that
    /// viewer; the state owner decides whether the focus may park or must
    /// retire an active structural operation. Other focused refusals retain
    /// the existing protocol retirement boundary.
    fn run_poll(&mut self, lease: &CommandLease, deadline: Instant) -> ReadyResult<()> {
        let poll_key = lease.poll_key().ok_or(CarrierFailure::Protocol)?;
        let key = poll_key.pane_ref().clone();
        if self.attached_ref(&key).is_err() || !self.pane_admitted(lease, &key)? {
            return Ok(());
        }
        if let Some(controller) = self.controller_for(&key)? {
            self.sync_host_colors_for_lease(lease, &key, controller, deadline)?;
        }
        self.prepare_control_frame(lease, &key, deadline)?;
        let Some((request_id, outcome)) = self.viewer_for_ready_lease(
            lease,
            &key,
            TerminalViewerAction::Poll {
                attachment_id: self.viewer_id_for(&key)?,
            },
            deadline,
        )?
        else {
            return Ok(());
        };
        let current = self.pane_admitted(lease, &key)?;
        if current {
            self.observe_controller(lease, &key, deadline)?;
        }
        if current
            && matches!(
                outcome,
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::TerminalRuntimeUnavailable
                }
            )
        {
            self.unavailable_poll = Some(
                crate::workbench_state::retired_membership::UnavailablePoll {
                    request_id,
                    poll: poll_key.clone(),
                },
            );
        }
        let record = match outcome {
            TerminalViewerOutcome::Empty {} => None,
            TerminalViewerOutcome::Record(record) => Some(record),
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. }
                if !current =>
            {
                return Ok(());
            }
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
            } => return self.dispose_lost_viewer(lease, &key, deadline),
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                return self.settle_pane_viewer_lost(lease, &key, deadline);
            }
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::FeedbackAccepted {}
            | TerminalViewerOutcome::Detached {}
            | TerminalViewerOutcome::RowsRejected { .. } => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
        };
        if current && record.as_deref().is_some_and(settled_record) {
            self.settle_pane_control_lost(lease, &key, deadline)?;
        }
        // Publish even a now-stale Record/Empty result so the mailbox retains its exact discard.
        // The publication probe rides on the poll so a peer Client's committed Workbench changes
        // surface through the ordinary refresh path without a separate event loop.
        let publication_hint = self.probe_workbench_publication(deadline)?;
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.mailbox
            .publish_poll_result_before(
                poll_key.clone(),
                record,
                publication_hint,
                cutoff,
                &self.clock,
            )
            .map_err(|error| self.ready_event_error(lease, &key, deadline, error))?;
        let Some(causal) = self
            .mailbox
            .take_causal_before(&poll_key, cutoff, &self.clock)
            .map_err(|error| self.ready_event_error(lease, &key, deadline, error))?
        else {
            return Ok(());
        };
        let CausalSettlement::Feedback(feedback) = causal else {
            return Ok(());
        };
        if !self.pane_admitted(lease, &key)? {
            self.mailbox.discard_taken_switch_causal(lease, &poll_key)?;
            return Ok(());
        }
        let frame_applied = matches!(feedback, TerminalViewerAction::FrameApplied { .. });
        let Some((_request_id, result)) =
            self.viewer_for_ready_lease(lease, &key, feedback, deadline)?
        else {
            self.mailbox.discard_taken_switch_causal(lease, &poll_key)?;
            return Ok(());
        };
        let current = self.pane_admitted(lease, &key)?;
        match result {
            TerminalViewerOutcome::FeedbackAccepted {} => {}
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. }
                if !current =>
            {
                self.mailbox.discard_taken_switch_causal(lease, &poll_key)?;
                return Ok(());
            }
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
            } => {
                self.mailbox.discard_taken_switch_causal(lease, &poll_key)?;
                return self.dispose_lost_viewer(lease, &key, deadline);
            }
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                self.mailbox.discard_taken_switch_causal(lease, &poll_key)?;
                return self.settle_pane_viewer_lost(lease, &key, deadline);
            }
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Detached {}
            | TerminalViewerOutcome::Empty {}
            | TerminalViewerOutcome::Record(_)
            | TerminalViewerOutcome::RowsRejected { .. } => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
        }
        if frame_applied && !current {
            self.mailbox.discard_taken_switch_causal(lease, &poll_key)?;
        }
        if frame_applied && current {
            let cutoff = self.effective_lease_cutoff(lease, deadline)?;
            let reply = self.publish_and_wait_ready(
                lease,
                &key,
                deadline,
                WorkerEvent::FeedbackAccepted {
                    key: key.clone(),
                    frame: FrameIdentity::new(poll_key.serial()),
                },
                cutoff,
            )?;
            match reply {
                UiReply::FeedbackSettled(SetupEventDisposition::Applied) => {}
                UiReply::FeedbackSettled(SetupEventDisposition::Superseded { .. }) => {
                    return Ok(());
                }
                _ => return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol)),
            }
            self.finish_control_frame(lease, &key, deadline)?;
        }
        Ok(())
    }

    fn run_switch(&mut self, lease: &CommandLease, target: AttachTarget) -> Result<()> {
        self.run_switch_membership(lease, target)
    }

    fn continue_switch_without_viewer(
        &mut self,
        lease: &CommandLease,
        target: AttachTarget,
    ) -> Result<()> {
        let target_key = target.selected().key().clone();
        let actual_desired = self.execution_desired(lease)?;
        if actual_desired != Some(target_key.clone()) {
            return Ok(());
        }
        if self.clock.now() >= target.deadlines().forward_cutoff() {
            return self.settle_switch_entry_expired(lease, &target);
        }
        self.setup_target(
            SetupTarget::operation(target)?,
            SetupOwner::Lease(lease),
            SetupEntryExpiry::SettleSwitch,
        )
        .map(|_completion| ())
    }

    fn settle_switch_entry_expired(
        &self,
        lease: &CommandLease,
        target: &AttachTarget,
    ) -> Result<()> {
        let total = target.deadlines().total();
        let target_key = target.selected().key().clone();
        if self.execution_desired(lease)? != Some(target_key.clone()) {
            return Ok(());
        }
        self.settle_switch_entry_expired_parts(
            lease,
            &target.pane_ref(),
            target.selected().index(),
            target.provenance(),
            total,
        )
        .map(|_completion| ())
    }

    fn settle_switch_entry_expired_setup(
        &self,
        owner: SetupOwner<'_>,
        target: &SetupTarget,
    ) -> Result<SetupCompletion> {
        let SetupOwner::Lease(lease) = owner else {
            return Err(CarrierFailure::Protocol);
        };
        let total = target
            .deadlines
            .map(OperationDeadlines::total)
            .ok_or(CarrierFailure::Protocol)?;
        self.settle_switch_entry_expired_parts(
            lease,
            &target.key,
            target.index,
            &target.provenance,
            total,
        )
    }

    fn settle_switch_entry_expired_parts(
        &self,
        lease: &CommandLease,
        target_key: &PaneRef,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
        total: Instant,
    ) -> Result<SetupCompletion> {
        if self.execution_desired(lease)?.as_ref() != Some(target_key.pane()) {
            return Ok(SetupCompletion::Superseded);
        }
        let reply = self.publish_and_wait(
            WorkerEvent::SwitchEntryExpired {
                key: target_key.clone(),
                target_index,
                target_provenance: target_provenance.clone(),
            },
            EventDeadline::TotalCleanup(total),
            total,
        )?;
        match reply {
            UiReply::SwitchEntryExpiredApplied(CleanupContinuation::Stable) => {
                Ok(SetupCompletion::StableWithoutViewer)
            }
            UiReply::SwitchEntryExpiredApplied(CleanupContinuation::Superseded { .. }) => {
                Ok(SetupCompletion::Superseded)
            }
            _ => Err(CarrierFailure::Protocol),
        }
    }

    fn run_create(
        &mut self,
        lease: &CommandLease,
        request: WorkbenchTabCreateRequest,
        operation: CreateOperation,
    ) -> Result<()> {
        let cutoff = self.effective_lease_cutoff(lease, operation.deadlines().forward_cutoff())?;
        self.ensure_before(cutoff)?;
        let submission = self.client.submit_workbench_tab_create(request, cutoff);
        match submission {
            WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(reason) => {
                if matches!(
                    reason,
                    WorkbenchTabCreateNotSubmitted::ConnectionUnavailable
                ) || self.client.generation().is_none()
                {
                    return Err(CarrierFailure::Transport);
                }
                self.restore_create_precommit(
                    lease,
                    &operation,
                    CreatePrecommitOutcome::NotSubmitted,
                )
            }
            WorkbenchTabCreateSubmission::DefinitePrecommit(outcome) => self
                .restore_create_precommit(
                    lease,
                    &operation,
                    CreatePrecommitOutcome::Returned(outcome),
                ),
            WorkbenchTabCreateSubmission::OutcomeUnknown { reason, .. } => {
                Err(tab_create_unknown(reason))
            }
            WorkbenchTabCreateSubmission::Committed(commit) => {
                if self.pending_create.is_some() {
                    return Err(CarrierFailure::Protocol);
                }
                self.pending_create =
                    Some(PendingCreateCommit::new(operation.clone(), commit.clone()));
                self.continue_committed_create(lease, operation, commit)
            }
        }
    }

    fn restore_create_precommit(
        &mut self,
        lease: &CommandLease,
        operation: &CreateOperation,
        outcome: CreatePrecommitOutcome,
    ) -> Result<()> {
        let applied_geometry = match operation.origin() {
            crate::workbench_state::CreateOriginKind::Ready => {
                let focus = self
                    .execution_desired(lease)?
                    .ok_or(CarrierFailure::Protocol)?;
                self.attachments
                    .iter()
                    .find(|attached| attached.key.pane() == &focus)
                    .ok_or(CarrierFailure::Protocol)?
                    .accepted_geometry
            }
            crate::workbench_state::CreateOriginKind::Unattached => None,
            crate::workbench_state::CreateOriginKind::Empty => {
                if !self.attachments.is_empty() {
                    return Err(CarrierFailure::Protocol);
                }
                None
            }
        };
        let cutoff = self.effective_lease_cutoff(lease, operation.deadlines().forward_cutoff())?;
        let reply = self.publish_and_wait(
            WorkerEvent::CreatePrecommit {
                operation: operation.key().clone(),
                outcome,
                applied_geometry,
            },
            EventDeadline::Forward(cutoff),
            cutoff,
        )?;
        if matches!(reply, UiReply::PrecommitRestored) {
            Ok(())
        } else {
            Err(CarrierFailure::Protocol)
        }
    }

    fn continue_committed_create(
        &mut self,
        lease: &CommandLease,
        operation: CreateOperation,
        commit: crate::local_client::workbench::WorkbenchTabCreateCommit,
    ) -> Result<()> {
        let total = operation.deadlines().total();
        let reply = self.publish_and_wait(
            WorkerEvent::CreateCommittedKnown {
                operation: operation.key().clone(),
                commit,
            },
            EventDeadline::TotalCleanup(total),
            total,
        )?;
        if !matches!(reply, UiReply::CommitRetained) {
            return Err(CarrierFailure::Protocol);
        }
        if self
            .pending_create
            .as_ref()
            .is_none_or(|pending| pending.operation() != &operation)
        {
            return Err(CarrierFailure::Protocol);
        }
        self.pending_create = None;

        let cutoff = self.effective_lease_cutoff(lease, operation.deadlines().forward_cutoff())?;
        self.ensure_before(cutoff)?;
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let outcome = self.read_workbench_before(request_id, cutoff)?;
        let cutoff = self.effective_lease_cutoff(lease, operation.deadlines().forward_cutoff())?;
        let wait = self
            .mailbox
            .publish_event_before(
                WorkerEvent::CreateSnapshotResult {
                    operation: operation.key().clone(),
                    outcome,
                },
                EventDeadline::PlacementOnly(cutoff),
                &self.clock,
            )
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply(&wait)
            .map_err(closed)?
            .ok_or(CarrierFailure::Protocol)?;
        match reply {
            UiReply::CreateProjection(CreateProjectionDecision::Cleanup(cleanup)) => {
                self.run_membership_cleanup(lease, cleanup)
            }
            UiReply::CreateProjection(CreateProjectionDecision::Attach(target)) => self
                .setup_target(
                    SetupTarget::operation(target)?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .map(|_completion| ()),
            UiReply::CreateProjection(
                CreateProjectionDecision::SettledWithoutAttachment
                | CreateProjectionDecision::ProjectionUnavailable,
            ) => Ok(()),
            UiReply::CreateProjection(CreateProjectionDecision::Retire) => {
                Err(CarrierFailure::AuthorityDivergence)
            }
            _ => Err(CarrierFailure::Protocol),
        }
    }

    fn run_refresh(
        &mut self,
        lease: &CommandLease,
        marker: crate::workbench_client::mailbox::RefreshMarker,
    ) -> Result<()> {
        if let Some(preparation) = marker.worktree_preparation().cloned() {
            return self.run_worktree_preparation(lease, &marker, preparation);
        }
        if let Some(operation) = marker.takeover().cloned() {
            return self.run_takeover(lease, marker, operation);
        }
        if let Some(operation) = marker.workspace_close().cloned() {
            return self.run_workspace_close(lease, marker, operation);
        }
        if self
            .mailbox
            .mark_refresh_in_flight(marker.generation())
            .map_err(closed)?
            != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        let snapshot_deadline = marker
            .context_deadlines()
            .map_or(marker.deadline(), OperationDeadlines::forward_cutoff);
        self.ensure_before(snapshot_deadline)?;
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let outcome = self.read_workbench_before(request_id, snapshot_deadline)?;
        let wait = if let Some(preparation) = marker.quick_preparation() {
            let cwd = self.prepare_quick_cwd(preparation, &outcome)?;
            self.mailbox
                .publish_event_before(
                    WorkerEvent::QuickWorkspacePrepared {
                        preparation: Box::new(preparation.clone()),
                        cwd,
                        outcome: Box::new(outcome),
                    },
                    EventDeadline::PlacementOnly(snapshot_deadline),
                    &self.clock,
                )
                .map_err(event)?
        } else {
            self.mailbox
                .publish_refresh_result_before(
                    marker.generation(),
                    outcome,
                    snapshot_deadline,
                    &self.clock,
                )
                .map_err(event)?
        };
        self.finish_refresh(lease, &marker, &wait)
    }

    fn finish_refresh(
        &mut self,
        lease: &CommandLease,
        marker: &crate::workbench_client::mailbox::RefreshMarker,
        wait: &crate::workbench_client::mailbox::UiWaitKey,
    ) -> Result<()> {
        let bounded_recovery = self.membership_recovery;
        let reply = if bounded_recovery || marker.context_deadlines().is_some() {
            self.mailbox
                .await_ui_reply_before(wait, marker.deadline(), &self.clock)
        } else {
            self.mailbox
                .await_ui_reply(wait)
                .map_err(EventError::Closed)
        }
        .map_err(event)?
        .ok_or(CarrierFailure::Protocol)?;
        let UiReply::Refresh(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        if self.membership_recovery {
            if matches!(
                settlement.decision(),
                RefreshDecision::Continue | RefreshDecision::Cleanup(_)
            ) {
                return Err(CarrierFailure::Protocol);
            }
            self.membership_recovery = false;
        }
        match settlement.decision() {
            RefreshDecision::Cleanup(cleanup) => {
                self.run_membership_cleanup(lease, cleanup.clone())
            }
            RefreshDecision::Continue | RefreshDecision::SettledWithoutAttachment => Ok(()),
            RefreshDecision::Retire => Err(CarrierFailure::AuthorityDivergence),
            RefreshDecision::AttachRecovered(target) => self
                .setup_target(
                    SetupTarget::operation(target.clone())?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .and_then(|completion| {
                    if bounded_recovery && !matches!(completion, SetupCompletion::Ready) {
                        Err(CarrierFailure::Transport)
                    } else {
                        Ok(())
                    }
                }),
        }
    }

    /// Reads the committed Workbench projection for `request_id` before `deadline`. With a retained
    /// baseline the read is a cursor-based `publication.read` folded into a single snapshot;
    /// without one it falls back to a full `snapshot.read`. Every accepted result replaces the
    /// baseline, and the wire-level `WorkbenchSnapshotOutcome` shape is preserved for callers.
    fn read_workbench_before(
        &mut self,
        request_id: orchbee_workbench_protocol::WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<WorkbenchSnapshotOutcome> {
        let Some(baseline) = self.workbench_baseline.clone() else {
            let result = self
                .client
                .submit_workbench_snapshot_read_before(
                    SnapshotReadIntent {
                        protocol_version: ProtocolVersion,
                        request_id,
                    },
                    deadline,
                )
                .map_err(snapshot)?;
            if let WorkbenchSnapshotOutcome::Available { snapshot: accepted } = &result.outcome {
                self.workbench_baseline = Some(accepted.clone());
            }
            return Ok(result.outcome);
        };
        let result = self
            .client
            .submit_workbench_publication_read_before(
                PublicationReadIntent {
                    protocol_version: ProtocolVersion,
                    request_id,
                    known_revision: baseline.revision,
                },
                deadline,
            )
            .map_err(publication)?;
        match result.outcome {
            WorkbenchPublicationOutcome::Current { .. } => {
                Ok(WorkbenchSnapshotOutcome::Available { snapshot: baseline })
            }
            WorkbenchPublicationOutcome::Published { publications } => {
                let accepted = apply_workbench_publication(&baseline, &publications)
                    .map_err(publication_fold_failure)?;
                self.workbench_baseline = Some(accepted.clone());
                Ok(WorkbenchSnapshotOutcome::Available { snapshot: accepted })
            }
            WorkbenchPublicationOutcome::Denied {} => Ok(WorkbenchSnapshotOutcome::Denied {}),
            WorkbenchPublicationOutcome::Unavailable { reason } => {
                Ok(WorkbenchSnapshotOutcome::Unavailable { reason })
            }
        }
    }

    /// Probes committed Workbench publications on top of an ordinary poll. A `published` outcome
    /// is folded into the retained baseline and reported as a hint; `current`, `denied` and
    /// `unavailable` stay silent so a transient read failure never disturbs terminal polling.
    fn probe_workbench_publication(&mut self, deadline: Instant) -> ReadyResult<bool> {
        let Some(baseline) = self.workbench_baseline.clone() else {
            return Ok(false);
        };
        let Some(subscription) = &self.publications else {
            return Ok(false);
        };
        match subscription.poll() {
            Ok(Some(crate::local_client::multiplex::StreamEvent::Record(_))) => {}
            Ok(Some(
                crate::local_client::multiplex::StreamEvent::Gap
                | crate::local_client::multiplex::StreamEvent::Disconnected,
            ))
            | Ok(None) => return Ok(false),
            Err(_) => return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol)),
        }
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(|error| ReadyFlowError::Carrier(closed(error)))?;
        let result = self
            .client
            .submit_workbench_publication_read_before(
                PublicationReadIntent {
                    protocol_version: ProtocolVersion,
                    request_id,
                    known_revision: baseline.revision,
                },
                deadline,
            )
            .map_err(|error| ReadyFlowError::Carrier(publication(error)))?;
        match result.outcome {
            WorkbenchPublicationOutcome::Published { publications } => {
                let accepted = apply_workbench_publication(&baseline, &publications)
                    .map_err(|reason| ReadyFlowError::Carrier(publication_fold_failure(reason)))?;
                self.workbench_baseline = Some(accepted);
                Ok(true)
            }
            WorkbenchPublicationOutcome::Current { .. }
            | WorkbenchPublicationOutcome::Denied {}
            | WorkbenchPublicationOutcome::Unavailable { .. } => Ok(false),
        }
    }

    fn publish_setup_configuration(&mut self, key: &PaneRef, cutoff: Instant) -> Result<()> {
        let (mode, applied_geometry) = self.attached_setup_facts(key)?;
        let reply = self.publish_and_wait(
            WorkerEvent::SetupConfigured {
                key: key.clone(),
                mode,
                applied_geometry,
            },
            EventDeadline::Forward(cutoff),
            cutoff,
        )?;
        match reply {
            UiReply::SetupConfigurationSettled(SetupEventDisposition::Applied) => Ok(()),
            UiReply::SetupConfigurationSettled(SetupEventDisposition::Superseded { .. }) => {
                Err(CarrierFailure::Deadline)
            }
            _ => Err(CarrierFailure::Protocol),
        }
    }

    fn publish_ready_setup_configuration(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
        cutoff: Instant,
    ) -> ReadyResult<()> {
        let (mode, applied_geometry) = self.attached_setup_facts(key)?;
        let reply = self.publish_and_wait_ready(
            lease,
            key,
            deadline,
            WorkerEvent::SetupConfigured {
                key: key.clone(),
                mode,
                applied_geometry,
            },
            cutoff,
        )?;
        match reply {
            UiReply::SetupConfigurationSettled(
                SetupEventDisposition::Applied | SetupEventDisposition::Superseded { .. },
            ) => Ok(()),
            _ => Err(ReadyFlowError::Carrier(CarrierFailure::Protocol)),
        }
    }

    fn publish_and_wait(
        &self,
        event_value: WorkerEvent,
        deadline: EventDeadline,
        cutoff: Instant,
    ) -> Result<UiReply> {
        let key = self
            .mailbox
            .publish_event_before(event_value, deadline, &self.clock)
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply_before(&key, cutoff, &self.clock)
            .map_err(event)?;
        match reply {
            Some(reply) => Ok(reply),
            None if self.clock.now() >= cutoff => Err(CarrierFailure::Deadline),
            None => Err(CarrierFailure::Protocol),
        }
    }

    fn publish_and_wait_ready(
        &self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
        event_value: WorkerEvent,
        cutoff: Instant,
    ) -> ReadyResult<UiReply> {
        let wait = self
            .mailbox
            .publish_event_before(event_value, EventDeadline::Forward(cutoff), &self.clock)
            .map_err(|error| self.ready_event_error(lease, key, deadline, error))?;
        let reply = self
            .mailbox
            .await_ui_reply_before(&wait, cutoff, &self.clock)
            .map_err(|error| self.ready_event_error(lease, key, deadline, error))?;
        match reply {
            Some(reply) => Ok(reply),
            None => Err(self.ready_missing_reply(lease, key, deadline)),
        }
    }

    fn settle_without_attachment(
        &self,
        target: &SetupTarget,
        cutoff: Instant,
    ) -> Result<SetupCompletion> {
        let reply = self.publish_and_wait(
            WorkerEvent::SetupWithoutAttachment {
                key: target.key.clone(),
                target_index: target.index,
                target_provenance: target.provenance.clone(),
            },
            EventDeadline::Forward(cutoff),
            cutoff,
        )?;
        match reply {
            UiReply::SetupWithoutAttachmentApplied(CleanupContinuation::Stable) => {
                Ok(SetupCompletion::StableWithoutViewer)
            }
            UiReply::SetupWithoutAttachmentApplied(CleanupContinuation::Superseded { .. }) => {
                Ok(SetupCompletion::Superseded)
            }
            _ => Err(CarrierFailure::Protocol),
        }
    }

    /// A definite attach refusal or carrier loss before any viewer existed:
    /// pane-local fault, published as a notice — never a session error.
    fn settle_attach_fault(
        &self,
        target: &SetupTarget,
        outcome: &TerminalViewerOutcome,
        cutoff: Instant,
    ) -> Result<SetupCompletion> {
        let fault = match outcome {
            TerminalViewerOutcome::Rejected { reason } => match reason {
                // The binding is occupied by another attachment — definite,
                // parked until the selection changes.
                ViewerRejectedReason::AlreadyAttached => PaneFault::CapacityExceeded,
                ViewerRejectedReason::TerminalUnavailable
                | ViewerRejectedReason::FeedbackMismatch
                | ViewerRejectedReason::HostRejected => PaneFault::AttachRejected,
            },
            TerminalViewerOutcome::Unavailable { .. } => PaneFault::AttachUnavailable,
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Empty {}
            | TerminalViewerOutcome::FeedbackAccepted {}
            | TerminalViewerOutcome::Detached {}
            | TerminalViewerOutcome::Record(_)
            | TerminalViewerOutcome::RowsRejected { .. } => {
                return Err(CarrierFailure::Protocol);
            }
        };
        self.mailbox
            .publish_notice_before(
                WorkerEvent::PaneFailed {
                    key: target.key.clone(),
                    fault,
                },
                cutoff,
                &self.clock,
            )
            .map_err(event)?;
        Ok(SetupCompletion::StableWithoutViewer)
    }

    fn cleanup_after_setup_failure(
        &mut self,
        owner: SetupOwner<'_>,
        target: &SetupTarget,
    ) -> Result<SetupCompletion> {
        let Some(deadlines) = target.deadlines else {
            return Err(CarrierFailure::Deadline);
        };
        if self.attached_ref(&target.key).is_err() {
            return Err(CarrierFailure::Deadline);
        }
        let outcome = self.viewer_for_cleanup(
            owner,
            TerminalViewerAction::Detach {
                attachment_id: self.viewer_id_for(&target.key)?,
            },
            deadlines.total(),
        )?;
        if !matches!(outcome, TerminalViewerOutcome::Detached {}) {
            return Err(CarrierFailure::Protocol);
        }
        self.remove_attachment(&target.key)?;
        if target.fault_on_failure {
            // The pane's attach pipeline died mid-way; the viewer is disposed
            // and the pane parks with a transient fault the next reconcile
            // may retry.
            self.mailbox
                .publish_notice_before(
                    WorkerEvent::PaneFailed {
                        key: target.key.clone(),
                        fault: PaneFault::ViewerLost,
                    },
                    deadlines.total(),
                    &self.clock,
                )
                .map_err(event)?;
            return Ok(SetupCompletion::StableWithoutViewer);
        }
        let wait = self
            .mailbox
            .publish_event_before(
                WorkerEvent::CleanupSettled {
                    key: target.key.clone(),
                    target_index: target.index,
                    target_provenance: target.provenance.clone(),
                },
                EventDeadline::TotalCleanup(deadlines.total()),
                &self.clock,
            )
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply_before(&wait, deadlines.total(), &self.clock)
            .map_err(event)?
            .ok_or(CarrierFailure::Protocol)?;
        match reply {
            UiReply::CleanupApplied(CleanupContinuation::Stable) => {
                Ok(SetupCompletion::StableWithoutViewer)
            }
            UiReply::CleanupApplied(CleanupContinuation::Superseded { .. }) => {
                Ok(SetupCompletion::Superseded)
            }
            _ => Err(CarrierFailure::Protocol),
        }
    }

    fn cleanup_startup_setup(&mut self, owner: SetupOwner<'_>, key: &PaneRef) -> Result<()> {
        let SetupOwner::Startup { cutoff } = owner else {
            return Err(CarrierFailure::Protocol);
        };
        if self.attached_ref(key).is_err() {
            return Err(CarrierFailure::Protocol);
        }
        let outcome = self.viewer_for_cleanup(
            owner,
            TerminalViewerAction::Detach {
                attachment_id: self.viewer_id_for(key)?,
            },
            cutoff,
        )?;
        if !matches!(outcome, TerminalViewerOutcome::Detached {}) {
            return Err(CarrierFailure::Protocol);
        }
        self.remove_attachment(key)?;
        Ok(())
    }

    fn viewer_for_initial_attach(
        &mut self,
        owner: SetupOwner<'_>,
        key: &PaneRef,
        action: TerminalViewerAction,
        cutoff: Instant,
        entry_expiry: SetupEntryExpiry,
    ) -> Result<InitialAttachResult> {
        let mut attempted = false;
        loop {
            if !self.setup_is_current(owner, key)? {
                return Ok(InitialAttachResult::Superseded);
            }
            if self.clock.now() >= cutoff {
                return if !attempted && matches!(entry_expiry, SetupEntryExpiry::SettleSwitch) {
                    Ok(InitialAttachResult::EntryExpired)
                } else {
                    Err(CarrierFailure::Deadline)
                };
            }
            let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
            let outcome = match self.client.submit_terminal_viewer_before(
                TerminalViewerRequest::new(request_id, action.clone()),
                cutoff,
            ) {
                Ok(result) => result.into_outcome(),
                Err(SubmissionError::RequestNotSubmitted(error)) if !attempted => {
                    if !self.setup_is_current(owner, key)? {
                        return Ok(InitialAttachResult::Superseded);
                    }
                    if matches!(entry_expiry, SetupEntryExpiry::SettleSwitch)
                        && error.kind() == std::io::ErrorKind::TimedOut
                        && self.clock.now() >= cutoff
                    {
                        return Ok(InitialAttachResult::EntryExpired);
                    }
                    return Err(submission(SubmissionError::RequestNotSubmitted(error)));
                }
                Err(error) => return Err(submission(error)),
            };
            attempted = true;
            if !matches!(
                outcome,
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy
                }
            ) {
                return Ok(InitialAttachResult::Outcome(outcome));
            }
            self.await_setup_retry(owner, key, cutoff)?;
        }
    }

    fn viewer_for_setup(
        &mut self,
        owner: SetupOwner<'_>,
        key: &PaneRef,
        action: TerminalViewerAction,
        cutoff: Instant,
    ) -> Result<Option<TerminalViewerOutcome>> {
        loop {
            if !self.setup_is_current(owner, key)? {
                return Ok(None);
            }
            self.ensure_before(cutoff)?;
            let outcome = self.viewer_once(action.clone(), cutoff)?;
            if !matches!(
                outcome,
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy
                }
            ) {
                return Ok(Some(outcome));
            }
            self.await_setup_retry(owner, key, cutoff)?;
        }
    }

    fn control_for_setup(
        &mut self,
        owner: SetupOwner<'_>,
        key: &PaneRef,
        action: TerminalControlAction,
        cutoff: Instant,
    ) -> Result<Option<TerminalControlOutcome>> {
        let mut action = action;
        loop {
            if !self.setup_is_current(owner, key)? {
                return Ok(None);
            }
            self.ensure_before(cutoff)?;
            let outcome = self.control_once(action.clone(), cutoff)?;
            if !matches!(
                outcome,
                TerminalControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::ServerBusy
                }
            ) {
                return Ok(Some(outcome));
            }
            self.await_setup_retry(owner, key, cutoff)?;
            if !self.setup_is_current(owner, key)? {
                return Ok(None);
            }
            action = self.refresh_control_sequence(key, action)?;
        }
    }

    fn resize_for_setup(
        &mut self,
        owner: SetupOwner<'_>,
        key: &PaneRef,
        controller_id: orchbee_client_server_protocol::ControllerId,
        geometry: Geometry,
        cutoff: Instant,
    ) -> Result<bool> {
        let action = TerminalControlAction::Resize {
            attachment_id: self.viewer_id_for(key)?,
            controller_id,
            sequence: self.next_control_sequence(key)?,
            geometry,
        };
        let Some(outcome) = self.control_for_setup(owner, key, action, cutoff)? else {
            return Ok(false);
        };
        match outcome {
            TerminalControlOutcome::Observed { .. } => return Err(CarrierFailure::Protocol),
            TerminalControlOutcome::Resized { geometry: actual } if actual == geometry => {}
            TerminalControlOutcome::Rejected {
                reason: ControlRejectedReason::HostRejected,
            }
            | TerminalControlOutcome::Unavailable {
                reason: ControlUnavailableReason::TerminalRuntimeUnavailable,
            } => {
                self.set_attached_mode(key, ControllerMode::ReadOnly)?;
                self.attached_mut(key)?.accepted_geometry = None;
                self.publish_setup_configuration(key, cutoff)?;
                return Ok(true);
            }
            TerminalControlOutcome::Rejected { .. }
            | TerminalControlOutcome::Unavailable { .. } => return Ok(false),
            TerminalControlOutcome::Acquired { .. }
            | TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Pasted { .. }
            | TerminalControlOutcome::HostColorsAccepted {}
            | TerminalControlOutcome::Cleared { .. }
            | TerminalControlOutcome::ResetAccepted { .. } => {
                return Err(CarrierFailure::Protocol);
            }
            TerminalControlOutcome::OutcomeUnknown {} => {
                return Err(CarrierFailure::OutcomeUnknown);
            }
        }
        if !self.setup_is_current(owner, key)? {
            return Ok(false);
        }
        self.set_accepted_geometry(key, geometry)?;
        self.publish_setup_configuration(key, cutoff)?;
        Ok(true)
    }

    fn control_for_lease(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        action: TerminalControlAction,
        deadline: Instant,
    ) -> ReadyResult<TerminalControlOutcome> {
        let mut action = action;
        loop {
            let cutoff = self.effective_lease_cutoff(lease, deadline)?;
            self.ensure_ready_before(lease, key, deadline, cutoff)?;
            let outcome = self.control_once_ready(lease, key, action.clone(), deadline, cutoff)?;
            if !matches!(
                outcome,
                TerminalControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::ServerBusy
                }
            ) {
                return Ok(outcome);
            }
            self.mailbox
                .await_execution_change_before(lease, cutoff, &self.clock)
                .map_err(closed)?;
            action = self.refresh_control_sequence(key, action)?;
        }
    }

    fn viewer_for_cleanup(
        &mut self,
        owner: SetupOwner<'_>,
        action: TerminalViewerAction,
        total: Instant,
    ) -> Result<TerminalViewerOutcome> {
        loop {
            self.ensure_before(total)?;
            let outcome = self.viewer_once(action.clone(), total)?;
            if !matches!(
                outcome,
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy
                }
            ) {
                return Ok(outcome);
            }
            match owner {
                SetupOwner::Startup { .. } => {
                    let key = self
                        .attachments
                        .first()
                        .map(|attached| attached.key.clone())
                        .ok_or(CarrierFailure::Protocol)?;
                    self.mailbox
                        .await_startup_change_before(&key, total, &self.clock)
                        .map_err(event)?;
                }
                SetupOwner::Lease(lease) => {
                    self.mailbox
                        .await_execution_change_before(lease, total, &self.clock)
                        .map_err(closed)?;
                }
                // Same yield contract as `await_setup_retry`: background
                // cleanup never sleeps on the lease. The half-disposed
                // attachment keeps `setup.is_some()`, so the requeued
                // reconcile picks it up as stale and retries the Detach.
                SetupOwner::Reconcile(_) => return Err(CarrierFailure::Deadline),
            }
        }
    }

    /// Viewer work for a live pane: admitted while the pane stays in the
    /// reconciled set — focus is irrelevant here, so background viewers keep
    /// polling and applying frames.
    fn viewer_for_ready_lease(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        action: TerminalViewerAction,
        deadline: Instant,
    ) -> ReadyResult<
        Option<(
            orchbee_client_server_protocol::RequestId,
            TerminalViewerOutcome,
        )>,
    > {
        loop {
            if !self.pane_admitted(lease, key)? {
                return Ok(None);
            }
            let cutoff = self.effective_lease_cutoff(lease, deadline)?;
            self.ensure_ready_before(lease, key, deadline, cutoff)?;
            let Some((request_id, outcome)) =
                self.viewer_once_ready(lease, key, action.clone(), deadline, cutoff)?
            else {
                return Ok(None);
            };
            if !matches!(
                outcome,
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy
                }
            ) {
                return Ok(Some((request_id, outcome)));
            }
            if !self.pane_admitted(lease, key)? {
                return Ok(None);
            }
            let fence = self
                .mailbox
                .await_execution_change_before(lease, cutoff, &self.clock)
                .map_err(closed)?;
            if !fence.lease_current() {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
            if !fence.desired().admits(key) {
                return Ok(None);
            }
        }
    }

    fn viewer_once_ready(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        action: TerminalViewerAction,
        deadline: Instant,
        cutoff: Instant,
    ) -> ReadyResult<
        Option<(
            orchbee_client_server_protocol::RequestId,
            TerminalViewerOutcome,
        )>,
    > {
        if action.attachment_id() != Some(&self.attached_ref(key)?.attachment_id) {
            return Err(CarrierFailure::Protocol.into());
        }
        let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        match self.client.submit_terminal_viewer_before(
            TerminalViewerRequest::new(request_id.clone(), action),
            cutoff,
        ) {
            Ok(result) => Ok(Some((request_id, result.into_outcome()))),
            Err(SubmissionError::RequestNotSubmitted(_)) if !self.pane_admitted(lease, key)? => {
                Ok(None)
            }
            Err(error) => Err(self.ready_submission_error(lease, key, deadline, error)),
        }
    }

    fn control_once_ready(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        action: TerminalControlAction,
        deadline: Instant,
        cutoff: Instant,
    ) -> ReadyResult<TerminalControlOutcome> {
        if action.attachment_id() != &self.attached_ref(key)?.attachment_id {
            return Err(CarrierFailure::Protocol.into());
        }
        let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        self.client
            .submit_terminal_control_before(TerminalControlRequest::new(request_id, action), cutoff)
            .map(orchbee_client_server_protocol::TerminalControlResult::into_outcome)
            .map_err(|error| self.ready_submission_error(lease, key, deadline, error))
    }

    fn ensure_ready_before(
        &self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
        cutoff: Instant,
    ) -> ReadyResult<()> {
        if self.clock.now() < cutoff {
            Ok(())
        } else {
            Err(self.ready_deadline_error(lease, key, deadline))
        }
    }

    fn ready_submission_error(
        &self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
        error: SubmissionError,
    ) -> ReadyFlowError {
        match error {
            SubmissionError::RequestNotSubmitted(error)
                if error.kind() == std::io::ErrorKind::TimedOut =>
            {
                self.ready_deadline_error(lease, key, deadline)
            }
            error => ReadyFlowError::Carrier(submission(error)),
        }
    }

    fn ready_event_error(
        &self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
        error: EventError,
    ) -> ReadyFlowError {
        match error {
            EventError::ForwardExpired => self.ready_deadline_error(lease, key, deadline),
            EventError::Closed(closed) => ReadyFlowError::Carrier(closed.failure()),
        }
    }

    fn ready_missing_reply(
        &self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
    ) -> ReadyFlowError {
        match self.effective_lease_cutoff(lease, deadline) {
            Ok(cutoff) if self.clock.now() >= cutoff => {
                self.ready_deadline_error(lease, key, deadline)
            }
            Ok(_) => ReadyFlowError::Carrier(CarrierFailure::Protocol),
            Err(failure) => ReadyFlowError::Carrier(failure),
        }
    }

    fn ready_deadline_error(
        &self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
    ) -> ReadyFlowError {
        if let Some(failure) = self.mailbox.is_closed() {
            return ReadyFlowError::Carrier(failure);
        }
        let fence = self.mailbox.execution_fence(lease);
        if !fence.lease_current() {
            return ReadyFlowError::Carrier(CarrierFailure::Protocol);
        }
        let switch_cutoff = fence.barrier().filter(|barrier| {
            barrier.kind() == BarrierKind::Switch && barrier.forward_cutoff() <= deadline
        });
        if self.attached_ref(key).is_ok()
            && switch_cutoff.is_some_and(|barrier| self.clock.now() >= barrier.forward_cutoff())
        {
            ReadyFlowError::ForwardDrainExpired
        } else {
            ReadyFlowError::Carrier(CarrierFailure::Deadline)
        }
    }

    fn viewer_once(
        &mut self,
        action: TerminalViewerAction,
        cutoff: Instant,
    ) -> Result<TerminalViewerOutcome> {
        if let Some(attachment_id) = action.attachment_id() {
            self.ensure_attachment_id(attachment_id)?;
        }
        let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        self.client
            .submit_terminal_viewer_before(TerminalViewerRequest::new(request_id, action), cutoff)
            .map(orchbee_client_server_protocol::TerminalViewerResult::into_outcome)
            .map_err(submission)
    }

    fn control_once(
        &mut self,
        action: TerminalControlAction,
        cutoff: Instant,
    ) -> Result<TerminalControlOutcome> {
        self.ensure_attachment_id(action.attachment_id())?;
        let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        self.client
            .submit_terminal_control_before(TerminalControlRequest::new(request_id, action), cutoff)
            .map(orchbee_client_server_protocol::TerminalControlResult::into_outcome)
            .map_err(submission)
    }

    /// Whether the mailbox still admits setup work for this exact pane
    /// incarnation: the focus pane (logical fencing — its incarnation is
    /// minted state-side) or any member of the latest reconcile snapshot.
    fn setup_is_current(&self, owner: SetupOwner<'_>, key: &PaneRef) -> Result<bool> {
        match owner {
            // Native admission remains closed throughout Starting. The startup-specific poll
            // allocator and Busy wait validate the exact desired key under the mailbox gate.
            SetupOwner::Startup { .. } => Ok(true),
            SetupOwner::Lease(lease) | SetupOwner::Reconcile(lease) => {
                let fence = self.mailbox.execution_fence(lease);
                if !fence.lease_current() {
                    return Err(CarrierFailure::Protocol);
                }
                Ok(fence.desired().admits(key))
            }
        }
    }

    fn await_setup_retry(
        &self,
        owner: SetupOwner<'_>,
        key: &PaneRef,
        cutoff: Instant,
    ) -> Result<()> {
        match owner {
            SetupOwner::Startup { .. } => {
                let desired = self
                    .mailbox
                    .await_startup_change_before(key, cutoff, &self.clock)
                    .map_err(event)?;
                if desired.key() != Some(key.pane()) {
                    return Err(CarrierFailure::Protocol);
                }
            }
            SetupOwner::Lease(lease) => {
                self.mailbox
                    .await_execution_change_before(lease, cutoff, &self.clock)
                    .map_err(closed)?;
            }
            // A reconcile-owned step is background work: its ServerBusy wait
            // must not pin the command lease, so the retry wait reports as a
            // step yield. `attach_reconcile_pane` turns it into a deferred
            // requeue instead of an in-lease sleep.
            SetupOwner::Reconcile(_) => return Err(CarrierFailure::Deadline),
        }
        Ok(())
    }

    fn allocate_setup_poll(&self, owner: SetupOwner<'_>, key: &PaneRef) -> Result<Option<PollKey>> {
        match owner {
            SetupOwner::Startup { .. } => self
                .mailbox
                .allocate_startup_poll_key(key)
                .map(Some)
                .map_err(closed),
            SetupOwner::Lease(lease) | SetupOwner::Reconcile(lease) => match self
                .mailbox
                .allocate_setup_poll_key(lease, key)
                .map_err(closed)?
            {
                SetupPollKey::Allocated(key) => Ok(Some(key)),
                SetupPollKey::Superseded(_) => Ok(None),
            },
        }
    }

    fn effective_lease_cutoff(&self, lease: &CommandLease, deadline: Instant) -> Result<Instant> {
        let fence = self.mailbox.execution_fence(lease);
        if !fence.lease_current() {
            return Err(CarrierFailure::Protocol);
        }
        Ok(fence
            .barrier()
            .map_or(deadline, |barrier| deadline.min(barrier.forward_cutoff())))
    }

    /// The mailbox's admitted-pane snapshot for `lease` — exact incarnation
    /// membership for background-pane work.
    fn pane_admitted(&self, lease: &CommandLease, key: &PaneRef) -> ReadyResult<bool> {
        let fence = self.mailbox.execution_fence(lease);
        if !fence.lease_current() {
            return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
        }
        Ok(fence.desired().admits(key))
    }

    /// The focus pane the mailbox admits, if any.
    fn execution_desired(&self, lease: &CommandLease) -> Result<Option<PaneKey>> {
        let fence = self.mailbox.execution_fence(lease);
        if !fence.lease_current() {
            return Err(CarrierFailure::Protocol);
        }
        Ok(fence.desired().key().cloned())
    }

    /// Whether this setup completion may publish the session's global
    /// `ReadyCandidate`: Startup and operation-Lease setups whose target
    /// still holds focus own the Ready phase. A Reconcile setup is background
    /// convergence — it completes its own record's chain and returns, never
    /// publishing the global candidate even when its target currently holds
    /// focus, so an already-Ready session is not pushed into a second Ready.
    fn owns_global_ready(&self, owner: SetupOwner<'_>, key: &PaneRef) -> Result<bool> {
        match owner {
            SetupOwner::Startup { .. } => Ok(true),
            SetupOwner::Reconcile(_) => Ok(false),
            SetupOwner::Lease(lease) => {
                Ok(self.execution_desired(lease)?.as_ref() == Some(key.pane()))
            }
        }
    }

    fn ensure_before(&self, cutoff: Instant) -> Result<()> {
        if self.clock.now() < cutoff {
            Ok(())
        } else {
            Err(CarrierFailure::Deadline)
        }
    }

    fn viewer_id_for(&self, key: &PaneRef) -> Result<orchbee_client_server_protocol::ViewerId> {
        Ok(self.attached_ref(key)?.attachment_id.clone())
    }

    fn ensure_attachment_id(
        &self,
        attachment_id: &orchbee_client_server_protocol::ViewerId,
    ) -> Result<()> {
        if !self
            .attachments
            .iter()
            .any(|attached| &attached.attachment_id == attachment_id)
        {
            return Err(CarrierFailure::Protocol);
        }
        Ok(())
    }

    fn set_attached_mode(&mut self, key: &PaneRef, mode: ControllerMode) -> Result<()> {
        let attached = self.attached_mut(key)?;
        if attached.mode.as_ref() != Some(&mode) {
            // Another controller may have installed its palette while this Client was read-only.
            attached.reported_host_colors = None;
        }
        attached.mode = Some(mode);
        Ok(())
    }

    fn set_accepted_geometry(&mut self, key: &PaneRef, geometry: Geometry) -> Result<()> {
        let attached = self.attached_mut(key)?;
        attached.accepted_geometry = Some(geometry);
        Ok(())
    }

    fn attached_setup_facts(&self, key: &PaneRef) -> Result<(ControllerMode, Option<Geometry>)> {
        let attached = self.attached_ref(key)?;
        let mode = attached.mode.clone().ok_or(CarrierFailure::Protocol)?;
        let geometry = match mode {
            ControllerMode::Controlling(_) => {
                Some(attached.accepted_geometry.ok_or(CarrierFailure::Protocol)?)
            }
            ControllerMode::ReadOnly => None,
        };
        Ok((mode, geometry))
    }

    /// The pane's live controller id, or `None` when the pane is degraded
    /// (view-only or not attached): focus-scoped commands to a degraded pane
    /// are dropped rather than retired the session.
    fn controller_for(
        &self,
        key: &PaneRef,
    ) -> Result<Option<orchbee_client_server_protocol::ControllerId>> {
        let Some(ControllerMode::Controlling(controller)) = self.attached_ref(key)?.mode.as_ref()
        else {
            return Ok(None);
        };
        Ok(Some(controller.clone()))
    }

    fn next_control_sequence(&mut self, key: &PaneRef) -> Result<NonZeroU64> {
        let attached = self.attached_mut(key)?;
        attached.next_sequence = attached
            .next_sequence
            .checked_add(1)
            .ok_or(CarrierFailure::IdentityExhausted)?;
        NonZeroU64::new(attached.next_sequence).ok_or(CarrierFailure::IdentityExhausted)
    }

    fn refresh_control_sequence(
        &mut self,
        key: &PaneRef,
        action: TerminalControlAction,
    ) -> Result<TerminalControlAction> {
        match action {
            TerminalControlAction::Input {
                attachment_id,
                controller_id,
                bytes,
                ..
            } => Ok(TerminalControlAction::Input {
                attachment_id,
                controller_id,
                sequence: self.next_control_sequence(key)?,
                bytes,
            }),
            TerminalControlAction::SemanticInput {
                attachment_id,
                controller_id,
                input_kind,
                payload,
                ..
            } => Ok(TerminalControlAction::SemanticInput {
                attachment_id,
                controller_id,
                sequence: self.next_control_sequence(key)?,
                input_kind,
                payload,
            }),
            TerminalControlAction::Resize {
                attachment_id,
                controller_id,
                geometry,
                ..
            } => Ok(TerminalControlAction::Resize {
                attachment_id,
                controller_id,
                sequence: self.next_control_sequence(key)?,
                geometry,
            }),
            TerminalControlAction::Paste {
                attachment_id,
                controller_id,
                content,
                ..
            } => Ok(TerminalControlAction::Paste {
                attachment_id,
                controller_id,
                sequence: self.next_control_sequence(key)?,
                content,
            }),
            TerminalControlAction::HostColors {
                attachment_id,
                controller_id,
                foreground,
                background,
                cursor,
                palette,
                ..
            } => Ok(TerminalControlAction::HostColors {
                attachment_id,
                controller_id,
                sequence: self.next_control_sequence(key)?,
                foreground,
                background,
                cursor,
                palette,
            }),
            action @ (TerminalControlAction::Observe { .. }
            | TerminalControlAction::Activate { .. }
            | TerminalControlAction::Acquire { .. }
            | TerminalControlAction::Takeover { .. }
            | TerminalControlAction::Clear { .. }
            | TerminalControlAction::Reset { .. }) => Ok(action),
        }
    }

    /// The next effective palette report for this controller incarnation, when it differs from
    /// the last report under the same grant.
    fn host_colors_action(
        &mut self,
        key: &PaneRef,
        controller_id: orchbee_client_server_protocol::ControllerId,
    ) -> Result<
        Option<(
            TerminalControlAction,
            crate::client_view::TerminalQueryColors,
        )>,
    > {
        let Some(colors) = self.mailbox.host_colors().map_err(closed)? else {
            return Ok(None);
        };
        if self.attached_ref(key)?.reported_host_colors == Some(colors) {
            return Ok(None);
        }
        let (Ok(foreground), Ok(background)) = (
            orchbee_client_server_protocol::TerminalRgb::new(colors.foreground),
            orchbee_client_server_protocol::TerminalRgb::new(colors.background),
        ) else {
            return Ok(None);
        };
        Ok(Some((
            TerminalControlAction::HostColors {
                attachment_id: self.viewer_id_for(key)?,
                controller_id,
                sequence: self.next_control_sequence(key)?,
                foreground,
                background,
                cursor: colors.cursor.map(|color| {
                    orchbee_client_server_protocol::TerminalRgb::new(color)
                        .expect("catalog and observed query colors are RGB")
                }),
                palette: colors.palette.map(|color| {
                    color.map(|color| {
                        orchbee_client_server_protocol::TerminalRgb::new(color)
                            .expect("catalog and observed query colors are RGB")
                    })
                }),
            },
            colors,
        )))
    }

    /// Best effort: only an unknown outcome is a carrier fact. Any settled outcome marks the
    /// colors reported so a refused report is not retried on every command.
    fn report_host_colors_for_setup(
        &mut self,
        owner: SetupOwner<'_>,
        key: &PaneRef,
        controller_id: orchbee_client_server_protocol::ControllerId,
        cutoff: Instant,
    ) -> Result<()> {
        let Some((action, colors)) = self.host_colors_action(key, controller_id)? else {
            return Ok(());
        };
        match self.control_for_setup(owner, key, action, cutoff)? {
            Some(TerminalControlOutcome::Observed { .. }) => Err(CarrierFailure::Protocol),
            Some(TerminalControlOutcome::OutcomeUnknown {}) => Err(CarrierFailure::OutcomeUnknown),
            Some(
                TerminalControlOutcome::Acquired { .. }
                | TerminalControlOutcome::InputAccepted {}
                | TerminalControlOutcome::Resized { .. }
                | TerminalControlOutcome::Pasted { .. }
                | TerminalControlOutcome::HostColorsAccepted {}
                | TerminalControlOutcome::Cleared { .. }
                | TerminalControlOutcome::ResetAccepted { .. }
                | TerminalControlOutcome::Rejected { .. }
                | TerminalControlOutcome::Unavailable { .. },
            ) => {
                self.attached_mut(key)?.reported_host_colors = Some(colors);
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Covers an outer-terminal probe that settled after this pane acquired control.
    fn sync_host_colors_for_lease(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        controller_id: orchbee_client_server_protocol::ControllerId,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let Some((action, colors)) = self.host_colors_action(key, controller_id)? else {
            return Ok(());
        };
        match self.control_for_lease(lease, key, action, deadline)? {
            TerminalControlOutcome::Observed { .. } => Err(CarrierFailure::Protocol.into()),
            TerminalControlOutcome::OutcomeUnknown {} => {
                Err(ReadyFlowError::Carrier(CarrierFailure::OutcomeUnknown))
            }
            TerminalControlOutcome::Acquired { .. }
            | TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Pasted { .. }
            | TerminalControlOutcome::HostColorsAccepted {}
            | TerminalControlOutcome::Cleared { .. }
            | TerminalControlOutcome::ResetAccepted { .. }
            | TerminalControlOutcome::Rejected { .. }
            | TerminalControlOutcome::Unavailable { .. } => {
                if let Ok(attached) = self.attached_mut(key) {
                    attached.reported_host_colors = Some(colors);
                }
                Ok(())
            }
        }
    }

    /// The pane's controller grant ended or was refused: the viewer stays
    /// live but the pane is parked view-only. `ControlLost` is a definite
    /// fact — no automatic re-acquire — so the local mode is pinned ReadOnly
    /// and the notice parks the record.
    fn settle_pane_control_lost(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
    ) -> ReadyResult<()> {
        if let Ok(attached) = self.attached_mut(key) {
            attached.mode = Some(ControllerMode::ReadOnly);
            attached.accepted_geometry = None;
        }
        self.publish_pane_fault(lease, key, PaneFault::ControlLost, deadline)
    }

    /// The pane's viewer carrier is gone: local removal plus a `ViewerLost`
    /// notice parks the pane until an explicit selection re-arms it — the dead
    /// carrier makes any wire detach meaningless, so disposal is canonical
    /// here. The focus pane is the session's terminal: its loss retires the
    /// session exactly like a protocol failure.
    fn settle_pane_viewer_lost(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
    ) -> ReadyResult<()> {
        if self.execution_desired(lease)?.as_ref() == Some(key.pane()) {
            return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
        }
        self.dispose_lost_viewer(lease, key, deadline)
    }

    /// Only a typed runtime-loss outcome reaches this path for the focus.
    /// State-side settlement preserves structural owners by refusing to park
    /// anything other than a quiescent Ready focus.
    fn dispose_lost_viewer(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
    ) -> ReadyResult<()> {
        self.execution_desired(lease)?;
        if self.attached_ref(key).is_ok() {
            self.remove_attachment(key)?;
        }
        self.publish_pane_fault(lease, key, PaneFault::ViewerLost, deadline)
    }

    fn publish_pane_fault(
        &self,
        lease: &CommandLease,
        key: &PaneRef,
        fault: PaneFault,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.mailbox
            .publish_notice_before(
                WorkerEvent::PaneFailed {
                    key: key.clone(),
                    fault,
                },
                cutoff,
                &self.clock,
            )
            .map_err(|error| self.ready_event_error(lease, key, deadline, error))?;
        Ok(())
    }

    /// One reconcile execution performs at most one pane-progress action —
    /// a single stale disposal or a single target setup — then returns the
    /// lease and requeues the remaining plan under the marker's unchanged
    /// absolute deadline. The plan is computed once (the first execution
    /// diffs live attachments against the snapshot) and travels with the
    /// marker: a step that cannot finish rotates to the back so a busy or
    /// refused first target never starves the steps behind it, and queued
    /// live work always runs after at most one bounded background action.
    /// Reaching the shared deadline ends the episode: the marker is dropped
    /// without requeue and the state side settles the expiry as the terminal
    /// fact — no fresh budget is minted here.
    fn run_reconcile(&mut self, lease: &CommandLease, marker: &ReconcileMarker) -> Result<()> {
        let desired = self.execution_desired(lease)?;
        if desired.as_ref().map(PaneKey::anchor) != Some(marker.anchor()) {
            return Ok(());
        }
        if self.clock.now() >= marker.deadlines().total() {
            return Ok(());
        }
        let mut marker = marker.clone();
        if marker.pending().is_none() {
            let plan = self.plan_reconcile_steps(&marker);
            marker.set_pending(plan);
        }
        let mut pending = marker.take_pending().unwrap_or_default();
        let Some(step) = pending.pop_front() else {
            return Ok(());
        };
        let progress = self.run_reconcile_step(lease, &marker, &step, desired.as_ref())?;
        let deferred = match progress {
            ReconcileProgress::Done => false,
            ReconcileProgress::Retry => {
                pending.push_back(step);
                true
            }
            ReconcileProgress::Expired => return Ok(()),
        };
        if pending.is_empty() {
            return Ok(());
        }
        marker.set_pending(pending);
        let retry_at = if deferred {
            Instant::now() + RECONCILE_BUSY_RETRY
        } else {
            Instant::now()
        };
        match self.mailbox.requeue_reconcile(marker, retry_at) {
            // A full queue keeps the pending marker in state — the next
            // observation re-derives and re-admits it — and a superseded
            // anchor means a newer reconcile already owns the diff.
            Ok(()) | Err(AdmissionError::Full | AdmissionError::StaleAttachment) => {}
            Err(AdmissionError::Closed(closed)) => return Err(closed.failure()),
            Err(_) => return Err(CarrierFailure::Protocol),
        }
        Ok(())
    }

    /// Plans the reconcile once from the live attachment view: every held
    /// attachment the snapshot does not name — plus a named target whose
    /// setup was abandoned mid-way (`setup.is_some()`), which is disposed and
    /// re-attached rather than resumed — becomes a `Detach` step, followed by
    /// an `Attach` step for every target not already fully installed.
    fn plan_reconcile_steps(&self, marker: &ReconcileMarker) -> VecDeque<ReconcileStep> {
        let mut steps: VecDeque<ReconcileStep> = self
            .attachments
            .iter()
            .filter(|attached| attached.key.anchor() == marker.anchor())
            .filter_map(|attached| {
                let targeted = marker
                    .targets()
                    .iter()
                    .any(|target| target.pane_ref() == &attached.key);
                if targeted && attached.setup.is_none() {
                    return None;
                }
                Some(ReconcileStep::Detach {
                    key: attached.key.clone(),
                    announced: attached.announced,
                })
            })
            .collect();
        for (index, target) in marker.targets().iter().enumerate() {
            let converged = self
                .attachments
                .iter()
                .any(|attached| &attached.key == target.pane_ref() && attached.setup.is_none());
            if !converged {
                steps.push_back(ReconcileStep::Attach(index));
            }
        }
        steps
    }

    /// Executes one planned step. Every step re-validates against live
    /// attachments so interleaved changes between executions stay correct:
    /// an evaporated disposal or an already-converged target is consumed
    /// without a wire action; an attach blocked by a half-installed zombie
    /// rotates behind the disposal step that still owns it; and the `focus`
    /// pane is never attached here (see the Attach arm).
    fn run_reconcile_step(
        &mut self,
        lease: &CommandLease,
        marker: &ReconcileMarker,
        step: &ReconcileStep,
        focus: Option<&PaneKey>,
    ) -> Result<ReconcileProgress> {
        let total = marker.deadlines().total();
        match step {
            ReconcileStep::Detach { key, announced } => {
                if self.attachments.iter().all(|held| &held.key != key) {
                    return Ok(ReconcileProgress::Done);
                }
                match self.detach_pane(key, total) {
                    Ok(true) => {}
                    Ok(false) => return Ok(ReconcileProgress::Retry),
                    Err(CarrierFailure::Deadline) => {
                        return Ok(self.reconcile_deadline_progress(total));
                    }
                    Err(failure) => return Err(failure),
                }
                // The disposal notice follows the *newest* snapshot
                // membership, not this marker's targets: a pane re-admitted
                // by a fresher reconcile is live, so its disposal is a
                // `ViewerLost` the record survives — `PaneDetached` on a
                // non-departing record is a protocol violation. A pane the
                // newest snapshot dropped is departing and retired by the
                // canonical proof. A half-setup the record never learned is
                // silent either way.
                let fence = self.mailbox.execution_fence(lease);
                if !fence.lease_current() {
                    return Ok(ReconcileProgress::Done);
                }
                match (fence.desired().contains(key), *announced) {
                    (true, true) => {
                        self.mailbox
                            .publish_notice_before(
                                WorkerEvent::PaneFailed {
                                    key: key.clone(),
                                    fault: PaneFault::ViewerLost,
                                },
                                total,
                                &self.clock,
                            )
                            .map_err(event)?;
                    }
                    (true, false) => {}
                    (false, _) => self.publish_pane_detached(key, total)?,
                }
                Ok(ReconcileProgress::Done)
            }
            ReconcileStep::Attach(index) => {
                let Some(target) = marker.targets().get(*index) else {
                    return Ok(ReconcileProgress::Done);
                };
                if let Some(attached) = self
                    .attachments
                    .iter()
                    .find(|held| &held.key == target.pane_ref())
                {
                    return if attached.setup.is_none() {
                        Ok(ReconcileProgress::Done)
                    } else {
                        // A half-installed zombie still occupies the pane:
                        // its Detach step owns the disposal — rotate.
                        Ok(ReconcileProgress::Retry)
                    };
                }
                // Only its own setup — startup, a switch or a refresh
                // recovery — attaches the focus pane; the Client state never
                // installs a reconcile-attached focus. An unheld focus is
                // parked, typically on the viewer loss this executor just
                // disposed, and this snapshot predates the UI settling it. An
                // attach here would retire the Client: an attachment the state
                // cannot install at once, or a refusal later, because it
                // replaces the parked loss that membership repair requires
                // when the Panel's removal lands.
                if focus == Some(target.pane_ref().pane()) {
                    return Ok(ReconcileProgress::Done);
                }
                match self.attach_reconcile_pane(lease, marker, target) {
                    Ok(()) => Ok(ReconcileProgress::Done),
                    Err(CarrierFailure::Deadline) => Ok(self.reconcile_deadline_progress(total)),
                    Err(failure) => Err(failure),
                }
            }
        }
    }

    /// `Deadline` from a bounded step is overloaded: before the shared total
    /// it is a `ServerBusy` yield or a submission that timed out unsent —
    /// the step rotates and retries deferred; at or past the total it is the
    /// episode's real expiry — terminal, no requeue, no fresh budget.
    fn reconcile_deadline_progress(&self, total: Instant) -> ReconcileProgress {
        if self.clock.now() >= total {
            ReconcileProgress::Expired
        } else {
            ReconcileProgress::Retry
        }
    }

    /// One bounded disposal step for one pane attachment: `Detached` is the
    /// only canonical proof — the wire protocol already answers `Detached`
    /// for a previously disposed viewer, so `Rejected` means the request was
    /// genuinely refused and `Unavailable` means the server never decided.
    /// Neither releases local state: the attachment survives and the caller
    /// marks the reconcile for a deferred retry.
    fn detach_pane(&mut self, key: &PaneRef, total: Instant) -> Result<bool> {
        let viewer_id = self.viewer_id_for(key)?;
        let outcome = self.viewer_once(
            TerminalViewerAction::Detach {
                attachment_id: viewer_id,
            },
            total,
        )?;
        match outcome {
            TerminalViewerOutcome::Detached {} => {}
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                return Ok(false);
            }
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Empty {}
            | TerminalViewerOutcome::FeedbackAccepted {}
            | TerminalViewerOutcome::Record(_)
            | TerminalViewerOutcome::RowsRejected { .. } => {
                return Err(CarrierFailure::Protocol);
            }
        }
        self.remove_attachment(key)?;
        Ok(true)
    }

    /// Publishes the canonical disposal proof for one pane incarnation — the
    /// fire-and-forget notice that retires the departing `PaneRecord`.
    fn publish_pane_detached(&self, key: &PaneRef, cutoff: Instant) -> Result<()> {
        self.mailbox
            .publish_notice_before(
                WorkerEvent::PaneDetached { key: key.clone() },
                cutoff,
                &self.clock,
            )
            .map_err(event)?;
        Ok(())
    }

    /// Attaches one reconcile target through the ordinary setup pipeline —
    /// one bounded step under the shared deadline. A `ServerBusy` wait yields
    /// the lease instead of sleeping on it: it surfaces as `Deadline` and the
    /// caller decides whether that is a pre-total retry or the episode's real
    /// expiry. Definite attach failures (`Rejected`, non-`ServerBusy`
    /// `Unavailable`) are pane-local faults reported by notice; unknown or
    /// submitted-but-uncertain outcomes stay carrier failures.
    fn attach_reconcile_pane(
        &mut self,
        lease: &CommandLease,
        marker: &ReconcileMarker,
        target: &ReconcileTarget,
    ) -> Result<()> {
        let setup = SetupTarget::reconcile(target, marker.deadlines())?;
        self.setup_target(
            setup,
            SetupOwner::Reconcile(lease),
            SetupEntryExpiry::Retire,
        )
        .map(|_| ())
    }

    fn attached_ref(&self, key: &PaneRef) -> Result<&AttachedTerminal> {
        self.attachments
            .iter()
            .find(|attached| &attached.key == key)
            .ok_or(CarrierFailure::Protocol)
    }

    fn attached_mut(&mut self, key: &PaneRef) -> Result<&mut AttachedTerminal> {
        self.attachments
            .iter_mut()
            .find(|attached| &attached.key == key)
            .ok_or(CarrierFailure::Protocol)
    }

    /// Removes the attachment slot for `key`; canonical worker-side disposal
    /// — the caller publishes `PaneDetached`/`PaneFailed` as the proof.
    fn remove_attachment(&mut self, key: &PaneRef) -> Result<AttachedTerminal> {
        let position = self
            .attachments
            .iter()
            .position(|attached| &attached.key == key)
            .ok_or(CarrierFailure::Protocol)?;
        Ok(self.attachments.remove(position))
    }
}

trait BootstrapReply {
    fn into_bootstrap(self) -> Result<Option<BootstrapTarget>>;
}

impl BootstrapReply for UiReply {
    fn into_bootstrap(self) -> Result<Option<BootstrapTarget>> {
        match self {
            Self::BootstrapApplied(target) => Ok(target),
            _ => Err(CarrierFailure::Protocol),
        }
    }
}

fn settle_ready_flow(result: ReadyResult<()>) -> Result<()> {
    match result {
        Ok(()) | Err(ReadyFlowError::ForwardDrainExpired) => Ok(()),
        Err(ReadyFlowError::Carrier(failure)) => Err(failure),
    }
}

fn closed(error: MailboxClosed) -> CarrierFailure {
    error.failure()
}

fn event(error: EventError) -> CarrierFailure {
    error.closed_failure().unwrap_or(CarrierFailure::Deadline)
}

fn submission(error: SubmissionError) -> CarrierFailure {
    match error {
        SubmissionError::ConnectionUnavailable => CarrierFailure::Transport,
        SubmissionError::RequestEncoding(_) => CarrierFailure::Protocol,
        SubmissionError::RequestNotSubmitted(error) => io_failure(&error),
        SubmissionError::OutcomeUnknown { .. } => CarrierFailure::OutcomeUnknown,
    }
}

fn snapshot(error: SnapshotReadError) -> CarrierFailure {
    match error {
        SnapshotReadError::ConnectionUnavailable => CarrierFailure::Transport,
        SnapshotReadError::RequestEncoding(_) => CarrierFailure::Protocol,
        SnapshotReadError::RequestNotSubmitted(error) => io_failure(&error),
        SnapshotReadError::Failed { reason, .. } => match reason {
            SnapshotReadFailure::Response(orchbee_workbench_protocol::WorkbenchWireError::Io(
                error,
            )) => io_failure(&error),
            SnapshotReadFailure::Response(_)
            | SnapshotReadFailure::UnexpectedResponse
            | SnapshotReadFailure::CorrelationMismatch { .. } => CarrierFailure::Protocol,
            SnapshotReadFailure::Write(error) | SnapshotReadFailure::ResponseDeadline(error) => {
                io_failure(&error)
            }
        },
    }
}

fn publication(error: PublicationReadError) -> CarrierFailure {
    match error {
        PublicationReadError::ConnectionUnavailable => CarrierFailure::Transport,
        PublicationReadError::RequestEncoding(_) => CarrierFailure::Protocol,
        PublicationReadError::RequestNotSubmitted(error) => io_failure(&error),
        PublicationReadError::Failed { reason, .. } => match reason {
            PublicationReadFailure::Response(
                orchbee_workbench_protocol::WorkbenchWireError::Io(error),
            ) => io_failure(&error),
            PublicationReadFailure::Response(_)
            | PublicationReadFailure::UnexpectedResponse
            | PublicationReadFailure::CorrelationMismatch { .. } => CarrierFailure::Protocol,
            PublicationReadFailure::Write(error)
            | PublicationReadFailure::ResponseDeadline(error) => io_failure(&error),
        },
    }
}

/// A publication that cannot fold onto the retained baseline either contradicts the committed
/// history the baseline came from (authority divergence) or is malformed on the wire (protocol).
fn publication_fold_failure(reason: DeltaRejectionReason) -> CarrierFailure {
    match reason {
        DeltaRejectionReason::SessionMismatch
        | DeltaRejectionReason::BaseRevisionMismatch
        | DeltaRejectionReason::NonIncreasingRevision => CarrierFailure::AuthorityDivergence,
        DeltaRejectionReason::InvalidProjection | DeltaRejectionReason::DuplicateChangeTarget => {
            CarrierFailure::Protocol
        }
    }
}

fn tab_create_unknown(_error: WorkbenchTabCreateOutcomeUnknownReason) -> CarrierFailure {
    CarrierFailure::OutcomeUnknown
}

fn io_failure(error: &std::io::Error) -> CarrierFailure {
    if error.kind() == std::io::ErrorKind::TimedOut {
        CarrierFailure::Deadline
    } else {
        CarrierFailure::Transport
    }
}

#[cfg(test)]
mod tests;
