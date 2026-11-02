//! Owns Client-local attachment, membership, selection, and pending-operation transitions
//! against validated canonical projections and exact completion evidence. It performs no
//! transport I/O and cannot mint Server membership or infer physical cleanup from a missing
//! projection.

use crate::client_view::ClientAction;
use crate::exchange_values::workbench::WorkbenchTabCreateCommit;
use crate::workbench_projection::{
    ClientContextTabs, ClientTab, ClientTerminalBinding, CreatedTabExpectation,
    validate_canonical_refresh, validate_created_tab,
};
use orchbee_client_server_protocol::{ControllerId, Geometry};
use orchbee_workbench_protocol::{
    SessionId, WorkbenchLabel, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchTerminalFailureReason, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
};
use std::time::{Duration, Instant};

#[cfg(test)]
use crate::workbench_projection::project_preferred_workspace;
mod close_impact;
pub(crate) use close_impact::CloseTerminal;
mod context_removal;
mod context_selection;
pub(crate) mod control_takeover;
mod field_window;
pub(crate) use crate::terminal_text::line_editor;
mod tab_groups;
pub(crate) use tab_groups::TabSelectionChange;
pub(crate) use tab_groups::TabStripStart;
pub(crate) mod workspace_close;
pub(crate) mod workspace_setup;
pub(crate) mod worktree_launch;
pub(crate) use context_selection::ClientContextView;
pub(crate) use field_window::FieldWindow;
mod create_membership;
mod tab_duplication;
pub(crate) use tab_duplication::{DuplicateProgress, DuplicateStop};
mod membership;
mod move_tab;
mod navigation_memory;
mod panel;
mod panel_close;
mod plugin_overlays;
mod presentations;
mod recovery_fence;
pub(crate) mod rename_draft;
pub(crate) mod retired_membership;
pub(crate) use plugin_overlays::{OverlayJob, OverlayResult};
mod switch_membership;
mod tab_close;
pub(crate) mod tab_rename;
use create_membership::CreateAnchor;
pub(crate) use create_membership::LaunchDescription;
pub(crate) use create_membership::{CreateOriginKind, PreparedCreateSnapshot};
#[cfg(test)]
pub(crate) use membership::PreparedMembershipInstall;
pub(crate) use membership::{
    MembershipCleanup, MembershipInstallDecision, MembershipResultKey, PreparedMembershipCandidate,
};
pub(crate) use move_tab::{MoveDecision, MoveOperation, MoveResult, PendingMoveCommit};
#[cfg(test)]
pub(crate) use panel::PanelPrecommit;
pub(crate) use panel::{
    PanelCommit, PanelDecision, PanelIntent, PanelOperation, PanelRequestKind, PanelResult,
    PendingPanelCommit,
};
pub(crate) use panel_close::PanelCloseConfirmationToken;
pub(crate) use panel_close::{PanelCloseConfirmationView, PanelCloseState};
use switch_membership::SwitchMembership;
pub(crate) use switch_membership::SwitchRestoreKey;
pub use tab_close::CloseConfirmationToken;
pub(crate) use tab_close::{
    CloseConfirmationKind, CloseSetStatus, CloseStatusKind, CloseStatusToken,
};
pub(crate) use tab_close::{
    CloseConfirmationView, CloseFlightKey, CloseLeaseAcknowledgement, CloseOperation,
    CloseOperationKind, CloseReconciliationMarker, CloseResult, CloseState, CloseStateError,
    CloseStatusView, CloseWorkerReturn, DefinitiveClosedAttachment, PendingCloseResult,
    TabCloseResumePermit,
};
#[cfg(test)]
pub(crate) use tab_close::{CloseIntent, CloseReadMode, CloseSelection, CloseTarget};
pub(crate) use tab_close::{CloseSetKind, close_set_premises_match, close_set_targets};

pub(crate) const OPERATION_TOTAL: Duration = Duration::from_secs(50);
pub(crate) const CLEANUP_RESERVE: Duration = Duration::from_secs(5);

fn contains_presentation(
    node: &orchbee_workbench_protocol::PanelNode,
    id: &orchbee_workbench_protocol::TerminalPresentationId,
) -> bool {
    match node {
        orchbee_workbench_protocol::PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } => {
            &primary_presentation.presentation_id == id
                || child_presentations
                    .iter()
                    .any(|member| &member.presentation_id == id)
        }
        orchbee_workbench_protocol::PanelNode::Split { children, .. } => children
            .iter()
            .any(|child| contains_presentation(&child.node, id)),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentKey {
    generation: u64,
    tab_id: WorkbenchTabId,
}

impl AttachmentKey {
    pub(crate) const fn new(generation: u64, tab_id: WorkbenchTabId) -> Self {
        Self { generation, tab_id }
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }
}

/// Logical pane identity: the anchor (selection epoch) plus the stable
/// workbench panel it was minted for. Two panes never share a key; a panel
/// move that keeps the same anchor and panel keeps the same key. Equality is
/// exact — anchor-generation or tab comparisons must go through `anchor()`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneKey {
    anchor: AttachmentKey,
    panel: WorkbenchPanelId,
}

impl PaneKey {
    pub(crate) const fn new(anchor: AttachmentKey, panel: WorkbenchPanelId) -> Self {
        Self { anchor, panel }
    }

    pub(crate) const fn anchor(&self) -> &AttachmentKey {
        &self.anchor
    }

    pub(crate) const fn panel(&self) -> &WorkbenchPanelId {
        &self.panel
    }

    #[cfg(test)]
    pub(crate) const fn generation(&self) -> u64 {
        self.anchor.generation()
    }

    #[cfg(test)]
    pub(crate) const fn tab_id(&self) -> &WorkbenchTabId {
        self.anchor.tab_id()
    }
}

/// Physical attachment identity for one pane: the logical pane plus the
/// incarnation minted when its attach target was issued. Every queued command,
/// poll key, and attachment-scoped worker event carries a `PaneRef` so a stale
/// incarnation (pane departed and re-attached) is rejected by exact equality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneRef {
    pane: PaneKey,
    incarnation: u32,
}

impl PaneRef {
    pub(crate) const fn new(pane: PaneKey, incarnation: u32) -> Self {
        Self { pane, incarnation }
    }

    pub(crate) const fn pane(&self) -> &PaneKey {
        &self.pane
    }

    pub(crate) const fn anchor(&self) -> &AttachmentKey {
        self.pane.anchor()
    }

    pub(crate) const fn incarnation(&self) -> u32 {
        self.incarnation
    }
}

/// One Agent start's lifecycle phase as the board and its notice report it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentStartPhase {
    Starting,
    WaitingInput,
    Unconfirmed,
    Cancelling,
    Started,
    Failed,
    Unknown,
    Cancelled,
}

/// Not Clone: derived under the install's exclusive state borrow, consumed once after pane minting.
struct DuplicateInstallWitness {
    operation: CreateOperationKey,
    candidate: ClientContextTabs,
    pane: PaneRef,
}

/// One queued poll's admission serial and the exact pane incarnation it was issued for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PollKey {
    serial: u64,
    key: PaneRef,
}

impl PollKey {
    pub(crate) const fn new(serial: u64, key: PaneRef) -> Self {
        Self { serial, key }
    }

    pub(crate) const fn serial(&self) -> u64 {
        self.serial
    }

    /// The exact pane incarnation this poll was issued for.
    pub(crate) const fn pane_ref(&self) -> &PaneRef {
        &self.key
    }

    /// The logical pane of this poll.
    pub(crate) const fn pane(&self) -> &PaneKey {
        self.key.pane()
    }
}

/// Why a pane's viewer is unavailable while the pane itself stays selected.
/// Pane-local degradation: the rest of the membership is unaffected, and only
/// an explicit new reconcile (re-observe or selection change) may mint a fresh
/// incarnation for the pane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaneFault {
    /// The workbench refused the attach (binding gone, terminal retired).
    AttachRejected,
    /// The attach carrier failed or was unavailable before a viewer existed.
    AttachUnavailable,
    /// The live viewer's carrier failed or reported Unavailable; disposal
    /// proof is owed before the record may be re-attached.
    ViewerLost,
    /// Controller ownership was lost (`NotController`) or the session's
    /// controller grant ended; the pane keeps rendering but stays view-only.
    ControlLost,
    /// The shared host viewer budget refused the attach; parked until the
    /// user removes panes or the selection changes.
    CapacityExceeded,
    /// The reconcile episode's shared deadline expired before this pane's
    /// attach converged: terminal for that episode — the record parks with
    /// the deadline retained as the dead-episode marker, and only fresh
    /// membership divergence mints a new episode and revives it.
    ConvergenceExpired,
}

/// One attach target inside a reconcile snapshot: the exact pane incarnation
/// plus the binding facts and geometry the worker must converge it to. Detach
/// targets are not listed — the executor derives them by diffing its live
/// attachments of the marker's anchor against this set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReconcileTarget {
    pane_ref: PaneRef,
    index: usize,
    session_id: SessionId,
    binding: ClientTerminalBinding,
    provenance: CreateTerminalProvenance,
    geometry: Geometry,
}

impl ReconcileTarget {
    pub(crate) fn new(record: &PaneRecord) -> Self {
        Self {
            pane_ref: record.pane_ref(),
            index: record.index,
            session_id: record.session_id.clone(),
            binding: record.binding.clone(),
            provenance: record.provenance.clone(),
            geometry: record.observed_geometry,
        }
    }

    pub(crate) const fn pane_ref(&self) -> &PaneRef {
        &self.pane_ref
    }

    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    pub(crate) const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub(crate) const fn binding(&self) -> &ClientTerminalBinding {
        &self.binding
    }

    pub(crate) const fn provenance(&self) -> &CreateTerminalProvenance {
        &self.provenance
    }

    pub(crate) const fn geometry(&self) -> Geometry {
        self.geometry
    }
}

/// One bounded pane-progress action inside a reconcile plan. The executor
/// plans the sequence once — stale disposals first, then pending attaches —
/// and consumes exactly one step per lease execution, requeuing the rest
/// under the episode's unchanged absolute deadline. A step that cannot
/// finish rotates to the back so a busy or refused pane never starves the
/// steps behind it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReconcileStep {
    /// Dispose one held attachment: the exact pane incarnation plus whether
    /// its record ever observed the `Attached` announcement — that fact
    /// decides which disposal notice, if any, the executor may publish.
    Detach { key: PaneRef, announced: bool },
    /// Converge the snapshot target at this index of the marker's `targets`.
    Attach(usize),
}

/// One pane's canonical viewer state. Records live in `panes`, keyed by
/// `PaneKey`; at most one record exists per pane. `departing` means the pane
/// left the desired membership set — it keeps its record (and possibly its
/// attachment) until a canonical disposal proof drops it, so it is never
/// silently freed and never double-detached. Membership (`departing`), the
/// last admitted resize, and the fault are independent of the attachment;
/// every viewer fact lives inside `attachment`, so a detached record cannot
/// carry a mode, a projection, frame evidence, or accepted geometry.
#[derive(Clone, Debug)]
pub(crate) struct PaneRecord {
    key: PaneKey,
    incarnation: u32,
    index: usize,
    panel_index: usize,
    session_id: SessionId,
    binding: ClientTerminalBinding,
    provenance: CreateTerminalProvenance,
    observed_geometry: Geometry,
    attachment: Option<AttachedPane>,
    departing: bool,
    last_admitted_geometry: Option<Geometry>,
    fault: Option<PaneFault>,
}

/// The viewer this Client holds for one pane incarnation. It exists from the
/// canonical `Attached` proof until a disposal path or a viewer loss drops it.
#[derive(Clone, Debug)]
struct AttachedPane {
    /// Unknown until the attachment's setup configuration reports it.
    mode: Option<ControllerMode>,
    projection: PaneProjection,
    // First accepted display for this attachment; a later blocked frame does not undo setup.
    display_initialized: bool,
    last_accepted_geometry: Option<Geometry>,
}

impl AttachedPane {
    /// A freshly installed viewer: projected, without frame evidence or setup facts.
    const fn installed() -> Self {
        Self {
            mode: None,
            projection: PaneProjection::Installed(FrameAcknowledgement::Unstaged),
            display_initialized: false,
            last_accepted_geometry: None,
        }
    }

    /// Withdraws the local projection and the geometry it accepted; the viewer
    /// stays held until its disposal proof.
    const fn withdraw_projection(&mut self) {
        self.projection = PaneProjection::Withdrawn;
        self.last_accepted_geometry = None;
    }

    /// Voids frame evidence on an installed projection; a withdrawn projection has none.
    const fn void_frames(&mut self) {
        if let PaneProjection::Installed(frame) = &mut self.projection {
            *frame = FrameAcknowledgement::Unstaged;
        }
    }
}

/// Whether an attached viewer is installed in the local projection. Only an
/// installed projection carries frame evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PaneProjection {
    Withdrawn,
    Installed(FrameAcknowledgement),
}

/// Draw-receipt evidence for an installed projection. A frame is accepted only
/// after it was staged, so the accepted frame is always the staged frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameAcknowledgement {
    Unstaged,
    Staged(FrameIdentity),
    Accepted(FrameIdentity),
}

impl FrameAcknowledgement {
    /// The frame awaiting or holding acknowledgement.
    const fn staged(self) -> Option<FrameIdentity> {
        match self {
            Self::Unstaged => None,
            Self::Staged(frame) | Self::Accepted(frame) => Some(frame),
        }
    }

    const fn accepted(self) -> Option<FrameIdentity> {
        match self {
            Self::Accepted(frame) => Some(frame),
            Self::Unstaged | Self::Staged(_) => None,
        }
    }
}

impl PaneRecord {
    /// The local pane dimensions, including read-only observations that never resize the Host.
    pub(crate) const fn observed_geometry(&self) -> Geometry {
        self.observed_geometry
    }

    pub(crate) const fn provenance(&self) -> &CreateTerminalProvenance {
        &self.provenance
    }
    pub(crate) const fn binding(&self) -> &ClientTerminalBinding {
        &self.binding
    }
    fn new(
        pane_ref: PaneRef,
        index: usize,
        panel_index: usize,
        session_id: SessionId,
        binding: ClientTerminalBinding,
        provenance: CreateTerminalProvenance,
        observed_geometry: Geometry,
    ) -> Self {
        let PaneRef {
            pane: key,
            incarnation,
        } = pane_ref;
        Self {
            key,
            incarnation,
            index,
            panel_index,
            session_id,
            binding,
            provenance,
            observed_geometry,
            attachment: None,
            departing: false,
            last_admitted_geometry: None,
            fault: None,
        }
    }

    pub(crate) const fn key(&self) -> &PaneKey {
        &self.key
    }

    pub(crate) const fn incarnation(&self) -> u32 {
        self.incarnation
    }

    pub(crate) fn pane_ref(&self) -> PaneRef {
        PaneRef::new(self.key.clone(), self.incarnation)
    }

    pub(crate) const fn attachable(&self) -> bool {
        self.provenance.attachable()
    }

    /// Whether reconcile may still try to attach this pane: definite attach
    /// failures, a known lost viewer, and dead-episode parks do not become
    /// pending work again on an ordinary tick. A live read-only viewer stays
    /// in the target set; losing control is not losing the attachment.
    pub(crate) const fn retryable(&self) -> bool {
        !matches!(
            self.fault,
            Some(
                PaneFault::AttachRejected
                    | PaneFault::CapacityExceeded
                    | PaneFault::ViewerLost
                    | PaneFault::ConvergenceExpired
            )
        )
    }

    /// Departure voids the pane's frame claims: a pending draw for a
    /// departing leaf must never stage or be acknowledged — the draw receipt
    /// is proof the frame reached a live leaf, which departure revoked. The
    /// physical projection and attachment survive so a leaf that reappears
    /// before its disposal proof resumes drawing without a re-install path.
    const fn mark_departing(&mut self) {
        self.departing = true;
        if let Some(attachment) = &mut self.attachment {
            attachment.void_frames();
        }
    }

    pub(crate) const fn attached(&self) -> bool {
        self.attachment.is_some()
    }

    const fn projected(&self) -> bool {
        matches!(
            &self.attachment,
            Some(AttachedPane {
                projection: PaneProjection::Installed(_),
                ..
            })
        )
    }

    /// Frame evidence of the installed projection; anything else has none.
    const fn frame(&self) -> FrameAcknowledgement {
        match &self.attachment {
            Some(AttachedPane {
                projection: PaneProjection::Installed(frame),
                ..
            }) => *frame,
            _ => FrameAcknowledgement::Unstaged,
        }
    }

    const fn frame_mut(&mut self) -> Option<&mut FrameAcknowledgement> {
        match &mut self.attachment {
            Some(AttachedPane {
                projection: PaneProjection::Installed(frame),
                ..
            }) => Some(frame),
            _ => None,
        }
    }

