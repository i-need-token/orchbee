use super::{NativeWorkerExit, attach_binding};
use crate::{
    local_client::{
        LocalClient, LocalOperationClock, SubmissionError,
        workbench::{
            SnapshotReadError, SnapshotReadFailure, WorkbenchTabCreateNotSubmitted,
            WorkbenchTabCreateOutcomeUnknownReason, WorkbenchTabCreateRequest,
            WorkbenchTabCreateSubmission,
        },
    },
    terminal_projection::TerminalProjection,
    terminal_session::{
        mailbox::{
            BarrierKind, CausalSettlement, CommandLease, CommandMailbox, CreatePrecommitOutcome,
            EventDeadline, EventError, MailboxClosed, MutationResult, PollKey, QueuedCommand,
            SetupPollKey, Tagged, UiReply, WorkerEvent,
        },
        state::{
            AttachTarget, AttachmentKey, BootstrapTarget, CarrierFailure, CleanupContinuation,
            ControllerMode, CreateOperation, CreateProjectionDecision, CreateTerminalProvenance,
            FrameIdentity, OperationDeadlines, PendingCloseResult, PendingCreateCommit,
            PendingMoveCommit, ReadyDecision, RefreshDecision, SetupEventDisposition,
        },
        workbench_projection::project_native_standalone_tabs,
    },
};
use orchbee_client_server_protocol::{
    ControlRejectedReason, ControlUnavailableReason, Geometry, OpenOutcome, TerminalControlAction,
    TerminalControlOutcome, TerminalControlRequest, TerminalOpenRequest, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerRequest, ViewerUnavailableReason,
};
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use orchbee_workbench_protocol::{ProtocolVersion, SnapshotReadIntent, WorkbenchSnapshotOutcome};
use std::{num::NonZeroU64, sync::Arc, time::Instant};

mod close_reconciliation;
mod membership;
#[cfg(target_os = "linux")]
mod status_recovery;
mod switch_membership;
mod tab_close;
#[cfg(target_os = "linux")]
pub(in crate::terminal_session) use status_recovery::{RecoveryShutdown, run_status_recovery};
mod tab_move;
mod tab_rename;

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

