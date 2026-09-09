use super::{
    state::{
        AttachTarget, AttachmentKey, BootstrapTarget, CarrierFailure, CleanupContinuation,
        CloseFlightKey, CloseLeaseAcknowledgement, CloseOperation, CloseReconciliationMarker,
        CloseResult, ControllerMode, CreateOperation, CreateOperationKey, CreateProjectionDecision,
        CreateTerminalProvenance, FrameIdentity, MembershipCleanup, MembershipResultKey,
        MoveDecision, MoveOperation, MoveResult, OperationDeadlines, PendingRefreshStage,
        ReadyDecision, RefreshDecision, SetupEventDisposition,
    },
    workbench_projection::NativeContextTabs,
};
use crate::{
    local_client::{
        LocalOperationClock,
        workbench::{
            WorkbenchTabCreateCommit, WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest,
        },
    },
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{ControlInput, Geometry, RequestId, TerminalViewerAction};
use orchbee_workbench_protocol::{WorkbenchRequestId, WorkbenchSnapshotOutcome};
use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex, MutexGuard, TryLockError},
    time::{Duration, Instant},
};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

const COMMAND_CAPACITY: usize = 8;
const CONDITION_WAIT_SLICE: Duration = Duration::from_millis(10);

mod tab_close;
mod workspace_setup;
use super::state::workspace_setup::{SetupOperation, SetupResult};
use workspace_setup::SetupFlight;
pub(super) use workspace_setup::WorkspaceSetupMarker;
mod tab_rename;
use super::state::tab_rename::{RenameOperation, RenameResult};
use tab_rename::RenameFlight;
pub(super) use tab_rename::{RenameMarker, RenameSettlement};
mod tab_close_reconciliation;
use tab_close::CloseFlight;
#[cfg(target_os = "linux")]
pub(super) use tab_close::RetiredMailboxSeed;
pub(super) use tab_close::{CloseAdmissionError, CloseMarker};
use tab_close_reconciliation::CloseReconciliationFlight;
pub(super) use tab_close_reconciliation::CloseReconciliationSettlement;
mod membership;
pub(super) use membership::DesiredInstall;
#[cfg(test)]
pub(super) use membership::PreparedDesiredInstall;
mod switch_membership;
pub(super) use switch_membership::{SwitchAdmissionFacts, SwitchExpiryPermit};
use switch_membership::{SwitchAnchorInvalidation, SwitchHandoffRecord};

trait MailboxClock {
    fn now(&self) -> Instant;

    #[cfg(test)]
    fn after_mailbox_contention(&self) {}
}

