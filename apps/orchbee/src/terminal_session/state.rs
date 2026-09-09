use super::workbench_projection::{
    CreatedTabExpectation, NativeContextTabs, NativeSimpleTab, NativeTerminalBinding,
    validate_canonical_refresh, validate_created_tab,
};
use crate::client_view::ClientAction;
use crate::local_client::workbench::WorkbenchTabCreateCommit;
use orchbee_client_server_protocol::{ControllerId, Geometry};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalFailureReason,
    WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
};
use std::time::{Duration, Instant};

#[cfg(test)]
use super::workbench_projection::project_native_standalone_tabs;
mod context_selection;
pub(super) mod workspace_setup;
pub(crate) use context_selection::NativeContextView;
mod create_membership;
mod membership;
mod move_tab;
pub(super) mod rename_draft;
pub(super) mod rename_text;
#[cfg(target_os = "linux")]
pub(super) mod retired_membership;
mod switch_membership;
mod tab_close;
pub(super) mod tab_rename;
use create_membership::CreateAnchor;
pub(super) use create_membership::{CreateOriginKind, PreparedCreateSnapshot};
#[cfg(test)]
pub(super) use membership::PreparedMembershipInstall;
pub(super) use membership::{
    MembershipCleanup, MembershipInstallDecision, MembershipResultKey, PreparedMembershipCandidate,
};
pub(super) use move_tab::{MoveDecision, MoveOperation, MoveResult, PendingMoveCommit};
use switch_membership::SwitchMembership;
pub(super) use switch_membership::SwitchRestoreKey;
pub(crate) use tab_close::{
    CloseConfirmationKind, CloseConfirmationToken, CloseStatusKind, CloseStatusToken,
};
pub(super) use tab_close::{
    CloseConfirmationView, CloseFlightKey, CloseLeaseAcknowledgement, CloseOperation,
    CloseOperationKind, CloseReconciliationMarker, CloseResult, CloseState, CloseStateError,
    CloseStatusView, CloseWorkerReturn, DefinitiveClosedAttachment, PendingCloseResult,
};
#[cfg(test)]
pub(super) use tab_close::{CloseIntent, CloseReadMode, CloseSelection, CloseTarget};

pub(super) const OPERATION_TOTAL: Duration = Duration::from_secs(50);
pub(super) const CLEANUP_RESERVE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttachmentKey {
    generation: u64,
    tab_id: WorkbenchTabId,
}

impl AttachmentKey {
    pub(super) const fn new(generation: u64, tab_id: WorkbenchTabId) -> Self {
        Self { generation, tab_id }
    }

    pub(super) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) const fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OperationDeadlines {
    forward_cutoff: Instant,
    total: Instant,
}

impl OperationDeadlines {
    pub(super) fn from_sample(sampled_at: Instant) -> Option<Self> {
        let total = sampled_at.checked_add(OPERATION_TOTAL)?;
        let forward_cutoff = total.checked_sub(CLEANUP_RESERVE)?;
        Some(Self {
            forward_cutoff,
            total,
        })
    }

    pub(super) const fn forward_cutoff(self) -> Instant {
        self.forward_cutoff
    }