pub(super) fn run(
    client: LocalClient,
    initial_geometry: Geometry,
    mailbox: Arc<CommandMailbox>,
) -> NativeWorkerExit {
    let mut exit = WorkerExit::new(Arc::clone(&mailbox));
    let mut executor = NativeExecutor::new(client, mailbox);
    let failure = executor.run(initial_geometry).err();
    if let Some(failure) = failure {
        exit.failure = failure;
    }
    let returned = NativeWorkerExit::new(
        executor.pending_create.take(),
        executor.pending_move.take(),
        executor.pending_close.take(),
        executor.pending_rename.take(),
    );
    #[cfg(target_os = "linux")]
    let returned = returned.with_unavailable_poll(executor.unavailable_poll.take());
    returned
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

struct NativeExecutor {
    status_only: bool,
    #[cfg(target_os = "linux")]
    membership_recovery: bool,
    #[cfg(target_os = "linux")]
    unavailable_poll: Option<crate::terminal_session::state::retired_membership::UnavailablePoll>,
    client: LocalClient,
    clock: LocalOperationClock,
    mailbox: Arc<CommandMailbox>,
    attached: Option<AttachedTerminal>,
    pending_create: Option<PendingCreateCommit>,
    pending_move: Option<PendingMoveCommit>,
    pending_close: Option<PendingCloseResult>,
    pending_rename: Option<crate::terminal_session::state::tab_rename::RenameWorkerEvidence>,
}

#[derive(Clone)]
struct SetupTarget {
    key: AttachmentKey,
    index: usize,
    provenance: CreateTerminalProvenance,
    attach: TerminalViewerAction,
    deadlines: Option<OperationDeadlines>,
}

impl SetupTarget {
    fn bootstrap(target: BootstrapTarget) -> Result<Self> {
        Ok(Self {
            key: target.selected().key().clone(),
            index: target.selected().index(),
            provenance: CreateTerminalProvenance::BindingOnly,
            attach: attach_binding(target.site_id(), target.binding())
                .map_err(|_status| CarrierFailure::Protocol)?,
            deadlines: None,
        })
    }

    fn operation(target: AttachTarget) -> Result<Self> {
        Ok(Self {
            key: target.selected().key().clone(),
            index: target.selected().index(),
            provenance: target.provenance().clone(),
            attach: attach_binding(target.site_id(), target.binding())
                .map_err(|_status| CarrierFailure::Protocol)?,
            deadlines: Some(target.deadlines()),
        })
    }
}

struct AttachedTerminal {
    key: AttachmentKey,
    setup: Option<SetupTarget>,
    mode: Option<ControllerMode>,
    next_sequence: u64,
    accepted_geometry: Option<Geometry>,
}

#[derive(Clone, Copy)]
enum SetupOwner<'lease> {
    Startup { cutoff: Instant },
    Lease(&'lease CommandLease),
}

impl SetupOwner<'_> {
    fn forward_cutoff(self, target: &SetupTarget) -> Result<Instant> {
        match self {
            Self::Startup { cutoff } => Ok(cutoff),
            Self::Lease(_) => target
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

impl NativeExecutor {
    fn new(client: LocalClient, mailbox: Arc<CommandMailbox>) -> Self {
        let clock = client.operation_clock();
        Self {
            client,
            status_only: false,
            #[cfg(target_os = "linux")]
            membership_recovery: false,
            #[cfg(target_os = "linux")]
            unavailable_poll: None,
            clock,
            mailbox,
            attached: None,
            pending_create: None,
            pending_move: None,
            pending_close: None,
            pending_rename: None,
        }
    }

    fn run(&mut self, initial_geometry: Geometry) -> Result<()> {
        let startup_deadline = self
            .client
            .submission_deadline()
            .ok_or(CarrierFailure::Protocol)?;
        let bootstrap = self.bootstrap(initial_geometry, startup_deadline)?;
        let target = SetupTarget::bootstrap(bootstrap)?;
        if !matches!(
            self.setup_target(
                target,
                SetupOwner::Startup {
                    cutoff: startup_deadline,
                },
                SetupEntryExpiry::Retire,
            )?,
            SetupCompletion::Ready
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
    ) -> Result<BootstrapTarget> {
        self.ensure_before(startup_deadline)?;
        let open_request = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        let open = self
            .client
            .submit_terminal_open(TerminalOpenRequest::new(open_request, initial_geometry))
            .map_err(submission)?;
        match open.into_outcome() {
            OpenOutcome::Opened { .. } => {}
            OpenOutcome::OutcomeUnknown {} => return Err(CarrierFailure::OutcomeUnknown),
            OpenOutcome::Unavailable { .. } | OpenOutcome::Failed { .. } => {
                return Err(CarrierFailure::Transport);
            }
        }

        self.ensure_before(startup_deadline)?;
        let snapshot_request = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let snapshot = self
            .client
            .submit_workbench_snapshot_read_before(
                SnapshotReadIntent {
                    protocol_version: ProtocolVersion,
                    request_id: snapshot_request,
                },
                startup_deadline,
            )
            .map_err(snapshot)?;
        let WorkbenchSnapshotOutcome::Available { snapshot } = snapshot.outcome else {
            return Err(CarrierFailure::Transport);
        };
        let cache = project_native_standalone_tabs(&snapshot)
            .map_err(|_error| CarrierFailure::AuthorityDivergence)?;
        if cache.tabs().is_empty() {
            return Err(CarrierFailure::AuthorityDivergence);
        }
        self.publish_and_wait(
            WorkerEvent::Bootstrap { cache },
            EventDeadline::Startup(startup_deadline),
            startup_deadline,
        )?
        .into_bootstrap()
    }

    fn setup_target(
        &mut self,
        target: SetupTarget,
        owner: SetupOwner<'_>,
        entry_expiry: SetupEntryExpiry,
    ) -> Result<SetupCompletion> {
        if self.attached.is_some() {
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
        } = outcome
        else {
            return match outcome {
                TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {
                    if matches!(owner, SetupOwner::Startup { .. }) {
                        Err(CarrierFailure::Transport)
                    } else if self.setup_is_current(owner, &target.key)? {
                        self.settle_without_attachment(&target, forward_cutoff)
                    } else {
                        Ok(SetupCompletion::Superseded)
                    }
                }
                TerminalViewerOutcome::Empty {}
                | TerminalViewerOutcome::FeedbackAccepted {}
                | TerminalViewerOutcome::Detached {}
                | TerminalViewerOutcome::Record(_) => Err(CarrierFailure::Protocol),
                TerminalViewerOutcome::Attached { .. } => unreachable!("matched attached outcome"),
            };
        };
        self.attached = Some(AttachedTerminal {
            key: target.key.clone(),
            setup: Some(target.clone()),
            mode: None,
            next_sequence: 0,
            accepted_geometry: None,
        });
        let setup = self.finish_attached_setup(
            &target,
            owner,
            forward_cutoff,
            attachment_id.as_str(),
            host_epoch.as_str(),
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
            Err(_failure) => self.cleanup_after_setup_failure(owner),
        }
    }

    fn finish_attached_setup(
        &mut self,
        target: &SetupTarget,
        owner: SetupOwner<'_>,
        forward_cutoff: Instant,
        attachment_id: &str,
        host_epoch: &str,
    ) -> Result<SetupCompletion> {
        let projection = TerminalProjection::new(
            AttachmentId::parse(attachment_id).map_err(|_error| CarrierFailure::Protocol)?,
            HostEpoch::parse(host_epoch).map_err(|_error| CarrierFailure::Protocol)?,
        );
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
        if !self.setup_is_current(owner, &target.key)? {
            return Err(CarrierFailure::Deadline);
        }

        let acquire = self
            .control_for_setup(
                owner,
                &target.key,
                TerminalControlAction::Acquire {},
                forward_cutoff,
            )?
            .ok_or(CarrierFailure::Deadline)?;
        let mode = match acquire {
            TerminalControlOutcome::Acquired { controller_id } => {
                ControllerMode::Controlling(controller_id)
            }
            TerminalControlOutcome::Rejected {
                reason: ControlRejectedReason::ControllerBusy,
            } => ControllerMode::ReadOnly,
            TerminalControlOutcome::Rejected { .. }
            | TerminalControlOutcome::Unavailable { .. } => {
                return Err(CarrierFailure::Deadline);
            }
            TerminalControlOutcome::InputAccepted {} | TerminalControlOutcome::Resized { .. } => {
                return Err(CarrierFailure::Protocol);
            }
            TerminalControlOutcome::OutcomeUnknown {} => {
                return Err(CarrierFailure::OutcomeUnknown);
            }
        };
        if !self.setup_is_current(owner, &target.key)? {
            return Err(CarrierFailure::Deadline);
        }
        self.set_attached_mode(&target.key, mode.clone())?;
        if let ControllerMode::Controlling(controller_id) = &mode {
            let geometry = self.mailbox.latest_geometry().map_err(closed)?;
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
                    TerminalViewerAction::Poll {},
                    forward_cutoff,
                )?
                .ok_or(CarrierFailure::Deadline)?;
            let record = match outcome {
                TerminalViewerOutcome::Empty {} => None,
                TerminalViewerOutcome::Record(record) => Some(record),
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::FeedbackAccepted {}
                | TerminalViewerOutcome::Detached {} => {
                    return Err(CarrierFailure::Protocol);
                }
                TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {
                    return Err(CarrierFailure::Deadline);
                }
            };
            self.mailbox
                .publish_poll_result_before(poll_key.clone(), record, forward_cutoff, &self.clock)
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
                | TerminalViewerOutcome::Record(_) => {
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
            #[cfg(target_os = "linux")]
            if self.membership_recovery && !matches!(lease.record(), QueuedCommand::Refresh(_)) {
                return Err(CarrierFailure::Protocol);
            }
            if self.status_only
                && !matches!(lease.record(), QueuedCommand::ReconcileClose(marker) if marker.membership_recovery())
                && !matches!(lease.record(), QueuedCommand::Close(marker)
                if matches!(marker.operation().kind(), crate::terminal_session::state::CloseOperationKind::Read { .. }
                    | crate::terminal_session::state::CloseOperationKind::Resume { .. }))
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
            if let Some((request, operation)) = create {
                self.run_create(&lease, request, operation)?;
            } else {
                match lease.record() {
                    QueuedCommand::Input(tagged) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_input(&lease, tagged, deadline))?;
                    }
                    QueuedCommand::Resize(tagged) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_resize(&lease, tagged, deadline))?;
                    }
                    QueuedCommand::Poll(_) => {
                        let deadline = self.client.next_operation_deadline();
                        settle_ready_flow(self.run_poll(&lease, deadline))?;
                    }
                    QueuedCommand::Switch(marker) => {
                        self.run_switch(&lease, marker.target().clone())?
                    }
                    QueuedCommand::Create(_) => return Err(CarrierFailure::Protocol),
                    QueuedCommand::Refresh(marker) => self.run_refresh(&lease, *marker)?,
                    QueuedCommand::Move(marker) => {
                        self.run_move(&lease, marker.operation().clone())?
                    }
                    QueuedCommand::Rename(marker) => {
                        self.run_rename(&lease, marker.operation().clone())?
                    }
                    QueuedCommand::Close(marker) => {
                        self.run_close(&lease, marker.operation().clone())?
                    }
                    QueuedCommand::ReconcileClose(marker) => {
                        self.run_close_reconciliation(&lease, marker)?
                    }
                }
            }
            self.mailbox.finish_lease(lease).map_err(closed)?;
        }
    }

    fn run_input(
        &mut self,
        lease: &CommandLease,
        tagged: &Tagged<orchbee_client_server_protocol::ControlInput>,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let (key, input) = (tagged.key(), tagged.value().clone());
        let desired = self.execution_desired(lease)?;
        if self.attached_key() != Some(key) && desired.as_ref() != Some(key) {
            return Ok(());
        }
        let controller = self.controller_for(key)?;
        let sequence = self.next_control_sequence(key)?;
        let outcome = self.control_for_lease(
            lease,
            key,
            TerminalControlAction::Input {
                controller_id: controller,
                sequence,
                bytes: input,
            },
            deadline,
        )?;
        match outcome {
            TerminalControlOutcome::InputAccepted {} => Ok(()),
            TerminalControlOutcome::OutcomeUnknown {} => {
                Err(ReadyFlowError::Carrier(CarrierFailure::OutcomeUnknown))
            }
            TerminalControlOutcome::Acquired { .. }
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Rejected { .. }
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
        let desired = self.execution_desired(lease)?;
        if self.attached_key() != Some(&key) && desired.as_ref() != Some(&key) {
            return Ok(());
        }
        let controller = self.controller_for(&key)?;
        let sequence = self.next_control_sequence(&key)?;
        let outcome = self.control_for_lease(
            lease,
            &key,
            TerminalControlAction::Resize {
                controller_id: controller,
                sequence,
                geometry,
            },
            deadline,
        )?;
        match outcome {
            TerminalControlOutcome::Resized { geometry: actual } if actual == geometry => {}
            TerminalControlOutcome::OutcomeUnknown {} => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::OutcomeUnknown));
            }
            TerminalControlOutcome::Acquired { .. }
            | TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Rejected { .. }
            | TerminalControlOutcome::Unavailable { .. } => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
        }
        self.set_accepted_geometry(&key, geometry)?;
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.publish_ready_setup_configuration(lease, &key, deadline, cutoff)
    }

    fn run_poll(&mut self, lease: &CommandLease, deadline: Instant) -> ReadyResult<()> {
        let poll_key = lease.poll_key().ok_or(CarrierFailure::Protocol)?;
        let key = poll_key.attachment().clone();
        if self.attached_key() != Some(&key) || self.execution_desired(lease)? != Some(key.clone())
        {
            return Ok(());
        }
        let Some((request_id, outcome)) =
            self.viewer_for_ready_lease(lease, &key, TerminalViewerAction::Poll {}, deadline)?
        else {
            return Ok(());
        };
        let current = self.execution_desired(lease)? == Some(key.clone());
        #[cfg(target_os = "linux")]
        if current
            && matches!(
                outcome,
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::TerminalRuntimeUnavailable
                }
            )
        {
            self.unavailable_poll = Some(
                crate::terminal_session::state::retired_membership::UnavailablePoll {
                    request_id,
                    poll: poll_key.clone(),
                },
            );
        }
        #[cfg(not(target_os = "linux"))]
        let _request_id = request_id;
        let record = match outcome {
            TerminalViewerOutcome::Empty {} => None,
            TerminalViewerOutcome::Record(record) => Some(record),
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. }
                if !current =>
            {
                return Ok(());
            }
            TerminalViewerOutcome::Rejected { .. }
            | TerminalViewerOutcome::Unavailable { .. }
            | TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::FeedbackAccepted {}
            | TerminalViewerOutcome::Detached {} => {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
        };
        // Publish even a now-stale Record/Empty result so the mailbox retains its exact discard.
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.mailbox
            .publish_poll_result_before(poll_key.clone(), record, cutoff, &self.clock)
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
        if self.execution_desired(lease)? != Some(key.clone()) {
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
        let current = self.execution_desired(lease)? == Some(key.clone());
        match result {
            TerminalViewerOutcome::FeedbackAccepted {} => {}
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. }
                if !current =>
            {
                self.mailbox.discard_taken_switch_causal(lease, &poll_key)?;
                return Ok(());
            }
            TerminalViewerOutcome::Rejected { .. }
            | TerminalViewerOutcome::Unavailable { .. }
            | TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Detached {}
            | TerminalViewerOutcome::Empty {}
            | TerminalViewerOutcome::Record(_) => {
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
        if self.execution_desired(lease)? != Some(target_key.clone()) {
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
            target.selected().key(),
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
        target_key: &AttachmentKey,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
        total: Instant,
    ) -> Result<SetupCompletion> {
        if self.execution_desired(lease)?.as_ref() != Some(target_key) {
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
                if matches!(reason, WorkbenchTabCreateNotSubmitted::SessionUnavailable)
                    || self.client.generation().is_none()
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
        let applied_geometry = match (operation.origin(), self.attached.as_ref()) {
            (crate::terminal_session::state::CreateOriginKind::Ready, Some(attached)) => {
                attached.accepted_geometry
            }
            (crate::terminal_session::state::CreateOriginKind::Empty, None) => None,
            _ => return Err(CarrierFailure::Protocol),
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
        let result = self
            .client
            .submit_workbench_snapshot_read_before(
                SnapshotReadIntent {
                    protocol_version: ProtocolVersion,
                    request_id,
                },
                cutoff,
            )
            .map_err(snapshot)?;
        let cutoff = self.effective_lease_cutoff(lease, operation.deadlines().forward_cutoff())?;
        let wait = self
            .mailbox
            .publish_event_before(
                WorkerEvent::CreateSnapshotResult {
                    operation: operation.key().clone(),
                    outcome: result.outcome,
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
        marker: crate::terminal_session::mailbox::RefreshMarker,
    ) -> Result<()> {
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
        let result = self
            .client
            .submit_workbench_snapshot_read_before(
                SnapshotReadIntent {
                    protocol_version: ProtocolVersion,
                    request_id,
                },
                snapshot_deadline,
            )
            .map_err(snapshot)?;
        let wait = self
            .mailbox
            .publish_refresh_result_before(
                marker.generation(),
                result.outcome,
                snapshot_deadline,
                &self.clock,
            )
            .map_err(event)?;
        #[cfg(target_os = "linux")]
        let bounded_recovery = self.membership_recovery;
        #[cfg(not(target_os = "linux"))]
        let bounded_recovery = false;
        let reply = if bounded_recovery || marker.context_deadlines().is_some() {
            self.mailbox
                .await_ui_reply_before(&wait, marker.deadline(), &self.clock)
        } else {
            self.mailbox
                .await_ui_reply(&wait)
                .map_err(EventError::Closed)
        }
        .map_err(event)?
        .ok_or(CarrierFailure::Protocol)?;
        let UiReply::Refresh(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        #[cfg(target_os = "linux")]
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

    fn publish_setup_configuration(&mut self, key: &AttachmentKey, cutoff: Instant) -> Result<()> {
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
        key: &AttachmentKey,
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
        key: &AttachmentKey,
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

    fn cleanup_after_setup_failure(&mut self, owner: SetupOwner<'_>) -> Result<SetupCompletion> {
        let Some(attached) = self.attached.as_ref() else {
            return Err(CarrierFailure::Deadline);
        };
        let target = attached.setup.clone().ok_or(CarrierFailure::Protocol)?;
        let Some(deadlines) = target.deadlines else {
            return Err(CarrierFailure::Deadline);
        };
        let outcome =
            self.viewer_for_cleanup(owner, TerminalViewerAction::Detach {}, deadlines.total())?;
        if !matches!(outcome, TerminalViewerOutcome::Detached {}) {
            return Err(CarrierFailure::Protocol);
        }
        self.attached = None;
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

    fn cleanup_startup_setup(&mut self, owner: SetupOwner<'_>, key: &AttachmentKey) -> Result<()> {
        let SetupOwner::Startup { cutoff } = owner else {
            return Err(CarrierFailure::Protocol);
        };
        if self.attached_key() != Some(key) {
            return Err(CarrierFailure::Protocol);
        }
        let outcome = self.viewer_for_cleanup(owner, TerminalViewerAction::Detach {}, cutoff)?;
        if !matches!(outcome, TerminalViewerOutcome::Detached {}) {
            return Err(CarrierFailure::Protocol);
        }
        self.attached = None;
        Ok(())
    }

    fn viewer_for_initial_attach(
        &mut self,
        owner: SetupOwner<'_>,
        key: &AttachmentKey,
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
        key: &AttachmentKey,
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
        key: &AttachmentKey,
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
        key: &AttachmentKey,
        controller_id: orchbee_client_server_protocol::ControllerId,
        geometry: Geometry,
        cutoff: Instant,
    ) -> Result<bool> {
        let action = TerminalControlAction::Resize {
            controller_id,
            sequence: self.next_control_sequence(key)?,
            geometry,
        };
        let Some(outcome) = self.control_for_setup(owner, key, action, cutoff)? else {
            return Ok(false);
        };
        match outcome {
            TerminalControlOutcome::Resized { geometry: actual } if actual == geometry => {}
            TerminalControlOutcome::Rejected { .. }
            | TerminalControlOutcome::Unavailable { .. } => return Ok(false),
            TerminalControlOutcome::Acquired { .. }
            | TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. } => {
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
        key: &AttachmentKey,
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
                        .attached_key()
                        .cloned()
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
            }
        }
    }

    fn viewer_for_ready_lease(
        &mut self,
        lease: &CommandLease,
        key: &AttachmentKey,
        action: TerminalViewerAction,
        deadline: Instant,
    ) -> ReadyResult<
        Option<(
            orchbee_client_server_protocol::RequestId,
            TerminalViewerOutcome,
        )>,
    > {
        loop {
            if self.execution_desired(lease)?.as_ref() != Some(key) {
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
            if self.execution_desired(lease)?.as_ref() != Some(key) {
                return Ok(None);
            }
            let fence = self
                .mailbox
                .await_execution_change_before(lease, cutoff, &self.clock)
                .map_err(closed)?;
            if !fence.lease_current() {
                return Err(ReadyFlowError::Carrier(CarrierFailure::Protocol));
            }
            if fence.desired().key() != Some(key) {
                return Ok(None);
            }
        }
    }

    fn viewer_once_ready(
        &mut self,
        lease: &CommandLease,
        key: &AttachmentKey,
        action: TerminalViewerAction,
        deadline: Instant,
        cutoff: Instant,
    ) -> ReadyResult<
        Option<(
            orchbee_client_server_protocol::RequestId,
            TerminalViewerOutcome,
        )>,
    > {
        let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        match self.client.submit_terminal_viewer_before(
            TerminalViewerRequest::new(request_id.clone(), action),
            cutoff,
        ) {
            Ok(result) => Ok(Some((request_id, result.into_outcome()))),
            Err(SubmissionError::RequestNotSubmitted(_))
                if self.execution_desired(lease)?.as_ref() != Some(key) =>
            {
                Ok(None)
            }
            Err(error) => Err(self.ready_submission_error(lease, key, deadline, error)),
        }
    }

    fn control_once_ready(
        &mut self,
        lease: &CommandLease,
        key: &AttachmentKey,
        action: TerminalControlAction,
        deadline: Instant,
        cutoff: Instant,
    ) -> ReadyResult<TerminalControlOutcome> {
        let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        self.client
            .submit_terminal_control_before(TerminalControlRequest::new(request_id, action), cutoff)
            .map(orchbee_client_server_protocol::TerminalControlResult::into_outcome)
            .map_err(|error| self.ready_submission_error(lease, key, deadline, error))
    }

    fn ensure_ready_before(
        &self,
        lease: &CommandLease,
        key: &AttachmentKey,
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
        key: &AttachmentKey,
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
        key: &AttachmentKey,
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
        key: &AttachmentKey,
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
        key: &AttachmentKey,
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
        if self.attached_key() == Some(key)
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
        let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        self.client
            .submit_terminal_control_before(TerminalControlRequest::new(request_id, action), cutoff)
            .map(orchbee_client_server_protocol::TerminalControlResult::into_outcome)
            .map_err(submission)
    }

    fn setup_is_current(&self, owner: SetupOwner<'_>, key: &AttachmentKey) -> Result<bool> {
        match owner {
            // Native admission remains closed throughout Starting. The startup-specific poll
            // allocator and Busy wait validate the exact desired key under the mailbox gate.
            SetupOwner::Startup { .. } => Ok(true),
            SetupOwner::Lease(lease) => {
                let fence = self.mailbox.execution_fence(lease);
                if !fence.lease_current() {
                    return Err(CarrierFailure::Protocol);
                }
                Ok(fence.desired().key() == Some(key))
            }
        }
    }

    fn await_setup_retry(
        &self,
        owner: SetupOwner<'_>,
        key: &AttachmentKey,
        cutoff: Instant,
    ) -> Result<()> {
        match owner {
            SetupOwner::Startup { .. } => {
                let desired = self
                    .mailbox
                    .await_startup_change_before(key, cutoff, &self.clock)
                    .map_err(event)?;
                if desired.key() != Some(key) {
                    return Err(CarrierFailure::Protocol);
                }
            }
            SetupOwner::Lease(lease) => {
                self.mailbox
                    .await_execution_change_before(lease, cutoff, &self.clock)
                    .map_err(closed)?;
            }
        }
        Ok(())
    }

    fn allocate_setup_poll(
        &self,
        owner: SetupOwner<'_>,
        key: &AttachmentKey,
    ) -> Result<Option<PollKey>> {
        match owner {
            SetupOwner::Startup { .. } => self
                .mailbox
                .allocate_startup_poll_key(key)
                .map(Some)
                .map_err(closed),
            SetupOwner::Lease(lease) => match self
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

    fn execution_desired(&self, lease: &CommandLease) -> Result<Option<AttachmentKey>> {
        let fence = self.mailbox.execution_fence(lease);
        if !fence.lease_current() {
            return Err(CarrierFailure::Protocol);
        }
        Ok(fence.desired().key().cloned())
    }

    fn ensure_before(&self, cutoff: Instant) -> Result<()> {
        if self.clock.now() < cutoff {
            Ok(())
        } else {
            Err(CarrierFailure::Deadline)
        }
    }

    fn attached_key(&self) -> Option<&AttachmentKey> {
        self.attached.as_ref().map(|attached| &attached.key)
    }

    fn set_attached_mode(&mut self, key: &AttachmentKey, mode: ControllerMode) -> Result<()> {
        let attached = self.attached_mut(key)?;
        attached.mode = Some(mode);
        Ok(())
    }

    fn set_accepted_geometry(&mut self, key: &AttachmentKey, geometry: Geometry) -> Result<()> {
        let attached = self.attached_mut(key)?;
        attached.accepted_geometry = Some(geometry);
        Ok(())
    }

    fn attached_setup_facts(
        &self,
        key: &AttachmentKey,
    ) -> Result<(ControllerMode, Option<Geometry>)> {
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

    fn controller_for(
        &self,
        key: &AttachmentKey,
    ) -> Result<orchbee_client_server_protocol::ControllerId> {
        let ControllerMode::Controlling(controller) = self
            .attached_ref(key)?
            .mode
            .as_ref()
            .ok_or(CarrierFailure::Protocol)?
        else {
            return Err(CarrierFailure::Protocol);
        };
        Ok(controller.clone())
    }

    fn next_control_sequence(&mut self, key: &AttachmentKey) -> Result<NonZeroU64> {
        let attached = self.attached_mut(key)?;
        attached.next_sequence = attached
            .next_sequence
            .checked_add(1)
            .ok_or(CarrierFailure::IdentityExhausted)?;
        NonZeroU64::new(attached.next_sequence).ok_or(CarrierFailure::IdentityExhausted)
    }

    fn refresh_control_sequence(
        &mut self,
        key: &AttachmentKey,
        action: TerminalControlAction,
    ) -> Result<TerminalControlAction> {
        match action {
            TerminalControlAction::Input {
                controller_id,
                bytes,
                ..
            } => Ok(TerminalControlAction::Input {
                controller_id,
                sequence: self.next_control_sequence(key)?,
                bytes,
            }),
            TerminalControlAction::Resize {
                controller_id,
                geometry,
                ..
            } => Ok(TerminalControlAction::Resize {
                controller_id,
                sequence: self.next_control_sequence(key)?,
                geometry,
            }),
            TerminalControlAction::Acquire {} => Ok(TerminalControlAction::Acquire {}),
        }
    }

    fn attached_ref(&self, key: &AttachmentKey) -> Result<&AttachedTerminal> {
        self.attached
            .as_ref()
            .filter(|attached| &attached.key == key)
            .ok_or(CarrierFailure::Protocol)
    }

    fn attached_mut(&mut self, key: &AttachmentKey) -> Result<&mut AttachedTerminal> {
        self.attached
            .as_mut()
            .filter(|attached| &attached.key == key)
            .ok_or(CarrierFailure::Protocol)
    }
}

trait BootstrapReply {
    fn into_bootstrap(self) -> Result<BootstrapTarget>;
}

impl BootstrapReply for UiReply {
    fn into_bootstrap(self) -> Result<BootstrapTarget> {
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
        SubmissionError::SessionUnavailable => CarrierFailure::Transport,
        SubmissionError::RequestEncoding(_) => CarrierFailure::Protocol,
        SubmissionError::RequestNotSubmitted(error) => io_failure(&error),
        SubmissionError::OutcomeUnknown { .. } => CarrierFailure::OutcomeUnknown,
    }
}

fn snapshot(error: SnapshotReadError) -> CarrierFailure {
    match error {
        SnapshotReadError::SessionUnavailable => CarrierFailure::Transport,
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