impl MailboxClock for LocalOperationClock {
    fn now(&self) -> Instant {
        self.now()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Tagged<Value> {
    key: AttachmentKey,
    value: Value,
}

impl<Value> Tagged<Value> {
    pub(super) const fn new(key: AttachmentKey, value: Value) -> Self {
        Self { key, value }
    }

    pub(super) const fn key(&self) -> &AttachmentKey {
        &self.key
    }

    pub(super) const fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SwitchMarker {
    target: AttachTarget,
}

impl SwitchMarker {
    pub(super) const fn new(target: AttachTarget) -> Self {
        Self { target }
    }

    pub(super) const fn target(&self) -> &AttachTarget {
        &self.target
    }

    fn barrier(&self) -> BarrierFence {
        BarrierFence::from_deadlines(BarrierKind::Switch, self.target.deadlines())
    }
}

pub(super) struct CreateMarker {
    request: Option<WorkbenchTabCreateRequest>,
    operation: CreateOperation,
}

impl CreateMarker {
    pub(super) const fn new(
        request: WorkbenchTabCreateRequest,
        operation: CreateOperation,
    ) -> Self {
        Self {
            request: Some(request),
            operation,
        }
    }

    pub(super) fn take_request(&mut self) -> Option<WorkbenchTabCreateRequest> {
        self.request.take()
    }

    pub(super) const fn operation(&self) -> &CreateOperation {
        &self.operation
    }

    fn barrier(&self) -> BarrierFence {
        BarrierFence::from_deadlines(BarrierKind::Create, self.operation.deadlines())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RefreshMarker {
    generation: u64,
    deadline: Instant,
    context_deadlines: Option<OperationDeadlines>,
}

impl RefreshMarker {
    pub(super) const fn new(generation: u64, deadline: Instant) -> Self {
        Self {
            generation,
            deadline,
            context_deadlines: None,
        }
    }

    pub(super) const fn context_deadlines(self) -> Option<OperationDeadlines> {
        self.context_deadlines
    }

    pub(super) const fn generation(self) -> u64 {
        self.generation
    }

    pub(super) const fn deadline(self) -> Instant {
        self.deadline
    }

    const fn barrier(self) -> BarrierFence {
        BarrierFence {
            kind: BarrierKind::Refresh,
            forward_cutoff: self.deadline,
            total: self.deadline,
        }
    }
}

pub(super) enum QueuedCommand {
    Input(Tagged<ControlInput>),
    Resize(Tagged<Geometry>),
    Poll(Tagged<()>),
    Switch(SwitchMarker),
    Create(CreateMarker),
    Refresh(RefreshMarker),
    Move(MoveMarker),
    Rename(RenameMarker),
    WorkspaceSetup(WorkspaceSetupMarker),
    Close(CloseMarker),
    ReconcileClose(CloseReconciliationMarker),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MoveMarker {
    operation: MoveOperation,
}

impl MoveMarker {
    pub(super) const fn new(operation: MoveOperation) -> Self {
        Self { operation }
    }
    pub(super) const fn operation(&self) -> &MoveOperation {
        &self.operation
    }
    fn barrier(&self) -> BarrierFence {
        BarrierFence {
            kind: BarrierKind::Move,
            forward_cutoff: self.operation.deadline(),
            total: self.operation.deadline(),
        }
    }
}

impl QueuedCommand {
    fn barrier(&self) -> Option<BarrierFence> {
        match self {
            Self::Switch(marker) => Some(marker.barrier()),
            Self::Create(marker) => Some(marker.barrier()),
            Self::Refresh(marker) => Some(marker.barrier()),
            Self::Move(marker) => Some(marker.barrier()),
            Self::Rename(marker) => Some(marker.barrier()),
            Self::WorkspaceSetup(marker) => Some(marker.barrier()),
            Self::Close(marker) => Some(marker.barrier()),
            Self::ReconcileClose(marker) => Some(BarrierFence {
                kind: BarrierKind::Close,
                forward_cutoff: marker.deadline(),
                total: marker.deadline(),
            }),
            Self::Input(_) | Self::Resize(_) | Self::Poll(_) => None,
        }
    }
}

struct QueuedRecord {
    serial: u64,
    command: QueuedCommand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BarrierKind {
    WorkspaceSetup,
    Switch,
    Create,
    Refresh,
    Move,
    Rename,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BarrierFence {
    kind: BarrierKind,
    forward_cutoff: Instant,
    total: Instant,
}

impl BarrierFence {
    const fn from_deadlines(kind: BarrierKind, deadlines: OperationDeadlines) -> Self {
        Self {
            kind,
            forward_cutoff: deadlines.forward_cutoff(),
            total: deadlines.total(),
        }
    }

    pub(super) const fn kind(self) -> BarrierKind {
        self.kind
    }

    pub(super) const fn forward_cutoff(self) -> Instant {
        self.forward_cutoff
    }

    #[cfg(test)]
    pub(super) const fn total(self) -> Instant {
        self.total
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DesiredFence {
    key: Option<AttachmentKey>,
}

impl DesiredFence {
    pub(super) const fn key(&self) -> Option<&AttachmentKey> {
        self.key.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MailboxMembership {
    AwaitingBootstrap,
    Live { desired: Option<AttachmentKey> },
}

impl MailboxMembership {
    const fn key(&self) -> Option<&AttachmentKey> {
        match self {
            Self::AwaitingBootstrap => None,
            Self::Live { desired } => desired.as_ref(),
        }
    }

    const fn awaiting_bootstrap(&self) -> bool {
        matches!(self, Self::AwaitingBootstrap)
    }

    fn fence(&self) -> DesiredFence {
        DesiredFence {
            key: self.key().cloned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExecutionFence {
    desired: DesiredFence,
    barrier: Option<BarrierFence>,
    lease_current: bool,
}

impl ExecutionFence {
    pub(super) const fn desired(&self) -> &DesiredFence {
        &self.desired
    }

    pub(super) const fn barrier(&self) -> Option<BarrierFence> {
        self.barrier
    }

    pub(super) const fn lease_current(&self) -> bool {
        self.lease_current
    }
}

pub(super) struct CommandLease {
    serial: u64,
    record: QueuedCommand,
}

impl CommandLease {
    #[cfg(test)]
    pub(super) const fn serial(&self) -> u64 {
        self.serial
    }

    pub(super) const fn record(&self) -> &QueuedCommand {
        &self.record
    }

    pub(super) const fn record_mut(&mut self) -> &mut QueuedCommand {
        &mut self.record
    }

    pub(super) fn poll_key(&self) -> Option<PollKey> {
        let QueuedCommand::Poll(tagged) = &self.record else {
            return None;
        };
        Some(PollKey {
            serial: self.serial,
            key: tagged.key.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PollKey {
    serial: u64,
    key: AttachmentKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SetupPollKey {
    Allocated(PollKey),
    Superseded(DesiredFence),
}

impl PollKey {
    pub(super) const fn serial(&self) -> u64 {
        self.serial
    }

    pub(super) const fn attachment(&self) -> &AttachmentKey {
        &self.key
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EventDeadline {
    Startup(Instant),
    Forward(Instant),
    TotalCleanup(Instant),
    PlacementOnly(Instant),
    Unbounded,
}

impl EventDeadline {
    const fn placement(self) -> Option<Instant> {
        match self {
            Self::Startup(deadline)
            | Self::Forward(deadline)
            | Self::TotalCleanup(deadline)
            | Self::PlacementOnly(deadline) => Some(deadline),
            Self::Unbounded => None,
        }
    }

    const fn ui_settlement(self) -> Option<Instant> {
        match self {
            Self::Startup(deadline) | Self::Forward(deadline) | Self::TotalCleanup(deadline) => {
                Some(deadline)
            }
            Self::PlacementOnly(_) | Self::Unbounded => None,
        }
    }

    const fn for_ui(self) -> Self {
        match self {
            Self::PlacementOnly(_) => Self::Unbounded,
            deadline => deadline,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum UiCorrelation {
    Switch(AttachmentKey),
    Membership(MembershipResultKey, AttachmentKey),
    Bootstrap,
    Attachment(AttachmentKey),
    Create(CreateOperationKey),
    Refresh(u64),
    Move(WorkbenchRequestId),
    Rename(WorkbenchRequestId),
    Close(CloseFlightKey),
    CloseReconciliation(CloseFlightKey, WorkbenchRequestId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiReplyKind {
    SwitchExpired,
    Switch,
    Membership,
    BootstrapApplied,
    SetupConfigurationSettled,
    SwitchEntryExpiredApplied,
    SetupWithoutAttachmentApplied,
    PrecommitRestored,
    CommitRetained,
    CreateProjection,
    Refresh,
    Move,
    Rename,
    Close,
    CloseReconciliation,
    AttachedSettled,
    FeedbackSettled,
    Ready,
    CleanupApplied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UiWaitKey {
    serial: u64,
    correlation: UiCorrelation,
    reply_kind: UiReplyKind,
}

impl UiWaitKey {
    #[cfg(test)]
    pub(super) const fn serial(&self) -> u64 {
        self.serial
    }
}

pub(super) enum WorkerEvent {
    WorkspaceSetupResult {
        operation: SetupOperation,
        outcome: SetupResult,
    },
    RenameResult {
        operation: RenameOperation,
        outcome: RenameResult,
    },
    CloseReconciliationSnapshot {
        marker: CloseReconciliationMarker,
        outcome: WorkbenchSnapshotOutcome,
    },
    MembershipGone {
        cleanup: MembershipCleanup,
        proof: std::sync::Arc<super::state::DefinitiveClosedAttachment>,
    },
    SwitchForwardExpired {
        permit: SwitchExpiryPermit,
    },
    SwitchSnapshotResult {
        marker: AttachmentKey,
        outcome: WorkbenchSnapshotOutcome,
    },
    MembershipCleanupSkipped {
        cleanup: MembershipCleanup,
    },
    MembershipDetached {
        cleanup: MembershipCleanup,
        detached: AttachmentKey,
    },
    CloseResult {
        operation: CloseOperation,
        outcome: CloseResult,
    },
    MoveResult {
        operation: MoveOperation,
        outcome: MoveResult,
    },
    Bootstrap {
        cache: NativeContextTabs,
    },
    SetupConfigured {
        key: AttachmentKey,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    },
    SwitchEntryExpired {
        key: AttachmentKey,
        target_index: usize,
        target_provenance: CreateTerminalProvenance,
    },
    SetupWithoutAttachment {
        key: AttachmentKey,
        target_index: usize,
        target_provenance: CreateTerminalProvenance,
    },
    CreatePrecommit {
        operation: CreateOperationKey,
        outcome: CreatePrecommitOutcome,
        applied_geometry: Option<Geometry>,
    },
    CreateCommittedKnown {
        operation: CreateOperationKey,
        commit: WorkbenchTabCreateCommit,
    },
    CreateSnapshotResult {
        operation: CreateOperationKey,
        outcome: WorkbenchSnapshotOutcome,
    },
    RefreshSnapshotResult {
        generation: u64,
        outcome: WorkbenchSnapshotOutcome,
    },
    Attached {
        key: AttachmentKey,
        projection: TerminalProjection,
    },
    FeedbackAccepted {
        key: AttachmentKey,
        frame: FrameIdentity,
    },
    ReadyCandidate {
        key: AttachmentKey,
        mode: ControllerMode,
        last_accepted_geometry: Option<Geometry>,
        frame: FrameIdentity,
    },
    CleanupSettled {
        key: AttachmentKey,
        target_index: usize,
        target_provenance: CreateTerminalProvenance,
    },
    PollResult {
        poll_key: PollKey,
        record: Option<Vec<u8>>,
        publication_hint: bool,
    },
}

pub(super) enum CreatePrecommitOutcome {
    Returned(WorkbenchTabCreatePrecommit),
    NotSubmitted,
}

impl WorkerEvent {
    fn wait_key(&self, serial: u64) -> Option<UiWaitKey> {
        let (correlation, reply_kind) = match self {
            Self::CloseReconciliationSnapshot { marker, .. } => (
                UiCorrelation::CloseReconciliation(
                    marker.origin().key().clone(),
                    marker.snapshot_request_id().clone(),
                ),
                UiReplyKind::CloseReconciliation,
            ),
            Self::MembershipGone { cleanup, .. } => {
                let marker = cleanup.close_marker()?;
                (
                    UiCorrelation::CloseReconciliation(
                        marker.origin().key().clone(),
                        marker.snapshot_request_id().clone(),
                    ),
                    UiReplyKind::CloseReconciliation,
                )
            }
            Self::SwitchForwardExpired { permit } => (
                UiCorrelation::Switch(permit.target().selected().key().clone()),
                UiReplyKind::SwitchExpired,
            ),
            Self::SwitchSnapshotResult { marker, .. } => {
                (UiCorrelation::Switch(marker.clone()), UiReplyKind::Switch)
            }
            Self::MembershipCleanupSkipped { cleanup } => (
                UiCorrelation::Membership(cleanup.result().clone(), cleanup.old().clone()),
                UiReplyKind::Switch,
            ),
            Self::MembershipDetached { cleanup, detached } => (
                cleanup.close_marker().map_or_else(
                    || UiCorrelation::Membership(cleanup.result().clone(), detached.clone()),
                    |marker| {
                        UiCorrelation::CloseReconciliation(
                            marker.origin().key().clone(),
                            marker.snapshot_request_id().clone(),
                        )
                    },
                ),
                match cleanup.result() {
                    MembershipResultKey::Close(_) => UiReplyKind::CloseReconciliation,
                    MembershipResultKey::Refresh(_)
                    | MembershipResultKey::Create(_)
                    | MembershipResultKey::Move(_) => UiReplyKind::Membership,
                    MembershipResultKey::Switch(_) => UiReplyKind::Switch,
                    MembershipResultKey::Rename(_) => UiReplyKind::Rename,
                },
            ),
            Self::CloseResult { operation, .. } => (
                UiCorrelation::Close(operation.key().clone()),
                UiReplyKind::Close,
            ),
            Self::MoveResult { operation, .. } => (
                UiCorrelation::Move(operation.request_id().clone()),
                UiReplyKind::Move,
            ),
            Self::WorkspaceSetupResult { operation, .. } => (
                UiCorrelation::Refresh(operation.generation),
                UiReplyKind::Refresh,
            ),
            Self::RenameResult { operation, .. } => (
                UiCorrelation::Rename(operation.request_id().clone()),
                UiReplyKind::Rename,
            ),
            Self::Bootstrap { .. } => (UiCorrelation::Bootstrap, UiReplyKind::BootstrapApplied),
            Self::SetupConfigured { key, .. } => (
                UiCorrelation::Attachment(key.clone()),
                UiReplyKind::SetupConfigurationSettled,
            ),
            Self::SwitchEntryExpired { key, .. } => (
                UiCorrelation::Attachment(key.clone()),
                UiReplyKind::SwitchEntryExpiredApplied,
            ),
            Self::SetupWithoutAttachment { key, .. } => (
                UiCorrelation::Attachment(key.clone()),
                UiReplyKind::SetupWithoutAttachmentApplied,
            ),
            Self::CreatePrecommit { operation, .. } => (
                UiCorrelation::Create(operation.clone()),
                UiReplyKind::PrecommitRestored,
            ),
            Self::CreateCommittedKnown { operation, .. } => (
                UiCorrelation::Create(operation.clone()),
                UiReplyKind::CommitRetained,
            ),
            Self::CreateSnapshotResult { operation, .. } => (
                UiCorrelation::Create(operation.clone()),
                UiReplyKind::CreateProjection,
            ),
            Self::RefreshSnapshotResult { generation, .. } => {
                (UiCorrelation::Refresh(*generation), UiReplyKind::Refresh)
            }
            Self::Attached { key, .. } => (
                UiCorrelation::Attachment(key.clone()),
                UiReplyKind::AttachedSettled,
            ),
            Self::FeedbackAccepted { key, .. } => (
                UiCorrelation::Attachment(key.clone()),
                UiReplyKind::FeedbackSettled,
            ),
            Self::ReadyCandidate { key, .. } => {
                (UiCorrelation::Attachment(key.clone()), UiReplyKind::Ready)
            }
            Self::CleanupSettled { key, .. } => (
                UiCorrelation::Attachment(key.clone()),
                UiReplyKind::CleanupApplied,
            ),
            Self::PollResult { .. } => return None,
        };
        Some(UiWaitKey {
            serial,
            correlation,
            reply_kind,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum UiReply {
    Rename(RenameSettlement),
    CloseReconciliation(Box<CloseReconciliationSettlement>),
    SwitchExpired(RefreshSettlement),
    Switch(RefreshSettlement),
    Membership(RefreshSettlement),
    Close(CloseFlightKey),
    Move(MoveSettlement),
    BootstrapApplied(BootstrapTarget),
    SetupConfigurationSettled(SetupEventDisposition),
    SwitchEntryExpiredApplied(CleanupContinuation),
    SetupWithoutAttachmentApplied(CleanupContinuation),
    PrecommitRestored,
    CommitRetained,
    CreateProjection(CreateProjectionDecision),
    Refresh(RefreshSettlement),
    AttachedSettled(SetupEventDisposition),
    FeedbackSettled(SetupEventDisposition),
    Ready(ReadyDecision),
    CleanupApplied(CleanupContinuation),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MoveSettlement {
    decision: MoveDecision,
    desired: Option<AttachmentKey>,
}

impl MoveSettlement {
    pub(super) fn new(decision: MoveDecision, desired: impl Into<Option<AttachmentKey>>) -> Self {
        Self {
            decision,
            desired: desired.into(),
        }
    }
    pub(super) const fn decision(&self) -> &MoveDecision {
        &self.decision
    }
    pub(super) const fn desired(&self) -> Option<&AttachmentKey> {
        self.desired.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RefreshSettlement {
    decision: RefreshDecision,
    desired: Option<AttachmentKey>,
}

impl RefreshSettlement {
    pub(super) fn new(
        decision: RefreshDecision,
        desired: impl Into<Option<AttachmentKey>>,
    ) -> Self {
        Self {
            decision,
            desired: desired.into(),
        }
    }

    pub(super) const fn decision(&self) -> &RefreshDecision {
        &self.decision
    }

    pub(super) const fn desired(&self) -> Option<&AttachmentKey> {
        self.desired.as_ref()
    }
}

impl UiReply {
    const fn kind(&self) -> UiReplyKind {
        match self {
            Self::CloseReconciliation(_) => UiReplyKind::CloseReconciliation,
            Self::SwitchExpired(_) => UiReplyKind::SwitchExpired,
            Self::Switch(_) => UiReplyKind::Switch,
            Self::Membership(_) => UiReplyKind::Membership,
            Self::Close(_) => UiReplyKind::Close,
            Self::Move(_) => UiReplyKind::Move,
            Self::Rename(_) => UiReplyKind::Rename,
            Self::BootstrapApplied(_) => UiReplyKind::BootstrapApplied,
            Self::SetupConfigurationSettled(_) => UiReplyKind::SetupConfigurationSettled,
            Self::SwitchEntryExpiredApplied(_) => UiReplyKind::SwitchEntryExpiredApplied,
            Self::SetupWithoutAttachmentApplied(_) => UiReplyKind::SetupWithoutAttachmentApplied,
            Self::PrecommitRestored => UiReplyKind::PrecommitRestored,
            Self::CommitRetained => UiReplyKind::CommitRetained,
            Self::CreateProjection(_) => UiReplyKind::CreateProjection,
            Self::Refresh(_) => UiReplyKind::Refresh,
            Self::AttachedSettled(_) => UiReplyKind::AttachedSettled,
            Self::FeedbackSettled(_) => UiReplyKind::FeedbackSettled,
            Self::Ready(_) => UiReplyKind::Ready,
            Self::CleanupApplied(_) => UiReplyKind::CleanupApplied,
        }
    }
}

pub(super) struct TakenEvent {
    wait_key: Option<UiWaitKey>,
    event: WorkerEvent,
}

impl TakenEvent {
    #[cfg(test)]
    pub(super) const fn wait_key(&self) -> Option<&UiWaitKey> {
        self.wait_key.as_ref()
    }

    #[cfg(test)]
    pub(super) fn into_event(self) -> WorkerEvent {
        self.event
    }

    pub(super) fn into_parts(self) -> (Option<UiWaitKey>, WorkerEvent) {
        (self.wait_key, self.event)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiReplyAdmission {
    Current,
    Stale,
}

enum EventCell {
    Empty,
    Event(Box<WorkerEvent>),
    AwaitingUi(UiWaitKey),
    Reply(UiWaitKey, Box<UiReply>),
    ForwardExpired(UiWaitKey),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CausalSettlement {
    NoFeedback,
    Feedback(TerminalViewerAction),
}

enum CausalCell {
    Empty,
    Awaiting(PollKey),
    Settled(PollKey, CausalSettlement),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MutationResult {
    Applied,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MailboxClosed {
    failure: CarrierFailure,
}

impl MailboxClosed {
    pub(super) const fn failure(self) -> CarrierFailure {
        self.failure
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EventError {
    Closed(MailboxClosed),
    ForwardExpired,
}

impl From<MailboxClosed> for EventError {
    fn from(closed: MailboxClosed) -> Self {
        Self::Closed(closed)
    }
}

impl EventError {
    pub(super) const fn closed_failure(self) -> Option<CarrierFailure> {
        match self {
            Self::Closed(closed) => Some(closed.failure()),
            Self::ForwardExpired => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum AdmissionError<BuildError> {
    Closed(MailboxClosed),
    BootstrapPending,
    Full,
    StaleAttachment,
    AttachmentGenerationExhausted,
    RefreshGenerationExhausted,
    Build(BuildError),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum MoveAdmissionError<BuildError> {
    Closed(MailboxClosed),
    Deadline,
    Busy,
    Full,
    IdentityExhausted,
    InvalidTarget,
    Build(BuildError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveLease {
    serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RefreshFlight {
    generation: u64,
    stage: PendingRefreshStage,
}

struct MoveFlight {
    serial: u64,
    operation: MoveOperation,
    stage: PendingRefreshStage,
}

enum StructuralMutationFlight {
    Move(Box<MoveFlight>),
    Rename(Box<RenameFlight>),
    Setup(Box<SetupFlight>),
}

impl StructuralMutationFlight {
    fn as_move(&self) -> Option<&MoveFlight> {
        match self {
            Self::Move(flight) => Some(flight),
            Self::Rename(_) | Self::Setup(_) => None,
        }
    }
}

struct MailboxState {
    switch_anchor_invalidation: Option<SwitchAnchorInvalidation>,
    switch_handoff: Option<SwitchHandoffRecord>,
    queue: VecDeque<QueuedRecord>,
    active_lease: Option<ActiveLease>,
    next_command_serial: u64,
    desired: MailboxMembership,
    attachment_generation: u64,
    latest_geometry: Geometry,
    refresh_generation: u64,
    identity_cursor: u128,
    next_ui_wait_serial: u64,
    event: EventCell,
    event_deadline: EventDeadline,
    event_anchor_serial: Option<u64>,
    event_wait_key: Option<UiWaitKey>,
    causal: CausalCell,
    refresh_flight: Option<RefreshFlight>,
    structural_mutation_pending: Option<StructuralMutationFlight>,
    close_flight: Option<CloseFlight>,
    close_reconciliation: Option<CloseReconciliationFlight>,
    close_acknowledgement: Option<CloseLeaseAcknowledgement>,
    close_queued_key: Option<CloseFlightKey>,
    close_deposited_result: Option<super::state::PendingCloseResult>,
    closed: Option<CarrierFailure>,
}

pub(super) struct UiReplyTransaction<'a> {
    state: &'a mut MailboxState,
    observed_at: Instant,
}

impl UiReplyTransaction<'_> {
    pub(super) const fn observed_at(&self) -> Instant {
        self.observed_at
    }

    pub(super) const fn latest_geometry(&self) -> Geometry {
        self.state.latest_geometry
    }

    pub(super) fn enqueue_catch_up_resize(
        &mut self,
        key: AttachmentKey,
        geometry: Geometry,
    ) -> Result<(), CarrierFailure> {
        if self.state.desired.key() != Some(&key)
            || !self.state.queue.is_empty()
            || self.state.queue.len() == COMMAND_CAPACITY
        {
            return Err(CarrierFailure::Protocol);
        }
        let serial = self
            .state
            .next_command_serial
            .checked_add(1)
            .ok_or(CarrierFailure::IdentityExhausted)?;
        self.state.next_command_serial = serial;
        self.state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::Resize(Tagged::new(key, geometry)),
        });
        Ok(())
    }
}

pub(super) struct CommandMailbox {
    state: Mutex<MailboxState>,
    changed: Condvar,
    #[cfg(test)]
    waiting_threads: AtomicUsize,
    #[cfg(test)]
    ui_reply_resume_checkpoint: AtomicUsize,
    #[cfg(test)]
    causal_take_resume_checkpoint: AtomicUsize,
    #[cfg(test)]
    switch_snapshot_wait_checkpoint: AtomicUsize,
}

#[cfg(test)]
pub(super) struct WorkerResumeCheckpoint<'a> {
    checkpoint: &'a AtomicUsize,
}

#[cfg(test)]
impl WorkerResumeCheckpoint<'_> {
    pub(super) fn await_reached_before(&self, wall_deadline: Instant) -> bool {
        while Instant::now() < wall_deadline {
            if self.checkpoint.load(Ordering::SeqCst) == 2 {
                return true;
            }
            std::thread::yield_now();
        }
        self.checkpoint.load(Ordering::SeqCst) == 2
    }
}

#[cfg(test)]
impl Drop for WorkerResumeCheckpoint<'_> {
    fn drop(&mut self) {
        self.checkpoint.store(0, Ordering::SeqCst);
    }
}

impl CommandMailbox {
    pub(super) fn new(identity_seed: u128, initial_geometry: Geometry) -> Self {
        Self {
            state: Mutex::new(MailboxState {
                switch_anchor_invalidation: None,
                switch_handoff: None,
                queue: VecDeque::with_capacity(COMMAND_CAPACITY),
                active_lease: None,
                next_command_serial: 0,
                desired: MailboxMembership::AwaitingBootstrap,
                attachment_generation: 0,
                latest_geometry: initial_geometry,
                refresh_generation: 0,
                identity_cursor: identity_seed,
                next_ui_wait_serial: 0,
                event: EventCell::Empty,
                event_deadline: EventDeadline::Unbounded,
                event_anchor_serial: None,
                event_wait_key: None,
                causal: CausalCell::Empty,
                refresh_flight: None,
                structural_mutation_pending: None,
                close_flight: None,
                close_reconciliation: None,
                close_acknowledgement: None,
                close_queued_key: None,
                close_deposited_result: None,
                closed: None,
            }),
            changed: Condvar::new(),
            #[cfg(test)]
            waiting_threads: AtomicUsize::new(0),
            #[cfg(test)]
            ui_reply_resume_checkpoint: AtomicUsize::new(0),
            #[cfg(test)]
            causal_take_resume_checkpoint: AtomicUsize::new(0),
            #[cfg(test)]
            switch_snapshot_wait_checkpoint: AtomicUsize::new(0),
        }
    }

    pub(super) fn observe_geometry(&self, geometry: Geometry) -> Result<(), MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        state.latest_geometry = geometry;
        Ok(())
    }

    pub(super) fn latest_geometry(&self) -> Result<Geometry, MailboxClosed> {
        let state = self.lock();
        Self::ensure_open(&state)?;
        Ok(state.latest_geometry)
    }

    pub(super) fn try_enqueue(
        &self,
        command: QueuedCommand,
    ) -> Result<u64, AdmissionError<std::convert::Infallible>> {
        self.try_enqueue_with(|| Ok(command))
    }

    pub(super) fn try_enqueue_with<BuildError>(
        &self,
        build: impl FnOnce() -> Result<QueuedCommand, BuildError>,
    ) -> Result<u64, AdmissionError<BuildError>> {
        let mut state = self.lock();
        Self::ensure_open_for_admission(&state)?;
        Self::ensure_bootstrapped_for_admission(&state)?;
        if state.close_flight.is_some() || state.close_reconciliation.is_some() {
            return Err(AdmissionError::Full);
        }
        if state.queue.len() == COMMAND_CAPACITY {
            return Err(AdmissionError::Full);
        }
        let serial = match state.next_command_serial.checked_add(1) {
            Some(serial) => serial,
            None => {
                return Err(self.close_admission(&mut state, CarrierFailure::IdentityExhausted));
            }
        };
        let command = build().map_err(AdmissionError::Build)?;
        let tagged_key = match &command {
            QueuedCommand::Input(tagged) => tagged.key(),
            QueuedCommand::Resize(tagged) => tagged.key(),
            QueuedCommand::Poll(tagged) => tagged.key(),
            QueuedCommand::Switch(_)
            | QueuedCommand::Create(_)
            | QueuedCommand::Refresh(_)
            | QueuedCommand::Move(_)
            | QueuedCommand::Rename(_)
            | QueuedCommand::WorkspaceSetup(_)
            | QueuedCommand::Close(_)
            | QueuedCommand::ReconcileClose(_) => {
                return Err(self.close_admission(&mut state, CarrierFailure::Protocol));
            }
        };
        if state.desired.key() != Some(tagged_key) {
            return Err(AdmissionError::StaleAttachment);
        }
        state.next_command_serial = serial;
        state.queue.push_back(QueuedRecord { serial, command });
        self.changed.notify_all();
        Ok(serial)
    }

    pub(super) fn apply_local_for<Canonical, Output>(
        &self,
        canonical: &mut Canonical,
        apply: impl FnOnce(&mut Canonical) -> Output,
    ) -> Output {
        let _state = self.lock();
        apply(canonical)
    }

    #[cfg(test)]
    pub(super) fn try_enqueue_switch<BuildError>(
        &self,
        build: impl FnOnce(u64) -> Result<SwitchMarker, BuildError>,
    ) -> Result<u64, AdmissionError<BuildError>> {
        self.try_enqueue_switch_for(
            &mut (),
            |_unit, generation| build(generation),
            |_unit, _result, _facts| {},
        )
    }

    pub(super) fn try_enqueue_switch_for<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        build: impl FnOnce(&mut Canonical, u64) -> Result<SwitchMarker, BuildError>,
        finish: impl FnOnce(
            &mut Canonical,
            &Result<u64, AdmissionError<BuildError>>,
            SwitchAdmissionFacts,
        ),
    ) -> Result<u64, AdmissionError<BuildError>> {
        let mut state = self.lock();
        let mut abandoned_causal = None;
        let mut inherited_abandon = None;
        let result = (|| {
            Self::ensure_open_for_admission(&state)?;
            Self::ensure_bootstrapped_for_admission(&state)?;
            if state.structural_mutation_pending.is_some()
                || (state.close_flight.is_some() || state.close_reconciliation.is_some())
            {
                return Err(AdmissionError::Full);
            }
            if state.queue.len() == COMMAND_CAPACITY {
                return Err(AdmissionError::Full);
            }
            let generation = state
                .attachment_generation
                .checked_add(1)
                .ok_or(AdmissionError::AttachmentGenerationExhausted)?;
            let serial = match state.next_command_serial.checked_add(1) {
                Some(serial) => serial,
                None => {
                    return Err(self.close_admission(&mut state, CarrierFailure::IdentityExhausted));
                }
            };
            let marker = build(canonical, generation).map_err(AdmissionError::Build)?;
            if marker.target().selected().key().generation() != generation {
                return Err(self.close_admission(&mut state, CarrierFailure::Protocol));
            }
            if state
                .switch_anchor_invalidation
                .as_ref()
                .is_some_and(|fact| state.desired.key() != Some(&fact.marker))
            {
                return Err(AdmissionError::Closed(
                    self.close_locked(&mut state, CarrierFailure::Protocol),
                ));
            }
            state.attachment_generation = generation;
            state.next_command_serial = serial;
            state.desired = MailboxMembership::Live {
                desired: Some(marker.target().selected().key().clone()),
            };
            abandoned_causal = self
                .abandon_stale_poll_locked(&mut state)
                .map_err(|failure| self.close_admission(&mut state, failure))?;
            state.queue.push_back(QueuedRecord {
                serial,
                command: QueuedCommand::Switch(marker),
            });
            inherited_abandon = state
                .switch_anchor_invalidation
                .take()
                .map(|fact| fact.discarded);
            self.changed.notify_all();
            Ok(generation)
        })();
        let facts = SwitchAdmissionFacts::new(
            abandoned_causal,
            inherited_abandon,
            state
                .switch_handoff
                .as_ref()
                .is_some_and(SwitchHandoffRecord::is_cleanup),
        );
        finish(canonical, &result, facts);
        result
    }

    #[cfg(test)]
    pub(super) fn try_enqueue_create<BuildError>(
        &self,
        build: impl FnOnce(u64, WorkbenchRequestId) -> Result<CreateMarker, BuildError>,
    ) -> Result<CreateOperationKey, AdmissionError<BuildError>> {
        self.try_enqueue_create_for(
            &mut (),
            |_unit, generation, request_id| build(generation, request_id),
            |_unit, _result| {},
        )
    }

    pub(super) fn try_enqueue_create_for<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        build: impl FnOnce(&mut Canonical, u64, WorkbenchRequestId) -> Result<CreateMarker, BuildError>,
        finish: impl FnOnce(&mut Canonical, &Result<CreateOperationKey, AdmissionError<BuildError>>),
    ) -> Result<CreateOperationKey, AdmissionError<BuildError>> {
        let mut state = self.lock();
        let result = (|| {
            Self::ensure_open_for_admission(&state)?;
            Self::ensure_bootstrapped_for_admission(&state)?;
            if state.structural_mutation_pending.is_some()
                || (state.close_flight.is_some() || state.close_reconciliation.is_some())
            {
                return Err(AdmissionError::Full);
            }
            if state.queue.len() == COMMAND_CAPACITY {
                return Err(AdmissionError::Full);
            }
            let generation = state
                .attachment_generation
                .checked_add(1)
                .ok_or(AdmissionError::AttachmentGenerationExhausted)?;
            let (next_identity, request_id) =
                match workbench_identity_candidate(state.identity_cursor) {
                    Some(candidate) => candidate,
                    None => {
                        let closed =
                            self.close_locked(&mut state, CarrierFailure::IdentityExhausted);
                        return Err(AdmissionError::Closed(closed));
                    }
                };
            let serial = match state.next_command_serial.checked_add(1) {
                Some(serial) => serial,
                None => {
                    return Err(self.close_admission(&mut state, CarrierFailure::IdentityExhausted));
                }
            };
            let marker =
                build(canonical, generation, request_id.clone()).map_err(AdmissionError::Build)?;
            let operation_key = marker.operation().key().clone();
            if operation_key.reserved_generation() != generation
                || operation_key.request_id() != &request_id
            {
                return Err(self.close_admission(&mut state, CarrierFailure::Protocol));
            }
            state.attachment_generation = generation;
            state.identity_cursor = next_identity;
            state.next_command_serial = serial;
            state.queue.push_back(QueuedRecord {
                serial,
                command: QueuedCommand::Create(marker),
            });
            self.changed.notify_all();
            Ok(operation_key)
        })();
        finish(canonical, &result);
        result
    }

    #[cfg(test)]
    pub(super) fn try_enqueue_refresh<BuildError>(
        &self,
        deadline: Instant,
        prepare: impl FnOnce(u64) -> Result<(), BuildError>,
    ) -> Result<u64, AdmissionError<BuildError>> {
        self.try_enqueue_refresh_for(
            &mut (),
            deadline,
            |_unit, generation| prepare(generation),
            |_unit, _result| {},
        )
    }

    pub(super) fn try_enqueue_refresh_for<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadline: Instant,
        prepare: impl FnOnce(&mut Canonical, u64) -> Result<(), BuildError>,
        finish: impl FnOnce(&mut Canonical, &Result<u64, AdmissionError<BuildError>>),
    ) -> Result<u64, AdmissionError<BuildError>> {
        self.try_enqueue_refresh_intent_for(self.lock(), canonical, deadline, None, prepare, finish)
    }

    pub(super) fn try_enqueue_context_selection_for<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
        prepare: impl FnOnce(&mut Canonical, u64) -> Result<(), BuildError>,
        finish: impl FnOnce(&mut Canonical, &Result<u64, AdmissionError<BuildError>>),
    ) -> Result<Option<u64>, AdmissionError<BuildError>> {
        self.try_enqueue_context_selection_using(canonical, deadlines, clock, prepare, finish)
    }

    fn try_enqueue_context_selection_using<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadlines: OperationDeadlines,
        clock: &dyn MailboxClock,
        prepare: impl FnOnce(&mut Canonical, u64) -> Result<(), BuildError>,
        finish: impl FnOnce(&mut Canonical, &Result<u64, AdmissionError<BuildError>>),
    ) -> Result<Option<u64>, AdmissionError<BuildError>> {
        let Some((state, _)) = self.lock_before(deadlines.forward_cutoff(), clock) else {
            return Ok(None);
        };
        if clock.now() >= deadlines.forward_cutoff() {
            return Ok(None);
        }
        self.try_enqueue_refresh_intent_for(
            state,
            canonical,
            deadlines.total(),
            Some(deadlines),
            prepare,
            finish,
        )
        .map(Some)
    }

    fn try_enqueue_refresh_intent_for<Canonical, BuildError>(
        &self,
        mut state: MutexGuard<'_, MailboxState>,
        canonical: &mut Canonical,
        deadline: Instant,
        context_deadlines: Option<OperationDeadlines>,
        prepare: impl FnOnce(&mut Canonical, u64) -> Result<(), BuildError>,
        finish: impl FnOnce(&mut Canonical, &Result<u64, AdmissionError<BuildError>>),
    ) -> Result<u64, AdmissionError<BuildError>> {
        let result = (|| {
            Self::ensure_open_for_admission(&state)?;
            Self::ensure_bootstrapped_for_admission(&state)?;
            if state.queue.len() == COMMAND_CAPACITY
                || state.refresh_flight.is_some()
                || state.structural_mutation_pending.is_some()
                || (state.close_flight.is_some() || state.close_reconciliation.is_some())
            {
                return Err(AdmissionError::Full);
            }
            let generation = state
                .refresh_generation
                .checked_add(1)
                .ok_or(AdmissionError::RefreshGenerationExhausted)?;
            let serial = match state.next_command_serial.checked_add(1) {
                Some(serial) => serial,
                None => {
                    return Err(self.close_admission(&mut state, CarrierFailure::IdentityExhausted));
                }
            };
            prepare(canonical, generation).map_err(AdmissionError::Build)?;
            state.refresh_generation = generation;
            state.next_command_serial = serial;
            state.refresh_flight = Some(RefreshFlight {
                generation,
                stage: PendingRefreshStage::Queued,
            });
            let mut marker = RefreshMarker::new(generation, deadline);
            marker.context_deadlines = context_deadlines;
            state.queue.push_back(QueuedRecord {
                serial,
                command: QueuedCommand::Refresh(marker),
            });
            self.changed.notify_all();
            Ok(generation)
        })();
        finish(canonical, &result);
        result
    }

    pub(super) fn try_enqueue_move_for<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadline: Instant,
        clock: &LocalOperationClock,
        build: impl FnOnce(&Canonical, WorkbenchRequestId) -> Result<MoveOperation, BuildError>,
        commit: impl FnOnce(&mut Canonical, &MoveOperation) -> Result<(), BuildError>,
    ) -> Result<MoveOperation, MoveAdmissionError<BuildError>> {
        self.try_enqueue_move_for_using(canonical, deadline, clock, build, commit)
    }

    fn try_enqueue_move_for_using<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadline: Instant,
        clock: &dyn MailboxClock,
        build: impl FnOnce(&Canonical, WorkbenchRequestId) -> Result<MoveOperation, BuildError>,
        commit: impl FnOnce(&mut Canonical, &MoveOperation) -> Result<(), BuildError>,
    ) -> Result<MoveOperation, MoveAdmissionError<BuildError>> {
        let (mut state, _) = self
            .lock_before(deadline, clock)
            .ok_or(MoveAdmissionError::Deadline)?;
        Self::ensure_open(&state).map_err(MoveAdmissionError::Closed)?;
        if state.desired.key().is_none()
            || state.structural_mutation_pending.is_some()
            || (state.close_flight.is_some() || state.close_reconciliation.is_some())
            || state.refresh_flight.is_some()
            || state
                .queue
                .iter()
                .any(|record| record.command.barrier().is_some())
        {
            return Err(MoveAdmissionError::Busy);
        }
        if state.queue.len() == COMMAND_CAPACITY {
            return Err(MoveAdmissionError::Full);
        }
        let (cursor, request_id) = workbench_identity_candidate(state.identity_cursor)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let serial = state
            .next_command_serial
            .checked_add(1)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let operation = build(canonical, request_id.clone()).map_err(MoveAdmissionError::Build)?;
        if operation.request_id() != &request_id
            || operation.deadline() != deadline
            || state.desired.key() != Some(operation.target())
            || operation
                .baseline()
                .tab_index(operation.target().tab_id())
                .is_none()
        {
            return Err(MoveAdmissionError::InvalidTarget);
        }
        let flight = Box::new(MoveFlight {
            serial,
            operation: operation.clone(),
            stage: PendingRefreshStage::Queued,
        });
        commit(canonical, &operation).map_err(MoveAdmissionError::Build)?;
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.structural_mutation_pending = Some(StructuralMutationFlight::Move(flight));
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::Move(MoveMarker::new(operation.clone())),
        });
        self.changed.notify_all();
        Ok(operation)
    }

    pub(super) fn mark_move_in_flight(
        &self,
        operation: &MoveOperation,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let active_serial = state.active_lease.map(|lease| lease.serial);
        match state.structural_mutation_pending.as_mut() {
            Some(StructuralMutationFlight::Move(flight))
                if flight.operation == *operation
                    && flight.stage == PendingRefreshStage::Queued
                    && active_serial == Some(flight.serial) =>
            {
                flight.stage = PendingRefreshStage::InFlight;
                self.changed.notify_all();
                Ok(MutationResult::Applied)
            }
            _ => Ok(MutationResult::Stale),
        }
    }

    pub(super) fn publish_move_result_before(
        &self,
        operation: MoveOperation,
        outcome: MoveResult,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        self.publish_event_before(
            WorkerEvent::MoveResult { operation, outcome },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }

    pub(super) fn recv(&self) -> Result<CommandLease, MailboxClosed> {
        let mut state = self.lock();
        loop {
            Self::ensure_open(&state)?;
            if state.desired.awaiting_bootstrap() {
                state = self.wait(state);
                continue;
            }
            if state.active_lease.is_none()
                && let Some(record) = state.queue.pop_front()
            {
                state.active_lease = Some(ActiveLease {
                    serial: record.serial,
                });
                return Ok(CommandLease {
                    serial: record.serial,
                    record: record.command,
                });
            }
            state = self.wait(state);
        }
    }

    #[cfg(test)]
    pub(super) fn try_recv(&self) -> Result<Option<CommandLease>, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        if state.desired.awaiting_bootstrap() {
            return Ok(None);
        }
        if state.active_lease.is_some() {
            return Ok(None);
        }
        let Some(record) = state.queue.pop_front() else {
            return Ok(None);
        };
        state.active_lease = Some(ActiveLease {
            serial: record.serial,
        });
        Ok(Some(CommandLease {
            serial: record.serial,
            record: record.command,
        }))
    }

    pub(super) fn finish_lease(&self, lease: CommandLease) -> Result<(), MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        if state.active_lease
            != Some(ActiveLease {
                serial: lease.serial,
            })
        {
            return Err(self.close_locked(&mut state, CarrierFailure::Protocol));
        }
        let close_ack = match lease.record() {
            QueuedCommand::Close(marker) => Some(CloseLeaseAcknowledgement::Exchange(
                marker.operation().key().clone(),
            )),
            QueuedCommand::ReconcileClose(marker) => Some(
                CloseLeaseAcknowledgement::Reconciliation(Box::new(marker.clone())),
            ),
            _ => None,
        };
        drop(lease);
        if close_ack.is_some() {
            if state.close_acknowledgement.is_some() {
                return Err(self.close_locked(&mut state, CarrierFailure::Protocol));
            }
            state.close_acknowledgement = close_ack;
        }
        state.active_lease = None;
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn allocate_startup_poll_key(
        &self,
        key: &AttachmentKey,
    ) -> Result<PollKey, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        if state.desired.key() != Some(key)
            || state.active_lease.is_some()
            || !state.queue.is_empty()
        {
            return Err(self.close_locked(&mut state, CarrierFailure::Protocol));
        }
        self.allocate_poll_key_locked(&mut state, key.clone())
    }

    pub(super) fn allocate_setup_poll_key(
        &self,
        parent: &CommandLease,
        key: &AttachmentKey,
    ) -> Result<SetupPollKey, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let marker_parent = matches!(
            &parent.record,
            QueuedCommand::Switch(_)
                | QueuedCommand::Create(_)
                | QueuedCommand::Refresh(_)
                | QueuedCommand::ReconcileClose(_)
                | QueuedCommand::Move(_)
                | QueuedCommand::Rename(_)
                | QueuedCommand::WorkspaceSetup(_)
        );
        if !marker_parent
            || state.active_lease
                != Some(ActiveLease {
                    serial: parent.serial,
                })
        {
            return Err(self.close_locked(&mut state, CarrierFailure::Protocol));
        }
        if state.desired.key() != Some(key) {
            return Ok(SetupPollKey::Superseded(state.desired.fence()));
        }
        self.allocate_poll_key_locked(&mut state, key.clone())
            .map(SetupPollKey::Allocated)
    }

    pub(super) fn await_startup_change_before(
        &self,
        key: &AttachmentKey,
        cutoff: Instant,
        clock: &LocalOperationClock,
    ) -> Result<DesiredFence, EventError> {
        self.await_startup_change_before_using(key, cutoff, clock)
    }

    fn await_startup_change_before_using(
        &self,
        key: &AttachmentKey,
        cutoff: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<DesiredFence, EventError> {
        let (mut state, mut observed_at) = self.lock_with_fresh_sample(clock);
        Self::ensure_open(&state).map_err(EventError::Closed)?;
        if state.desired.key() != Some(key) {
            return Ok(state.desired.fence());
        }
        if observed_at >= cutoff {
            return Err(self.close_deadline(&mut state));
        }
        let backoff_end = observed_at
            .checked_add(CONDITION_WAIT_SLICE)
            .map_or(cutoff, |end| end.min(cutoff));
        loop {
            let remaining = backoff_end.saturating_duration_since(observed_at);
            let (next, _timed_out) = self.wait_timeout(state, remaining);
            drop(next);
            let sampled;
            (state, sampled) = self.lock_with_fresh_sample(clock);
            Self::ensure_open(&state).map_err(EventError::Closed)?;
            if state.desired.key() != Some(key) || sampled >= backoff_end {
                if state.desired.key() == Some(key) && sampled >= cutoff {
                    return Err(self.close_deadline(&mut state));
                }
                return Ok(state.desired.fence());
            }
            observed_at = sampled;
        }
    }

    pub(super) fn execution_fence(&self, lease: &CommandLease) -> ExecutionFence {
        let state = self.lock();
        Self::execution_fence_locked(&state, lease)
    }

    pub(super) fn await_execution_change_before(
        &self,
        lease: &CommandLease,
        cutoff: Instant,
        clock: &LocalOperationClock,
    ) -> Result<ExecutionFence, MailboxClosed> {
        self.await_execution_change_before_using(lease, cutoff, clock)
    }

    fn await_execution_change_before_using(
        &self,
        lease: &CommandLease,
        cutoff: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<ExecutionFence, MailboxClosed> {
        let (mut state, mut observed_at) = self.lock_with_fresh_sample(clock);
        Self::ensure_open(&state)?;
        let initial = Self::execution_fence_locked(&state, lease);
        if observed_at >= cutoff {
            return Ok(initial);
        }
        let backoff_end = observed_at
            .checked_add(CONDITION_WAIT_SLICE)
            .map_or(cutoff, |end| end.min(cutoff));
        loop {
            let remaining = backoff_end.saturating_duration_since(observed_at);
            let (next, _timed_out) = self.wait_timeout(state, remaining);
            drop(next);
            let sampled;
            (state, sampled) = self.lock_with_fresh_sample(clock);
            Self::ensure_open(&state)?;
            let current = Self::execution_fence_locked(&state, lease);
            if current != initial || sampled >= backoff_end {
                return Ok(current);
            }
            observed_at = sampled;
        }
    }

    pub(super) fn allocate_obcs_request_id(&self) -> Result<RequestId, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let Some((next, request_id)) = obcs_identity_candidate(state.identity_cursor) else {
            return Err(self.close_locked(&mut state, CarrierFailure::IdentityExhausted));
        };
        state.identity_cursor = next;
        Ok(request_id)
    }

    pub(super) fn allocate_workbench_request_id(
        &self,
    ) -> Result<WorkbenchRequestId, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let Some((next, request_id)) = workbench_identity_candidate(state.identity_cursor) else {
            return Err(self.close_locked(&mut state, CarrierFailure::IdentityExhausted));
        };
        state.identity_cursor = next;
        Ok(request_id)
    }

    #[cfg(test)]
    pub(super) fn publish_desired(&self, key: AttachmentKey) -> Result<(), MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        if let Err(failure) = Self::publish_desired_locked(&mut state, key) {
            return Err(self.close_locked(&mut state, failure));
        }
        self.abandon_stale_poll_locked(&mut state)
            .map_err(|failure| self.close_locked(&mut state, failure))?;
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn publish_event_before(
        &self,
        event: WorkerEvent,
        deadline: EventDeadline,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        self.publish_event_before_using(event, deadline, clock)
    }

    fn publish_event_before_using(
        &self,
        event: WorkerEvent,
        deadline: EventDeadline,
        clock: &dyn MailboxClock,
    ) -> Result<UiWaitKey, EventError> {
        let (mut state, mut observed_at) = self.lock_with_fresh_sample(clock);
        let event_anchor_serial = state.active_lease.map(|lease| lease.serial);
        let is_bootstrap = matches!(&event, WorkerEvent::Bootstrap { .. });
        if is_bootstrap != state.desired.awaiting_bootstrap() {
            return Err(self.close_protocol(&mut state));
        }
        loop {
            Self::ensure_open(&state).map_err(EventError::Closed)?;
            if matches!(state.event, EventCell::Empty) {
                break;
            }
            let effective =
                Self::forward_event_deadline_locked(&state, deadline, event_anchor_serial);
            state = self.wait_for_event_deadline_observed(state, effective, observed_at)?;
            drop(state);
            (state, observed_at) = self.lock_with_fresh_sample(clock);
        }
        let effective = Self::forward_event_deadline_locked(&state, deadline, event_anchor_serial);
        self.ensure_event_before_locked(&mut state, effective, observed_at, false)?;
        let Some(serial) = state.next_ui_wait_serial.checked_add(1) else {
            return Err(self.close_identity_exhausted(&mut state));
        };
        let Some(wait_key) = event.wait_key(serial) else {
            return Err(self.close_protocol(&mut state));
        };
        if !Self::switch_event_matches_receipt(&state, &event, deadline, event_anchor_serial) {
            return Err(self.close_protocol(&mut state));
        }
        if let WorkerEvent::RefreshSnapshotResult { generation, .. } = &event {
            if matches!(
                state.structural_mutation_pending,
                Some(StructuralMutationFlight::Setup(_))
            ) || !matches!(deadline, EventDeadline::PlacementOnly(_))
                || !matches!(
                    state.refresh_flight,
                    Some(RefreshFlight {
                        generation: current,
                        stage: PendingRefreshStage::InFlight,
                    }) if current == *generation
                )
            {
                return Err(self.close_protocol(&mut state));
            }
            state.refresh_flight = Some(RefreshFlight {
                generation: *generation,
                stage: PendingRefreshStage::AwaitingUi,
            });
        }
        if let WorkerEvent::MoveResult { operation, .. } = &event {
            let valid = deadline == EventDeadline::PlacementOnly(operation.deadline())
                && state
                    .structural_mutation_pending
                    .as_ref()
                    .and_then(StructuralMutationFlight::as_move)
                    .is_some_and(|flight| {
                        flight.operation == *operation
                            && flight.stage == PendingRefreshStage::InFlight
                            && event_anchor_serial == Some(flight.serial)
                    });
            if !valid {
                return Err(self.close_protocol(&mut state));
            }
            if let Some(StructuralMutationFlight::Move(flight)) =
                state.structural_mutation_pending.as_mut()
            {
                flight.stage = PendingRefreshStage::AwaitingUi;
            }
        }
        if let WorkerEvent::CloseReconciliationSnapshot { marker, .. } = &event
            && !state.place_close_reconciliation(marker, deadline, event_anchor_serial)
        {
            return Err(self.close_protocol(&mut state));
        }
        if let WorkerEvent::CloseResult { operation, .. } = &event
            && !state.place_close_result(operation, deadline, event_anchor_serial)
        {
            return Err(self.close_protocol(&mut state));
        }
        if let WorkerEvent::WorkspaceSetupResult { operation, outcome } = &event
            && !state.place_setup_result(operation, outcome, deadline, event_anchor_serial)
        {
            return Err(self.close_protocol(&mut state));
        }
        if let WorkerEvent::RenameResult { operation, outcome } = &event
            && !state.place_rename_result(operation, outcome, deadline, event_anchor_serial)
        {
            return Err(self.close_protocol(&mut state));
        }
        if let WorkerEvent::CloseResult { operation, outcome } = &event {
            state.close_deposited_result = Some(super::state::PendingCloseResult::new(
                operation.clone(),
                outcome.clone(),
            ));
        }
        state.next_ui_wait_serial = serial;
        state.event_wait_key = Some(wait_key.clone());
        state.event_deadline = deadline;
        state.event_anchor_serial = event_anchor_serial;
        state.event = EventCell::Event(Box::new(event));
        self.changed.notify_all();
        Ok(wait_key)
    }

    pub(super) fn publish_refresh_result_before(
        &self,
        generation: u64,
        outcome: WorkbenchSnapshotOutcome,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        self.publish_event_before(
            WorkerEvent::RefreshSnapshotResult {
                generation,
                outcome,
            },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }

    pub(super) fn publish_poll_result_before(
        &self,
        poll_key: PollKey,
        record: Option<Vec<u8>>,
        publication_hint: bool,
        cutoff: Instant,
        clock: &LocalOperationClock,
    ) -> Result<MutationResult, EventError> {
        self.publish_poll_result_with_hint_using(poll_key, record, publication_hint, cutoff, clock)
    }

    #[cfg(test)]
    fn publish_poll_result_before_using(
        &self,
        poll_key: PollKey,
        record: Option<Vec<u8>>,
        cutoff: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<MutationResult, EventError> {
        self.publish_poll_result_with_hint_using(poll_key, record, false, cutoff, clock)
    }

    fn publish_poll_result_with_hint_using(
        &self,
        poll_key: PollKey,
        record: Option<Vec<u8>>,
        publication_hint: bool,
        cutoff: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<MutationResult, EventError> {
        let (mut state, mut observed_at) = self.lock_with_fresh_sample(clock);
        loop {
            Self::ensure_open(&state).map_err(EventError::Closed)?;
            if state.desired.key() != Some(poll_key.attachment()) {
                Self::record_switch_discard_locked(&mut state, poll_key.attachment())
                    .map_err(|failure| self.close_failure(&mut state, failure))?;
                return Ok(MutationResult::Stale);
            }
            if matches!(state.event, EventCell::Empty) {
                break;
            }
            let deadline = Self::forward_event_deadline_locked(
                &state,
                EventDeadline::Forward(cutoff),
                Some(poll_key.serial),
            );
            state = self.wait_for_event_deadline_observed(state, deadline, observed_at)?;
            drop(state);
            (state, observed_at) = self.lock_with_fresh_sample(clock);
        }
        let deadline = Self::forward_event_deadline_locked(
            &state,
            EventDeadline::Forward(cutoff),
            Some(poll_key.serial),
        );
        self.ensure_event_before_locked(&mut state, deadline, observed_at, false)?;
        if !matches!(state.causal, CausalCell::Empty) {
            return Err(self.close_protocol(&mut state));
        }
        state.event_wait_key = None;
        state.event_deadline = deadline;
        state.event_anchor_serial = Some(poll_key.serial);
        state.causal = CausalCell::Awaiting(poll_key.clone());
        state.event = EventCell::Event(Box::new(WorkerEvent::PollResult {
            poll_key,
            record,
            publication_hint,
        }));
        self.changed.notify_all();
        Ok(MutationResult::Applied)
    }

    pub(super) fn take_event_before(
        &self,
        clock: &LocalOperationClock,
    ) -> Result<Option<TakenEvent>, EventError> {
        self.take_event_before_using(clock)
    }

    #[cfg(test)]
    pub(super) fn await_event_before(
        &self,
        clock: &LocalOperationClock,
        wall_deadline: Instant,
    ) -> Result<Option<TakenEvent>, EventError> {
        self.await_event_before_using(clock, wall_deadline)
    }

    #[cfg(test)]
    fn await_event_before_using(
        &self,
        clock: &dyn MailboxClock,
        wall_deadline: Instant,
    ) -> Result<Option<TakenEvent>, EventError> {
        loop {
            let state = self.lock();
            Self::ensure_open(&state).map_err(EventError::Closed)?;
            if matches!(state.event, EventCell::Event(_)) {
                drop(state);
                return self.take_event_before_using(clock);
            }
            let observed_at = Instant::now();
            if observed_at >= wall_deadline {
                return Ok(None);
            }
            let remaining = wall_deadline.saturating_duration_since(observed_at);
            drop(self.wait_timeout(state, remaining).0);
        }
    }

    fn take_event_before_using(
        &self,
        clock: &dyn MailboxClock,
    ) -> Result<Option<TakenEvent>, EventError> {
        let (mut state, observed_at) = self.lock_with_fresh_sample(clock);
        Self::ensure_open(&state).map_err(EventError::Closed)?;
        if !matches!(state.event, EventCell::Event(_)) {
            return Ok(None);
        }
        let deadline = match &state.event {
            EventCell::Event(_) => Self::forward_event_deadline_locked(
                &state,
                state.event_deadline.for_ui(),
                state.event_anchor_serial,
            ),
            EventCell::Empty
            | EventCell::AwaitingUi(_)
            | EventCell::Reply(_, _)
            | EventCell::ForwardExpired(_) => unreachable!("only a published event can be taken"),
        };
        self.ensure_event_before_locked(&mut state, deadline, observed_at, true)?;
        let event = std::mem::replace(&mut state.event, EventCell::Empty);
        let EventCell::Event(event) = event else {
            return Err(self.close_protocol(&mut state));
        };
        if let WorkerEvent::PollResult { poll_key, .. } = &*event {
            if !matches!(&state.causal, CausalCell::Awaiting(current) if current == poll_key) {
                return Err(self.close_protocol(&mut state));
            }
            state.event_wait_key = None;
            state.event_deadline = EventDeadline::Unbounded;
            state.event_anchor_serial = None;
            self.changed.notify_all();
            return Ok(Some(TakenEvent {
                wait_key: None,
                event: *event,
            }));
        }
        let Some(wait_key) = state.event_wait_key.clone() else {
            return Err(self.close_protocol(&mut state));
        };
        state.event = EventCell::AwaitingUi(wait_key.clone());
        self.changed.notify_all();
        Ok(Some(TakenEvent {
            wait_key: Some(wait_key),
            event: *event,
        }))
    }

    fn admit_ui_reply_locked(
        &self,
        state: &mut MailboxState,
        wait_key: &UiWaitKey,
    ) -> Result<UiReplyAdmission, EventError> {
        match &state.event {
            EventCell::AwaitingUi(current) if current == wait_key => Ok(UiReplyAdmission::Current),
            EventCell::Event(_)
                if state
                    .event_wait_key
                    .as_ref()
                    .is_some_and(|current| wait_key.serial < current.serial) =>
            {
                Ok(UiReplyAdmission::Stale)
            }
            EventCell::Event(_) if state.event_wait_key.as_ref() == Some(wait_key) => {
                Err(self.close_protocol(state))
            }
            EventCell::AwaitingUi(current) if wait_key.serial < current.serial => {
                Ok(UiReplyAdmission::Stale)
            }
            EventCell::Reply(current, _) | EventCell::ForwardExpired(current)
                if current == wait_key || wait_key.serial < current.serial =>
            {
                Ok(UiReplyAdmission::Stale)
            }
            EventCell::Event(event)
                if matches!(&**event, WorkerEvent::PollResult { .. })
                    && wait_key.serial <= state.next_ui_wait_serial =>
            {
                Ok(UiReplyAdmission::Stale)
            }
            EventCell::Empty if wait_key.serial <= state.next_ui_wait_serial => {
                Ok(UiReplyAdmission::Stale)
            }
            EventCell::AwaitingUi(_)
            | EventCell::Empty
            | EventCell::Event(_)
            | EventCell::Reply(_, _)
            | EventCell::ForwardExpired(_) => Err(self.close_protocol(state)),
        }
    }

    fn ensure_reply_current_locked(
        &self,
        state: &mut MailboxState,
        wait_key: &UiWaitKey,
    ) -> Result<(), EventError> {
        if wait_key.reply_kind == UiReplyKind::BootstrapApplied {
            return Err(self.close_protocol(state));
        }
        // Validate the exact answered receipt before the caller can commit either owner plan.
        if wait_key.reply_kind == UiReplyKind::Switch
            && !Self::switch_ui_matches_receipt(state, wait_key)
        {
            return Err(self.close_protocol(state));
        }
        if wait_key.reply_kind == UiReplyKind::SwitchExpired
            && (state.switch_handoff.is_some()
                || state.event_anchor_serial != state.active_lease.map(|lease| lease.serial))
        {
            return Err(self.close_protocol(state));
        }
        if matches!(wait_key.correlation, UiCorrelation::CloseReconciliation(..))
            && !state.exact_close_reconciliation(&wait_key.correlation)
        {
            return Err(self.close_protocol(state));
        }
        let move_request = match &wait_key.correlation {
            UiCorrelation::Move(request) => Some((request, None)),
            UiCorrelation::Membership(MembershipResultKey::Move(request), old) => {
                Some((request, Some(old)))
            }
            _ => None,
        };
        if let Some((request, old)) = move_request
            && !state
                .structural_mutation_pending
                .as_ref()
                .and_then(StructuralMutationFlight::as_move)
                .is_some_and(|flight| {
                    flight.operation.request_id() == request
                        && flight.stage == PendingRefreshStage::AwaitingUi
                        && state.active_lease.map(|lease| lease.serial) == Some(flight.serial)
                        && state.event_anchor_serial == Some(flight.serial)
                        && state.desired.key() == Some(flight.operation.target())
                        && old.is_none_or(|old| old == flight.operation.target())
                })
        {
            return Err(self.close_protocol(state));
        }
        if !state.rename_reply_current(&wait_key.correlation)
            || !state.setup_reply_current(&wait_key.correlation)
        {
            return Err(self.close_protocol(state));
        }
        Ok(())
    }

    fn settle_reply_locked(
        &self,
        state: &mut MailboxState,
        wait_key: &UiWaitKey,
        reply: &UiReply,
    ) -> Result<(), EventError> {
        match reply {
            UiReply::Rename(settlement) => {
                let exact = match &wait_key.correlation {
                    UiCorrelation::Rename(id)
                    | UiCorrelation::Membership(MembershipResultKey::Rename(id), _) => {
                        id == settlement.request_id()
                    }
                    _ => false,
                };
                if !exact || state.desired.key() != settlement.desired() {
                    return Err(self.close_protocol(state));
                }
                if !matches!(settlement.decision(), RefreshDecision::Cleanup(_)) {
                    state.structural_mutation_pending = None;
                }
            }
            UiReply::CloseReconciliation(settlement) => {
                if !state.settle_close_reconciliation(&wait_key.correlation, settlement) {
                    return Err(self.close_protocol(state));
                }
            }
            UiReply::Close(key) => {
                if !state.settle_close_result(&wait_key.correlation, key) {
                    return Err(self.close_protocol(state));
                }
            }
            UiReply::Switch(_) => {
                // Publication and the pre-closure check already sealed this exact UI settlement.
                state.switch_handoff = None;
            }
            UiReply::Move(settlement) => {
                let valid = matches!(&wait_key.correlation, UiCorrelation::Move(request_id)
                if state.structural_mutation_pending.as_ref().and_then(StructuralMutationFlight::as_move).is_some_and(|flight| {
                    flight.operation.request_id() == request_id
                        && flight.stage == PendingRefreshStage::AwaitingUi
                }));
                if !valid || state.desired.key() != settlement.desired() {
                    return Err(self.close_protocol(state));
                }
                if !matches!(settlement.decision(), MoveDecision::Cleanup(_)) {
                    state.structural_mutation_pending = None;
                }
            }
            UiReply::Membership(settlement)
                if matches!(
                    &wait_key.correlation,
                    UiCorrelation::Membership(MembershipResultKey::Create(_), _)
                ) =>
            {
                if state.desired.key() != settlement.desired()
                    || matches!(settlement.decision(), RefreshDecision::Cleanup(_))
                    || matches!(settlement.decision(), RefreshDecision::AttachRecovered(target)
                        if Some(target.selected().key()) != settlement.desired())
                {
                    return Err(self.close_protocol(state));
                }
                self.abandon_stale_poll_locked(state)
                    .map_err(|failure| self.close_failure(state, failure))?;
            }
            UiReply::Refresh(settlement) | UiReply::Membership(settlement) => {
                let generation = match &wait_key.correlation {
                    UiCorrelation::Refresh(generation) => Some(*generation),
                    UiCorrelation::Membership(MembershipResultKey::Refresh(generation), _) => {
                        Some(*generation)
                    }
                    UiCorrelation::Membership(MembershipResultKey::Move(_), _) => None,
                    _ => {
                        return Err(self.close_protocol(state));
                    }
                };
                if generation.is_some_and(|generation| {
                    !matches!(
                        state.refresh_flight,
                        Some(RefreshFlight {
                            generation: current,
                            stage: PendingRefreshStage::AwaitingUi,
                        }) if current == generation
                    )
                }) {
                    return Err(self.close_protocol(state));
                }
                if let RefreshDecision::AttachRecovered(target) = settlement.decision()
                    && Some(target.selected().key()) != settlement.desired()
                {
                    return Err(self.close_protocol(state));
                }
                if state.desired.key() != settlement.desired() {
                    return Err(self.close_protocol(state));
                }
                if !matches!(settlement.decision(), RefreshDecision::Cleanup(_)) {
                    if generation.is_some() {
                        state.refresh_flight = None;
                        if matches!(
                            state.structural_mutation_pending,
                            Some(StructuralMutationFlight::Setup(_))
                        ) {
                            state.structural_mutation_pending = None;
                        }
                    } else {
                        state.structural_mutation_pending = None;
                    }
                }
            }
            UiReply::BootstrapApplied(_)
            | UiReply::SwitchExpired(_)
            | UiReply::SetupConfigurationSettled(_)
            | UiReply::SwitchEntryExpiredApplied(_)
            | UiReply::SetupWithoutAttachmentApplied(_)
            | UiReply::PrecommitRestored
            | UiReply::CommitRetained
            | UiReply::CreateProjection(_)
            | UiReply::AttachedSettled(_)
            | UiReply::FeedbackSettled(_)
            | UiReply::Ready(_)
            | UiReply::CleanupApplied(_) => {}
        }
        Ok(())
    }

    pub(super) fn deposit_ui_reply_before<Settle>(
        &self,
        wait_key: &UiWaitKey,
        clock: &LocalOperationClock,
        settle: Settle,
    ) -> Result<MutationResult, EventError>
    where
        Settle: FnOnce(&mut UiReplyTransaction<'_>) -> Result<UiReply, CarrierFailure>,
    {
        self.deposit_ui_reply_before_using(wait_key, clock, settle)
    }

    fn deposit_ui_reply_before_using<Settle>(
        &self,
        wait_key: &UiWaitKey,
        clock: &dyn MailboxClock,
        settle: Settle,
    ) -> Result<MutationResult, EventError>
    where
        Settle: FnOnce(&mut UiReplyTransaction<'_>) -> Result<UiReply, CarrierFailure>,
    {
        let (mut state, observed_at) = self.lock_with_fresh_sample(clock);
        Self::ensure_open(&state).map_err(EventError::Closed)?;
        if let UiReplyAdmission::Stale = self.admit_ui_reply_locked(&mut state, wait_key)? {
            return Ok(MutationResult::Stale);
        }
        let deadline = Self::forward_event_deadline_locked(
            &state,
            state.event_deadline.for_ui(),
            state.event_anchor_serial,
        );
        self.ensure_event_before_locked(&mut state, deadline, observed_at, true)?;
        self.ensure_reply_current_locked(&mut state, wait_key)?;
        let reply = match settle(&mut UiReplyTransaction {
            state: &mut state,
            observed_at,
        }) {
            Ok(reply) => reply,
            Err(failure) => {
                return Err(self.close_failure(&mut state, failure));
            }
        };
        if wait_key.reply_kind != reply.kind() {
            return Err(self.close_protocol(&mut state));
        }
        self.settle_reply_locked(&mut state, wait_key, &reply)?;
        state.event = EventCell::Reply(wait_key.clone(), Box::new(reply));
        self.changed.notify_all();
        Ok(MutationResult::Applied)
    }

    pub(super) fn deposit_bootstrap_reply_before<Settle>(
        &self,
        wait_key: &UiWaitKey,
        clock: &LocalOperationClock,
        settle: Settle,
    ) -> Result<MutationResult, EventError>
    where
        Settle: FnOnce(u64, Geometry) -> Result<BootstrapTarget, CarrierFailure>,
    {
        self.deposit_bootstrap_reply_before_using(wait_key, clock, settle)
    }

    fn deposit_bootstrap_reply_before_using<Settle>(
        &self,
        wait_key: &UiWaitKey,
        clock: &dyn MailboxClock,
        settle: Settle,
    ) -> Result<MutationResult, EventError>
    where
        Settle: FnOnce(u64, Geometry) -> Result<BootstrapTarget, CarrierFailure>,
    {
        let (mut state, observed_at) = self.lock_with_fresh_sample(clock);
        Self::ensure_open(&state).map_err(EventError::Closed)?;
        if let UiReplyAdmission::Stale = self.admit_ui_reply_locked(&mut state, wait_key)? {
            return Ok(MutationResult::Stale);
        }
        if wait_key.reply_kind != UiReplyKind::BootstrapApplied
            || wait_key.correlation != UiCorrelation::Bootstrap
            || !state.desired.awaiting_bootstrap()
        {
            return Err(self.close_protocol(&mut state));
        }
        let deadline = state.event_deadline.for_ui();
        self.ensure_event_before_locked(&mut state, deadline, observed_at, true)?;
        let Some(generation) = state.attachment_generation.checked_add(1) else {
            return Err(self.close_identity_exhausted(&mut state));
        };
        let target = match settle(generation, state.latest_geometry) {
            Ok(target) => target,
            Err(failure) => {
                return Err(self.close_failure(&mut state, failure));
            }
        };
        if target.selected().key().generation() != generation {
            return Err(self.close_protocol(&mut state));
        }
        state.attachment_generation = generation;
        state.desired = MailboxMembership::Live {
            desired: Some(target.selected().key().clone()),
        };
        state.event = EventCell::Reply(
            wait_key.clone(),
            Box::new(UiReply::BootstrapApplied(target)),
        );
        self.changed.notify_all();
        Ok(MutationResult::Applied)
    }

    #[cfg(test)]
    pub(super) fn take_deposited_reply(
        &self,
        wait_key: &UiWaitKey,
    ) -> Result<Option<UiReply>, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        self.take_deposited_reply_locked(&mut state, wait_key)
    }

    pub(super) fn await_ui_reply_before(
        &self,
        wait_key: &UiWaitKey,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<Option<UiReply>, EventError> {
        self.await_ui_reply_before_using(wait_key, deadline, clock)
    }

    fn await_ui_reply_before_using(
        &self,
        wait_key: &UiWaitKey,
        deadline: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<Option<UiReply>, EventError> {
        let (mut state, mut observed_at) = self.lock_with_fresh_sample(clock);
        loop {
            Self::ensure_open(&state).map_err(EventError::Closed)?;
            if matches!(&state.event, EventCell::Reply(current, _) if current == wait_key) {
                return self
                    .take_deposited_reply_locked(&mut state, wait_key)
                    .map_err(EventError::Closed);
            }
            if matches!(&state.event, EventCell::ForwardExpired(current) if current == wait_key) {
                Self::clear_event_locked(&mut state);
                self.changed.notify_all();
                return Err(EventError::ForwardExpired);
            }
            let exact_pending = matches!(&state.event, EventCell::AwaitingUi(current) if current == wait_key)
                || matches!(state.event, EventCell::Event(_))
                    && state.event_wait_key.as_ref() == Some(wait_key);
            if !exact_pending {
                return self
                    .take_deposited_reply_locked(&mut state, wait_key)
                    .map_err(EventError::Closed);
            }
            let effective = match state.event_deadline {
                EventDeadline::Startup(event_deadline) => {
                    EventDeadline::Startup(event_deadline.min(deadline))
                }
                EventDeadline::Forward(event_deadline) => {
                    EventDeadline::Forward(event_deadline.min(deadline))
                }
                EventDeadline::TotalCleanup(event_deadline) => {
                    EventDeadline::TotalCleanup(event_deadline.min(deadline))
                }
                EventDeadline::PlacementOnly(_) | EventDeadline::Unbounded => {
                    EventDeadline::Forward(deadline)
                }
            };
            let effective =
                Self::forward_event_deadline_locked(&state, effective, state.event_anchor_serial);
            if let Err(error) =
                self.ensure_event_before_locked(&mut state, effective, observed_at, true)
            {
                if error == EventError::ForwardExpired
                    && matches!(&state.event, EventCell::ForwardExpired(current) if current == wait_key)
                {
                    Self::clear_event_locked(&mut state);
                    self.changed.notify_all();
                }
                return Err(error);
            }
            let remaining = effective
                .placement()
                .unwrap_or(deadline)
                .saturating_duration_since(observed_at);
            let (next, _timed_out) = self.wait_timeout(state, remaining.min(CONDITION_WAIT_SLICE));
            #[cfg(test)]
            let exact_reply_deposited =
                matches!(&next.event, EventCell::Reply(current, _) if current == wait_key);
            drop(next);
            #[cfg(test)]
            if exact_reply_deposited {
                self.stop_at_ui_reply_resume_checkpoint();
            }
            (state, observed_at) = self.lock_with_fresh_sample(clock);
        }
    }

    pub(super) fn await_ui_reply(
        &self,
        wait_key: &UiWaitKey,
    ) -> Result<Option<UiReply>, MailboxClosed> {
        let mut state = self.lock();
        loop {
            Self::ensure_open(&state)?;
            if matches!(&state.event, EventCell::Reply(current, _) if current == wait_key) {
                return self.take_deposited_reply_locked(&mut state, wait_key);
            }
            let exact_pending = matches!(&state.event, EventCell::AwaitingUi(current) if current == wait_key)
                || matches!(state.event, EventCell::Event(_))
                    && state.event_wait_key.as_ref() == Some(wait_key);
            if !exact_pending {
                return self.take_deposited_reply_locked(&mut state, wait_key);
            }
            if state.event_deadline.ui_settlement().is_some() {
                return Err(self.close_locked(&mut state, CarrierFailure::Protocol));
            }
            state = self.wait(state);
            #[cfg(test)]
            if matches!(&state.event, EventCell::Reply(current, _) if current == wait_key) {
                drop(state);
                self.stop_at_ui_reply_resume_checkpoint();
                state = self.lock();
            }
        }
    }

    pub(super) fn complete_empty_poll(
        &self,
        poll_key: &PollKey,
    ) -> Result<MutationResult, MailboxClosed> {
        self.deposit_causal(poll_key, CausalSettlement::NoFeedback)
    }

    pub(super) fn deposit_causal(
        &self,
        poll_key: &PollKey,
        settlement: CausalSettlement,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        match &state.causal {
            CausalCell::Awaiting(current) if current == poll_key => {
                state.causal = CausalCell::Settled(poll_key.clone(), settlement);
                self.changed.notify_all();
                Ok(MutationResult::Applied)
            }
            CausalCell::Awaiting(current) | CausalCell::Settled(current, _)
                if poll_key.serial < current.serial =>
            {
                Ok(MutationResult::Stale)
            }
            CausalCell::Empty if poll_key.serial <= state.next_command_serial => {
                Ok(MutationResult::Stale)
            }
            CausalCell::Settled(current, _) if current == poll_key => Ok(MutationResult::Stale),
            CausalCell::Empty | CausalCell::Awaiting(_) | CausalCell::Settled(_, _) => {
                Err(self.close_locked(&mut state, CarrierFailure::Protocol))
            }
        }
    }

    pub(super) fn take_causal_before(
        &self,
        poll_key: &PollKey,
        cutoff: Instant,
        clock: &LocalOperationClock,
    ) -> Result<Option<CausalSettlement>, EventError> {
        let settlement = self.take_causal_before_using(poll_key, cutoff, clock);
        #[cfg(test)]
        if matches!(&settlement, Ok(Some(CausalSettlement::Feedback(_)))) {
            Self::stop_at_resume_checkpoint(&self.causal_take_resume_checkpoint);
        }
        settlement
    }

    fn take_causal_before_using(
        &self,
        poll_key: &PollKey,
        cutoff: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<Option<CausalSettlement>, EventError> {
        let (mut state, mut observed_at) = self.lock_with_fresh_sample(clock);
        loop {
            Self::ensure_open(&state).map_err(EventError::Closed)?;
            if state.desired.key() != Some(&poll_key.key) {
                if matches!(&state.causal, CausalCell::Awaiting(current) | CausalCell::Settled(current, _) if current == poll_key)
                {
                    Self::record_switch_discard_locked(&mut state, poll_key.attachment())
                        .map_err(|failure| self.close_failure(&mut state, failure))?;
                    state.causal = CausalCell::Empty;
                    self.changed.notify_all();
                }
                return Ok(None);
            }
            let effective_cutoff = state
                .queue
                .iter()
                .filter(|record| record.serial > poll_key.serial)
                .filter_map(|record| record.command.barrier())
                .map(BarrierFence::forward_cutoff)
                .fold(cutoff, Instant::min);
            match &state.causal {
                CausalCell::Settled(current, _) if current == poll_key => {
                    let cell = std::mem::replace(&mut state.causal, CausalCell::Empty);
                    let CausalCell::Settled(_, settlement) = cell else {
                        return Err(self.close_protocol(&mut state));
                    };
                    self.changed.notify_all();
                    return Ok(Some(settlement));
                }
                CausalCell::Awaiting(current) if current == poll_key => {
                    if observed_at >= effective_cutoff {
                        Self::clear_poll_event_locked(&mut state, poll_key);
                        state.causal = CausalCell::Empty;
                        self.changed.notify_all();
                        return Err(EventError::ForwardExpired);
                    }
                    let remaining = effective_cutoff.saturating_duration_since(observed_at);
                    let (next, _timed_out) =
                        self.wait_timeout(state, remaining.min(CONDITION_WAIT_SLICE));
                    state = next;
                    drop(state);
                    (state, observed_at) = self.lock_with_fresh_sample(clock);
                }
                CausalCell::Empty if poll_key.serial <= state.next_command_serial => {
                    return if observed_at >= effective_cutoff {
                        Err(EventError::ForwardExpired)
                    } else {
                        Ok(None)
                    };
                }
                CausalCell::Awaiting(current) | CausalCell::Settled(current, _)
                    if poll_key.serial < current.serial =>
                {
                    return Ok(None);
                }
                CausalCell::Empty | CausalCell::Awaiting(_) | CausalCell::Settled(_, _) => {
                    return Err(self.close_protocol(&mut state));
                }
            }
        }
    }

    pub(super) fn mark_refresh_in_flight(
        &self,
        generation: u64,
    ) -> Result<MutationResult, MailboxClosed> {
        self.transition_refresh(
            generation,
            PendingRefreshStage::Queued,
            PendingRefreshStage::InFlight,
        )
    }

    #[cfg(test)]
    pub(super) fn pending_refresh_stage(&self) -> Option<(u64, PendingRefreshStage)> {
        self.lock()
            .refresh_flight
            .map(|flight| (flight.generation, flight.stage))
    }

    pub(super) fn close(&self, failure: CarrierFailure) -> MailboxClosed {
        let mut state = self.lock();
        self.close_locked(&mut state, failure)
    }

    #[cfg(test)]
    pub(super) fn wake_waiters(&self) {
        self.changed.notify_all();
    }

    pub(super) fn is_closed(&self) -> Option<CarrierFailure> {
        self.lock().closed
    }

    fn transition_refresh(
        &self,
        generation: u64,
        from: PendingRefreshStage,
        to: PendingRefreshStage,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        match state.refresh_flight {
            Some(flight) if flight.generation == generation && flight.stage == from => {
                state.refresh_flight = Some(RefreshFlight {
                    generation,
                    stage: to,
                });
                self.changed.notify_all();
                Ok(MutationResult::Applied)
            }
            Some(flight) if generation < flight.generation => Ok(MutationResult::Stale),
            None if generation <= state.refresh_generation => Ok(MutationResult::Stale),
            Some(_) | None => Err(self.close_locked(&mut state, CarrierFailure::Protocol)),
        }
    }

    fn execution_fence_locked(state: &MailboxState, lease: &CommandLease) -> ExecutionFence {
        let lease_current = state.active_lease
            == Some(ActiveLease {
                serial: lease.serial,
            });
        let barrier = state
            .queue
            .iter()
            .find_map(|record| record.command.barrier());
        ExecutionFence {
            desired: state.desired.fence(),
            barrier,
            lease_current,
        }
    }

    fn forward_event_deadline_locked(
        state: &MailboxState,
        deadline: EventDeadline,
        anchor_serial: Option<u64>,
    ) -> EventDeadline {
        let EventDeadline::Forward(cutoff) = deadline else {
            return deadline;
        };
        let effective_cutoff = state
            .queue
            .iter()
            .filter(|record| anchor_serial.is_none_or(|serial| record.serial > serial))
            .filter_map(|record| record.command.barrier())
            .map(BarrierFence::forward_cutoff)
            .fold(cutoff, Instant::min);
        EventDeadline::Forward(effective_cutoff)
    }

    fn allocate_poll_key_locked(
        &self,
        state: &mut MailboxState,
        key: AttachmentKey,
    ) -> Result<PollKey, MailboxClosed> {
        let Some(serial) = state.next_command_serial.checked_add(1) else {
            return Err(self.close_locked(state, CarrierFailure::IdentityExhausted));
        };
        state.next_command_serial = serial;
        Ok(PollKey { serial, key })
    }

    fn abandon_stale_poll_locked(
        &self,
        state: &mut MailboxState,
    ) -> Result<Option<AttachmentKey>, CarrierFailure> {
        let event = match &state.event {
            EventCell::Event(event) => match &**event {
                WorkerEvent::PollResult { poll_key, .. } => Some(poll_key),
                _ => None,
            },
            _ => None,
        };
        let causal = match &state.causal {
            CausalCell::Awaiting(poll_key) | CausalCell::Settled(poll_key, _) => Some(poll_key),
            CausalCell::Empty => None,
        };
        if event.is_some() && event != causal {
            return Err(CarrierFailure::Protocol);
        }
        let discarded = event
            .or(causal)
            .filter(|poll| state.desired.key() != Some(poll.attachment()))
            .map(|poll| poll.attachment().clone());
        if discarded.is_some() {
            if event.is_some() {
                Self::clear_event_locked(state);
            }
            state.causal = CausalCell::Empty;
        }
        Ok(discarded)
    }

    #[cfg(test)]
    fn publish_desired_locked(
        state: &mut MailboxState,
        key: AttachmentKey,
    ) -> Result<(), CarrierFailure> {
        let Some(current) = state.desired.key() else {
            return Err(CarrierFailure::Protocol);
        };
        if key.generation() < current.generation() {
            return Ok(());
        }
        if key.generation() == current.generation() {
            return if &key == current {
                Ok(())
            } else {
                Err(CarrierFailure::Protocol)
            };
        }
        if key.generation() > state.attachment_generation {
            return Err(CarrierFailure::Protocol);
        }
        state.desired = MailboxMembership::Live { desired: Some(key) };
        Ok(())
    }

    fn take_deposited_reply_locked(
        &self,
        state: &mut MailboxState,
        wait_key: &UiWaitKey,
    ) -> Result<Option<UiReply>, MailboxClosed> {
        match &state.event {
            EventCell::Reply(current, _) if current == wait_key => {
                let cell = std::mem::replace(&mut state.event, EventCell::Empty);
                let EventCell::Reply(_, reply) = cell else {
                    return Err(self.close_locked(state, CarrierFailure::Protocol));
                };
                state.event_wait_key = None;
                state.event_deadline = EventDeadline::Unbounded;
                state.event_anchor_serial = None;
                self.changed.notify_all();
                Ok(Some(*reply))
            }
            EventCell::AwaitingUi(current) if current == wait_key => Ok(None),
            EventCell::Event(_) if state.event_wait_key.as_ref() == Some(wait_key) => Ok(None),
            EventCell::Reply(current, _) | EventCell::AwaitingUi(current)
                if wait_key.serial < current.serial =>
            {
                Ok(None)
            }
            EventCell::Event(_)
                if state
                    .event_wait_key
                    .as_ref()
                    .is_some_and(|current| wait_key.serial < current.serial) =>
            {
                Ok(None)
            }
            EventCell::ForwardExpired(current)
                if current == wait_key || wait_key.serial < current.serial =>
            {
                Ok(None)
            }
            EventCell::Event(event)
                if matches!(&**event, WorkerEvent::PollResult { .. })
                    && wait_key.serial <= state.next_ui_wait_serial =>
            {
                Ok(None)
            }
            EventCell::Empty if wait_key.serial <= state.next_ui_wait_serial => Ok(None),
            EventCell::Empty
            | EventCell::Event(_)
            | EventCell::Reply(_, _)
            | EventCell::AwaitingUi(_)
            | EventCell::ForwardExpired(_) => {
                Err(self.close_locked(state, CarrierFailure::Protocol))
            }
        }
    }

    fn ensure_event_before_locked(
        &self,
        state: &mut MailboxState,
        deadline: EventDeadline,
        observed_at: Instant,
        invalidate_current: bool,
    ) -> Result<(), EventError> {
        let Some(expires_at) = deadline.placement() else {
            return Ok(());
        };
        if observed_at < expires_at {
            return Ok(());
        }
        if matches!(deadline, EventDeadline::Forward(_)) {
            if invalidate_current {
                Self::expire_forward_event_locked(state);
                self.changed.notify_all();
            }
            return Err(EventError::ForwardExpired);
        }
        Err(EventError::Closed(
            self.close_locked(state, CarrierFailure::Deadline),
        ))
    }

    fn wait_for_event_deadline_observed<'a>(
        &self,
        state: MutexGuard<'a, MailboxState>,
        deadline: EventDeadline,
        observed_at: Instant,
    ) -> Result<MutexGuard<'a, MailboxState>, EventError> {
        let Some(expires_at) = deadline.placement() else {
            return Ok(self.wait(state));
        };
        if observed_at >= expires_at {
            let mut state = state;
            return if matches!(deadline, EventDeadline::Forward(_)) {
                Err(EventError::ForwardExpired)
            } else {
                Err(EventError::Closed(
                    self.close_locked(&mut state, CarrierFailure::Deadline),
                ))
            };
        }
        let remaining = expires_at.saturating_duration_since(observed_at);
        let (state, _timed_out) = self.wait_timeout(state, remaining.min(CONDITION_WAIT_SLICE));
        Ok(state)
    }

    fn clear_event_locked(state: &mut MailboxState) {
        if let EventCell::Event(event) = &state.event
            && let WorkerEvent::PollResult { poll_key, .. } = &**event
            && matches!(
                &state.causal,
                CausalCell::Awaiting(current) | CausalCell::Settled(current, _)
                    if current == poll_key
            )
        {
            state.causal = CausalCell::Empty;
        }
        state.event = EventCell::Empty;
        state.event_wait_key = None;
        state.event_deadline = EventDeadline::Unbounded;
        state.event_anchor_serial = None;
    }

    fn expire_forward_event_locked(state: &mut MailboxState) {
        if let EventCell::Event(event) = &state.event
            && matches!(&**event, WorkerEvent::PollResult { .. })
        {
            Self::clear_event_locked(state);
            return;
        }
        let wait_key = match &state.event {
            EventCell::Event(_) => state.event_wait_key.clone(),
            EventCell::AwaitingUi(wait_key) => Some(wait_key.clone()),
            EventCell::Empty | EventCell::Reply(_, _) | EventCell::ForwardExpired(_) => None,
        };
        let Some(wait_key) = wait_key else {
            Self::clear_event_locked(state);
            return;
        };
        state.event = EventCell::ForwardExpired(wait_key.clone());
        state.event_wait_key = Some(wait_key);
        state.event_deadline = EventDeadline::Unbounded;
        state.event_anchor_serial = None;
    }

    fn clear_poll_event_locked(state: &mut MailboxState, poll_key: &PollKey) {
        let exact = matches!(
            &state.event,
            EventCell::Event(event)
                if matches!(&**event, WorkerEvent::PollResult { poll_key: current, .. } if current == poll_key)
        );
        if exact {
            Self::clear_event_locked(state);
        }
    }

    fn ensure_open_for_admission<BuildError>(
        state: &MailboxState,
    ) -> Result<(), AdmissionError<BuildError>> {
        Self::ensure_open(state).map_err(AdmissionError::Closed)
    }

    fn ensure_bootstrapped_for_admission<BuildError>(
        state: &MailboxState,
    ) -> Result<(), AdmissionError<BuildError>> {
        if state.desired.awaiting_bootstrap() {
            Err(AdmissionError::BootstrapPending)
        } else {
            Ok(())
        }
    }

    fn ensure_open(state: &MailboxState) -> Result<(), MailboxClosed> {
        match state.closed {
            Some(failure) => Err(MailboxClosed { failure }),
            None => Ok(()),
        }
    }

    fn close_locked(&self, state: &mut MailboxState, failure: CarrierFailure) -> MailboxClosed {
        let failure = *state.closed.get_or_insert(failure);
        state.queue.clear();
        state.active_lease = None;
        state.event = EventCell::Empty;
        state.event_wait_key = None;
        state.event_deadline = EventDeadline::Unbounded;
        state.causal = CausalCell::Empty;
        state.refresh_flight = None;
        // Keep exact Rename handoff/result evidence until the current worker's join is settled.
        if !matches!(
            state.structural_mutation_pending,
            Some(StructuralMutationFlight::Rename(_) | StructuralMutationFlight::Setup(_))
        ) {
            state.structural_mutation_pending = None;
        }
        state.close_flight = None;
        state.close_reconciliation = None;
        self.changed.notify_all();
        MailboxClosed { failure }
    }

    fn close_admission<BuildError>(
        &self,
        state: &mut MailboxState,
        failure: CarrierFailure,
    ) -> AdmissionError<BuildError> {
        AdmissionError::Closed(self.close_locked(state, failure))
    }

    fn close_protocol(&self, state: &mut MailboxState) -> EventError {
        EventError::Closed(self.close_locked(state, CarrierFailure::Protocol))
    }

    fn close_identity_exhausted(&self, state: &mut MailboxState) -> EventError {
        EventError::Closed(self.close_locked(state, CarrierFailure::IdentityExhausted))
    }

    fn close_deadline(&self, state: &mut MailboxState) -> EventError {
        EventError::Closed(self.close_locked(state, CarrierFailure::Deadline))
    }

    fn close_failure(&self, state: &mut MailboxState, failure: CarrierFailure) -> EventError {
        EventError::Closed(self.close_locked(state, failure))
    }

    fn lock(&self) -> MutexGuard<'_, MailboxState> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn lock_with_fresh_sample(
        &self,
        clock: &dyn MailboxClock,
    ) -> (MutexGuard<'_, MailboxState>, Instant) {
        loop {
            let observed_at = clock.now();
            match self.state.try_lock() {
                Ok(state) => return (state, observed_at),
                Err(TryLockError::Poisoned(poison)) => {
                    return (poison.into_inner(), observed_at);
                }
                Err(TryLockError::WouldBlock) => {
                    #[cfg(test)]
                    clock.after_mailbox_contention();
                    std::thread::yield_now();
                }
            }
        }
    }

    fn lock_before(
        &self,
        deadline: Instant,
        clock: &dyn MailboxClock,
    ) -> Option<(MutexGuard<'_, MailboxState>, Instant)> {
        loop {
            let observed_at = clock.now();
            if observed_at >= deadline {
                return None;
            }
            match self.state.try_lock() {
                Ok(state) => return Some((state, observed_at)),
                Err(TryLockError::Poisoned(poison)) => {
                    return Some((poison.into_inner(), observed_at));
                }
                Err(TryLockError::WouldBlock) => {
                    #[cfg(test)]
                    clock.after_mailbox_contention();
                    std::thread::yield_now();
                }
            }
        }
    }

    fn wait<'a>(&self, state: MutexGuard<'a, MailboxState>) -> MutexGuard<'a, MailboxState> {
        #[cfg(test)]
        self.waiting_threads.fetch_add(1, Ordering::SeqCst);
        let state = self
            .changed
            .wait(state)
            .unwrap_or_else(|poison| poison.into_inner());
        #[cfg(test)]
        self.waiting_threads.fetch_sub(1, Ordering::SeqCst);
        state
    }

    fn wait_timeout<'a>(
        &self,
        state: MutexGuard<'a, MailboxState>,
        timeout: Duration,
    ) -> (MutexGuard<'a, MailboxState>, bool) {
        #[cfg(test)]
        self.waiting_threads.fetch_add(1, Ordering::SeqCst);
        let result = match self.changed.wait_timeout(state, timeout) {
            Ok((state, result)) => (state, result.timed_out()),
            Err(poison) => {
                let (state, result) = poison.into_inner();
                (state, result.timed_out())
            }
        };
        #[cfg(test)]
        self.waiting_threads.fetch_sub(1, Ordering::SeqCst);
        result
    }

    #[cfg(test)]
    pub(super) fn await_condition_waiter_before(&self, wall_deadline: Instant) -> bool {
        while Instant::now() < wall_deadline {
            if self.waiting_threads.load(Ordering::SeqCst) != 0 {
                return true;
            }
            std::thread::yield_now();
        }
        self.waiting_threads.load(Ordering::SeqCst) != 0
    }

    #[cfg(test)]
    pub(super) fn event_waiting_for_test(&self) -> bool {
        matches!(self.lock().event, EventCell::Event(_))
    }

    #[cfg(test)]
    pub(super) fn hold_lock_for_test(&self) -> impl Drop + '_ {
        self.lock()
    }

    #[cfg(test)]
    pub(super) fn pause_next_ui_reply_resume_for_test(&self) -> WorkerResumeCheckpoint<'_> {
        Self::arm_resume_checkpoint(&self.ui_reply_resume_checkpoint)
    }

    #[cfg(test)]
    pub(super) fn pause_next_causal_take_resume_for_test(&self) -> WorkerResumeCheckpoint<'_> {
        Self::arm_resume_checkpoint(&self.causal_take_resume_checkpoint)
    }

    #[cfg(test)]
    pub(super) fn pause_next_switch_snapshot_wait_for_test(&self) -> WorkerResumeCheckpoint<'_> {
        Self::arm_resume_checkpoint(&self.switch_snapshot_wait_checkpoint)
    }

    #[cfg(test)]
    pub(super) fn stop_at_switch_snapshot_wait_for_test(&self) {
        Self::stop_at_resume_checkpoint(&self.switch_snapshot_wait_checkpoint);
    }

    #[cfg(test)]
    fn arm_resume_checkpoint(checkpoint: &AtomicUsize) -> WorkerResumeCheckpoint<'_> {
        assert_eq!(
            checkpoint.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst),
            Ok(0),
            "only one worker resume checkpoint may be armed"
        );
        WorkerResumeCheckpoint { checkpoint }
    }

    #[cfg(test)]
    fn stop_at_ui_reply_resume_checkpoint(&self) {
        Self::stop_at_resume_checkpoint(&self.ui_reply_resume_checkpoint);
    }

    #[cfg(test)]
    fn stop_at_resume_checkpoint(checkpoint: &AtomicUsize) {
        if checkpoint
            .compare_exchange(1, 2, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        while checkpoint.load(Ordering::SeqCst) == 2 {
            std::thread::yield_now();
        }
    }

    #[cfg(test)]
    pub(super) fn cursors(&self) -> (u64, u64, u128) {
        let state = self.lock();
        (
            state.attachment_generation,
            state.refresh_generation,
            state.identity_cursor,
        )
    }

    #[cfg(test)]
    pub(super) fn switch_anchor_invalidation_for_test(
        &self,
    ) -> Option<(AttachmentKey, AttachmentKey)> {
        self.lock()
            .switch_anchor_invalidation
            .as_ref()
            .map(|fact| (fact.marker.clone(), fact.discarded.clone()))
    }

    #[cfg(test)]
    pub(super) fn cells(&self) -> (&'static str, &'static str) {
        let state = self.lock();
        let event = match state.event {
            EventCell::Empty => "empty",
            EventCell::Event(_) => "event",
            EventCell::AwaitingUi(_) => "awaiting-ui",
            EventCell::Reply(_, _) => "reply",
            EventCell::ForwardExpired(_) => "forward-expired",
        };
        let causal = match state.causal {
            CausalCell::Empty => "empty",
            CausalCell::Awaiting(_) => "awaiting",
            CausalCell::Settled(_, _) => "settled",
        };
        (event, causal)
    }
}

fn obcs_identity_candidate(cursor: u128) -> Option<(u128, RequestId)> {
    let next = cursor.checked_add(1)?;
    let request_id = RequestId::parse(&format!("{next:032x}")).ok()?;
    Some((next, request_id))
}

fn workbench_identity_candidate(cursor: u128) -> Option<(u128, WorkbenchRequestId)> {
    let next = cursor.checked_add(1)?;
    let request_id = WorkbenchRequestId::parse(&format!("{next:032x}")).ok()?;
    Some((next, request_id))
}

#[cfg(test)]
mod tests;