    pub(super) const fn total(self) -> Instant {
        self.total
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CreateTerminalProvenance {
    Started,
    Pending,
    Unavailable(WorkbenchTerminalUnavailableReason),
    Failed(WorkbenchTerminalFailureReason),
    OutcomeUnknown,
    BindingOnly,
}

impl CreateTerminalProvenance {
    pub(super) const fn attachable(&self) -> bool {
        matches!(self, Self::Started | Self::BindingOnly)
    }

    fn from_terminal_outcome(outcome: WorkbenchTerminalOutcome) -> Self {
        match outcome {
            WorkbenchTerminalOutcome::Started { .. } => Self::Started,
            WorkbenchTerminalOutcome::Unavailable { reason } => Self::Unavailable(reason),
            WorkbenchTerminalOutcome::Failed { reason } => Self::Failed(reason),
            WorkbenchTerminalOutcome::OutcomeUnknown {} => Self::OutcomeUnknown,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ControllerMode {
    Controlling(ControllerId),
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FrameIdentity(u64);

impl FrameIdentity {
    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SessionPhaseKind {
    Empty,
    Starting,
    Ready,
    Creating,
    CreateCommittedNeedDetach,
    ProjectionUnavailable,
    MoveProjectionUnavailable,
    RenameProjectionUnavailable,
    Switching,
    SelectedWithoutAttachment,
    RetryableAttachUnavailable,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SessionNotice {
    LocalBusy,
    NoTabAtPosition(u8),
    CreateNotCreated,
    MoveNotMoved,
    RenameNotRenamed,
    CloseRequestNotApplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NavigatorRefreshStatus {
    Refreshing,
    Success(WorkbenchRevision),
    Failure(WorkbenchRevision),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PendingRefreshStage {
    Queued,
    InFlight,
    AwaitingUi,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectedTab {
    key: AttachmentKey,
    index: usize,
    panel_index: usize,
}

impl SelectedTab {
    pub(super) const fn new(key: AttachmentKey, index: usize) -> Self {
        Self::new_panel(key, index, 0)
    }

    pub(super) const fn new_panel(key: AttachmentKey, index: usize, panel_index: usize) -> Self {
        Self {
            key,
            index,
            panel_index,
        }
    }

    pub(super) const fn key(&self) -> &AttachmentKey {
        &self.key
    }

    pub(super) const fn index(&self) -> usize {
        self.index
    }

    pub(super) const fn panel_index(&self) -> usize {
        self.panel_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReadyAttachment {
    selected: SelectedTab,
    mode: ControllerMode,
}

impl ReadyAttachment {
    pub(super) const fn new(selected: SelectedTab, mode: ControllerMode) -> Self {
        Self { selected, mode }
    }

    pub(super) const fn selected(&self) -> &SelectedTab {
        &self.selected
    }

    pub(super) const fn mode(&self) -> &ControllerMode {
        &self.mode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CreateOperationKey {
    request_id: WorkbenchRequestId,
    reserved_generation: u64,
}

impl CreateOperationKey {
    pub(super) const fn new(request_id: WorkbenchRequestId, reserved_generation: u64) -> Self {
        Self {
            request_id,
            reserved_generation,
        }
    }

    pub(super) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }

    pub(super) const fn reserved_generation(&self) -> u64 {
        self.reserved_generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CreateOperation {
    origin: CreateOriginKind,
    key: CreateOperationKey,
    site_id: SiteId,
    expected_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    label: WorkbenchLabel,
    geometry: Geometry,
    deadlines: OperationDeadlines,
}

impl CreateOperation {
    pub(super) const fn new(
        key: CreateOperationKey,
        site_id: SiteId,
        expected_revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        label: WorkbenchLabel,
        geometry: Geometry,
        deadlines: OperationDeadlines,
    ) -> Self {
        Self {
            origin: CreateOriginKind::Ready,
            key,
            site_id,
            expected_revision,
            terminal_context_id,
            label,
            geometry,
            deadlines,
        }
    }

    pub(super) const fn key(&self) -> &CreateOperationKey {
        &self.key
    }

    pub(super) const fn deadlines(&self) -> OperationDeadlines {
        self.deadlines
    }

    pub(super) const fn with_origin(mut self, origin: CreateOriginKind) -> Self {
        self.origin = origin;
        self
    }

    pub(super) const fn origin(&self) -> CreateOriginKind {
        self.origin
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatingBarrier {
    operation: CreateOperation,
    anchor: CreateAnchor,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PendingCreateCommit {
    operation: CreateOperation,
    commit: WorkbenchTabCreateCommit,
}

impl PendingCreateCommit {
    pub(super) const fn new(operation: CreateOperation, commit: WorkbenchTabCreateCommit) -> Self {
        Self { operation, commit }
    }

    pub(super) const fn operation(&self) -> &CreateOperation {
        &self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CommittedTabCreate {
    operation: CreateOperation,
    committed_key: AttachmentKey,
    expectation: CreatedTabExpectation,
    provenance: CreateTerminalProvenance,
    anchor: CreateAnchor,
}

impl CommittedTabCreate {
    fn from_adapter(
        barrier: CreatingBarrier,
        commit: WorkbenchTabCreateCommit,
    ) -> Result<Self, Box<CreatingBarrier>> {
        let (revision, terminal_context_id, tab_id, panel_id, presentation_id, provenance) =
            match commit {
                WorkbenchTabCreateCommit::Pending {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id,
                } => (
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id,
                    CreateTerminalProvenance::Pending,
                ),
                WorkbenchTabCreateCommit::Created {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id,
                    terminal_outcome,
                } => {
                    if let WorkbenchTerminalOutcome::Started { geometry } = &terminal_outcome {
                        let expected = barrier.operation.geometry;
                        if geometry.columns() != expected.columns()
                            || geometry.rows() != expected.rows()
                            || geometry.cell_pixel_width() != expected.cell_pixel_width()
                            || geometry.cell_pixel_height() != expected.cell_pixel_height()
                        {
                            return Err(Box::new(barrier));
                        }
                    }
                    (
                        revision,
                        terminal_context_id,
                        tab_id,
                        panel_id,
                        presentation_id,
                        CreateTerminalProvenance::from_terminal_outcome(terminal_outcome),
                    )
                }
            };
        if terminal_context_id != barrier.operation.terminal_context_id
            || barrier.operation.expected_revision.value().checked_add(1) != Some(revision.value())
            || barrier.operation.origin != barrier.anchor.kind()
            || barrier.operation.key.reserved_generation
                <= barrier
                    .anchor
                    .ready()
                    .map_or(0, |ready| ready.selected.key.generation)
        {
            return Err(Box::new(barrier));
        }
        let committed_key =
            AttachmentKey::new(barrier.operation.key.reserved_generation, tab_id.clone());
        let expectation = CreatedTabExpectation::new(
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            barrier.operation.label.clone(),
        );
        Ok(Self {
            operation: barrier.operation,
            committed_key,
            expectation,
            provenance,
            anchor: barrier.anchor,
        })
    }

    pub(super) const fn operation(&self) -> &CreateOperation {
        &self.operation
    }

    #[cfg(test)]
    pub(super) const fn expectation(&self) -> &CreatedTabExpectation {
        &self.expectation
    }

    pub(super) const fn committed_key(&self) -> &AttachmentKey {
        &self.committed_key
    }

    #[cfg(test)]
    pub(super) const fn provenance(&self) -> &CreateTerminalProvenance {
        &self.provenance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttachTarget {
    selected: SelectedTab,
    site_id: SiteId,
    binding: NativeTerminalBinding,
    provenance: CreateTerminalProvenance,
    deadlines: OperationDeadlines,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BootstrapTarget {
    selected: SelectedTab,
    site_id: SiteId,
    binding: NativeTerminalBinding,
}

impl BootstrapTarget {
    pub(super) const fn selected(&self) -> &SelectedTab {
        &self.selected
    }

    pub(super) const fn site_id(&self) -> &SiteId {
        &self.site_id
    }

    pub(super) const fn binding(&self) -> &NativeTerminalBinding {
        &self.binding
    }
}

impl AttachTarget {
    pub(super) const fn selected(&self) -> &SelectedTab {
        &self.selected
    }

    pub(super) const fn site_id(&self) -> &SiteId {
        &self.site_id
    }

    pub(super) const fn binding(&self) -> &NativeTerminalBinding {
        &self.binding
    }

    pub(super) const fn provenance(&self) -> &CreateTerminalProvenance {
        &self.provenance
    }

    pub(super) const fn deadlines(&self) -> OperationDeadlines {
        self.deadlines
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SwitchSetup {
    target: AttachTarget,
    previous: Option<ExistingAttachment>,
    mode: Option<ControllerMode>,
    accepted_geometry: Option<Geometry>,
    membership: Option<SwitchMembership>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExistingAttachment {
    selected: SelectedTab,
    mode: Option<ControllerMode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StartingSetup {
    selected: SelectedTab,
    mode: Option<ControllerMode>,
    accepted_geometry: Option<Geometry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetiredState {
    failure: CarrierFailure,
    diagnostic_active: Option<SelectedTab>,
    committed: Option<CommittedTabCreate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SessionPhase {
    Empty,
    ReconcilingMembership(SelectedTab),
    Starting(StartingSetup),
    Ready(ReadyAttachment),
    Creating(CreatingBarrier),
    CreateCommittedNeedDetach(CommittedTabCreate),
    ProjectionUnavailableBeforeCleanup(CommittedTabCreate),
    Switching(SwitchSetup),
    SelectedWithoutAttachment(SelectedTab),
    RetryableAttachUnavailable(SelectedTab),
    Retired(RetiredState),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CarrierFailure {
    AuthorityDivergence,
    Deadline,
    IdentityExhausted,
    OutcomeUnknown,
    Protocol,
    Transport,
    Closed,
    WorkerExited,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CreateProjectionDecision {
    Cleanup(MembershipCleanup),
    Attach(AttachTarget),
    SettledWithoutAttachment,
    ProjectionUnavailable,
    Retire,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RefreshDecision {
    Continue,
    Cleanup(MembershipCleanup),
    AttachRecovered(AttachTarget),
    SettledWithoutAttachment,
    Retire,
}

#[derive(Debug)]
pub(super) enum PreparedRefresh {
    Stale,
    CreateMembership {
        generation: u64,
        operation: CreateOperationKey,
        prepared: PreparedMembershipCandidate,
    },
    Membership {
        generation: u64,
        prepared: PreparedMembershipCandidate,
    },
    DefiniteFailure {
        generation: u64,
        baseline_revision: WorkbenchRevision,
    },
    Append {
        generation: u64,
        baseline_revision: WorkbenchRevision,
        baseline_phase: SessionPhaseKind,
        active: SelectedTab,
        candidate: NativeContextTabs,
    },
    Divergent {
        generation: u64,
    },
}

impl PreparedRefresh {
    pub(super) fn needs_recovery_deadlines(&self) -> bool {
        if let Self::Membership { prepared, .. } = self {
            return !prepared.preserves_selection();
        }
        matches!(self, Self::CreateMembership { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadyDecision {
    Ready,
    CatchUpResize(Geometry),
    Superseded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SetupEventDisposition {
    Applied,
    Superseded { desired: AttachmentKey },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CleanupContinuation {
    Stable,
    Superseded { desired: AttachmentKey },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SelectionPlan {
    Busy,
    NoOp,
    OutOfRange(u8),
    Switch { index: usize },
}

enum PendingMutation {
    Move(MoveOperation),
    Rename(tab_rename::RenameOperation),
}

pub(super) struct NativeSessionState {
    cache: NativeContextTabs,
    context_selection: Option<context_selection::ContextSelection>,
    retained_contexts: Vec<context_selection::RetainedContext>,
    provenance: Vec<CreateTerminalProvenance>,
    phase: SessionPhase,
    desired: Option<AttachmentKey>,
    attached_key: Option<AttachmentKey>,
    projection_key: Option<AttachmentKey>,
    staged_frame: Option<(AttachmentKey, FrameIdentity)>,
    accepted_frame: Option<(AttachmentKey, FrameIdentity)>,
    latest_geometry: Geometry,
    last_admitted_geometry: Option<(AttachmentKey, Geometry)>,
    last_accepted_geometry: Option<(AttachmentKey, Geometry)>,
    notice: Option<SessionNotice>,
    pending_refresh_generation: Option<u64>,
    refresh_status: Option<NavigatorRefreshStatus>,
    pending_mutation: Option<PendingMutation>,
    move_recovery: Option<PendingMoveCommit>,
    settled_move: Option<PendingMoveCommit>,
    settled_rename: Option<tab_rename::PendingRenameCommit>,
    rename_recovery: Option<tab_rename::PendingRenameCommit>,
    rename_uncertain: Option<tab_rename::RenameOperation>,
    close: CloseState,
    rename: rename_draft::RenameDraftState,
    setup: workspace_setup::SetupState,
    pending_membership: Option<membership::PendingMembership>,
    settled_create: Option<CommittedTabCreate>,
}

impl NativeSessionState {
    pub(super) fn starting(
        cache: NativeContextTabs,
        generation: u64,
        geometry: Geometry,
    ) -> Option<(Self, BootstrapTarget)> {
        if generation == 0 {
            return None;
        }
        let first = cache.tabs().first()?;
        let selected = SelectedTab::new(AttachmentKey::new(generation, first.tab_id().clone()), 0);
        let desired = selected.key.clone();
        let target = BootstrapTarget {
            selected: selected.clone(),
            site_id: cache.site_id().clone(),
            binding: first.panel(0)?.binding().clone(),
        };
        let state = Self {
            context_selection: None,
            retained_contexts: Vec::new(),
            provenance: vec![CreateTerminalProvenance::BindingOnly; cache.tabs().len()],
            cache,
            phase: SessionPhase::Starting(StartingSetup {
                selected,
                mode: None,
                accepted_geometry: None,
            }),
            desired: Some(desired),
            attached_key: None,
            projection_key: None,
            staged_frame: None,
            accepted_frame: None,
            latest_geometry: geometry,
            last_admitted_geometry: None,
            last_accepted_geometry: None,
            notice: None,
            pending_refresh_generation: None,
            refresh_status: None,
            pending_mutation: None,
            move_recovery: None,
            settled_move: None,
            settled_rename: None,
            rename_recovery: None,
            rename_uncertain: None,
            close: CloseState::default(),
            rename: rename_draft::RenameDraftState::default(),
            setup: workspace_setup::SetupState::default(),
            pending_membership: None,
            settled_create: None,
        };
        Some((state, target))
    }

    pub(super) const fn phase_kind(&self) -> SessionPhaseKind {
        if self.rename_recovery.is_some() && !matches!(&self.phase, SessionPhase::Retired(_)) {
            return SessionPhaseKind::RenameProjectionUnavailable;
        }
        if self.move_recovery.is_some() && !matches!(&self.phase, SessionPhase::Retired(_)) {
            return SessionPhaseKind::MoveProjectionUnavailable;
        }
        match &self.phase {
            SessionPhase::Empty => SessionPhaseKind::Empty,
            SessionPhase::ReconcilingMembership(_) => SessionPhaseKind::Switching,
            SessionPhase::Starting(_) => SessionPhaseKind::Starting,
            SessionPhase::Ready(_) => SessionPhaseKind::Ready,
            SessionPhase::Creating(_) => SessionPhaseKind::Creating,
            SessionPhase::CreateCommittedNeedDetach(_) => {
                SessionPhaseKind::CreateCommittedNeedDetach
            }
            SessionPhase::ProjectionUnavailableBeforeCleanup(_) => {
                SessionPhaseKind::ProjectionUnavailable
            }
            SessionPhase::Switching(_) => SessionPhaseKind::Switching,
            SessionPhase::SelectedWithoutAttachment(_) => {
                SessionPhaseKind::SelectedWithoutAttachment
            }
            SessionPhase::RetryableAttachUnavailable(_) => {
                SessionPhaseKind::RetryableAttachUnavailable
            }
            SessionPhase::Retired(_) => SessionPhaseKind::Retired,
        }
    }

    pub(super) const fn cache(&self) -> &NativeContextTabs {
        &self.cache
    }

    pub(super) const fn desired(&self) -> Option<&AttachmentKey> {
        self.desired.as_ref()
    }

    pub(super) fn active(&self) -> Option<&SelectedTab> {
        match &self.phase {
            SessionPhase::Empty => None,
            SessionPhase::Starting(setup) => Some(&setup.selected),
            SessionPhase::SelectedWithoutAttachment(selected)
            | SessionPhase::ReconcilingMembership(selected)
            | SessionPhase::RetryableAttachUnavailable(selected) => Some(selected),
            SessionPhase::Ready(ready) => Some(&ready.selected),
            SessionPhase::Creating(barrier) => barrier.anchor.ready().map(|ready| &ready.selected),
            SessionPhase::CreateCommittedNeedDetach(committed)
            | SessionPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                committed.anchor.ready().map(|ready| &ready.selected)
            }
            SessionPhase::Switching(setup) => Some(&setup.target.selected),
            SessionPhase::Retired(retired) => retired.diagnostic_active.as_ref(),
        }
    }

    pub(super) fn controller_mode(&self) -> Option<&ControllerMode> {
        match &self.phase {
            SessionPhase::Empty => None,
            SessionPhase::ReconcilingMembership(_) => None,
            SessionPhase::Ready(ready) => Some(&ready.mode),
            SessionPhase::Creating(barrier) => barrier.anchor.ready().map(|ready| &ready.mode),
            SessionPhase::CreateCommittedNeedDetach(committed)
            | SessionPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                committed.anchor.ready().map(|ready| &ready.mode)
            }
            SessionPhase::Starting(setup) => setup.mode.as_ref(),
            SessionPhase::Switching(setup) => setup.mode.as_ref(),
            SessionPhase::SelectedWithoutAttachment(_)
            | SessionPhase::RetryableAttachUnavailable(_)
            | SessionPhase::Retired(_) => None,
        }
    }

    pub(super) const fn latest_geometry(&self) -> Geometry {
        self.latest_geometry
    }

    pub(super) fn last_admitted_geometry(&self, key: &AttachmentKey) -> Option<Geometry> {
        self.last_admitted_geometry
            .as_ref()
            .filter(|(recorded, _geometry)| recorded == key)
            .map(|(_recorded, geometry)| *geometry)
    }

    pub(super) fn last_accepted_geometry(&self, key: &AttachmentKey) -> Option<Geometry> {
        self.last_accepted_geometry
            .as_ref()
            .filter(|(recorded, _geometry)| recorded == key)
            .map(|(_recorded, geometry)| *geometry)
    }

    #[cfg(test)]
    pub(super) const fn notice(&self) -> Option<SessionNotice> {
        self.notice
    }

    pub(super) const fn pending_refresh_generation(&self) -> Option<u64> {
        self.pending_refresh_generation
    }

    #[cfg(test)]
    pub(super) const fn refresh_status(&self) -> Option<NavigatorRefreshStatus> {
        self.refresh_status
    }

    pub(super) fn provenance(&self, index: usize) -> Option<&CreateTerminalProvenance> {
        self.provenance.get(index)
    }

    pub(super) fn tab(&self, index: usize) -> Option<&NativeSimpleTab> {
        self.cache.tabs().get(index)
    }

    pub(super) fn ready(&self) -> Option<&ReadyAttachment> {
        let SessionPhase::Ready(ready) = &self.phase else {
            return None;
        };
        Some(ready)
    }

    pub(super) fn committed_target(&self) -> Option<&CommittedTabCreate> {
        match &self.phase {
            SessionPhase::CreateCommittedNeedDetach(committed)
            | SessionPhase::ProjectionUnavailableBeforeCleanup(committed) => Some(committed),
            SessionPhase::Retired(retired) => retired.committed.as_ref(),
            _ => None,
        }
    }

    pub(super) const fn retired_failure(&self) -> Option<CarrierFailure> {
        let SessionPhase::Retired(retired) = &self.phase else {
            return None;
        };
        Some(retired.failure)
    }

    pub(super) const fn projection_key(&self) -> Option<&AttachmentKey> {
        self.projection_key.as_ref()
    }

    pub(super) const fn attached_key(&self) -> Option<&AttachmentKey> {
        self.attached_key.as_ref()
    }

    pub(super) fn expected_frame_geometry(&self, key: &AttachmentKey) -> Option<Geometry> {
        if self.attached_key.as_ref() != Some(key) || self.projection_key.as_ref() != Some(key) {
            return None;
        }
        match self.controller_mode() {
            Some(ControllerMode::Controlling(_)) => self.last_accepted_geometry(key),
            Some(ControllerMode::ReadOnly) | None => None,
        }
    }

    pub(super) fn clear_notice(&mut self) {
        self.notice = None;
    }

    pub(super) fn set_notice(&mut self, notice: SessionNotice) {
        self.notice = Some(notice);
    }

    pub(super) fn observe_geometry(&mut self, geometry: Geometry) {
        self.latest_geometry = geometry;
    }

    pub(super) fn record_admitted_geometry(
        &mut self,
        key: &AttachmentKey,
        geometry: Geometry,
    ) -> bool {
        if self.attached_key.as_ref() != Some(key)
            || self.desired.as_ref() != Some(key)
            || !matches!(&self.phase, SessionPhase::Ready(ready)
                if ready.selected.key == *key
                    && matches!(ready.mode, ControllerMode::Controlling(_)))
        {
            return false;
        }
        self.last_admitted_geometry = Some((key.clone(), geometry));
        true
    }

    pub(super) fn record_setup_configuration(
        &mut self,
        key: &AttachmentKey,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> bool {
        if matches!(&mode, ControllerMode::Controlling(_)) != applied_geometry.is_some() {
            return false;
        }
        let accepted_geometry = match &mut self.phase {
            SessionPhase::Starting(setup)
                if setup.selected.key == *key && self.attached_key.as_ref() == Some(key) =>
            {
                setup.mode = Some(mode.clone());
                Some(&mut setup.accepted_geometry)
            }
            SessionPhase::Ready(ready)
                if ready.selected.key == *key
                    && ready.mode == mode
                    && self.attached_key.as_ref() == Some(key) =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            SessionPhase::Creating(barrier)
                if barrier
                    .anchor
                    .ready()
                    .is_some_and(|ready| ready.selected.key == *key && ready.mode == mode)
                    && self.attached_key.as_ref() == Some(key) =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            SessionPhase::CreateCommittedNeedDetach(committed)
                if committed
                    .anchor
                    .ready()
                    .is_some_and(|ready| ready.selected.key == *key && ready.mode == mode)
                    && self.attached_key.as_ref() == Some(key) =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            SessionPhase::Switching(setup)
                if setup.target.selected.key == *key
                    && setup.membership.is_none()
                    && setup.previous.is_none()
                    && self.attached_key.as_ref() == Some(key) =>
            {
                setup.mode = Some(mode.clone());
                Some(&mut setup.accepted_geometry)
            }
            SessionPhase::Switching(setup)
                if setup.previous.as_ref().is_some_and(|previous| {
                    previous.selected.key == *key && previous.mode.as_ref() == Some(&mode)
                }) && self.attached_key.as_ref() == Some(key) =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            _ => return false,
        };
        if let Some(accepted_geometry) = accepted_geometry {
            *accepted_geometry = applied_geometry;
        }
        if matches!(&self.phase, SessionPhase::Switching(setup)
            if setup.previous.as_ref().is_some_and(|previous| previous.selected.key == *key))
        {
            self.invalidate_switch_anchor();
        }
        self.last_accepted_geometry = applied_geometry.map(|geometry| (key.clone(), geometry));
        if self.projection_key.as_ref() == Some(key) {
            self.staged_frame = None;
            self.accepted_frame = None;
        }
        true
    }

    pub(super) fn settle_setup_configuration_event(
        &mut self,
        key: &AttachmentKey,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> Option<SetupEventDisposition> {
        if self.desired.as_ref() == Some(key) {
            return self
                .record_setup_configuration(key, mode, applied_geometry)
                .then_some(SetupEventDisposition::Applied);
        }
        if key.generation >= self.desired.as_ref()?.generation
            || matches!(&mode, ControllerMode::Controlling(_)) != applied_geometry.is_some()
            || self.attached_key.as_ref() != Some(key)
            || self.projection_key.is_some()
            || self.staged_frame.is_some()
            || self.accepted_frame.is_some()
        {
            return None;
        }
        let SessionPhase::Switching(setup) = &mut self.phase else {
            return None;
        };
        let previous = setup.previous.as_mut()?;
        if previous.selected.key != *key
            || previous
                .mode
                .as_ref()
                .is_some_and(|recorded| recorded != &mode)
        {
            return None;
        }
        previous.mode = Some(mode);
        self.last_accepted_geometry = applied_geometry.map(|geometry| (key.clone(), geometry));
        self.invalidate_switch_anchor();
        Some(SetupEventDisposition::Superseded {
            desired: self.desired.clone()?,
        })
    }

    pub(super) fn stage_drawn_frame(&mut self, key: &AttachmentKey, frame: FrameIdentity) -> bool {
        if self.attached_key.as_ref() != Some(key)
            || self.projection_key.as_ref() != Some(key)
            || self.desired.as_ref() != Some(key)
        {
            return false;
        }
        self.staged_frame = Some((key.clone(), frame));
        self.accepted_frame = None;
        true
    }

    pub(super) fn invalidate_projection_frame(&mut self, key: &AttachmentKey) -> bool {
        if self.attached_key.as_ref() != Some(key)
            || self.projection_key.as_ref() != Some(key)
            || self.desired.as_ref() != Some(key)
        {
            return false;
        }
        self.staged_frame = None;
        self.accepted_frame = None;
        true
    }

    pub(super) fn publish_ready(
        &mut self,
        key: &AttachmentKey,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> bool {
        let selected = match &self.phase {
            SessionPhase::Starting(setup) if setup.selected.key == *key => setup.selected.clone(),
            SessionPhase::Switching(setup)
                if setup.target.selected.key == *key && setup.membership.is_none() =>
            {
                setup.target.selected.clone()
            }
            _ => return false,
        };
        if self.attached_key.as_ref() != Some(key)
            || self.projection_key.as_ref() != Some(key)
            || self.accepted_frame.as_ref() != Some(&(key.clone(), frame))
            || self.controller_mode() != Some(&mode)
            || matches!(&mode, ControllerMode::Controlling(_)) != accepted_geometry.is_some()
        {
            return false;
        }
        if matches!(&mode, ControllerMode::Controlling(_)) {
            let Some(geometry) = accepted_geometry else {
                return false;
            };
            if geometry != self.latest_geometry
                || self.last_accepted_geometry(key) != Some(geometry)
            {
                return false;
            }
        }
        self.projection_key = Some(key.clone());
        self.phase = SessionPhase::Ready(ReadyAttachment::new(selected, mode));
        true
    }

    pub(super) fn settle_ready_candidate(
        &mut self,
        key: &AttachmentKey,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> Result<ReadyDecision, CarrierFailure> {
        if self.desired.as_ref() != Some(key) {
            return Ok(ReadyDecision::Superseded);
        }
        let selected_matches = match &self.phase {
            SessionPhase::Starting(setup) => setup.selected.key == *key,
            SessionPhase::Switching(setup) => {
                setup.target.selected.key == *key && setup.membership.is_none()
            }
            _ => false,
        };
        if !selected_matches
            || self.projection_key.as_ref() != Some(key)
            || self.accepted_frame.as_ref() != Some(&(key.clone(), frame))
            || self.controller_mode() != Some(&mode)
            || matches!(&mode, ControllerMode::Controlling(_)) != accepted_geometry.is_some()
        {
            return Err(CarrierFailure::Protocol);
        }
        if let ControllerMode::Controlling(_) = &mode {
            let Some(geometry) = accepted_geometry else {
                return Err(CarrierFailure::Protocol);
            };
            if self.last_accepted_geometry(key) != Some(geometry) {
                return Err(CarrierFailure::Protocol);
            }
            if self.latest_geometry != geometry {
                return Ok(ReadyDecision::CatchUpResize(self.latest_geometry));
            }
        }
        if !self.publish_ready(key, frame, mode, accepted_geometry) {
            return Err(CarrierFailure::Protocol);
        }
        Ok(ReadyDecision::Ready)
    }

    pub(super) fn install_projection(&mut self, key: &AttachmentKey) -> bool {
        if self.desired.as_ref() != Some(key)
            || self.attached_key.is_some()
            || !matches!(&self.phase, SessionPhase::Starting(_))
                && !matches!(&self.phase, SessionPhase::Switching(setup)
                    if setup.previous.is_none() && setup.membership.is_none())
        {
            return false;
        }
        self.attached_key = Some(key.clone());
        self.projection_key = Some(key.clone());
        self.staged_frame = None;
        self.accepted_frame = None;
        true
    }

    pub(super) fn settle_attached_event(
        &mut self,
        key: &AttachmentKey,
    ) -> Option<SetupEventDisposition> {
        let desired = self.desired.clone()?;
        if self.install_projection(key) {
            return Some(SetupEventDisposition::Applied);
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (key.generation < self.desired.as_ref()?.generation
            && setup.previous.is_none()
            && self.attached_key.is_none()
            && self.projection_key.is_none()
            && self.staged_frame.is_none()
            && self.accepted_frame.is_none()
            && self
                .cache
                .tabs()
                .iter()
                .any(|tab| tab.tab_id() == key.tab_id()))
        .then_some(SetupEventDisposition::Superseded { desired })
    }

    pub(super) fn mark_feedback_accepted(
        &mut self,
        key: &AttachmentKey,
        frame: FrameIdentity,
    ) -> bool {
        if self.attached_key.as_ref() != Some(key)
            || self.projection_key.as_ref() != Some(key)
            || self.desired.as_ref() != Some(key)
            || self.staged_frame.as_ref() != Some(&(key.clone(), frame))
        {
            return false;
        }
        self.accepted_frame = Some((key.clone(), frame));
        true
    }

    pub(super) fn settle_feedback_event(
        &mut self,
        key: &AttachmentKey,
        frame: FrameIdentity,
    ) -> Option<SetupEventDisposition> {
        let desired = self.desired.clone()?;
        if self.mark_feedback_accepted(key, frame) {
            return Some(SetupEventDisposition::Applied);
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (key.generation < self.desired.as_ref()?.generation
            && setup
                .previous
                .as_ref()
                .is_some_and(|previous| previous.selected.key == *key)
            && self.attached_key.as_ref() == Some(key)
            && self.projection_key.is_none()
            && self.staged_frame.is_none()
            && self.accepted_frame.is_none())
        .then_some(SetupEventDisposition::Superseded { desired })
    }

    #[cfg(test)]
    pub(super) fn feedback_accepted(&self) -> bool {
        self.accepted_frame
            .as_ref()
            .is_some_and(|(key, _frame)| Some(key) == self.desired.as_ref())
    }

    pub(super) fn begin_create(&mut self, operation: CreateOperation) -> bool {
        if !self.invariant_holds()
            || self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
        {
            return false;
        }
        let anchor = match (&self.phase, operation.origin) {
            (SessionPhase::Ready(ready), CreateOriginKind::Ready) => {
                CreateAnchor::Ready(ready.clone())
            }
            (SessionPhase::Empty, CreateOriginKind::Empty) => CreateAnchor::Empty,
            _ => return false,
        };
        if anchor.ready().map(|ready| &ready.selected.key) != self.desired.as_ref()
            || self.attached_key.as_ref() != self.desired.as_ref()
            || self.projection_key.as_ref() != self.desired.as_ref()
            || self.pending_refresh_generation.is_some()
            || operation.site_id != *self.cache.site_id()
            || operation.expected_revision != self.cache.revision()
            || operation.terminal_context_id != *self.cache.terminal_context_id()
            || operation.geometry != self.latest_geometry
            || operation.key.reserved_generation
                <= anchor
                    .ready()
                    .map_or(0, |ready| ready.selected.key.generation)
        {
            return false;
        }
        self.phase = SessionPhase::Creating(CreatingBarrier { operation, anchor });
        true
    }

    pub(super) fn restore_precommit(
        &mut self,
        operation: &CreateOperationKey,
        applied_geometry: Option<Geometry>,
    ) -> bool {
        let SessionPhase::Creating(barrier) = &self.phase else {
            return false;
        };
        if barrier.operation.key != *operation {
            return false;
        }
        let Some(old_ready) = barrier.anchor.ready().cloned() else {
            if applied_geometry.is_some()
                || self.attached_key.is_some()
                || self.projection_key.is_some()
                || self.desired.is_some()
                || !self.cache.tabs().is_empty()
            {
                return false;
            }
            self.phase = SessionPhase::Empty;
            self.notice = Some(SessionNotice::CreateNotCreated);
            return true;
        };
        if self.attached_key.as_ref() != Some(&old_ready.selected.key)
            || self.projection_key.as_ref() != Some(&old_ready.selected.key)
            || matches!(&old_ready.mode, ControllerMode::Controlling(_))
                != applied_geometry.is_some()
        {
            return false;
        }
        if matches!(&old_ready.mode, ControllerMode::Controlling(_)) {
            let changed = self.last_accepted_geometry(&old_ready.selected.key) != applied_geometry;
            self.last_accepted_geometry =
                applied_geometry.map(|geometry| (old_ready.selected.key.clone(), geometry));
            if changed {
                self.staged_frame = None;
                self.accepted_frame = None;
            }
        }
        self.phase = SessionPhase::Ready(old_ready);
        self.notice = Some(SessionNotice::CreateNotCreated);
        true
    }

    pub(super) fn retain_create_commit(
        &mut self,
        operation: &CreateOperationKey,
        commit: WorkbenchTabCreateCommit,
    ) -> bool {
        let SessionPhase::Creating(barrier) = &self.phase else {
            return false;
        };
        if barrier.operation.key != *operation {
            return false;
        }
        let barrier = barrier.clone();
        let Ok(committed) = CommittedTabCreate::from_adapter(barrier, commit) else {
            self.retire(CarrierFailure::Protocol);
            return false;
        };
        self.phase = SessionPhase::CreateCommittedNeedDetach(committed);
        true
    }

    pub(super) fn begin_switch(&mut self, target: AttachTarget) -> bool {
        if self.rename_draft_blocks_structure() {
            return false;
        }
        self.begin_switch_membership(target)
    }
    pub(super) fn settle_switch_entry_expired(
        &mut self,
        key: &AttachmentKey,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
        settled_at: Instant,
    ) -> Option<CleanupContinuation> {
        let desired = self.desired.clone()?;
        if self.desired.as_ref() != Some(key) {
            return (key.generation < self.desired.as_ref()?.generation
                && self.cache.tabs().get(target_index)?.tab_id() == key.tab_id()
                && self.provenance.get(target_index)? == target_provenance)
                .then_some(CleanupContinuation::Superseded { desired });
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        if setup.previous.is_some()
            || setup.membership.is_some()
            || setup.target.selected.key != *key
            || setup.target.selected.index != target_index
            || &setup.target.provenance != target_provenance
            || settled_at < setup.target.deadlines.forward_cutoff()
            || self.attached_key.is_some()
            || self.projection_key.is_some()
            || self.staged_frame.is_some()
            || self.accepted_frame.is_some()
        {
            return None;
        }
        let selected = setup.target.selected.clone();
        self.phase = if target_provenance.attachable() {
            SessionPhase::RetryableAttachUnavailable(selected)
        } else {
            SessionPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    pub(super) fn settle_cleanup(
        &mut self,
        key: &AttachmentKey,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        if self.desired.as_ref() != Some(key) {
            let installed_previous = setup
                .previous
                .as_ref()
                .is_some_and(|previous| previous.selected.key == *key)
                && self.attached_key.as_ref() == Some(key);
            let uninstalled_setup = setup.previous.is_none()
                && self.attached_key.is_none()
                && self.projection_key.is_none()
                && key.generation < self.desired.as_ref()?.generation
                && self.cache.tabs().get(target_index)?.tab_id() == key.tab_id()
                && self.provenance.get(target_index)? == target_provenance;
            if !installed_previous && !uninstalled_setup {
                return None;
            }
            if installed_previous {
                let SessionPhase::Switching(setup) = &mut self.phase else {
                    return None;
                };
                setup.previous = None;
                if let Some(membership) = &mut setup.membership {
                    membership.invalidate();
                }
                self.attached_key = None;
                if self.projection_key.as_ref() == Some(key) {
                    self.projection_key = None;
                    self.staged_frame = None;
                    self.accepted_frame = None;
                }
                if self
                    .last_accepted_geometry
                    .as_ref()
                    .is_some_and(|(recorded, _)| recorded == key)
                {
                    self.last_accepted_geometry = None;
                }
            }
            return Some(CleanupContinuation::Superseded {
                desired: self.desired.clone()?,
            });
        }
        if setup.previous.is_some() || setup.membership.is_some() {
            return None;
        }
        if setup.target.selected.key != *key
            || self
                .attached_key
                .as_ref()
                .is_some_and(|attached| attached != key)
            || self.attached_key.is_none()
                && (self.projection_key.is_some()
                    || self.staged_frame.is_some()
                    || self.accepted_frame.is_some())
        {
            return None;
        }
        self.attached_key = None;
        self.projection_key = None;
        self.staged_frame = None;
        self.accepted_frame = None;
        self.last_accepted_geometry = None;
        if self.cache.tabs().get(target_index)?.tab_id() != key.tab_id()
            || self.provenance.get(target_index)? != target_provenance
        {
            return None;
        }
        let selected = SelectedTab::new(key.clone(), target_index);
        self.phase = if target_provenance.attachable() {
            SessionPhase::RetryableAttachUnavailable(selected)
        } else {
            SessionPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    pub(super) fn settle_unattached_setup(
        &mut self,
        key: &AttachmentKey,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        if self.desired.as_ref() != Some(key) {
            return Some(CleanupContinuation::Superseded {
                desired: self.desired.clone()?,
            });
        }
        if setup.previous.is_some()
            || setup.membership.is_some()
            || setup.target.selected.key != *key
            || self.attached_key.is_some()
            || self.cache.tabs().get(target_index)?.tab_id() != key.tab_id()
            || self.provenance.get(target_index)? != target_provenance
        {
            return None;
        }
        let selected = SelectedTab::new(key.clone(), target_index);
        self.phase = if target_provenance.attachable() {
            SessionPhase::RetryableAttachUnavailable(selected)
        } else {
            SessionPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    pub(super) fn start_pending_refresh(&mut self, generation: u64) -> bool {
        if self.pending_refresh_generation.is_some() || !self.phase_allows_refresh() {
            return false;
        }
        self.pending_refresh_generation = Some(generation);
        self.refresh_status = Some(NavigatorRefreshStatus::Refreshing);
        true
    }

    pub(super) fn prepare_refresh(
        &self,
        generation: u64,
        outcome: WorkbenchSnapshotOutcome,
    ) -> PreparedRefresh {
        if self.pending_refresh_generation != Some(generation) {
            return PreparedRefresh::Stale;
        }
        let baseline_revision = self.cache.revision();
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return PreparedRefresh::DefiniteFailure {
                generation,
                baseline_revision,
            };
        };
        if self.setup_recovery() {
            return self.prepare_setup_refresh(generation, &snapshot);
        }
        if self.context_selection.is_some() {
            return self.prepare_context_selection(generation, &snapshot);
        }
        let Ok(candidate) = self.cache.project_snapshot(&snapshot) else {
            return PreparedRefresh::Divergent { generation };
        };
        if let SessionPhase::ProjectionUnavailableBeforeCleanup(committed) = &self.phase {
            let Ok(projection) =
                validate_created_tab(&self.cache, &committed.expectation, candidate)
            else {
                return PreparedRefresh::Divergent { generation };
            };
            return match self.prepare_create_membership_candidate(
                &self.cache,
                committed,
                projection,
            ) {
                Ok(prepared) => PreparedRefresh::CreateMembership {
                    generation,
                    operation: committed.operation.key.clone(),
                    prepared,
                },
                Err(_) => PreparedRefresh::Divergent { generation },
            };
        }

        let active = self.active().cloned();
        let validated = if let Some(pending) = &self.move_recovery {
            pending.validate_projection(candidate)
        } else if let Some(pending) = &self.rename_recovery {
            pending
                .validate_projection(candidate)
                .map(|projection| projection.candidate)
        } else {
            validate_canonical_refresh(&self.cache, candidate)
        };
        let Ok(candidate) = validated else {
            return PreparedRefresh::Divergent { generation };
        };
        if active
            .as_ref()
            .is_none_or(|active| candidate.tab_index(active.key.tab_id()).is_none())
        {
            return match self.prepare_membership_candidate(
                candidate,
                active.as_ref().map(|active| active.key.tab_id()),
                active.as_ref().map(|active| active.index),
                None,
            ) {
                Ok(prepared) => PreparedRefresh::Membership {
                    generation,
                    prepared,
                },
                Err(_) => PreparedRefresh::Divergent { generation },
            };
        }
        let Some(active) = active else {
            return PreparedRefresh::Divergent { generation };
        };
        if candidate.tab_index(active.key.tab_id()).is_none() {
            return PreparedRefresh::Divergent { generation };
        }
        let baseline_phase = self.phase_kind();
        match baseline_phase {
            SessionPhaseKind::Ready
            | SessionPhaseKind::MoveProjectionUnavailable
            | SessionPhaseKind::RenameProjectionUnavailable
            | SessionPhaseKind::SelectedWithoutAttachment
            | SessionPhaseKind::RetryableAttachUnavailable => PreparedRefresh::Append {
                generation,
                baseline_revision,
                baseline_phase,
                active,
                candidate,
            },
            _ => PreparedRefresh::Divergent { generation },
        }
    }

    pub(super) fn apply_prepared_refresh(&mut self, prepared: PreparedRefresh) -> RefreshDecision {
        let generation = match &prepared {
            PreparedRefresh::Stale => return RefreshDecision::Continue,
            PreparedRefresh::DefiniteFailure { generation, .. }
            | PreparedRefresh::CreateMembership { generation, .. }
            | PreparedRefresh::Membership { generation, .. }
            | PreparedRefresh::Append { generation, .. }
            | PreparedRefresh::Divergent { generation } => *generation,
        };
        if self.pending_refresh_generation != Some(generation) {
            return RefreshDecision::Continue;
        }
        let decision = match prepared {
            PreparedRefresh::Membership { .. } | PreparedRefresh::CreateMembership { .. } => {
                self.retire(CarrierFailure::Protocol);
                RefreshDecision::Retire
            }
            PreparedRefresh::Stale => RefreshDecision::Continue,
            PreparedRefresh::Divergent { .. } => {
                self.retire(CarrierFailure::AuthorityDivergence);
                RefreshDecision::Retire
            }
            PreparedRefresh::DefiniteFailure {
                baseline_revision, ..
            } => {
                if self.cache.revision() != baseline_revision {
                    self.retire(CarrierFailure::Protocol);
                    RefreshDecision::Retire
                } else {
                    self.refresh_status = Some(NavigatorRefreshStatus::Failure(baseline_revision));
                    RefreshDecision::Continue
                }
            }
            PreparedRefresh::Append {
                baseline_revision,
                baseline_phase,
                active,
                candidate,
                ..
            } => {
                if self.cache.revision() != baseline_revision
                    || self.phase_kind() != baseline_phase
                    || self.active() != Some(&active)
                {
                    self.retire(CarrierFailure::Protocol);
                    RefreshDecision::Retire
                } else if self.merge_projection(candidate) {
                    self.move_recovery = None;
                    self.rename_recovery = None;
                    self.refresh_status =
                        Some(NavigatorRefreshStatus::Success(self.cache.revision()));
                    RefreshDecision::Continue
                } else {
                    self.retire(CarrierFailure::AuthorityDivergence);
                    RefreshDecision::Retire
                }
            }
        };
        if !matches!(decision, RefreshDecision::Retire) {
            self.pending_refresh_generation = None;
            self.context_selection = None;
        }
        decision
    }

    fn merge_projection(&mut self, candidate: NativeContextTabs) -> bool {
        if !self.invariant_holds() {
            return false;
        }
        let Some(desired_index) = self
            .desired
            .as_ref()
            .and_then(|desired| candidate.tab_index(desired.tab_id()))
        else {
            return false;
        };
        let selected_panel_id = self
            .active()
            .and_then(|active| {
                self.cache
                    .tabs()
                    .get(active.index())
                    .and_then(|tab| tab.panel(active.panel_index()))
            })
            .map(|panel| panel.panel_id().clone());
        let Some(selected_panel_index) = candidate.tabs().get(desired_index).and_then(|tab| {
            selected_panel_id
                .as_ref()
                .map_or(Some(0), |panel_id| tab.panel_index(panel_id))
        }) else {
            return false;
        };
        let mut phase = self.phase.clone();
        match &mut phase {
            SessionPhase::Ready(ready) => {
                ready.selected.index = desired_index;
                ready.selected.panel_index = selected_panel_index;
            }
            SessionPhase::SelectedWithoutAttachment(selected)
            | SessionPhase::RetryableAttachUnavailable(selected) => {
                selected.index = desired_index;
                selected.panel_index = selected_panel_index;
            }
            _ => return false,
        }
        let provenance: Vec<_> = candidate
            .tabs()
            .iter()
            .map(|tab| {
                self.cache
                    .tab_index(tab.tab_id())
                    .map_or(CreateTerminalProvenance::BindingOnly, |index| {
                        self.provenance[index].clone()
                    })
            })
            .collect();
        self.phase = phase;
        self.provenance = provenance;
        self.cache = candidate;
        true
    }

    fn attach_target(
        &self,
        selected: SelectedTab,
        provenance: CreateTerminalProvenance,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        let tab = &self.cache.tabs()[selected.index];
        let binding = tab.panel(selected.panel_index())?.binding().clone();
        Some(AttachTarget {
            selected,
            site_id: self.cache.site_id().clone(),
            binding,
            provenance,
            deadlines,
        })
    }

    pub(super) fn target_for_switch(
        &self,
        index: usize,
        key: AttachmentKey,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        if key.generation <= self.desired.as_ref()?.generation
            || self.cache.tabs().get(index)?.tab_id() != key.tab_id()
        {
            return None;
        }
        let provenance = self.provenance.get(index)?.clone();
        self.attach_target(SelectedTab::new(key, index), provenance, deadlines)
    }

    pub(super) fn plan_panel_focus(
        &self,
        direction: crate::client_view::PanelDirection,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.pending_refresh_generation.is_some()
        {
            return None;
        }
        let active = self.active()?;
        let tab = self.cache.tabs().get(active.index())?;
        let panel_index = tab.panel_focus_target(active.panel_index(), direction)?;
        if panel_index == active.panel_index() || generation <= self.desired()?.generation() {
            return None;
        }
        let key = AttachmentKey::new(generation, tab.tab_id().clone());
        let selected = SelectedTab::new_panel(key, active.index(), panel_index);
        let provenance = self.provenance.get(active.index())?.clone();
        self.attach_target(selected, provenance, deadlines)
    }

    pub(super) fn plan_selection(&self, action: ClientAction) -> SelectionPlan {
        if self.mutation_blocks_structure() || self.rename_draft_blocks_structure() {
            return SelectionPlan::Busy;
        }
        if matches!(&self.phase, SessionPhase::Retired(_)) {
            return SelectionPlan::Busy;
        }
        let count = self.cache.tabs().len();
        let Some(anchor) = self.active().map(SelectedTab::index) else {
            return SelectionPlan::Busy;
        };
        let index = match action {
            ClientAction::SelectNextTab if count != 0 => (anchor + 1) % count,
            ClientAction::SelectPreviousTab if count != 0 => (anchor + count - 1) % count,
            ClientAction::SelectTab(position) => {
                let Some(index) = usize::from(position).checked_sub(1) else {
                    return SelectionPlan::OutOfRange(position);
                };
                if index >= count {
                    return SelectionPlan::OutOfRange(position);
                }
                index
            }
            _ => return SelectionPlan::Busy,
        };
        if self.pending_refresh_generation.is_some() {
            return if index == anchor {
                SelectionPlan::NoOp
            } else {
                SelectionPlan::Busy
            };
        }
        if !matches!(
            &self.phase,
            SessionPhase::Ready(_)
                | SessionPhase::Switching(_)
                | SessionPhase::SelectedWithoutAttachment(_)
                | SessionPhase::RetryableAttachUnavailable(_)
        ) {
            return SelectionPlan::Busy;
        }
        if index != anchor {
            return SelectionPlan::Switch { index };
        }
        match &self.phase {
            SessionPhase::RetryableAttachUnavailable(_)
                if self
                    .provenance
                    .get(index)
                    .is_some_and(CreateTerminalProvenance::attachable) =>
            {
                SelectionPlan::Switch { index }
            }
            SessionPhase::Ready(_)
            | SessionPhase::Switching(_)
            | SessionPhase::SelectedWithoutAttachment(_)
            | SessionPhase::RetryableAttachUnavailable(_) => SelectionPlan::NoOp,
            _ => SelectionPlan::Busy,
        }
    }

    pub(super) fn phase_allows_refresh(&self) -> bool {
        self.pending_mutation.is_none()
            && !self.setup_pending_or_draft()
            && !self.rename_draft_blocks_structure()
            && matches!(
                &self.phase,
                SessionPhase::Empty
                    | SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
                    | SessionPhase::ProjectionUnavailableBeforeCleanup(_)
            )
    }

    #[cfg(test)]
    pub(super) fn selected_has_viewer(&self) -> bool {
        self.attached_key.is_some()
    }

    pub(super) fn finalize_retirement(
        &mut self,
        failure: CarrierFailure,
        pending: Option<PendingCreateCommit>,
    ) -> bool {
        let accepted = pending.is_none_or(|pending| self.retain_retirement_commit(pending));
        self.retire(failure);
        accepted
    }

    fn retain_retirement_commit(&mut self, pending: PendingCreateCommit) -> bool {
        let barrier = match &self.phase {
            SessionPhase::Creating(barrier) if barrier.operation == pending.operation => {
                barrier.clone()
            }
            _ => {
                let Some(retained) = self.committed_target() else {
                    return false;
                };
                if retained.operation != pending.operation {
                    return false;
                }
                CreatingBarrier {
                    operation: retained.operation.clone(),
                    anchor: retained.anchor.clone(),
                }
            }
        };
        let Ok(committed) = CommittedTabCreate::from_adapter(barrier, pending.commit) else {
            return false;
        };
        if matches!(&self.phase, SessionPhase::Creating(_)) {
            self.phase = SessionPhase::CreateCommittedNeedDetach(committed);
            true
        } else {
            self.committed_target() == Some(&committed)
        }
    }

    pub(super) fn retire(&mut self, failure: CarrierFailure) {
        self.retire_close_reconciliations();
        self.cancel_rename_draft();
        self.cancel_setup_on_retirement();
        if matches!(&self.phase, SessionPhase::Retired(_)) {
            return;
        }
        let diagnostic_active = self.active().cloned();
        let committed = self.committed_target().cloned();
        self.phase = SessionPhase::Retired(RetiredState {
            failure,
            diagnostic_active,
            committed,
        });
        self.attached_key = None;
        self.projection_key = None;
        self.staged_frame = None;
        self.accepted_frame = None;
        self.pending_refresh_generation = None;
        self.context_selection = None;
        self.pending_membership = None;
        self.refresh_status = None;
    }

    pub(super) fn view(&self) -> NativeSessionView<'_> {
        NativeSessionView { state: self }
    }

    pub(super) fn invariant_holds(&self) -> bool {
        if !self.membership_invariant_holds() || !self.context_invariant_holds() {
            return false;
        }
        if !self.move_invariant_holds()
            || !self.rename_invariant_holds()
            || !self.setup_invariant_holds()
        {
            return false;
        }
        if self.provenance.len() != self.cache.tabs().len() {
            return false;
        }
        if let Some(active) = self.active()
            && self.cache.tabs().get(active.index).is_none_or(|tab| {
                tab.tab_id() != active.key.tab_id() || active.panel_index() >= tab.panel_count()
            })
        {
            return false;
        }
        if matches!(
            &self.phase,
            SessionPhase::SelectedWithoutAttachment(_)
                | SessionPhase::RetryableAttachUnavailable(_)
                | SessionPhase::Retired(_)
        ) && self.projection_key.is_some()
        {
            return false;
        }
        if self
            .projection_key
            .as_ref()
            .is_some_and(|key| self.attached_key.as_ref() != Some(key))
            || self
                .staged_frame
                .as_ref()
                .is_some_and(|(key, _)| self.projection_key.as_ref() != Some(key))
            || self.accepted_frame.as_ref().is_some_and(|accepted| {
                self.staged_frame.as_ref() != Some(accepted)
                    || self.projection_key.as_ref() != Some(&accepted.0)
            })
        {
            return false;
        }
        let attachment_shape_valid = match &self.phase {
            SessionPhase::ReconcilingMembership(selected) => {
                Some(&selected.key) == self.desired.as_ref()
                    && self.attached_key.as_ref() == self.desired.as_ref()
            }
            SessionPhase::Empty => {
                self.cache.tabs().is_empty()
                    && self.provenance.is_empty()
                    && self.desired.is_none()
                    && self.attached_key.is_none()
                    && self.projection_key.is_none()
                    && self.staged_frame.is_none()
                    && self.accepted_frame.is_none()
                    && self.last_admitted_geometry.is_none()
                    && self.last_accepted_geometry.is_none()
            }
            SessionPhase::Starting(setup) => {
                Some(&setup.selected.key) == self.desired.as_ref()
                    && self
                        .attached_key
                        .as_ref()
                        .is_none_or(|key| Some(key) == self.desired.as_ref())
            }
            SessionPhase::Ready(ready) => {
                Some(&ready.selected.key) == self.desired.as_ref()
                    && self.attached_key.as_ref() == self.desired.as_ref()
                    && self.projection_key.as_ref() == self.desired.as_ref()
            }
            SessionPhase::Creating(barrier) => {
                barrier.anchor.ready().map(|ready| &ready.selected.key) == self.desired.as_ref()
                    && self.attached_key.as_ref() == self.desired.as_ref()
                    && self.projection_key.as_ref() == self.desired.as_ref()
            }
            SessionPhase::CreateCommittedNeedDetach(committed)
            | SessionPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                committed.anchor.ready().map(|ready| &ready.selected.key) == self.desired.as_ref()
                    && self.attached_key.as_ref() == self.desired.as_ref()
                    && self.projection_key.as_ref() == self.desired.as_ref()
            }
            SessionPhase::Switching(setup) => {
                Some(&setup.target.selected.key) == self.desired.as_ref()
                    && match &setup.previous {
                        Some(previous) => {
                            self.attached_key.as_ref() == Some(&previous.selected.key)
                        }
                        None => self
                            .attached_key
                            .as_ref()
                            .is_none_or(|key| Some(key) == self.desired.as_ref()),
                    }
            }
            SessionPhase::SelectedWithoutAttachment(selected)
            | SessionPhase::RetryableAttachUnavailable(selected) => {
                Some(&selected.key) == self.desired.as_ref() && self.attached_key.is_none()
            }
            SessionPhase::Retired(retired) => {
                let diagnostic_matches_desired = retired
                    .diagnostic_active
                    .as_ref()
                    .is_none_or(|selected| Some(&selected.key) == self.desired.as_ref());
                let committed_shape_valid = match (
                    retired.diagnostic_active.as_ref(),
                    retired.committed.as_ref(),
                ) {
                    (Some(active), Some(committed)) => {
                        committed
                            .anchor
                            .ready()
                            .is_some_and(|ready| ready.selected == *active)
                            && Some(&active.key) == self.desired.as_ref()
                    }
                    (None, Some(committed)) => {
                        Some(&committed.committed_key) == self.desired.as_ref()
                            || (committed.anchor.kind() == CreateOriginKind::Empty
                                && self.desired.is_none()
                                && self.cache.tabs().is_empty())
                    }
                    (Some(_), None) => true,
                    (None, None) => self.cache.tabs().is_empty() && self.desired.is_none(),
                };
                self.attached_key.is_none()
                    && self.pending_refresh_generation.is_none()
                    && diagnostic_matches_desired
                    && committed_shape_valid
            }
        };
        if !attachment_shape_valid {
            return false;
        }
        true
    }
}

pub(super) struct NativeSessionView<'a> {
    state: &'a NativeSessionState,
}

impl<'a> NativeSessionView<'a> {
    pub(super) const fn phase(&self) -> SessionPhaseKind {
        self.state.phase_kind()
    }

    pub(super) fn tab_count(&self) -> usize {
        self.state.cache.tabs().len()
    }

    pub(super) fn tab(&self, index: usize) -> Option<NativeTabView<'a>> {
        let tab = self.state.cache.tabs().get(index)?;
        let active_panel = self
            .state
            .active()
            .filter(|active| active.index() == index)
            .map(SelectedTab::panel_index);
        Some(NativeTabView {
            tab,
            position: index + 1,
            active: active_panel.is_some(),
            active_panel,
        })
    }

    pub(super) fn active_position(&self) -> Option<usize> {
        self.state
            .active()
            .map(|selected| selected.index().saturating_add(1))
    }

    pub(super) fn controller_mode(&self) -> Option<&ControllerMode> {
        self.state.controller_mode()
    }

    pub(super) fn selected_provenance(&self) -> Option<&CreateTerminalProvenance> {
        self.state
            .active()
            .and_then(|selected| self.state.provenance(selected.index()))
    }

    pub(super) const fn notice(&self) -> Option<SessionNotice> {
        self.state.notice
    }

    pub(super) const fn refresh_status(&self) -> Option<NavigatorRefreshStatus> {
        self.state.refresh_status
    }

    #[cfg(test)]
    pub(super) const fn retired_failure(&self) -> Option<CarrierFailure> {
        self.state.retired_failure()
    }
}

#[derive(Clone, Copy)]
pub(super) struct NativeTabView<'a> {
    tab: &'a NativeSimpleTab,
    position: usize,
    active: bool,
    active_panel: Option<usize>,
}

impl<'a> NativeTabView<'a> {
    pub(super) const fn position(&self) -> usize {
        self.position
    }

    pub(super) const fn active(&self) -> bool {
        self.active
    }

    pub(super) fn tab_label(&self) -> &'a str {
        self.tab.tab_label().as_str()
    }

    pub(super) fn panel_count(&self) -> usize {
        self.tab.panel_count()
    }

    pub(super) fn panel_tree(&self) -> &'a super::NativePanelTree {
        self.tab.panel_tree()
    }

    pub(super) fn active_panel(&self) -> Option<usize> {
        self.active_panel
    }

    pub(super) fn panel(&self, index: usize) -> Option<&'a super::NativePanel> {
        self.tab.panel(index)
    }

    #[cfg(test)]
    pub(super) fn presentation_label(&self) -> &'a str {
        self.tab.presentation_label().as_str()
    }
}

#[cfg(test)]
mod tests;