    const fn staged_frame(&self) -> Option<FrameIdentity> {
        self.frame().staged()
    }

    const fn accepted_frame(&self) -> Option<FrameIdentity> {
        self.frame().accepted()
    }

    fn mode(&self) -> Option<&ControllerMode> {
        self.attachment.as_ref()?.mode.as_ref()
    }

    fn last_accepted_geometry(&self) -> Option<Geometry> {
        self.attachment.as_ref()?.last_accepted_geometry
    }

    fn display_initialized(&self) -> bool {
        self.attachment
            .as_ref()
            .is_some_and(|attachment| attachment.display_initialized)
    }

    pub(crate) const fn fault(&self) -> Option<PaneFault> {
        self.fault
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperationDeadlines {
    forward_cutoff: Instant,
    total: Instant,
}

impl OperationDeadlines {
    pub(crate) fn from_sample(sampled_at: Instant) -> Option<Self> {
        let total = sampled_at.checked_add(OPERATION_TOTAL)?;
        let forward_cutoff = total.checked_sub(CLEANUP_RESERVE)?;
        Some(Self {
            forward_cutoff,
            total,
        })
    }

    pub(crate) const fn forward_cutoff(self) -> Instant {
        self.forward_cutoff
    }

    pub(crate) const fn total(self) -> Instant {
        self.total
    }

    pub(crate) fn bounded_by(self, cutoff: Instant) -> Self {
        Self {
            forward_cutoff: self.forward_cutoff.min(cutoff),
            total: self.total.min(cutoff),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CreateTerminalProvenance {
    Started,
    Pending,
    Unavailable(WorkbenchTerminalUnavailableReason),
    Failed(WorkbenchTerminalFailureReason),
    OutcomeUnknown,
    BindingOnly,
}

impl CreateTerminalProvenance {
    pub(crate) const fn attachable(&self) -> bool {
        matches!(self, Self::Started | Self::BindingOnly)
    }

    /// A committed Server launch failure supersedes retained or observer provenance, so no
    /// Client attaches to a terminal that never started.
    pub(crate) fn observed_for(
        self,
        member: &crate::workbench_projection::ClientPresentation,
    ) -> Self {
        member
            .launch_failure()
            .map_or(self, |failure| Self::Failed(failure.reason))
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
pub(crate) enum ControllerMode {
    Controlling(ControllerId),
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameIdentity(u64);

impl FrameIdentity {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientPhaseKind {
    Empty,
    Starting,
    Ready,
    Creating,
    CreateCommittedNeedDetach,
    ProjectionUnavailable,
    MoveProjectionUnavailable,
    PanelProjectionUnavailable,
    RenameProjectionUnavailable,
    Switching,
    SelectedWithoutAttachment,
    RetryableAttachUnavailable,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientNotice {
    Unsupported(crate::tui_capabilities::Capability),
    LocalBusy,
    NoTabAtPosition(u8),
    CreateNotCreated,
    CreateOrderConflict,
    DuplicatePending,
    DuplicateNotStarted,
    DuplicateOutcomeUnknown,
    MoveNotMoved,
    MoveOrderConflict,
    RenameNotRenamed,
    CloseRequestNotApplied,
    WorkspaceCloseUnavailable,
    /// A conditional Workspace Close reply no longer matched its target; nothing was closed.
    WorkspaceChanged,
    NoWorkspace,
    /// The current Workspace has no settled view to start a Worktree action from.
    WorkspaceNotReady,
    PanelNotApplied,
    ConfirmationCancelled,
    DraftCancelled,
    /// A dialog could not be drawn when it was requested; nothing ran.
    DialogRefused,
    /// A modal surface or armed confirmation owns the input layer — the
    /// Agent board refused to open rather than fight for capture.
    ModalOwnsInput,
    AgentStart(AgentStartPhase),
    /// The viewport lost its facts/anchor and is returning to live —
    /// shown while the recovery demand pair is in flight.
    HistoryRecovering,
    /// Retained history could not be entered (demand refused or reset);
    /// the pane stayed Live.
    HistoryUnavailable,
    /// The previous reading position could not be reproduced after navigation.
    HistoryPositionLost,
    CopyModeUnavailable,
    CopyModeBusy,
    CopyModeInvalidated,
    CopyModeQueryFailed,
    CopyModeNoMatch,
    CopyModeSearchLimit,
    CopyModeNoSelection,
    SelectionEvicted,
    CopyFailed,
    CopyUnknown,
    BellFailed,
    BellUnknown,
    PositionChanged,
    PositionChangedEmpty,
    PositionLocationChanged,
    PositionLocationChangedEmpty,
    PositionUnavailable,
}

impl ClientNotice {
    pub(crate) const fn transient(self) -> bool {
        match self {
            Self::DuplicatePending
            | Self::DuplicateOutcomeUnknown
            | Self::CopyUnknown
            | Self::BellUnknown => false,
            Self::AgentStart(phase) => matches!(
                phase,
                AgentStartPhase::Started | AgentStartPhase::Failed | AgentStartPhase::Cancelled
            ),
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigatorRefreshStatus {
    Refreshing,
    Success(WorkbenchRevision),
    Failure(WorkbenchRevision),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingRefreshStage {
    Queued,
    InFlight,
    AwaitingUi,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectedTab {
    key: PaneKey,
    index: usize,
    panel_index: usize,
}

impl SelectedTab {
    pub(crate) const fn new_panel(key: PaneKey, index: usize, panel_index: usize) -> Self {
        Self {
            key,
            index,
            panel_index,
        }
    }

    pub(crate) const fn key(&self) -> &PaneKey {
        &self.key
    }

    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    pub(crate) const fn panel_index(&self) -> usize {
        self.panel_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadyAttachment {
    selected: SelectedTab,
    mode: ControllerMode,
}

impl ReadyAttachment {
    pub(crate) const fn new(selected: SelectedTab, mode: ControllerMode) -> Self {
        Self { selected, mode }
    }

    pub(crate) const fn selected(&self) -> &SelectedTab {
        &self.selected
    }

    pub(crate) const fn mode(&self) -> &ControllerMode {
        &self.mode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CreateOperationKey {
    request_id: WorkbenchRequestId,
    reserved_generation: u64,
}

impl CreateOperationKey {
    pub(crate) const fn new(request_id: WorkbenchRequestId, reserved_generation: u64) -> Self {
        Self {
            request_id,
            reserved_generation,
        }
    }

    pub(crate) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }

    pub(crate) const fn reserved_generation(&self) -> u64 {
        self.reserved_generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CreateOperation {
    launch_description: Option<LaunchDescription>,
    placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
    automatic_label: bool,
    origin: CreateOriginKind,
    key: CreateOperationKey,
    session_id: SessionId,
    expected_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    label: WorkbenchLabel,
    geometry: Geometry,
    deadlines: OperationDeadlines,
}

impl CreateOperation {
    pub(crate) const fn new(
        key: CreateOperationKey,
        session_id: SessionId,
        expected_revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        label: WorkbenchLabel,
        geometry: Geometry,
        deadlines: OperationDeadlines,
    ) -> Self {
        Self {
            launch_description: None,
            origin: CreateOriginKind::Ready,
            placement: None,
            automatic_label: false,
            key,
            session_id,
            expected_revision,
            terminal_context_id,
            label,
            geometry,
            deadlines,
        }
    }

    pub(crate) const fn key(&self) -> &CreateOperationKey {
        &self.key
    }

    pub(crate) const fn deadlines(&self) -> OperationDeadlines {
        self.deadlines
    }

    pub(crate) const fn with_origin(mut self, origin: CreateOriginKind) -> Self {
        self.origin = origin;
        self
    }

    pub(crate) fn with_launch_description(
        mut self,
        description: Option<LaunchDescription>,
    ) -> Self {
        self.launch_description = description;
        self
    }

    pub(crate) const fn with_automatic_label(mut self) -> Self {
        self.automatic_label = true;
        self
    }

    pub(crate) fn with_placement(
        mut self,
        placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
    ) -> Self {
        self.placement = placement;
        self
    }

    pub(crate) const fn preserves_selection(&self) -> bool {
        matches!(
            self.placement,
            Some(orchbee_workbench_protocol::TabCreationPlacement::Duplicate { .. })
        )
    }

    pub(crate) const fn origin(&self) -> CreateOriginKind {
        self.origin
    }

    #[cfg(test)]
    pub(crate) fn placement_for_test(
        &self,
    ) -> Option<&orchbee_workbench_protocol::TabCreationPlacement> {
        self.placement.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatingBarrier {
    operation: CreateOperation,
    anchor: CreateAnchor,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PendingCreateCommit {
    operation: CreateOperation,
    commit: WorkbenchTabCreateCommit,
}

impl PendingCreateCommit {
    pub(crate) const fn new(operation: CreateOperation, commit: WorkbenchTabCreateCommit) -> Self {
        Self { operation, commit }
    }

    pub(crate) const fn operation(&self) -> &CreateOperation {
        &self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommittedTabCreate {
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
                    .selected()
                    .map_or(0, |selected| selected.key.anchor().generation())
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
            barrier.operation.automatic_label,
        )
        .with_placement(barrier.operation.placement.clone());
        Ok(Self {
            operation: barrier.operation,
            committed_key,
            expectation,
            provenance,
            anchor: barrier.anchor,
        })
    }

    pub(crate) const fn operation(&self) -> &CreateOperation {
        &self.operation
    }

    #[cfg(test)]
    pub(crate) const fn expectation(&self) -> &CreatedTabExpectation {
        &self.expectation
    }

    pub(crate) const fn committed_key(&self) -> &AttachmentKey {
        &self.committed_key
    }

    #[cfg(test)]
    pub(crate) const fn provenance(&self) -> &CreateTerminalProvenance {
        &self.provenance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachTarget {
    selected: SelectedTab,
    presentation_id: Box<orchbee_workbench_protocol::TerminalPresentationId>,
    session_id: SessionId,
    binding: Option<ClientTerminalBinding>,
    provenance: CreateTerminalProvenance,
    deadlines: OperationDeadlines,
    incarnation: u32,
    /// Exact physical terminal required by a verified Agent selection, before any frame intake.
    pub(crate) expected_host: Option<Box<orchbee_agent_observation_protocol::AgentLaunchBinding>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BootstrapTarget {
    selected: SelectedTab,
    session_id: SessionId,
    binding: Option<ClientTerminalBinding>,
    incarnation: u32,
}

impl BootstrapTarget {
    pub(crate) const fn selected(&self) -> &SelectedTab {
        &self.selected
    }

    pub(crate) const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub(crate) const fn binding(&self) -> Option<&ClientTerminalBinding> {
        self.binding.as_ref()
    }

    pub(crate) fn pane_ref(&self) -> PaneRef {
        PaneRef::new(self.selected.key.clone(), self.incarnation)
    }
}

impl AttachTarget {
    pub(crate) const fn selected(&self) -> &SelectedTab {
        &self.selected
    }

    pub(crate) const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub(crate) const fn binding(&self) -> Option<&ClientTerminalBinding> {
        self.binding.as_ref()
    }

    pub(crate) const fn provenance(&self) -> &CreateTerminalProvenance {
        &self.provenance
    }

    pub(crate) const fn deadlines(&self) -> OperationDeadlines {
        self.deadlines
    }

    pub(crate) fn pane_ref(&self) -> PaneRef {
        PaneRef::new(self.selected.key.clone(), self.incarnation)
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
enum ClientPhase {
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
pub(crate) enum CarrierFailure {
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
pub(crate) enum CreateProjectionDecision {
    Cleanup(MembershipCleanup),
    Attach(AttachTarget),
    SettledWithoutAttachment,
    ProjectionUnavailable,
    Retire,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RefreshDecision {
    Continue,
    Cleanup(MembershipCleanup),
    AttachRecovered(AttachTarget),
    SettledWithoutAttachment,
    Retire,
}

#[derive(Debug)]
pub(crate) enum PreparedRefresh {
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
        baseline_phase: ClientPhaseKind,
        active: SelectedTab,
        candidate: ClientContextTabs,
    },
    Divergent {
        generation: u64,
    },
}

impl PreparedRefresh {
    pub(crate) fn needs_recovery_deadlines(&self) -> bool {
        if let Self::Membership { prepared, .. } = self {
            return !prepared.preserves_selection();
        }
        matches!(self, Self::CreateMembership { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadyDecision {
    Ready,
    CatchUpResize(Geometry),
    Superseded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SetupEventDisposition {
    Applied,
    Superseded { desired: PaneKey },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CleanupContinuation {
    Stable,
    Superseded { desired: PaneKey },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionPlan {
    Busy,
    NoOp,
    OutOfRange(u8),
    Switch { index: usize },
}

enum PendingMutation {
    Move(MoveOperation),
    Panel(PanelOperation),
    Rename(tab_rename::RenameOperation),
}

/// Bounded refresh debt owed by committed peer publications that the executor folded
/// into its baseline but the native UI has not yet installed. `Pending` survives Busy
/// and Full refresh admission so a later fair tick retries; `InFlight` tracks the
/// refresh flight that adopted the demand (its generation lives in
/// `pending_refresh`) plus whether a later hint arrived during that flight;
/// `Parked` holds demand after a definite refusal until an accepted hint or an explicit
/// refresh re-arms it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PublicationDebt {
    #[default]
    Clean,
    Pending,
    InFlight {
        late: bool,
    },
    Parked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefreshSource {
    Explicit,
    Publication,
    Recovery,
}

/// Source and generation expire together; an old user intent cannot authorize
/// a later automatic or recovery flight.
#[derive(Clone, Copy, Debug)]
struct PendingRefreshFlight {
    generation: u64,
    source: RefreshSource,
}

impl PendingRefreshFlight {
    const fn recovery(generation: u64) -> Self {
        Self {
            generation,
            source: RefreshSource::Recovery,
        }
    }
}

pub(crate) struct WorkbenchClientState {
    launch_descriptions: std::collections::HashMap<
        orchbee_workbench_protocol::TerminalPresentationId,
        LaunchDescription,
    >,
    cache: ClientContextTabs,
    context_selection: Option<context_selection::ContextSelection>,
    retained_contexts: Vec<context_selection::RetainedContext>,
    navigation_memory: navigation_memory::NavigationMemory,
    provenance: Vec<CreateTerminalProvenance>,
    phase: ClientPhase,
    desired: Option<PaneKey>,
    panes: Vec<PaneRecord>,
    next_incarnation: u32,
    reconcile_deadlines: Option<OperationDeadlines>,
    latest_geometry: Geometry,
    focus_new_panel: bool,
    notice: Option<ClientNotice>,
    pending_refresh: Option<PendingRefreshFlight>,
    publication_debt: PublicationDebt,
    refresh_status: Option<NavigatorRefreshStatus>,
    pending_mutation: Option<PendingMutation>,
    move_recovery: Option<PendingMoveCommit>,
    settled_move: Option<PendingMoveCommit>,
    panel_recovery: Option<PendingPanelCommit>,
    settled_panel: Option<PendingPanelCommit>,
    settled_rename: Option<tab_rename::PendingRenameCommit>,
    rename_recovery: Option<tab_rename::PendingRenameCommit>,
    rename_uncertain: Option<tab_rename::RenameOperation>,
    close: CloseState,
    panel_close: PanelCloseState,
    rename: rename_draft::RenameDraftState,
    setup: workspace_setup::SetupState,
    tab_groups: tab_groups::GroupView,
    workspace_close: workspace_close::WorkspaceCloseState,
    takeover: control_takeover::TakeoverState,
    pending_membership: Option<membership::PendingMembership>,
    settled_create: Option<CommittedTabCreate>,
    duplicate_sequence: Option<tab_duplication::DuplicateSequence>,
    /// Volatile per-Tab Client-local zoom flags (`Ctrl+B z`): a flag lives
    /// while its Tab is known alive with more than one leaf — the current
    /// cache answers first (its silence about its own location is already
    /// that location's complete truth), then only the departed contexts'
    /// retained snapshots — and is collected at every membership install.
    /// Never wired, shared between Clients, or persisted; bounded by the
    /// same navigation retention the context snapshots already obey.
    zoomed_tabs: std::collections::BTreeSet<WorkbenchTabId>,
    plugin_overlays: Vec<plugin_overlays::Overlay>,
}

impl WorkbenchClientState {
    #[cfg(test)]
    pub(crate) fn starting(
        cache: ClientContextTabs,
        generation: u64,
        geometry: Geometry,
    ) -> Option<(Self, BootstrapTarget)> {
        if generation == 0 {
            return None;
        }
        Self::starting_at(cache, generation, geometry, None)
    }
    fn starting_at(
        cache: ClientContextTabs,
        generation: u64,
        geometry: Geometry,
        selection: Option<&crate::workbench_projection::ClientMemberTarget>,
    ) -> Option<(Self, BootstrapTarget)> {
        if generation == 0 {
            return None;
        }
        let tab_index = match selection {
            Some(s) => cache.tab_index(&s.tab)?,
            None => 0,
        };
        let first = cache.tabs().get(tab_index)?;
        let panel_index = match selection {
            Some(s) => first.panel_index(&s.panel)?,
            None => 0,
        };
        let anchor = AttachmentKey::new(generation, first.tab_id().clone());
        let panel = first.panel(panel_index)?;
        let member = match selection {
            Some(s) => panel.presentation(panel.presentation_index(&s.presentation)?)?,
            None => panel.primary(),
        };
        let member_id = member.presentation_id().clone();
        let key = PaneKey::new(anchor, panel.panel_id().clone());
        let selected = SelectedTab::new_panel(key.clone(), tab_index, panel_index);
        let record = member.binding().map(|binding| {
            PaneRecord::new(
                PaneRef::new(key.clone(), 1),
                tab_index,
                panel_index,
                cache.session_id().clone(),
                binding.clone(),
                CreateTerminalProvenance::BindingOnly,
                geometry,
            )
        });
        let target = BootstrapTarget {
            selected: selected.clone(),
            session_id: cache.session_id().clone(),
            binding: member.binding().cloned(),
            incarnation: 1,
        };
        let mut state = Self::initial(cache, geometry);
        if selection.is_some() {
            state
                .navigation_memory
                .remember_presentation(&state.cache, &selected, member_id);
        }
        state.phase = if record.is_some() {
            ClientPhase::Starting(StartingSetup {
                selected,
                mode: None,
                accepted_geometry: None,
            })
        } else {
            ClientPhase::SelectedWithoutAttachment(selected)
        };
        state.desired = Some(key);
        state.panes = record.into_iter().collect();
        state.next_incarnation = 2;
        Some((state, target))
    }

    #[cfg(test)]
    pub(crate) fn bootstrap(
        cache: ClientContextTabs,
        generation: u64,
        geometry: Geometry,
    ) -> Option<(Self, Option<BootstrapTarget>)> {
        Self::bootstrap_at(cache, generation, geometry, None)
    }
    pub(crate) fn bootstrap_at(
        cache: ClientContextTabs,
        generation: u64,
        geometry: Geometry,
        selection: Option<&crate::workbench_projection::ClientMemberTarget>,
    ) -> Option<(Self, Option<BootstrapTarget>)> {
        if generation == 0 {
            return None;
        }
        if cache.tabs().is_empty() && selection.is_none() {
            Some((Self::initial(cache, geometry), None))
        } else {
            Self::starting_at(cache, generation, geometry, selection)
                .map(|(state, target)| (state, Some(target)))
        }
    }
    fn initial(cache: ClientContextTabs, geometry: Geometry) -> Self {
        Self {
            launch_descriptions: std::collections::HashMap::new(),
            context_selection: None,
            retained_contexts: Vec::new(),
            navigation_memory: navigation_memory::NavigationMemory::default(),
            provenance: vec![CreateTerminalProvenance::BindingOnly; cache.tabs().len()],
            cache,
            phase: ClientPhase::Empty,
            desired: None,
            panes: Vec::new(),
            next_incarnation: 1,
            reconcile_deadlines: None,
            latest_geometry: geometry,
            focus_new_panel: true,
            notice: None,
            pending_refresh: None,
            publication_debt: PublicationDebt::Clean,
            refresh_status: None,
            pending_mutation: None,
            move_recovery: None,
            settled_move: None,
            panel_recovery: None,
            settled_panel: None,
            settled_rename: None,
            rename_recovery: None,
            rename_uncertain: None,
            close: CloseState::default(),
            panel_close: PanelCloseState::default(),
            rename: rename_draft::RenameDraftState::default(),
            setup: workspace_setup::SetupState::default(),
            tab_groups: tab_groups::GroupView::default(),
            workspace_close: workspace_close::WorkspaceCloseState::default(),
            takeover: control_takeover::TakeoverState::default(),
            pending_membership: None,
            settled_create: None,
            duplicate_sequence: None,
            zoomed_tabs: std::collections::BTreeSet::new(),
            plugin_overlays: Vec::new(),
        }
    }

    pub(crate) const fn phase_kind(&self) -> ClientPhaseKind {
        if self.rename_recovery.is_some() && !matches!(&self.phase, ClientPhase::Retired(_)) {
            return ClientPhaseKind::RenameProjectionUnavailable;
        }
        if self.move_recovery.is_some() && !matches!(&self.phase, ClientPhase::Retired(_)) {
            return ClientPhaseKind::MoveProjectionUnavailable;
        }
        if self.panel_recovery.is_some() && !matches!(&self.phase, ClientPhase::Retired(_)) {
            return ClientPhaseKind::PanelProjectionUnavailable;
        }
        match &self.phase {
            ClientPhase::Empty => ClientPhaseKind::Empty,
            ClientPhase::ReconcilingMembership(_) => ClientPhaseKind::Switching,
            ClientPhase::Starting(_) => ClientPhaseKind::Starting,
            ClientPhase::Ready(_) => ClientPhaseKind::Ready,
            ClientPhase::Creating(_) => ClientPhaseKind::Creating,
            ClientPhase::CreateCommittedNeedDetach(_) => ClientPhaseKind::CreateCommittedNeedDetach,
            ClientPhase::ProjectionUnavailableBeforeCleanup(_) => {
                ClientPhaseKind::ProjectionUnavailable
            }
            ClientPhase::Switching(_) => ClientPhaseKind::Switching,
            ClientPhase::SelectedWithoutAttachment(_) => ClientPhaseKind::SelectedWithoutAttachment,
            ClientPhase::RetryableAttachUnavailable(_) => {
                ClientPhaseKind::RetryableAttachUnavailable
            }
            ClientPhase::Retired(_) => ClientPhaseKind::Retired,
        }
    }

    pub(crate) const fn cache(&self) -> &ClientContextTabs {
        &self.cache
    }

    pub(crate) const fn desired(&self) -> Option<&PaneKey> {
        self.desired.as_ref()
    }

    pub(crate) fn pane_record(&self, pane: &PaneKey) -> Option<&PaneRecord> {
        self.panes.iter().find(|record| &record.key == pane)
    }

    fn pane_record_mut(&mut self, pane: &PaneKey) -> Option<&mut PaneRecord> {
        self.panes.iter_mut().find(|record| &record.key == pane)
    }

    fn live_pane_record(&self, key: &PaneRef) -> Option<&PaneRecord> {
        self.panes
            .iter()
            .find(|record| &record.key == key.pane() && record.incarnation == key.incarnation())
    }

    fn live_pane_record_mut(&mut self, key: &PaneRef) -> Option<&mut PaneRecord> {
        self.panes
            .iter_mut()
            .find(|record| &record.key == key.pane() && record.incarnation == key.incarnation())
    }

    /// Whether any pane record — including departing ones awaiting disposal
    /// proof — still carries an attachment. Replaces the single
    /// `has_attachments()` gate: membership cleanup is staged whenever
    /// at least one pane viewer exists.
    pub(crate) fn has_attachments(&self) -> bool {
        self.panes.iter().any(|record| record.attached())
    }

    /// The attachment key the single-attachment model exposed: the focus
    /// pane's key while it carries an attachment, and — when the focus pane is
    /// unattached (e.g. mid-switch before candidate setup) — the sole
    /// surviving attached record's key. `None` when nothing is attached or
    /// several background attachments make the answer ambiguous.
    #[cfg(test)]
    pub(crate) fn attached_key(&self) -> Option<&PaneKey> {
        if let Some(record) = self.focus_record().filter(|record| record.attached()) {
            return Some(record.key());
        }
        let mut attached = self.panes.iter().filter(|record| record.attached());
        match (attached.next(), attached.next()) {
            (Some(record), None) => Some(record.key()),
            _ => None,
        }
    }

    /// The focus pane's key while its projection is installed — the successor
    /// of the single `projection_key`.
    #[cfg(test)]
    pub(crate) fn projection_key(&self) -> Option<&PaneKey> {
        self.focus_record()
            .filter(|record| record.projected())
            .map(PaneRecord::key)
    }

    /// The exact live incarnation of one logical pane — test code resolves a
    /// `PaneKey` into the `PaneRef` the current membership minted.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn pane_ref_for(&self, pane: &PaneKey) -> Option<PaneRef> {
        self.pane_record(pane).map(PaneRecord::pane_ref)
    }

    /// The logical pane key an anchor resolves to: the live record's key, or
    /// the key installation would mint on the tab's first leaf panel — used to
    /// phrase expectations in `PaneKey` terms before the pane exists.
    #[cfg(test)]
    pub(crate) fn pane_key_for_anchor(&self, anchor: &AttachmentKey) -> Option<PaneKey> {
        if let Some(record) = self
            .panes
            .iter()
            .find(|record| record.key.anchor() == anchor && !record.departing)
        {
            return Some(record.key.clone());
        }
        let tab = self
            .cache
            .tabs()
            .iter()
            .find(|tab| tab.tab_id() == anchor.tab_id())?;
        let panel = tab.leaf_panel_ids().into_iter().next()?;
        Some(PaneKey::new(anchor.clone(), panel))
    }

    /// The focus pane's record when it is attached and live — the pane the
    /// session's input, ready checks, and status surface describe.
    pub(crate) fn focus_record(&self) -> Option<&PaneRecord> {
        let desired = self.desired.as_ref()?;
        self.pane_record(desired).filter(|record| !record.departing)
    }

    /// The focus pane's accepted frame as a `(anchor, frame)` pair — the shape
    /// the single-attachment `accepted_frame` field exposed.
    #[cfg(test)]
    pub(crate) fn focus_accepted_frame(&self) -> Option<(AttachmentKey, FrameIdentity)> {
        self.focus_record()
            .and_then(|record| Some((record.key.anchor().clone(), record.accepted_frame()?)))
    }

    #[cfg(test)]
    pub(crate) fn focus_last_accepted_geometry(&self) -> Option<(AttachmentKey, Geometry)> {
        self.focus_record().and_then(|record| {
            Some((
                record.key.anchor().clone(),
                record.last_accepted_geometry()?,
            ))
        })
    }

    /// The live (non-departing) record for one panel of the focus epoch — the
    /// render path asks for the pane's attachment facts by stable panel id.
    pub(crate) fn pane_by_panel(&self, panel: &WorkbenchPanelId) -> Option<&PaneRecord> {
        self.panes
            .iter()
            .find(|record| record.key.panel() == panel && !record.departing)
    }

    pub(crate) fn active(&self) -> Option<&SelectedTab> {
        match &self.phase {
            ClientPhase::Empty => None,
            ClientPhase::Starting(setup) => Some(&setup.selected),
            ClientPhase::SelectedWithoutAttachment(selected)
            | ClientPhase::ReconcilingMembership(selected)
            | ClientPhase::RetryableAttachUnavailable(selected) => Some(selected),
            ClientPhase::Ready(ready) => Some(&ready.selected),
            ClientPhase::Creating(barrier) => barrier.anchor.selected(),
            ClientPhase::CreateCommittedNeedDetach(committed)
            | ClientPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                committed.anchor.selected()
            }
            ClientPhase::Switching(setup) => Some(&setup.target.selected),
            ClientPhase::Retired(retired) => retired.diagnostic_active.as_ref(),
        }
    }

    pub(crate) fn controller_mode(&self) -> Option<&ControllerMode> {
        match &self.phase {
            ClientPhase::Empty => None,
            ClientPhase::ReconcilingMembership(_) => None,
            ClientPhase::Ready(ready) => Some(&ready.mode),
            ClientPhase::Creating(barrier) => barrier.anchor.ready().map(|ready| &ready.mode),
            ClientPhase::CreateCommittedNeedDetach(committed)
            | ClientPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                committed.anchor.ready().map(|ready| &ready.mode)
            }
            ClientPhase::Starting(setup) => setup.mode.as_ref(),
            ClientPhase::Switching(setup) => setup.mode.as_ref(),
            ClientPhase::SelectedWithoutAttachment(_)
            | ClientPhase::RetryableAttachUnavailable(_)
            | ClientPhase::Retired(_) => None,
        }
    }

    pub(crate) fn configure_panel_focus(&mut self, focus_new_panel: bool) {
        self.focus_new_panel = focus_new_panel;
    }

    pub(crate) const fn latest_geometry(&self) -> Geometry {
        self.latest_geometry
    }

    #[cfg(test)]
    pub(crate) fn last_admitted_geometry(&self, key: &PaneRef) -> Option<Geometry> {
        self.live_pane_record(key)
            .and_then(|record| record.last_admitted_geometry)
    }

    #[cfg(test)]
    pub(crate) fn last_accepted_geometry(&self, key: &PaneRef) -> Option<Geometry> {
        self.live_pane_record(key)
            .and_then(|record| record.last_accepted_geometry())
    }

    pub(crate) const fn notice(&self) -> Option<ClientNotice> {
        self.notice
    }

    pub(crate) const fn pending_refresh_generation(&self) -> Option<u64> {
        match self.pending_refresh {
            Some(flight) => Some(flight.generation),
            None => None,
        }
    }

    /// Whether the refresh in flight is only a background publication read. Its result can
    /// rebase what an action targets but never claims that target, so it is no reason to make
    /// an action unavailable; an action that needs the refresh slot waits for it instead.
    pub(crate) const fn background_refresh_only(&self) -> bool {
        matches!(
            self.pending_refresh,
            Some(PendingRefreshFlight {
                source: RefreshSource::Publication,
                ..
            })
        )
    }

    /// Whether the user's own Context or Tab switch is still landing: its read is in flight or
    /// the terminal it lands on still attaches. Like a background read, it settles on its own,
    /// so an action pressed meanwhile waits for it instead of being refused as busy.
    pub(crate) const fn switch_landing(&self) -> bool {
        self.context_selection_pending() || matches!(self.phase, ClientPhase::Switching(_))
    }

    /// What a held invocation waits out before it runs through its ordinary entry point.
    pub(crate) const fn settling_for_user_action(&self) -> bool {
        self.background_refresh_only() || self.switch_landing()
    }

    /// A refresh this Client started for its own intent: an explicit refresh or the recovery
    /// read of its own selection or mutation.
    pub(crate) const fn own_refresh_pending(&self) -> bool {
        self.pending_refresh.is_some() && !self.background_refresh_only()
    }

    #[cfg(test)]
    pub(crate) const fn refresh_status(&self) -> Option<NavigatorRefreshStatus> {
        self.refresh_status
    }

    pub(crate) fn provenance(&self, index: usize) -> Option<&CreateTerminalProvenance> {
        self.provenance.get(index)
    }

    pub(crate) fn tab(&self, index: usize) -> Option<&ClientTab> {
        self.cache.tabs().get(index)
    }

    pub(crate) fn ready(&self) -> Option<&ReadyAttachment> {
        let ClientPhase::Ready(ready) = &self.phase else {
            return None;
        };
        Some(ready)
    }

    pub(crate) fn committed_target(&self) -> Option<&CommittedTabCreate> {
        match &self.phase {
            ClientPhase::CreateCommittedNeedDetach(committed)
            | ClientPhase::ProjectionUnavailableBeforeCleanup(committed) => Some(committed),
            ClientPhase::Retired(retired) => retired.committed.as_ref(),
            _ => None,
        }
    }

    pub(crate) const fn retired_failure(&self) -> Option<CarrierFailure> {
        let ClientPhase::Retired(retired) = &self.phase else {
            return None;
        };
        Some(retired.failure)
    }

    /// Whether the exact pane incarnation `key` still has a record, departing or not.
    pub(crate) fn pane_present(&self, key: &PaneRef) -> bool {
        self.live_pane_record(key).is_some()
    }

    /// Whether `pane` presents the only Presentation of its Panel, so closing it closes the Panel.
    pub(crate) fn sole_presentation(&self, pane: &PaneKey) -> bool {
        self.cache
            .tabs()
            .iter()
            .find(|tab| tab.tab_id() == pane.anchor().tab_id())
            .and_then(|tab| tab.panel(tab.panel_index(pane.panel())?))
            .is_some_and(|panel| panel.presentation_count() == 1)
    }

    pub(crate) fn pane_attached(&self, key: &PaneRef) -> bool {
        self.live_pane_record(key)
            .is_some_and(|record| record.attached())
    }

    /// Every pane record's live facts for the pointer probe — exact
    /// incarnation, attachment state, and controller mode. The probe
    /// caller pairs these with the view's per-incarnation projection
    /// status; positional and anchor spellings never enter the list.
    pub(crate) fn pane_facts(
        &self,
    ) -> impl Iterator<Item = (PaneRef, bool, Option<&ControllerMode>)> + '_ {
        self.panes
            .iter()
            .map(|record| (record.pane_ref(), record.attached(), record.mode()))
    }

    /// The exact live incarnation of the focus pane — the key input, resize,
    /// and focus-scoped setup commands are tagged with. `None` when the focus
    /// pane departed or has no record.
    pub(crate) fn focus_pane_ref(&self) -> Option<PaneRef> {
        self.desired
            .as_ref()
            .and_then(|pane| self.pane_record(pane))
            .filter(|record| !record.departing)
            .map(PaneRecord::pane_ref)
    }

    /// The panes that own a live viewer, an installed projection, and a
    /// settled controller mode — the ordinary-poll rotation set, in stable
    /// record order. A still-configuring leaf is excluded: its own setup
    /// pipeline polls it, and drawing its shell before `SetupConfigured`
    /// would show content the same leaf still refuses a focus chord for.
    pub(crate) fn pollable_panes(&self) -> Vec<PaneRef> {
        self.panes
            .iter()
            .filter(|record| record.projected() && !record.departing && record.mode().is_some())
            .map(PaneRecord::pane_ref)
            .collect()
    }

    /// The focus pane's panel identity — used to match leaf geometry reports
    /// and projection lookups against the focus.
    pub(crate) fn focus_panel_id(&self) -> Option<&WorkbenchPanelId> {
        self.desired.as_ref().map(PaneKey::panel)
    }

    /// Whether a context-selection admission is currently in flight —
    /// the Agent board's jump planner waits it out instead of stacking
    /// a second selection behind it.
    pub(crate) fn context_selecting(&self) -> bool {
        self.context_selection.is_some()
    }

    pub(crate) fn pane_projected(&self, key: &PaneRef) -> bool {
        self.live_pane_record(key)
            .is_some_and(|record| record.projected())
    }

    pub(crate) fn expected_frame_geometry(&self, key: &PaneRef) -> Option<Geometry> {
        let record = self.live_pane_record(key)?;
        if !record.projected() {
            return None;
        }
        // A peer can take control and resize without notifying this Client's control plane.
        // Once Ready, authenticated snapshots own display geometry; local resize receipts
        // must not freeze observation at the former controller's size. Initial setup and
        // the explicit handoff's first frame still require their acknowledged geometry.
        if matches!(self.phase, ClientPhase::Ready(_))
            && record.display_initialized()
            && self.takeover_input_ready()
        {
            return None;
        }
        match record.mode() {
            Some(ControllerMode::Controlling(_)) => record.last_accepted_geometry(),
            Some(ControllerMode::ReadOnly) | None => None,
        }
    }

    pub(crate) fn clear_notice(&mut self) {
        self.notice = None;
    }

    pub(crate) fn set_notice(&mut self, notice: ClientNotice) {
        self.dismiss_close_set_summary();
        self.notice = Some(notice);
    }

    pub(crate) fn observe_geometry(&mut self, geometry: Geometry) {
        self.latest_geometry = geometry;
    }

    /// Per-pane leaf observation: the shell projects every visible leaf each
    /// viewport sync and reports `(panel_id, panel_index, content_geometry)`.
    /// Surviving records keep their identity; a leaf with no record gets one
    /// (fresh incarnation); a non-departing record whose panel vanished becomes
    /// `departing` and waits for canonical disposal. Returns whether the
    /// membership set diverged, so the caller may schedule reconciliation.
    pub(crate) fn sync_pane_set(
        &mut self,
        leaves: &[(WorkbenchPanelId, usize, Geometry)],
        sampled_at: Instant,
    ) -> bool {
        if self
            .focus_panel_id()
            .is_some_and(|id| !leaves.iter().any(|(panel, _, _)| panel == id))
        {
            self.cancel_duplicate_sequence();
        }
        let Some(anchor) = self.desired.as_ref().map(|key| key.anchor().clone()) else {
            return false;
        };
        let active_index = self.active().map(SelectedTab::index);
        let mut changed = false;
        for (panel_id, panel_index, geometry) in leaves {
            let pane = PaneKey::new(anchor.clone(), panel_id.clone());
            match self.pane_record_mut(&pane) {
                Some(record) if record.departing => {
                    record.departing = false;
                    record.index = active_index.unwrap_or(record.index);
                    record.panel_index = *panel_index;
                    record.observed_geometry = *geometry;
                    changed = true;
                }
                Some(record) => {
                    record.panel_index = *panel_index;
                    record.observed_geometry = *geometry;
                }
                None => {
                    let Some(tab_index) = self.active().map(SelectedTab::index) else {
                        continue;
                    };
                    let Some(tab) = self.cache.tabs().get(tab_index) else {
                        continue;
                    };
                    let Some(panel) = tab.panel(*panel_index) else {
                        continue;
                    };
                    let member = self.selected_presentation(panel);
                    let Some(binding) = member.binding().cloned() else {
                        continue;
                    };
                    let provenance = self.member_provenance(tab_index, panel, member);
                    let incarnation = self.next_incarnation;
                    self.next_incarnation += 1;
                    self.panes.push(PaneRecord::new(
                        PaneRef::new(pane, incarnation),
                        tab_index,
                        *panel_index,
                        self.cache.session_id().clone(),
                        binding,
                        provenance,
                        *geometry,
                    ));
                    changed = true;
                }
            }
        }
        let leaf_ids: std::collections::BTreeSet<&WorkbenchPanelId> =
            leaves.iter().map(|(panel_id, _, _)| panel_id).collect();
        for record in &mut self.panes {
            if record.departing || record.key.anchor != anchor {
                continue;
            }
            if !leaf_ids.contains(record.key.panel()) {
                record.mark_departing();
                changed = true;
            }
        }
        // A fresh divergence mints the episode budget only when no live
        // episode exists: an identical re-observation (`changed == false`)
        // never renews one, and a dead episode's retained deadline is
        // replaced only because the membership evidence genuinely changed.
        // The mint revives records the expired episode parked — the new
        // divergence is itself the new evidence that justifies retrying them.
        if changed
            && self
                .reconcile_deadlines
                .is_none_or(|deadlines| deadlines.total() <= sampled_at)
        {
            self.reconcile_deadlines = OperationDeadlines::from_sample(sampled_at);
            for record in &mut self.panes {
                if record.key.anchor == anchor
                    && matches!(record.fault, Some(PaneFault::ConvergenceExpired))
                {
                    record.fault = None;
                }
            }
        }
        changed
    }

    /// Whether any record still owes work to the executor: a departed pane
    /// awaiting disposal proof, or a live attachable pane not yet attached.
    /// Old-anchor leftovers (owned by the epoch cleanup path) do not count.
    pub(crate) fn reconcile_pending(&self) -> bool {
        let Some(anchor) = self.desired.as_ref().map(|key| key.anchor().clone()) else {
            return false;
        };
        self.panes.iter().any(|record| {
            record.key.anchor == anchor
                && (record.departing
                    || (record.attachable() && !record.attached() && record.retryable()))
        })
    }

    /// The reconcile plan snapshot the UI enqueues: every non-departing,
    /// attachable record of the focus epoch as an attach target carrying the
    /// geometry it must converge to (the executor re-diffs against its live
    /// attachments, so already-live panes are free), plus the single absolute
    /// deadline minted when the divergence appeared. A deadline that already
    /// expired without converging is a dead episode — terminal, never
    /// silently renewed: pending attach records park as `ConvergenceExpired`,
    /// departing records keep their real attachment ownership (no disposal
    /// is fabricated), and the expired deadline is retained as the
    /// dead-episode marker so identical re-observations cannot mint a fresh
    /// one. Genuinely new membership or a successfully installed explicit
    /// refresh may open the next episode; admission and automatic drains may not.
    pub(crate) fn reconcile_targets(
        &mut self,
        sampled_at: Instant,
    ) -> Option<(AttachmentKey, Vec<ReconcileTarget>, OperationDeadlines)> {
        let anchor = self.desired.as_ref()?.anchor().clone();
        if !self.reconcile_pending() {
            self.reconcile_deadlines = None;
            return None;
        }
        if self
            .reconcile_deadlines
            .is_some_and(|deadlines| deadlines.total() <= sampled_at)
        {
            for record in &mut self.panes {
                if record.key.anchor == anchor
                    && !record.departing
                    && record.attachable()
                    && !record.attached()
                    && record.retryable()
                {
                    record.fault = Some(PaneFault::ConvergenceExpired);
                }
            }
            return None;
        }
        let deadlines = self.reconcile_deadlines?;
        let targets = self
            .panes
            .iter()
            .filter(|record| {
                record.key.anchor == anchor
                    && !record.departing
                    && record.attachable()
                    && record.retryable()
            })
            .map(ReconcileTarget::new)
            .collect();
        Some((anchor, targets, deadlines))
    }

    /// Records the worker's canonical disposal of one pane: the record drops
    /// only when the departing record still carries the exact incarnation the
    /// detach proof cites, so a re-incarnated record is never dropped by a
    /// stale proof.
    pub(crate) fn settle_pane_detached(&mut self, key: &PaneRef) -> bool {
        if self.focus_pane_ref().as_ref() == Some(key) {
            self.cancel_duplicate_sequence();
        }
        let Some(position) = self.panes.iter().position(|record| {
            &record.key == key.pane() && record.incarnation == key.incarnation() && record.departing
        }) else {
            return false;
        };
        self.panes.remove(position);
        true
    }

    /// Epoch-level disposal: every record of the retired anchor drops once the
    /// membership cleanup proves the whole anchor detached.
    pub(crate) fn drop_anchor_panes(&mut self, anchor: &AttachmentKey) {
        self.panes.retain(|record| &record.key.anchor != anchor);
    }

    /// Marks every pane departing: used when a new anchor is installed (tab
    /// switch or committed create) so the old epoch's viewers converge to
    /// disposal while the new focus pane's record is minted by the caller.
    /// Unlike a same-anchor leaf departure, epoch teardown also voids the
    /// projection fact — old-anchor records never reappear in place, and the
    /// cleanup guards read `projected` as their uninstalled proof.
    pub(crate) fn depart_all_panes(&mut self) {
        for record in &mut self.panes {
            record.mark_departing();
            if let Some(attachment) = &mut record.attachment {
                attachment.withdraw_projection();
            }
        }
    }

    /// Installs the fresh focus record for a newly minted target. The caller
    /// owns incarnation minting (attach_target); this only registers the record.
    pub(crate) fn install_pane_record(&mut self, target: &AttachTarget) {
        let Some(binding) = target.binding.clone() else {
            return;
        };
        let pane = target.selected.key.clone();
        if self.pane_record(&pane).is_some() {
            return;
        }
        self.panes.push(PaneRecord::new(
            PaneRef::new(pane, target.incarnation),
            target.selected.index,
            target.selected.panel_index,
            target.session_id.clone(),
            binding,
            target.provenance.clone(),
            self.latest_geometry,
        ));
    }

    /// Installs the focus record for a selection that will never attach (the
    /// pane's provenance is not attachable): the record carries the leaf facts
    /// so status and reconcile see a parked, fault-free pane.
    pub(crate) fn install_unattached_pane(&mut self, selected: &SelectedTab) {
        let pane = selected.key.clone();
        if self.pane_record(&pane).is_some() {
            return;
        }
        let Some(tab) = self.cache.tabs().get(selected.index) else {
            return;
        };
        let Some(panel) = tab.panel(selected.panel_index) else {
            return;
        };
        let member = self.selected_presentation(panel);
        let Some(binding) = member.binding().cloned() else {
            return;
        };
        let provenance = self.member_provenance(selected.index(), panel, member);
        let incarnation = self.next_incarnation;
        self.next_incarnation += 1;
        self.panes.push(PaneRecord::new(
            PaneRef::new(pane, incarnation),
            selected.index,
            selected.panel_index,
            self.cache.session_id().clone(),
            binding,
            provenance,
            self.latest_geometry,
        ));
    }

    /// Only a quiescent focus may park after a typed runtime-loss notice. A
    /// structural owner keeps its original retirement/recovery contract.
    fn focus_runtime_loss_admissible(&self, key: &PaneRef) -> bool {
        matches!(&self.phase, ClientPhase::Ready(ready)
            if ready.selected.key == *key.pane() && self.desired.as_ref() == Some(key.pane()))
            && self.invariant_holds()
            && self
                .pending_refresh
                .as_ref()
                .is_none_or(|flight| flight.source == RefreshSource::Publication)
            && !self.mutation_blocks_structure()
            && !self.rename_draft_blocks_structure()
            && self.rename_uncertain.is_none()
            && self.pending_membership.is_none()
            && self.close.permits_retired_membership()
            && self.panel_close.confirmation().is_none()
            && self
                .live_pane_record(key)
                .is_some_and(|record| record.attached() && !record.departing)
    }

    /// A lost focused viewer still owns its selection until canonical membership
    /// changes or a re-attach lands. Only the bounded automatic re-attach of
    /// [`Self::target_for_lost_focus_reattach`] and a reselection retry it; an
    /// unavailable Attach is never retried.
    pub(crate) fn focused_viewer_lost(&self) -> bool {
        matches!(&self.phase, ClientPhase::RetryableAttachUnavailable(selected)
            if self.desired.as_ref() == Some(&selected.key))
            && self.focus_has_lost_viewer()
    }

    fn focus_has_lost_viewer(&self) -> bool {
        self.focus_record().is_some_and(|record| {
            !record.attached() && !record.departing && record.fault == Some(PaneFault::ViewerLost)
        })
    }

    /// One pane's controller grant ended or was lost: the pane keeps its live
    /// viewer but is parked view-only until an explicit reconcile re-arms it.
    /// No automatic re-acquire — NotController and definite failures never
    /// silently retry.
    pub(crate) fn settle_pane_fault(&mut self, key: &PaneRef, fault: PaneFault) -> bool {
        if self.focus_pane_ref().as_ref() == Some(key) {
            self.cancel_duplicate_sequence();
        }
        if fault == PaneFault::ViewerLost
            && self.desired.as_ref() == Some(key.pane())
            && !self.focus_runtime_loss_admissible(key)
        {
            return false;
        }
        let eligible = self
            .live_pane_record(key)
            .is_some_and(|record| match fault {
                PaneFault::AttachRejected
                | PaneFault::AttachUnavailable
                | PaneFault::CapacityExceeded
                | PaneFault::ConvergenceExpired => !record.attached(),
                PaneFault::ViewerLost | PaneFault::ControlLost => record.attached(),
            });
        if !eligible {
            return false;
        }
        let pane = key.pane().clone();
        {
            let Some(record) = self.live_pane_record_mut(key) else {
                return false;
            };
            match (fault, &mut record.attachment) {
                (PaneFault::ViewerLost, attachment) => *attachment = None,
                (PaneFault::ControlLost, Some(attachment)) => {
                    attachment.mode = Some(ControllerMode::ReadOnly);
                    attachment.last_accepted_geometry = None;
                }
                _ => {}
            }
            record.fault = Some(fault);
        }
        // A focus pane losing its grant downgrades the session's controller mode
        // to view-only as well; other panes are unaffected. A focus pane whose
        // viewer was lost drops to the unattached-selection phases, exactly like
        // a failed setup: parked until the bounded automatic re-attach or an
        // explicit new selection retries.
        if self.desired.as_ref() == Some(&pane) {
            match fault {
                PaneFault::ControlLost => match &mut self.phase {
                    ClientPhase::Ready(ready) => ready.mode = ControllerMode::ReadOnly,
                    ClientPhase::Starting(setup) => {
                        setup.mode = Some(ControllerMode::ReadOnly);
                        setup.accepted_geometry = None;
                    }
                    ClientPhase::Switching(setup) => {
                        setup.mode = Some(ControllerMode::ReadOnly);
                        setup.accepted_geometry = None;
                    }
                    ClientPhase::Creating(barrier) => {
                        if let CreateAnchor::Ready(ready) = &mut barrier.anchor {
                            ready.mode = ControllerMode::ReadOnly;
                        }
                    }
                    ClientPhase::CreateCommittedNeedDetach(committed)
                    | ClientPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                        if let CreateAnchor::Ready(ready) = &mut committed.anchor {
                            ready.mode = ControllerMode::ReadOnly;
                        }
                    }
                    _ => {}
                },
                PaneFault::ViewerLost => {
                    self.clear_pane_attachment(&pane);
                    if let Some(selected) = self.active().cloned() {
                        let attachable = self
                            .pane_record(&pane)
                            .is_some_and(|record| record.attachable());
                        self.phase = if attachable {
                            ClientPhase::RetryableAttachUnavailable(selected)
                        } else {
                            ClientPhase::SelectedWithoutAttachment(selected)
                        };
                    }
                }
                _ => {}
            }
        }
        true
    }

    /// The resizes changed visible controlling leaves owe the executor. Zoom
    /// excludes hidden peers; the caller records each admission through
    /// [`Self::record_admitted_geometry`] once the command is queued, so a
    /// Full/Busy refusal simply retries on the next observation.
    pub(crate) fn admit_observed_geometries(
        &mut self,
        observed: &[(WorkbenchPanelId, usize, Geometry)],
    ) -> Vec<(PaneRef, Geometry)> {
        let mut resizes = Vec::new();
        for (panel_id, _index, geometry) in observed {
            let Some(focus) = self.desired.as_ref() else {
                break;
            };
            if self.zoom_active() && focus.panel() != panel_id {
                continue;
            }
            let pane = PaneKey::new(focus.anchor().clone(), panel_id.clone());
            let Some(record) = self.pane_record(&pane) else {
                continue;
            };
            if record.departing
                || !record.attached()
                || !matches!(record.mode(), Some(ControllerMode::Controlling(_)))
                || record.last_admitted_geometry == Some(*geometry)
            {
                continue;
            }
            resizes.push((record.pane_ref(), *geometry));
        }
        resizes
    }

    pub(crate) fn record_admitted_geometry(&mut self, key: &PaneRef, geometry: Geometry) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        if record.departing
            || !record.attached()
            || !matches!(record.mode(), Some(ControllerMode::Controlling(_)))
        {
            return false;
        }
        record.last_admitted_geometry = Some(geometry);
        true
    }

    pub(crate) fn record_setup_configuration(
        &mut self,
        key: &PaneRef,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> bool {
        // Every admitting phase below requires the live record to hold a viewer.
        if matches!(&mode, ControllerMode::Controlling(_)) != applied_geometry.is_some()
            || self
                .live_pane_record(key)
                .is_none_or(|record| !record.attached())
        {
            return false;
        }
        let pane_attached = self.pane_attached(key);
        let background_live = self.desired.as_ref() != Some(key.pane())
            && self
                .live_pane_record(key)
                .is_some_and(|record| record.attached() && !record.departing);
        let background_resize = background_live
            && applied_geometry.is_some()
            && self
                .live_pane_record(key)
                .is_some_and(|record| record.mode() == Some(&mode));
        let lost_focus_background_resize = background_resize
            && self.focused_viewer_lost()
            && self
                .desired
                .as_ref()
                .is_some_and(|desired| key.anchor() == desired.anchor());
        let accepted_geometry = match &mut self.phase {
            ClientPhase::Starting(setup) if setup.selected.key == *key.pane() && pane_attached => {
                setup.mode = Some(mode.clone());
                Some(&mut setup.accepted_geometry)
            }
            ClientPhase::Ready(ready)
                if ready.selected.key == *key.pane() && ready.mode == mode && pane_attached =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            ClientPhase::Creating(barrier)
                if barrier.anchor.ready().is_some_and(|ready| {
                    ready.selected.key == *key.pane() && ready.mode == mode
                }) && pane_attached =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            ClientPhase::CreateCommittedNeedDetach(committed)
                if committed.anchor.ready().is_some_and(|ready| {
                    ready.selected.key == *key.pane() && ready.mode == mode
                }) && pane_attached =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            ClientPhase::Switching(setup)
                if setup.target.selected.key == *key.pane()
                    && setup.membership.is_none()
                    && setup.previous.is_none()
                    && pane_attached =>
            {
                setup.mode = Some(mode.clone());
                Some(&mut setup.accepted_geometry)
            }
            ClientPhase::Switching(setup)
                if setup.previous.as_ref().is_some_and(|previous| {
                    previous.selected.key == *key.pane() && previous.mode.as_ref() == Some(&mode)
                }) && pane_attached =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            // A background pane finishing its reconcile setup while the session
            // is Ready: the record is live, not the focus, and not departing.
            ClientPhase::Ready(_) if background_live => None,
            // A typed loss of the focus viewer does not revoke a sibling's
            // previously admitted controlling Resize.
            ClientPhase::RetryableAttachUnavailable(_) if lost_focus_background_resize => None,
            // The executor drains earlier accepted Resize leases before Create.
            // Their geometry may precede a newer admission on this same pane.
            ClientPhase::Creating(barrier)
                if background_resize
                    && barrier.anchor.selected().is_some_and(|selected| {
                        self.desired.as_ref() == Some(&selected.key)
                            && key.anchor() == selected.key.anchor()
                    }) =>
            {
                None
            }
            _ => return false,
        };
        if let Some(accepted_geometry) = accepted_geometry {
            *accepted_geometry = applied_geometry;
        }
        if matches!(&self.phase, ClientPhase::Switching(setup)
            if setup.previous.as_ref().is_some_and(|previous| previous.selected.key == *key.pane()))
        {
            self.invalidate_switch_anchor();
        }
        let Some(attachment) = self
            .live_pane_record_mut(key)
            .and_then(|record| record.attachment.as_mut())
        else {
            return false;
        };
        attachment.mode = Some(mode);
        attachment.last_accepted_geometry = applied_geometry;
        attachment.void_frames();
        true
    }

    pub(crate) fn settle_setup_configuration_event(
        &mut self,
        key: &PaneRef,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> Option<SetupEventDisposition> {
        let desired = self.desired.clone()?;
        if &desired == key.pane() || key.anchor() == desired.anchor() {
            return self
                .record_setup_configuration(key, mode, applied_geometry)
                .then_some(SetupEventDisposition::Applied);
        }
        if key.anchor().generation() >= desired.anchor().generation()
            || matches!(&mode, ControllerMode::Controlling(_)) != applied_geometry.is_some()
            || !self.pane_attached(key)
            || self.live_pane_record(key).is_none_or(PaneRecord::projected)
        {
            return None;
        }
        let ClientPhase::Switching(setup) = &mut self.phase else {
            return None;
        };
        let previous = setup.previous.as_mut()?;
        if previous.selected.key != *key.pane()
            || previous
                .mode
                .as_ref()
                .is_some_and(|recorded| recorded != &mode)
        {
            return None;
        }
        previous.mode = Some(mode.clone());
        if let Some(attachment) = self
            .live_pane_record_mut(key)
            .and_then(|record| record.attachment.as_mut())
        {
            attachment.last_accepted_geometry = applied_geometry;
        }
        self.invalidate_switch_anchor();
        Some(SetupEventDisposition::Superseded { desired })
    }

    pub(crate) fn stage_drawn_frame(&mut self, key: &PaneRef, frame: FrameIdentity) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        if record.departing {
            return false;
        }
        let Some(acknowledgement) = record.frame_mut() else {
            return false;
        };
        *acknowledgement = FrameAcknowledgement::Staged(frame);
        true
    }

    pub(crate) fn invalidate_projection_frame(&mut self, key: &PaneRef) -> bool {
        let Some(acknowledgement) = self
            .live_pane_record_mut(key)
            .and_then(PaneRecord::frame_mut)
        else {
            return false;
        };
        *acknowledgement = FrameAcknowledgement::Unstaged;
        true
    }

    pub(crate) fn publish_ready(
        &mut self,
        key: &PaneRef,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> bool {
        let selected = match &self.phase {
            ClientPhase::Starting(setup) if setup.selected.key == *key.pane() => {
                setup.selected.clone()
            }
            ClientPhase::Switching(setup)
                if setup.target.selected.key == *key.pane() && setup.membership.is_none() =>
            {
                setup.target.selected.clone()
            }
            _ => return false,
        };
        let Some(record) = self.live_pane_record(key) else {
            return false;
        };
        if record.accepted_frame() != Some(frame)
            || self.controller_mode() != Some(&mode)
            || matches!(&mode, ControllerMode::Controlling(_)) != accepted_geometry.is_some()
        {
            return false;
        }
        if matches!(&mode, ControllerMode::Controlling(_)) {
            let Some(geometry) = accepted_geometry else {
                return false;
            };
            if geometry != self.latest_geometry || record.last_accepted_geometry() != Some(geometry)
            {
                return false;
            }
        }
        self.phase = ClientPhase::Ready(ReadyAttachment::new(selected, mode));
        true
    }

    pub(crate) fn settle_ready_candidate(
        &mut self,
        key: &PaneRef,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> Result<ReadyDecision, CarrierFailure> {
        if self.desired.as_ref() != Some(key.pane()) {
            return Ok(ReadyDecision::Superseded);
        }
        let selected_matches = match &self.phase {
            ClientPhase::Starting(setup) => setup.selected.key == *key.pane(),
            ClientPhase::Switching(setup) => {
                setup.target.selected.key == *key.pane() && setup.membership.is_none()
            }
            _ => false,
        };
        if !selected_matches {
            return Err(CarrierFailure::Protocol);
        }
        let Some(record) = self.live_pane_record(key) else {
            return Err(CarrierFailure::Protocol);
        };
        if record.accepted_frame() != Some(frame)
            || self.controller_mode() != Some(&mode)
            || matches!(&mode, ControllerMode::Controlling(_)) != accepted_geometry.is_some()
        {
            return Err(CarrierFailure::Protocol);
        }
        if let ControllerMode::Controlling(_) = &mode {
            let Some(geometry) = accepted_geometry else {
                return Err(CarrierFailure::Protocol);
            };
            if record.last_accepted_geometry() != Some(geometry) {
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

    /// The focus pane's setup attach installs its projection: only valid while
    /// the phase is Starting or a Switching target that has not yet installed.
    pub(crate) fn install_projection(&mut self, key: &PaneRef) -> bool {
        if self.desired.as_ref() != Some(key.pane())
            || self.pane_attached(key)
            || !matches!(&self.phase, ClientPhase::Starting(_))
                && !matches!(&self.phase, ClientPhase::Switching(setup)
                    if setup.previous.is_none() && setup.membership.is_none())
        {
            return false;
        }
        self.mark_pane_attached(key)
    }

    /// A reconcile-attached background pane installs its projection: the record
    /// must be live, not yet attached, not departing, and not the focus pane —
    /// focus attachment is phase-gated by `install_projection` so a mere
    /// switch intent cannot promote an `Attached` event into attachment
    /// authority before the candidate is installed.
    fn install_pane_attachment(&mut self, key: &PaneRef) -> bool {
        if self.desired.as_ref() == Some(key.pane()) {
            return false;
        }
        let Some(record) = self.live_pane_record(key) else {
            return false;
        };
        if record.attached() || record.departing {
            return false;
        }
        self.mark_pane_attached(key)
    }

    fn mark_pane_attached(&mut self, key: &PaneRef) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        record.attachment = Some(AttachedPane::installed());
        record.fault = None;
        true
    }

    pub(crate) fn settle_attached_event(&mut self, key: &PaneRef) -> Option<SetupEventDisposition> {
        let desired = self.desired.clone()?;
        if self.install_projection(key) || self.install_pane_attachment(key) {
            return Some(SetupEventDisposition::Applied);
        }
        // The exact incarnation's record is live but departing: the attach
        // proof landed after the leaf left the desired set. The setup is
        // superseded — never a protocol error, and never an install.
        if self
            .live_pane_record(key)
            .is_some_and(|record| record.departing)
        {
            return Some(SetupEventDisposition::Superseded { desired });
        }
        let ClientPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (key.anchor().generation() < desired.anchor().generation()
            && setup.previous.is_none()
            && !self.pane_attached(key)
            && !self.pane_projected(key)
            && self
                .cache
                .tabs()
                .iter()
                .any(|tab| tab.tab_id() == key.anchor().tab_id()))
        .then_some(SetupEventDisposition::Superseded { desired })
    }

    pub(crate) fn mark_feedback_accepted(&mut self, key: &PaneRef, frame: FrameIdentity) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        let Some(attachment) = record.attachment.as_mut() else {
            return false;
        };
        let PaneProjection::Installed(acknowledgement) = &mut attachment.projection else {
            return false;
        };
        if acknowledgement.staged() != Some(frame) {
            return false;
        }
        *acknowledgement = FrameAcknowledgement::Accepted(frame);
        attachment.display_initialized = true;
        self.accept_takeover_frame(key);
        true
    }

    pub(crate) fn settle_feedback_event(
        &mut self,
        key: &PaneRef,
        frame: FrameIdentity,
    ) -> Option<SetupEventDisposition> {
        let desired = self.desired.clone()?;
        if self.mark_feedback_accepted(key, frame) {
            return Some(SetupEventDisposition::Applied);
        }
        // Same departure window as `settle_attached_event`: a departing
        // record's late ack is superseded, not a protocol violation.
        if self
            .live_pane_record(key)
            .is_some_and(|record| record.departing)
        {
            return Some(SetupEventDisposition::Superseded { desired });
        }
        let ClientPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (key.anchor().generation() < desired.anchor().generation()
            && setup
                .previous
                .as_ref()
                .is_some_and(|previous| previous.selected.key == *key.pane())
            && self.pane_attached(key)
            && self
                .live_pane_record(key)
                .is_some_and(|record| !record.projected()))
        .then_some(SetupEventDisposition::Superseded { desired })
    }

    #[cfg(test)]
    pub(crate) fn feedback_accepted(&self) -> bool {
        self.focus_record()
            .is_some_and(|record| record.accepted_frame().is_some())
    }

    pub(crate) fn begin_create(&mut self, operation: CreateOperation) -> bool {
        let Some(context) = self.cache.canonical_context() else {
            return false;
        };
        if !self.invariant_holds()
            || orchbee_workbench_protocol::validate_tab_creation(
                context,
                operation.placement.as_ref(),
            )
            .is_err()
            || self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
        {
            return false;
        }
        let anchor = match (&self.phase, operation.origin) {
            (ClientPhase::Ready(ready), CreateOriginKind::Ready) => {
                CreateAnchor::Ready(ready.clone())
            }
            (ClientPhase::Empty, CreateOriginKind::Empty) => CreateAnchor::Empty,
            (ClientPhase::SelectedWithoutAttachment(selected), CreateOriginKind::Unattached) => {
                CreateAnchor::Unattached {
                    selected: selected.clone(),
                    retryable: false,
                }
            }
            (ClientPhase::RetryableAttachUnavailable(selected), CreateOriginKind::Unattached) => {
                CreateAnchor::Unattached {
                    selected: selected.clone(),
                    retryable: true,
                }
            }
            _ => return false,
        };
        let attachment_ok = match anchor.ready() {
            Some(ready) => {
                Some(&ready.selected.key) == self.desired.as_ref()
                    && self.focus_record().is_some_and(|record| record.projected())
            }
            None => {
                anchor.selected().map(|selected| &selected.key) == self.desired.as_ref()
                    && self.focus_record().is_none_or(|record| !record.attached())
            }
        };
        if !attachment_ok
            || self.pending_refresh.is_some()
            || operation.session_id != *self.cache.session_id()
            || operation.expected_revision != self.cache.revision()
            || Some(&operation.terminal_context_id) != self.cache.terminal_context_id()
            || operation.geometry != self.latest_geometry
            || operation.key.reserved_generation
                <= anchor
                    .selected()
                    .map_or(0, |selected| selected.key.anchor().generation())
        {
            return false;
        }
        self.remember_current_navigation();
        self.phase = ClientPhase::Creating(CreatingBarrier { operation, anchor });
        true
    }

    pub(crate) fn restore_precommit(
        &mut self,
        operation: &CreateOperationKey,
        applied_geometry: Option<Geometry>,
    ) -> bool {
        let ClientPhase::Creating(barrier) = &self.phase else {
            return false;
        };
        if barrier.operation.key != *operation {
            return false;
        }
        if let CreateAnchor::Unattached {
            selected,
            retryable,
        } = &barrier.anchor
        {
            if applied_geometry.is_some() || self.desired.as_ref() != Some(&selected.key) {
                return false;
            }
            self.phase = if *retryable {
                ClientPhase::RetryableAttachUnavailable(selected.clone())
            } else {
                ClientPhase::SelectedWithoutAttachment(selected.clone())
            };
            self.notice = Some(ClientNotice::CreateNotCreated);
            return true;
        }
        let Some(old_ready) = barrier.anchor.ready().cloned() else {
            if applied_geometry.is_some()
                || self.desired.is_some()
                || !self.panes.is_empty()
                || !self.cache.tabs().is_empty()
            {
                return false;
            }
            self.phase = ClientPhase::Empty;
            self.duplicate_precommit_failed(operation);
            self.notice = Some(ClientNotice::CreateNotCreated);
            return true;
        };
        let pane = old_ready.selected.key.clone();
        let Some(record) = self.pane_record(&pane) else {
            return false;
        };
        if !record.projected()
            || matches!(&old_ready.mode, ControllerMode::Controlling(_))
                != applied_geometry.is_some()
        {
            return false;
        }
        if matches!(&old_ready.mode, ControllerMode::Controlling(_)) {
            let Some(attachment) = self
                .pane_record_mut(&pane)
                .and_then(|record| record.attachment.as_mut())
            else {
                return false;
            };
            if attachment.last_accepted_geometry != applied_geometry {
                attachment.last_accepted_geometry = applied_geometry;
                attachment.void_frames();
            }
        }
        self.phase = ClientPhase::Ready(old_ready);
        self.duplicate_precommit_failed(operation);
        self.notice = Some(ClientNotice::CreateNotCreated);
        true
    }

    pub(crate) fn retain_create_commit(
        &mut self,
        operation: &CreateOperationKey,
        commit: WorkbenchTabCreateCommit,
    ) -> bool {
        let ClientPhase::Creating(barrier) = &self.phase else {
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
        // Keep only canonical members. The just-created identity is installed below.
        let snapshot = self.cache.canonical_snapshot();
        self.launch_descriptions.retain(|id, _| {
            snapshot.terminal_contexts.iter().any(|context| {
                context
                    .tabs
                    .iter()
                    .any(|tab| contains_presentation(&tab.root, id))
            })
        });
        if matches!(committed.provenance, CreateTerminalProvenance::Failed(_))
            && let Some(description) = committed.operation.launch_description.clone()
        {
            self.launch_descriptions
                .insert(committed.expectation.presentation_id().clone(), description);
        }
        self.duplicate_commit_fact(&committed);
        self.phase = ClientPhase::CreateCommittedNeedDetach(committed);
        true
    }

    pub(crate) fn begin_switch(&mut self, target: AttachTarget) -> bool {
        if self.rename_draft_blocks_structure() {
            return false;
        }
        self.begin_switch_membership(target)
    }
    pub(crate) fn settle_switch_entry_expired(
        &mut self,
        key: &PaneRef,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
        settled_at: Instant,
    ) -> Option<CleanupContinuation> {
        let desired = self.desired.clone()?;
        if self.desired.as_ref() != Some(key.pane()) {
            return (key.anchor().generation() < desired.anchor().generation()
                && self.cache.tabs().get(target_index)?.tab_id() == key.anchor().tab_id()
                && self.pane_provenance_matches(key, target_index, target_provenance))
            .then_some(CleanupContinuation::Superseded { desired });
        }
        let ClientPhase::Switching(setup) = &self.phase else {
            return None;
        };
        if setup.previous.is_some()
            || setup.membership.is_some()
            || setup.target.selected.key != *key.pane()
            || setup.target.selected.index != target_index
            || &setup.target.provenance != target_provenance
            || settled_at < setup.target.deadlines.forward_cutoff()
            || self.pane_attached(key)
            || self.pane_projected(key)
            || self
                .live_pane_record(key)
                .is_none_or(|record| record.staged_frame().is_some())
        {
            return None;
        }
        let selected = setup.target.selected.clone();
        self.phase = if target_provenance.attachable() {
            self.park_focus_pane();
            ClientPhase::RetryableAttachUnavailable(selected)
        } else {
            ClientPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    pub(crate) fn settle_cleanup(
        &mut self,
        key: &PaneRef,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        let ClientPhase::Switching(setup) = &self.phase else {
            return None;
        };
        if self.desired.as_ref() != Some(key.pane()) {
            let installed_previous = setup
                .previous
                .as_ref()
                .is_some_and(|previous| previous.selected.key == *key.pane())
                && self.pane_attached(key);
            let uninstalled_setup = setup.previous.is_none()
                && !self.pane_attached(key)
                && !self.pane_projected(key)
                && key.anchor().generation() < self.desired.as_ref()?.anchor().generation()
                && self.cache.tabs().get(target_index)?.tab_id() == key.anchor().tab_id()
                && self.pane_provenance_matches(key, target_index, target_provenance);
            if !installed_previous && !uninstalled_setup {
                return None;
            }
            if installed_previous {
                let ClientPhase::Switching(setup) = &mut self.phase else {
                    return None;
                };
                setup.previous = None;
                if let Some(membership) = &mut setup.membership {
                    membership.invalidate();
                }
                self.settle_pane_detached(key);
            }
            return Some(CleanupContinuation::Superseded {
                desired: self.desired.clone()?,
            });
        }
        if setup.previous.is_some() || setup.membership.is_some() {
            return None;
        }
        if setup.target.selected.key != *key.pane()
            || self.panes.iter().any(|record| {
                &record.key != key.pane()
                    && record.attached()
                    && record.incarnation != key.incarnation()
            })
            || self
                .live_pane_record(key)
                .is_none_or(|record| record.staged_frame().is_some())
            || self.cache.tabs().get(target_index)?.tab_id() != key.anchor().tab_id()
            || !self.pane_provenance_matches(key, target_index, target_provenance)
        {
            return None;
        }
        let selected = setup.target.selected.clone();
        self.clear_pane_attachment(key.pane());
        self.phase = if target_provenance.attachable() {
            self.park_focus_pane();
            ClientPhase::RetryableAttachUnavailable(selected)
        } else {
            ClientPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    pub(crate) fn settle_unattached_setup(
        &mut self,
        key: &PaneRef,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        if self.desired.as_ref() != Some(key.pane()) {
            return self
                .desired
                .clone()
                .map(|desired| CleanupContinuation::Superseded { desired });
        }
        if let ClientPhase::Starting(setup) = &self.phase {
            if setup.selected.key != *key.pane()
                || setup.selected.index() != target_index
                || self
                    .live_pane_record(key)
                    .is_none_or(|record| record.attached() || record.projected())
                || !self.pane_provenance_matches(key, target_index, target_provenance)
            {
                return None;
            }
            let selected = setup.selected.clone();
            self.park_focus_pane();
            self.phase = ClientPhase::RetryableAttachUnavailable(selected);
            return Some(CleanupContinuation::Stable);
        }
        let ClientPhase::Switching(setup) = &self.phase else {
            return None;
        };
        if setup.previous.is_some()
            || setup.membership.is_some()
            || setup.target.selected.key != *key.pane()
            || self.pane_attached(key)
            || self.cache.tabs().get(target_index)?.tab_id() != key.anchor().tab_id()
            || !self.pane_provenance_matches(key, target_index, target_provenance)
        {
            return None;
        }
        let selected = setup.target.selected.clone();
        self.phase = if target_provenance.attachable() {
            self.park_focus_pane();
            ClientPhase::RetryableAttachUnavailable(selected)
        } else {
            ClientPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    /// Clears one pane's attachment facts after a canonical disposal path (the
    /// record itself stays — it still owns the pane's leaf geometry and fault).
    fn clear_pane_attachment(&mut self, pane: &PaneKey) {
        if self.desired.as_ref() == Some(pane) {
            self.cancel_duplicate_sequence();
        }
        if let Some(record) = self.pane_record_mut(pane) {
            record.attachment = None;
        }
    }

    /// Parks the focus pane after a failed or expired setup: the record keeps
    /// its leaf facts but carries `AttachUnavailable` so reconciliation never
    /// silently re-attaches — only an explicit new selection may retry.
    fn park_focus_pane(&mut self) {
        let Some(desired) = self.desired.clone() else {
            return;
        };
        self.clear_pane_attachment(&desired);
        if let Some(record) = self.pane_record_mut(&desired) {
            record.fault = Some(PaneFault::AttachUnavailable);
        }
    }

    /// Registers a peer-publication hint captured from the mailbox ingress latch. The
    /// transfer is the only consumer of that latch: coalescing keeps the debt bounded,
    /// and a hint during a live flight only marks late demand for one follow-up flight.
    /// A parked demand re-arms here because a new accepted hint is itself new evidence.
    pub(crate) fn note_publication_hint(&mut self) {
        self.publication_debt = match self.publication_debt {
            PublicationDebt::Clean | PublicationDebt::Parked => PublicationDebt::Pending,
            PublicationDebt::Pending => PublicationDebt::Pending,
            PublicationDebt::InFlight { .. } => PublicationDebt::InFlight { late: true },
        };
    }

    /// Whether a captured publication demand is waiting for refresh admission.
    pub(crate) const fn publication_debt_pending(&self) -> bool {
        matches!(self.publication_debt, PublicationDebt::Pending)
    }

    #[cfg(test)]
    pub(crate) const fn publication_debt_kind(&self) -> PublicationDebt {
        self.publication_debt
    }

    /// Admits one pending publication demand into a fresh refresh flight. The flight
    /// adopts the demand, so its outcome settles exactly this debt and nothing else.
    pub(crate) fn start_publication_drain(&mut self, generation: u64) -> bool {
        self.publication_debt_pending()
            && self.start_refresh(generation, RefreshSource::Publication)
    }

    /// A refresh flight that starts while demand is outstanding adopts it; a parked
    /// demand is re-armed because this admission is itself an explicit refresh.
    fn adopt_publication_debt(&mut self) {
        if matches!(
            self.publication_debt,
            PublicationDebt::Pending | PublicationDebt::Parked
        ) {
            self.publication_debt = PublicationDebt::InFlight { late: false };
        }
    }

    /// The adopted flight installed a candidate: the demand is settled, unless a late
    /// hint during the flight keeps exactly one follow-up demand pending.
    fn install_publication_debt(&mut self) {
        if let PublicationDebt::InFlight { late } = self.publication_debt {
            self.publication_debt = if late {
                PublicationDebt::Pending
            } else {
                PublicationDebt::Clean
            };
        }
    }

    /// The adopted flight was definitely refused: the demand parks until an accepted
    /// hint or an explicit refresh re-arms it.
    fn park_publication_debt(&mut self) {
        if matches!(self.publication_debt, PublicationDebt::InFlight { .. }) {
            self.publication_debt = PublicationDebt::Parked;
        }
    }

    /// A flight that adopted demand but read no snapshot returns it unread instead of
    /// parking it. Stale evidence creates demand even when no hint was captured.
    fn release_unread_publication_debt(&mut self, stale: bool) {
        self.publication_debt = match self.publication_debt {
            PublicationDebt::InFlight { .. } => PublicationDebt::Pending,
            PublicationDebt::Clean | PublicationDebt::Parked if stale => PublicationDebt::Pending,
            debt => debt,
        };
    }

    pub(crate) fn start_pending_refresh(&mut self, generation: u64) -> bool {
        self.start_refresh(generation, RefreshSource::Explicit)
    }

    fn start_refresh(&mut self, generation: u64, source: RefreshSource) -> bool {
        if self.pending_refresh.is_some() || !self.phase_allows_refresh() {
            return false;
        }
        self.pending_refresh = Some(PendingRefreshFlight { generation, source });
        self.adopt_publication_debt();
        self.refresh_status = Some(NavigatorRefreshStatus::Refreshing);
        true
    }

    pub(crate) fn prepare_refresh(
        &self,
        generation: u64,
        outcome: WorkbenchSnapshotOutcome,
    ) -> PreparedRefresh {
        if self.pending_refresh_generation() != Some(generation) {
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
            // The current context no longer projects — either a peer workspace
            // close removed it (a permit-carrying landing candidate) or plain
            // divergence. The classifier decides; it never fabricates cleanup.
            return self.prepare_context_removal(generation, &snapshot);
        };
        if let ClientPhase::ProjectionUnavailableBeforeCleanup(committed) = &self.phase {
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
        } else if let Some(pending) = &self.panel_recovery {
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
        // The Tab survives but the selected leaf vanished (for example after our own panel.close):
        // repair the selection to a surviving leaf through the membership path instead of retiring.
        let leaf_repair = self
            .panel_recovery
            .as_ref()
            .and_then(|pending| pending.focus_index(&candidate))
            .or_else(|| {
                active
                    .as_ref()
                    .and_then(|active| self.selected_leaf_repair_index(&candidate, active))
            });
        if active
            .as_ref()
            .is_none_or(|active| candidate.tab_index(active.key.anchor().tab_id()).is_none())
            || leaf_repair.is_some()
        {
            return match self.prepare_membership_candidate(
                candidate,
                active.as_ref().map(|active| active.key.anchor().tab_id()),
                active.as_ref().map(|active| active.index),
                None,
                None,
            ) {
                Ok(mut prepared) => {
                    if let Some(panel_index) = leaf_repair {
                        prepared.retarget_selected_panel(panel_index);
                    }
                    PreparedRefresh::Membership {
                        generation,
                        prepared,
                    }
                }
                Err(_) => PreparedRefresh::Divergent { generation },
            };
        }
        let Some(active) = active else {
            return PreparedRefresh::Divergent { generation };
        };
        if candidate.tab_index(active.key.anchor().tab_id()).is_none() {
            return PreparedRefresh::Divergent { generation };
        }
        let baseline_phase = self.phase_kind();
        match baseline_phase {
            ClientPhaseKind::Ready
            | ClientPhaseKind::MoveProjectionUnavailable
            | ClientPhaseKind::PanelProjectionUnavailable
            | ClientPhaseKind::RenameProjectionUnavailable
            | ClientPhaseKind::SelectedWithoutAttachment
            | ClientPhaseKind::RetryableAttachUnavailable => PreparedRefresh::Append {
                generation,
                baseline_revision,
                baseline_phase,
                active,
                candidate,
            },
            _ => PreparedRefresh::Divergent { generation },
        }
    }

    pub(crate) fn apply_prepared_refresh(
        &mut self,
        prepared: PreparedRefresh,
        observed_at: Instant,
    ) -> RefreshDecision {
        let generation = match &prepared {
            PreparedRefresh::Stale => return RefreshDecision::Continue,
            PreparedRefresh::DefiniteFailure { generation, .. }
            | PreparedRefresh::CreateMembership { generation, .. }
            | PreparedRefresh::Membership { generation, .. }
            | PreparedRefresh::Append { generation, .. }
            | PreparedRefresh::Divergent { generation } => *generation,
        };
        if self.pending_refresh_generation() != Some(generation) {
            return RefreshDecision::Continue;
        }
        let explicit = self.pending_refresh.is_some_and(|flight| {
            flight.generation == generation && flight.source == RefreshSource::Explicit
        });
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
                    self.park_publication_debt();
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
                    // Only this successfully installed, generation-bound user
                    // flight may reopen expired work; admission and failed reads
                    // never create a recovery entitlement.
                    if explicit && let Err(failure) = self.reopen_expired_panes(observed_at) {
                        self.retire(failure);
                        return RefreshDecision::Retire;
                    }
                    self.install_publication_debt();
                    self.resolve_projection_recoveries();
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
            self.pending_refresh = None;
            self.context_selection = None;
        }
        decision
    }

    fn reopen_expired_panes(&mut self, observed_at: Instant) -> Result<(), CarrierFailure> {
        let Some(anchor) = self.desired.as_ref().map(|key| key.anchor().clone()) else {
            return Ok(());
        };
        // A refresh is not a keep-alive for an episode that still has budget.
        if self
            .reconcile_deadlines
            .is_some_and(|deadline| deadline.total() > observed_at)
        {
            return Ok(());
        }
        let eligible = |record: &PaneRecord| {
            record.key.anchor() == &anchor
                && !record.departing
                && record.attachable()
                && !record.attached()
                && record.fault == Some(PaneFault::ConvergenceExpired)
        };
        if !self.panes.iter().any(eligible) {
            return Ok(());
        }
        let deadline =
            OperationDeadlines::from_sample(observed_at).ok_or(CarrierFailure::Deadline)?;
        for record in &mut self.panes {
            if eligible(record) {
                record.fault = None;
            }
        }
        self.reconcile_deadlines = Some(deadline);
        Ok(())
    }

    /// The Tab keeps its identity in the candidate but the selected leaf vanished (for
    /// example after a peer panel.close folded into a later revision): the selection must
    /// repair onto the clamped surviving-leaf index through the membership path rather than
    /// a direct merge that cannot find it.
    fn selected_leaf_repair_index(
        &self,
        candidate: &ClientContextTabs,
        active: &SelectedTab,
    ) -> Option<usize> {
        let index = candidate.tab_index(active.key.anchor().tab_id())?;
        let previous = self
            .cache
            .tabs()
            .get(active.index)?
            .panel(active.panel_index())?;
        let tab = candidate.tabs().get(index)?;
        match tab.panel_index(previous.panel_id()) {
            Some(index) => {
                // A sibling Viewer shares this anchor and Panel-keyed records cannot be rebound.
                // Clean the complete anchor before installing any replacement in the active Tab.
                tab.has_replaced_terminals_from(self.cache.tabs().get(active.index)?)
                    .then_some(index)
            }
            None => {
                if let Some(index) = self.overlay_return_index(tab, previous.panel_id()) {
                    return Some(index);
                }
                Some(
                    active
                        .panel_index()
                        .min(tab.panel_count().saturating_sub(1)),
                )
            }
        }
    }

    fn merge_projection(&mut self, candidate: ClientContextTabs) -> bool {
        if !self.invariant_holds() {
            return false;
        }
        let Some(desired_index) = self
            .desired
            .as_ref()
            .and_then(|desired| candidate.tab_index(desired.anchor().tab_id()))
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
            ClientPhase::Ready(ready) => {
                ready.selected.index = desired_index;
                ready.selected.panel_index = selected_panel_index;
            }
            ClientPhase::SelectedWithoutAttachment(selected)
            | ClientPhase::RetryableAttachUnavailable(selected) => {
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
                    .filter(|index| !tab.has_replaced_terminals_from(&self.cache.tabs()[*index]))
                    .map_or(CreateTerminalProvenance::BindingOnly, |index| {
                        self.provenance[index].clone()
                    })
            })
            .collect();
        let overlay_focus = self.focus_panel_id().cloned();
        let zoomed_panels = self.zoomed_panels();
        self.phase = phase;
        self.provenance = provenance;
        self.cache = candidate;
        self.release_zoom_of_removed_panels(&zoomed_panels);
        self.prune_plugin_overlays(overlay_focus.as_ref());
        self.prune_zoomed_tabs();
        self.prune_navigation_memory();
        true
    }

    fn attach_target(
        &mut self,
        selected: SelectedTab,
        provenance: CreateTerminalProvenance,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        let tab = &self.cache.tabs()[selected.index];
        let panel = tab.panel(selected.panel_index())?;
        let member = self.selected_presentation(panel);
        let binding = member.binding().cloned();
        let presentation_id = member.presentation_id().clone();
        let provenance = if member.presentation_id() == panel.presentation_id() {
            provenance
        } else {
            CreateTerminalProvenance::BindingOnly
        }
        .observed_for(member);
        let incarnation = self.next_incarnation;
        self.next_incarnation += 1;
        Some(AttachTarget {
            selected,
            presentation_id: Box::new(presentation_id),
            session_id: self.cache.session_id().clone(),
            binding,
            provenance,
            deadlines,
            incarnation,
            expected_host: None,
        })
    }

    pub(crate) fn target_for_switch(
        &mut self,
        index: usize,
        key: AttachmentKey,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        if key.generation() <= self.desired.as_ref()?.anchor().generation()
            || self.cache.tabs().get(index)?.tab_id() != key.tab_id()
        {
            return None;
        }
        let provenance = self.provenance.get(index)?.clone();
        let tab = self.cache.tabs().get(index)?;
        let panel_index = self
            .navigation_memory
            .panel_index(self.cache.location(), tab);
        let panel = tab.panel(panel_index)?;
        let pane = PaneKey::new(key, panel.panel_id().clone());
        self.attach_target(
            SelectedTab::new_panel(pane, index, panel_index),
            provenance,
            deadlines,
        )
    }

    /// `target_for_switch` carrying the stable panel identity the caller
    /// wants focused after landing — the Agent board's jump variant. The
    /// identity is re-resolved to the at-that-moment `panel_index` here,
    /// so the switch can never fall back to `panel(0)` for a known target.
    /// `None` when the panel is not currently present in that tab.
    pub(crate) fn target_for_switch_for_panel(
        &mut self,
        index: usize,
        key: AttachmentKey,
        deadlines: OperationDeadlines,
        panel_id: &orchbee_workbench_protocol::WorkbenchPanelId,
    ) -> Option<AttachTarget> {
        if key.generation() <= self.desired.as_ref()?.anchor().generation()
            || self.cache.tabs().get(index)?.tab_id() != key.tab_id()
        {
            return None;
        }
        let provenance = self.provenance.get(index)?.clone();
        let tab = self.cache.tabs().get(index)?;
        let panel_index = (0..tab.panel_count()).find(|panel_index| {
            tab.panel(*panel_index)
                .is_some_and(|panel| panel.panel_id() == panel_id)
        })?;
        let panel = tab.panel(panel_index)?;
        let pane = PaneKey::new(key, panel.panel_id().clone());
        self.attach_target(
            SelectedTab::new_panel(pane, index, panel_index),
            provenance,
            deadlines,
        )
    }

    /// A local focus move selects an empty member or a live sibling viewer under
    /// the same anchor without detach/attach. Pending or unavailable bound panes
    /// remain ineligible; empty selection owns no runtime input route.
    pub(crate) fn plan_panel_focus_local(
        &self,
        direction: crate::client_view::PanelDirection,
    ) -> Option<PaneKey> {
        let active = self.active()?;
        let tab = self.cache.tabs().get(active.index())?;
        let index = tab.panel_focus_target(active.panel_index(), direction)?;
        if index == active.panel_index() {
            return None;
        }
        self.plan_panel_focus_local_at(tab.panel(index)?.panel_id())
    }

    pub(crate) fn plan_panel_focus_local_at(&self, panel: &WorkbenchPanelId) -> Option<PaneKey> {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.pending_refresh.is_some()
            || !matches!(
                &self.phase,
                ClientPhase::Ready(_) | ClientPhase::SelectedWithoutAttachment(_)
            )
        {
            return None;
        }
        let active = self.active()?;
        (self.panel_is_unbound(panel) || self.panel_viewer_live(panel))
            .then(|| PaneKey::new(active.key().anchor().clone(), panel.clone()))
    }

    /// Whether the named panel's viewer is live under the current desired
    /// anchor — attached, projected, not departing, and holding a
    /// controller mode. The Agent board's jump requires this for its
    /// `Landed` verdict: a resolved placement with a dead viewer record
    /// must reattach, never land.
    pub(crate) fn panel_viewer_live(&self, panel: &WorkbenchPanelId) -> bool {
        let Some(anchor) = self.desired.as_ref().map(PaneKey::anchor) else {
            return false;
        };
        self.pane_record(&PaneKey::new(anchor.clone(), panel.clone()))
            .is_some_and(|record| {
                record.projected() && !record.departing && record.mode().is_some()
            })
    }

    /// The Agent board's same-tab dead-viewer repair: the selected tab is
    /// already the anchor, but the target panel has no live viewer under
    /// it — a fresh-generation attach target on the same tab re-creates
    /// the viewer through the ordinary switch machinery (`begin_switch`),
    /// never `panel(0)`, never a different tab. `None` when the panel's
    /// viewer is already live, the panel is absent, the anchor moved, or
    /// the phase cannot legally mint an attach — the caller keeps its
    /// `NoOp` verdict in that case.
    pub(crate) fn target_for_dead_panel_reattach(
        &mut self,
        panel_id: &WorkbenchPanelId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        // Only the plain no-op phases may reuse the anchor this way —
        // `RetryableAttachUnavailable` already mints its own same-index
        // switch in `plan_selection`, and every other phase is Busy
        // rather than a reattach candidate.
        if self.pending_refresh.is_some()
            || !matches!(
                &self.phase,
                ClientPhase::Ready(_) | ClientPhase::SelectedWithoutAttachment(_)
            )
        {
            return None;
        }
        let active = self.active()?;
        let index = active.index();
        let tab = self.cache.tabs().get(index)?;
        let panel_index = (0..tab.panel_count()).find(|panel_index| {
            tab.panel(*panel_index)
                .is_some_and(|panel| panel.panel_id() == panel_id)
        })?;
        let panel = tab.panel(panel_index)?;
        // A pane already live under the current anchor needs no repair —
        // refusing here keeps `SelectTab` on the current tab a NoOp.
        let pane = PaneKey::new(active.key().anchor().clone(), panel.panel_id().clone());
        if let Some(record) = self.pane_record(&pane)
            && record.projected()
            && !record.departing
            && record.mode().is_some()
        {
            return None;
        }
        let key = AttachmentKey::new(generation, tab.tab_id().clone());
        self.target_for_switch_for_panel(index, key, deadlines, panel_id)
    }

    /// The same-Tab attach target that re-creates a lost focused viewer with its panel still
    /// focused: the reselection [`Self::plan_selection_index`] retries for the anchor, minted
    /// without a key press. `None` while the focus is live, while close work or a close
    /// confirmation that a new attachment generation would make stale is open, or while the
    /// planner would not retry the selection now.
    pub(crate) fn target_for_lost_focus_reattach(
        &mut self,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        if !self.focused_viewer_lost()
            || !self.close.permits_retired_membership()
            || self.panel_close.confirmation().is_some()
        {
            return None;
        }
        let selected = self.active()?;
        let index = selected.index();
        let panel = selected.key().panel().clone();
        if !matches!(self.plan_selection_index(index), SelectionPlan::Switch { index: planned } if planned == index)
        {
            return None;
        }
        let key = AttachmentKey::new(generation, self.cache.tabs().get(index)?.tab_id().clone());
        self.target_for_switch_for_panel(index, key, deadlines, &panel)
    }

    /// Installs a local focus move approved by [`plan_panel_focus_local`]: the
    /// focus pane, its observed geometry, and its controller mode swap without
    /// touching any attachment. Queued input/resize commands keep their frozen
    /// `PaneRef` tags and remain valid — the membership set does not change.
    pub(crate) fn move_panel_focus_local(&mut self, pane: PaneKey) -> bool {
        self.cancel_duplicate_sequence();
        if self.plan_panel_focus_local_at(pane.panel()).as_ref() != Some(&pane) {
            return false;
        }
        let Some(active) = self.active().cloned() else {
            return false;
        };
        let Some(panel_index) = self
            .cache
            .tabs()
            .get(active.index())
            .and_then(|tab| tab.panel_index(pane.panel()))
        else {
            return false;
        };
        let selected = SelectedTab::new_panel(pane.clone(), active.index(), panel_index);
        if self.panel_is_unbound(pane.panel()) {
            self.phase = ClientPhase::SelectedWithoutAttachment(selected);
        } else {
            let Some(record) = self.pane_record(&pane) else {
                return false;
            };
            let Some(mode) = record.mode().cloned() else {
                return false;
            };
            self.latest_geometry = record.observed_geometry;
            self.phase = ClientPhase::Ready(ReadyAttachment { selected, mode });
        }
        self.desired = Some(pane);
        true
    }

    /// `Ctrl+B z`: a volatile per-Tab Client-local zoom flag, sharing the
    /// guards the local focus transition owns — a Ready phase with no
    /// structural mutation in flight, no armed rename draft, and no pending
    /// refresh. A single-leaf active Tab is an honest no-op. Returns whether
    /// the rendered pane set changed, which the dispatcher translates into
    /// a layout re-observation and pointer-map invalidation.
    pub(crate) fn toggle_zoom(&mut self) -> bool {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.pending_refresh.is_some()
            || !matches!(
                &self.phase,
                ClientPhase::Ready(_) | ClientPhase::SelectedWithoutAttachment(_)
            )
        {
            return false;
        }
        let Some(index) = self.active().map(SelectedTab::index) else {
            return false;
        };
        let Some(tab) = self.cache.tabs().get(index) else {
            return false;
        };
        if tab.panel_count() <= 1 {
            return false;
        }
        let tab_id = tab.tab_id().clone();
        for overlay in &mut self.plugin_overlays {
            if overlay.tab == tab_id {
                overlay.owns_zoom = false;
            }
        }
        if !self.zoomed_tabs.remove(&tab_id) {
            self.zoomed_tabs.insert(tab_id);
        }
        true
    }

    /// Read a ratio from the admitted selected-context projection with exact ordered slots.
    pub(crate) fn split_ratio(
        &self,
        context: &WorkbenchTerminalContextId,
        tab: &WorkbenchTabId,
        split: &orchbee_workbench_protocol::WorkbenchSplitId,
        first: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
        second: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
    ) -> Option<orchbee_workbench_protocol::ClientSplitRatio> {
        if Some(context) != self.cache.terminal_context_id() {
            return None;
        }
        self.cache
            .tab_index(tab)
            .and_then(|index| self.cache.tabs().get(index))
            .and_then(|tab| tab.panel_tree().split_ratio(split, first, second))
    }
    pub(crate) fn split_revision(&self) -> WorkbenchRevision {
        self.cache.revision()
    }

    /// Read-side adjudication: the active Tab draws zoomed exactly while its
    /// flag survives AND its current cache membership stays multi-leaf — a
    /// stale retained flag can never paint a zoom.
    /// Observe the retained flag against the queried Tab's authoritative membership, without GC.
    pub(crate) fn layout_zoomed(&self, tab: &ClientTab) -> bool {
        tab.panel_count() > 1 && self.zoomed_tabs.contains(tab.tab_id())
    }

    pub(crate) fn zoom_active(&self) -> bool {
        let Some(index) = self.active().map(SelectedTab::index) else {
            return false;
        };
        self.cache
            .tabs()
            .get(index)
            .is_some_and(|tab| self.layout_zoomed(tab))
    }

    /// A flag's latest-known membership: the current cache answers for its
    /// own location first — its silence about a Tab there is already the
    /// complete truth, so only a departed context's retained snapshot may
    /// answer next; anything else is unknown (zero leaves). A retained
    /// entry sharing the current location is stale for this purpose and
    /// never consulted.
    fn latest_known_panel_count(&self, tab_id: &WorkbenchTabId) -> usize {
        self.latest_known_tab(tab_id)
            .map_or(0, ClientTab::panel_count)
    }

    fn latest_known_tab(&self, tab_id: &WorkbenchTabId) -> Option<&ClientTab> {
        if let Some(tab) = self.cache.tabs().iter().find(|tab| tab.tab_id() == tab_id) {
            return Some(tab);
        }
        let current = self.cache.location();
        self.retained_contexts
            .iter()
            .filter(|retained| retained.location() != current)
            .find_map(|retained| retained.tabs().iter().find(|tab| tab.tab_id() == tab_id))
    }

    /// The Panel each zoomed Tab draws: the focus in the selected Tab, the remembered Panel in any
    /// other. Read before an install replaces the membership and moves the focus.
    fn zoomed_panels(&self) -> Vec<(WorkbenchTabId, WorkbenchPanelId)> {
        self.zoomed_tabs
            .iter()
            .filter_map(|tab_id| {
                let panel = self
                    .desired
                    .as_ref()
                    .filter(|desired| desired.anchor().tab_id() == tab_id)
                    .map(PaneKey::panel)
                    .or_else(|| self.navigation_memory.panel(tab_id))?;
                Some((tab_id.clone(), panel.clone()))
            })
            .collect()
    }

    /// Zoom belongs to the Panel it draws. When that Panel leaves its Tab, by any route, the Tab
    /// returns to its normal layout; the flag must not carry over to whichever Panel takes the
    /// focus. A plugin overlay restoring the zoom of its return Panel runs after this.
    fn release_zoom_of_removed_panels(&mut self, zoomed: &[(WorkbenchTabId, WorkbenchPanelId)]) {
        for (tab_id, panel_id) in zoomed {
            if self
                .latest_known_tab(tab_id)
                .is_some_and(|tab| tab.panel_index(panel_id).is_none())
            {
                self.zoomed_tabs.remove(tab_id);
            }
        }
    }

    /// Every install collects flags whose last known truth is no longer
    /// multi-leaf — a shrink to one leaf, a deletion, or a recycled retained
    /// entry — never lazily at read time.
    fn prune_zoomed_tabs(&mut self) {
        let flags = std::mem::take(&mut self.zoomed_tabs);
        self.zoomed_tabs = flags
            .into_iter()
            .filter(|tab_id| self.latest_known_panel_count(tab_id) > 1)
            .collect();
    }

    pub(crate) fn plan_selection(&self, action: ClientAction) -> SelectionPlan {
        if self.mutation_blocks_structure() || self.rename_draft_blocks_structure() {
            return SelectionPlan::Busy;
        }
        if matches!(&self.phase, ClientPhase::Retired(_)) {
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
        self.plan_selection_index(index)
    }

    pub(crate) fn plan_selection_index(&self, index: usize) -> SelectionPlan {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || index >= self.cache.tabs().len()
        {
            return SelectionPlan::Busy;
        }
        let Some(anchor) = self.active().map(SelectedTab::index) else {
            return SelectionPlan::Busy;
        };
        if self.pending_refresh.is_some() {
            return if index == anchor {
                SelectionPlan::NoOp
            } else {
                SelectionPlan::Busy
            };
        }
        if !matches!(
            &self.phase,
            ClientPhase::Ready(_)
                | ClientPhase::Switching(_)
                | ClientPhase::SelectedWithoutAttachment(_)
                | ClientPhase::RetryableAttachUnavailable(_)
        ) {
            return SelectionPlan::Busy;
        }
        if index != anchor {
            return SelectionPlan::Switch { index };
        }
        match &self.phase {
            ClientPhase::RetryableAttachUnavailable(_)
                if self
                    .provenance
                    .get(index)
                    .is_some_and(CreateTerminalProvenance::attachable) =>
            {
                SelectionPlan::Switch { index }
            }
            ClientPhase::Ready(_)
            | ClientPhase::Switching(_)
            | ClientPhase::SelectedWithoutAttachment(_)
            | ClientPhase::RetryableAttachUnavailable(_) => SelectionPlan::NoOp,
            _ => SelectionPlan::Busy,
        }
    }

    pub(crate) fn phase_allows_refresh(&self) -> bool {
        self.pending_mutation.is_none()
            && !self.close.set_active()
            && !self.workspace_close_blocks_structure()
            && !self.setup_pending_or_draft()
            && !self.rename_draft_blocks_structure()
            && matches!(
                &self.phase,
                ClientPhase::Empty
                    | ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
                    | ClientPhase::ProjectionUnavailableBeforeCleanup(_)
            )
    }

    #[cfg(test)]
    pub(crate) fn selected_has_viewer(&self) -> bool {
        self.focus_record().is_some_and(|record| record.attached())
    }

    pub(crate) fn finalize_retirement(
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
            ClientPhase::Creating(barrier) if barrier.operation == pending.operation => {
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
        if matches!(&self.phase, ClientPhase::Creating(_)) {
            self.duplicate_commit_fact(&committed);
            self.phase = ClientPhase::CreateCommittedNeedDetach(committed);
            true
        } else {
            self.committed_target() == Some(&committed)
        }
    }

    pub(crate) fn retire(&mut self, failure: CarrierFailure) {
        self.close.halt_set();
        self.stop_duplicate_sequence(DuplicateStop::Recovery);
        self.retire_close_reconciliations();
        self.cancel_close_confirmation();
        self.cancel_workspace_close();
        self.cancel_panel_close_confirmation();
        self.cancel_rename_draft();
        self.cancel_setup_on_retirement();
        if matches!(&self.phase, ClientPhase::Retired(_)) {
            return;
        }
        let diagnostic_active = self.active().cloned();
        let committed = self.committed_target().cloned();
        self.phase = ClientPhase::Retired(RetiredState {
            failure,
            diagnostic_active,
            committed,
        });
        self.panes.clear();
        self.reconcile_deadlines = None;
        self.pending_refresh = None;
        self.publication_debt = PublicationDebt::Clean;
        self.context_selection = None;
        self.pending_membership = None;
        self.refresh_status = None;
    }

    pub(crate) fn view(&self) -> WorkbenchClientView<'_> {
        WorkbenchClientView { state: self }
    }

    pub(crate) fn invariant_holds(&self) -> bool {
        if matches!(self.publication_debt, PublicationDebt::InFlight { .. })
            && self.pending_refresh.is_none()
        {
            return false;
        }
        if !self.membership_invariant_holds() || !self.context_invariant_holds() {
            return false;
        }
        // Every zoom flag rides a latest-known multi-leaf membership — the
        // current cache first, then the departed context's retained
        // snapshot; installs collect flags whose truth is no longer
        // multi-leaf, so a stale flag can never sit in the set.
        if self
            .zoomed_tabs
            .iter()
            .any(|tab_id| self.latest_known_panel_count(tab_id) <= 1)
        {
            return false;
        }
        if !self.move_invariant_holds()
            || !self.panel_invariant_holds()
            || !self.rename_invariant_holds()
            || !self.setup_invariant_holds()
            || !self.workspace_close_invariant_holds()
            || !self.takeover_invariant_holds()
        {
            return false;
        }
        if self.provenance.len() != self.cache.tabs().len() {
            return false;
        }
        if let Some(active) = self.active()
            && self.cache.tabs().get(active.index).is_none_or(|tab| {
                tab.tab_id() != active.key.anchor().tab_id()
                    || active.panel_index() >= tab.panel_count()
            })
        {
            return false;
        }
        if matches!(
            &self.phase,
            ClientPhase::SelectedWithoutAttachment(_)
                | ClientPhase::RetryableAttachUnavailable(_)
                | ClientPhase::Retired(_)
        ) && self
            .panes
            .iter()
            .any(|record| record.projected() && Some(&record.key) == self.desired.as_ref())
        {
            return false;
        }
        // Per-record shape: at most one record per pane, and every non-departing
        // record belongs to the desired anchor. `AttachedPane` carries the
        // attachment-scoped facts, so projection, frame, mode, and accepted
        // geometry state cannot outlive the attachment.
        let unique = !self.panes.iter().enumerate().any(|(index, record)| {
            self.panes[index + 1..]
                .iter()
                .any(|other| other.key == record.key)
        });
        let records_valid = unique
            && self.panes.iter().all(|record| {
                record.departing
                    || self
                        .desired
                        .as_ref()
                        .is_some_and(|desired| record.key.anchor() == desired.anchor())
            });
        if !records_valid {
            return false;
        }
        let attachment_shape_valid = match &self.phase {
            ClientPhase::ReconcilingMembership(selected) => {
                Some(&selected.key) == self.desired.as_ref()
                    && (self.focus_record().is_some_and(|record| record.attached())
                        || ((self.selected_is_unbound() || self.focus_has_lost_viewer())
                            && self.panes.iter().any(|record| {
                                record.key.anchor() == selected.key.anchor() && record.attached()
                            })))
            }
            ClientPhase::Empty => {
                self.cache.tabs().is_empty()
                    && self.provenance.is_empty()
                    && self.desired.is_none()
                    && self.panes.is_empty()
            }
            ClientPhase::Starting(setup) => {
                Some(&setup.selected.key) == self.desired.as_ref() && self.focus_record().is_some()
            }
            ClientPhase::Ready(ready) => {
                Some(&ready.selected.key) == self.desired.as_ref()
                    && self.focus_record().is_some_and(|record| {
                        record.projected() && record.mode() == Some(&ready.mode)
                    })
            }
            ClientPhase::Creating(barrier) => {
                barrier.anchor.selected().map(|selected| &selected.key) == self.desired.as_ref()
                    && match barrier.anchor.ready() {
                        Some(_) => self.focus_record().is_some_and(|record| record.projected()),
                        None => self.focus_record().is_none_or(|record| !record.attached()),
                    }
            }
            ClientPhase::CreateCommittedNeedDetach(committed)
            | ClientPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                committed.anchor.selected().map(|selected| &selected.key) == self.desired.as_ref()
                    && match committed.anchor.ready() {
                        Some(_) => self.focus_record().is_some_and(|record| record.projected()),
                        None => self.focus_record().is_none_or(|record| !record.attached()),
                    }
            }
            ClientPhase::Switching(setup) => {
                Some(&setup.target.selected.key) == self.desired.as_ref()
                    && match &setup.previous {
                        Some(previous) => self
                            .pane_record(&previous.selected.key)
                            .is_some_and(|record| record.attached()),
                        None => setup.target.binding.is_none() || self.focus_record().is_some(),
                    }
            }
            ClientPhase::SelectedWithoutAttachment(selected)
            | ClientPhase::RetryableAttachUnavailable(selected) => {
                Some(&selected.key) == self.desired.as_ref()
                    && self.focus_record().is_none_or(|record| !record.attached())
            }
            ClientPhase::Retired(retired) => {
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
                            .selected()
                            .is_some_and(|selected| *selected == *active)
                            && Some(&active.key) == self.desired.as_ref()
                    }
                    (None, Some(committed)) => {
                        self.desired.as_ref().is_some_and(|desired| {
                            desired.anchor() == &committed.committed_key
                                && desired.panel() == committed.expectation.panel_id()
                        }) || (committed.anchor.kind() == CreateOriginKind::Empty
                            && self.desired.is_none()
                            && self.cache.tabs().is_empty())
                    }
                    (Some(_), None) => true,
                    (None, None) => self.cache.tabs().is_empty() && self.desired.is_none(),
                };
                self.panes.is_empty()
                    && self.pending_refresh.is_none()
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

pub(crate) struct WorkbenchClientView<'a> {
    state: &'a WorkbenchClientState,
}

impl<'a> WorkbenchClientView<'a> {
    /// The canonical revision this view projects; painted witnesses fence against it.
    pub(crate) fn revision(&self) -> WorkbenchRevision {
        self.state.cache.revision()
    }

    /// The current navigation location.
    pub(crate) fn location(&self) -> &'a crate::client_context::ClientContextLocation {
        self.state.cache.location()
    }

    /// Every navigation entry's location, in navigation order.
    pub(crate) fn navigation_locations(
        &self,
    ) -> impl Iterator<Item = &'a crate::client_context::ClientContextLocation> + 'a {
        self.state
            .cache
            .navigation()
            .iter()
            .map(|entry| &entry.location)
    }

    /// Every Tab's identity, in strip order.
    pub(crate) fn tab_ids(&self) -> impl Iterator<Item = &'a WorkbenchTabId> + 'a {
        self.state.cache.tabs().iter().map(|tab| tab.tab_id())
    }

    /// The active Tab's identity.
    pub(crate) fn active_tab_id(&self) -> Option<&'a WorkbenchTabId> {
        let active = self.state.active()?;
        self.state
            .cache
            .tabs()
            .get(active.index())
            .map(|tab| tab.tab_id())
    }

    /// The focus pane's panel identity.
    pub(crate) fn focus_panel_id(&self) -> Option<&'a WorkbenchPanelId> {
        self.state.focus_panel_id()
    }

    pub(crate) fn session_id(&self) -> &SessionId {
        self.state.cache.session_id()
    }

    pub(crate) fn terminal_context_id(&self) -> Option<&WorkbenchTerminalContextId> {
        self.state.cache.terminal_context_id()
    }

    pub(crate) fn unbound_panel_key(
        &self,
        panel: &crate::workbench_projection::ClientPanel,
    ) -> Option<PaneKey> {
        let active = self.state.active()?;
        let tab = self.state.cache.tabs().get(active.index())?;
        if tab.panel_index(panel.panel_id()).is_none()
            || self.selected_presentation(panel).binding().is_some()
        {
            return None;
        }
        Some(PaneKey::new(
            active.key().anchor().clone(),
            panel.panel_id().clone(),
        ))
    }
    pub(crate) fn panel_is_unbound(&self, panel: &WorkbenchPanelId) -> bool {
        self.state.panel_is_unbound(panel)
    }

    pub(crate) fn selected_presentation(
        &self,
        panel: &'a crate::workbench_projection::ClientPanel,
    ) -> &'a crate::workbench_projection::ClientPresentation {
        self.state.selected_presentation(panel)
    }
    pub(crate) const fn phase(&self) -> ClientPhaseKind {
        self.state.phase_kind()
    }

    pub(crate) fn tab_count(&self) -> usize {
        self.state.cache.tabs().len()
    }

    pub(crate) fn tab(&self, index: usize) -> Option<ClientTabView<'a>> {
        let tab = self.state.cache.tabs().get(index)?;
        let active_panel = self
            .state
            .active()
            .filter(|active| active.index() == index)
            .map(SelectedTab::panel_index);
        Some(ClientTabView {
            tab,
            position: index + 1,
            active: active_panel.is_some(),
            active_panel,
            naming_panel: active_panel.unwrap_or_else(|| {
                self.state
                    .navigation_memory
                    .panel_index(self.state.cache.location(), tab)
            }),
        })
    }

    pub(crate) fn active_position(&self) -> Option<usize> {
        self.state
            .active()
            .map(|selected| selected.index().saturating_add(1))
    }

    /// The active Tab renders zoomed — the volatile per-Tab flag read
    /// through current-cache adjudication only.
    pub(crate) fn zoom_active(&self) -> bool {
        self.state.zoom_active()
    }

    pub(crate) fn controller_mode(&self) -> Option<&ControllerMode> {
        self.state.controller_mode()
    }

    pub(crate) fn selected_launch_failure(
        &self,
    ) -> Option<&'a orchbee_workbench_protocol::PresentationLaunchFailure> {
        let active = self.state.active()?;
        let panel = self
            .state
            .cache
            .tabs()
            .get(active.index())?
            .panel(active.panel_index())?;
        self.state.selected_presentation(panel).launch_failure()
    }

    pub(crate) fn selected_provenance(&self) -> Option<&CreateTerminalProvenance> {
        self.state
            .focus_record()
            .map(|record| &record.provenance)
            .or_else(|| {
                self.state
                    .active()
                    .and_then(|selected| self.state.provenance(selected.index()))
            })
    }

    pub(crate) fn launch_description(
        &self,
        panel: &crate::workbench_projection::ClientPanel,
    ) -> Option<&'a LaunchDescription> {
        self.state
            .launch_descriptions
            .get(self.state.selected_presentation(panel).presentation_id())
    }

    pub(crate) const fn notice(&self) -> Option<ClientNotice> {
        self.state.notice
    }

    pub(crate) const fn refresh_status(&self) -> Option<NavigatorRefreshStatus> {
        self.state.refresh_status
    }

    /// The live record for one panel of the focus epoch — the pane a leaf's
    /// projection and geometry belong to.
    pub(crate) fn pane_by_panel(&self, panel: &WorkbenchPanelId) -> Option<&PaneRecord> {
        self.state.pane_by_panel(panel)
    }

    #[cfg(test)]
    pub(crate) const fn retired_failure(&self) -> Option<CarrierFailure> {
        self.state.retired_failure()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ClientTabView<'a> {
    tab: &'a ClientTab,
    position: usize,
    active: bool,
    active_panel: Option<usize>,
    naming_panel: usize,
}

impl<'a> ClientTabView<'a> {
    pub(crate) fn group(&self) -> Option<&'a orchbee_workbench_protocol::TabGroup> {
        self.tab.group()
    }

    pub(crate) fn pinned(&self) -> bool {
        self.tab.pinned()
    }

    pub(crate) fn group_label(&self) -> Option<&'a str> {
        self.tab.group_label()
    }

    pub(crate) fn split_ratio(
        &self,
        split: &orchbee_workbench_protocol::WorkbenchSplitId,
        first: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
        second: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
    ) -> orchbee_workbench_protocol::ClientSplitRatio {
        self.tab
            .panel_tree()
            .split_ratio(split, first, second)
            .unwrap_or_default()
    }

    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    pub(crate) const fn active(&self) -> bool {
        self.active
    }

    pub(crate) fn tab_label(&self) -> std::borrow::Cow<'a, str> {
        self.tab.display_label(self.position - 1)
    }

    pub(crate) const fn automatic_label(&self) -> bool {
        self.tab.automatic_label()
    }

    pub(crate) fn panel_count(&self) -> usize {
        self.tab.panel_count()
    }

    pub(crate) fn panel_tree(&self) -> &'a crate::workbench_projection::ClientPanelTree {
        self.tab.panel_tree()
    }

    pub(crate) fn active_panel(&self) -> Option<usize> {
        self.active_panel
    }

    /// The Panel whose terminal names this Tab: the focused one while it is selected, else the
    /// one selecting it would focus. Selecting another Tab therefore never renames it.
    pub(crate) fn naming_panel(&self) -> Option<&'a crate::workbench_projection::ClientPanel> {
        self.tab.panel(self.naming_panel)
    }

    pub(crate) fn panel(
        &self,
        index: usize,
    ) -> Option<&'a crate::workbench_projection::ClientPanel> {
        self.tab.panel(index)
    }

    pub(crate) fn tab_id(&self) -> &'a WorkbenchTabId {
        self.tab.tab_id()
    }

    #[cfg(test)]
    pub(crate) fn presentation_label(&self) -> &'a str {
        self.tab.presentation_label().as_str()
    }
}

/// Crate-internal `AttachmentKey` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait AttachmentKeyTestAccess {
    fn new(generation: u64, tab_id: WorkbenchTabId) -> Self;
}

#[cfg(any(test, feature = "test-support"))]
impl AttachmentKeyTestAccess for AttachmentKey {
    fn new(generation: u64, tab_id: WorkbenchTabId) -> Self {
        AttachmentKey::new(generation, tab_id)
    }
}

/// Crate-internal `PaneKey` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait PaneKeyTestAccess {
    fn new(anchor: AttachmentKey, panel: WorkbenchPanelId) -> Self;
    fn panel(&self) -> &WorkbenchPanelId;
}

#[cfg(any(test, feature = "test-support"))]
impl PaneKeyTestAccess for PaneKey {
    fn new(anchor: AttachmentKey, panel: WorkbenchPanelId) -> Self {
        PaneKey::new(anchor, panel)
    }
    fn panel(&self) -> &WorkbenchPanelId {
        PaneKey::panel(self)
    }
}

/// Crate-internal `PaneRef` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait PaneRefTestAccess {
    fn new(pane: PaneKey, incarnation: u32) -> Self;
    fn pane(&self) -> &PaneKey;
}

#[cfg(any(test, feature = "test-support"))]
impl PaneRefTestAccess for PaneRef {
    fn new(pane: PaneKey, incarnation: u32) -> Self {
        PaneRef::new(pane, incarnation)
    }
    fn pane(&self) -> &PaneKey {
        PaneRef::pane(self)
    }
}

#[cfg(test)]
mod tests;
