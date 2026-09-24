use super::workbench_projection::{
    CreatedTabExpectation, NativeContextTabs, NativeSimpleTab, NativeTerminalBinding,
    validate_canonical_refresh, validate_created_tab,
};
use crate::client_view::ClientAction;
use crate::local_client::workbench::WorkbenchTabCreateCommit;
use orchbee_client_server_protocol::{ControllerId, Geometry};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchLabel, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchTerminalFailureReason, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
};
use std::time::{Duration, Instant};

#[cfg(test)]
use super::workbench_projection::project_native_standalone_tabs;
mod context_removal;
mod context_selection;
mod field_window;
pub(super) mod line_editor;
pub(super) mod workspace_setup;
pub(crate) use context_selection::NativeContextView;
pub(crate) use field_window::NativeFieldWindow;
mod create_membership;
mod membership;
mod move_tab;
mod panel;
mod panel_close;
pub(super) mod rename_draft;
#[cfg(any(target_os = "linux", windows))]
pub(super) mod retired_membership;
mod split_ratios;
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
#[cfg(test)]
pub(super) use panel::PanelPrecommit;
pub(super) use panel::{
    PanelCommit, PanelDecision, PanelIntent, PanelOperation, PanelRequestKind, PanelResult,
    PendingPanelCommit,
};
pub(crate) use panel_close::PanelCloseConfirmationToken;
pub(super) use panel_close::{PanelCloseConfirmationView, PanelCloseState};
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
pub(crate) struct AttachmentKey {
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
pub(crate) struct PaneKey {
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
    pub(super) const fn generation(&self) -> u64 {
        self.anchor.generation()
    }

    #[cfg(test)]
    pub(super) const fn tab_id(&self) -> &WorkbenchTabId {
        self.anchor.tab_id()
    }
}

/// Test-only anchor comparisons: legacy single-pane tests assert against
/// `AttachmentKey` values, which name exactly the pane's anchor.
#[cfg(test)]
impl PartialEq<AttachmentKey> for PaneKey {
    fn eq(&self, other: &AttachmentKey) -> bool {
        self.anchor() == other
    }
}

#[cfg(test)]
impl PartialEq<PaneKey> for AttachmentKey {
    fn eq(&self, other: &PaneKey) -> bool {
        self == other.anchor()
    }
}

#[cfg(test)]
impl PartialEq<AttachmentKey> for PaneRef {
    fn eq(&self, other: &AttachmentKey) -> bool {
        self.anchor() == other
    }
}

// Weak legacy comparisons for single-pane test assertions: anchor spelling
// ignores the panel axis and the `PaneRef` comparisons ignore incarnation, so
// they can never distinguish a stale incarnation or a sibling pane. Identity
// proofs must assert on exact `PaneRef` values directly.
#[cfg(test)]
impl PartialEq<PaneRef> for AttachmentKey {
    fn eq(&self, other: &PaneRef) -> bool {
        self == other.anchor()
    }
}

#[cfg(test)]
impl PartialEq<PaneKey> for PaneRef {
    fn eq(&self, other: &PaneKey) -> bool {
        self.pane() == other
    }
}

#[cfg(test)]
impl PartialEq<PaneRef> for PaneKey {
    fn eq(&self, other: &PaneRef) -> bool {
        self == other.pane()
    }
}

/// Resolves any spelling of a pane identity — attachment anchor, logical pane
/// key, or exact incarnation — into the `PaneRef` the live membership minted.
/// Legacy single-pane tests keep their `AttachmentKey` fixtures and route
/// through the `_t` compatibility methods below.
///
/// WEAK CLAIM BOUNDARY: anchor and `PaneKey` spellings resolve the CURRENT
/// record at call time — an anchor ignores the panel axis entirely and a stale
/// incarnation can never be expressed this way. These adapters exist so legacy
/// single-pane assertions keep compiling; they can never prove stale-identity
/// fencing. Tests that exercise detach/rebind/stale/poll/draw/ack identity
/// must construct or capture the exact `PaneRef` before the rebind and call
/// the production methods directly, never this layer.
#[cfg(test)]
pub(super) trait TestPaneRef {
    fn resolve_pane_ref(&self, state: &NativeSessionState) -> Option<PaneRef>;
}

#[cfg(test)]
impl TestPaneRef for PaneRef {
    fn resolve_pane_ref(&self, _state: &NativeSessionState) -> Option<PaneRef> {
        Some(self.clone())
    }
}

#[cfg(test)]
impl TestPaneRef for PaneKey {
    fn resolve_pane_ref(&self, state: &NativeSessionState) -> Option<PaneRef> {
        state.pane_ref_for(self)
    }
}

#[cfg(test)]
impl TestPaneRef for AttachmentKey {
    fn resolve_pane_ref(&self, state: &NativeSessionState) -> Option<PaneRef> {
        state.pane_ref_for_anchor(self)
    }
}

/// Resolves any spelling of pane identity into the logical `PaneKey` — switch
/// markers and membership fences are pane-keyed while legacy tests name the
/// anchor.
#[cfg(test)]
pub(super) trait TestPaneKey {
    fn resolve_pane_key(&self, state: &NativeSessionState) -> Option<PaneKey>;
}

#[cfg(test)]
impl TestPaneKey for PaneKey {
    fn resolve_pane_key(&self, _state: &NativeSessionState) -> Option<PaneKey> {
        Some(self.clone())
    }
}

#[cfg(test)]
impl TestPaneKey for PaneRef {
    fn resolve_pane_key(&self, _state: &NativeSessionState) -> Option<PaneKey> {
        Some(self.pane().clone())
    }
}

#[cfg(test)]
impl TestPaneKey for AttachmentKey {
    fn resolve_pane_key(&self, state: &NativeSessionState) -> Option<PaneKey> {
        state.pane_key_for_anchor(self)
    }
}

/// Physical attachment identity for one pane: the logical pane plus the
/// incarnation minted when its attach target was issued. Every queued command,
/// poll key, and attachment-scoped worker event carries a `PaneRef` so a stale
/// incarnation (pane departed and re-attached) is rejected by exact equality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneRef {
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

/// Why a pane's viewer is unavailable while the pane itself stays selected.
/// Pane-local degradation: the rest of the membership is unaffected, and only
/// an explicit new reconcile (re-observe or selection change) may mint a fresh
/// incarnation for the pane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PaneFault {
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
pub(super) struct ReconcileTarget {
    pane_ref: PaneRef,
    index: usize,
    site_id: SiteId,
    binding: NativeTerminalBinding,
    provenance: CreateTerminalProvenance,
    geometry: Geometry,
}

impl ReconcileTarget {
    pub(super) fn new(record: &PaneRecord) -> Self {
        Self {
            pane_ref: record.pane_ref(),
            index: record.index,
            site_id: record.site_id.clone(),
            binding: record.binding.clone(),
            provenance: record.provenance.clone(),
            geometry: record.observed_geometry,
        }
    }

    pub(super) const fn pane_ref(&self) -> &PaneRef {
        &self.pane_ref
    }

    pub(super) const fn index(&self) -> usize {
        self.index
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

    pub(super) const fn geometry(&self) -> Geometry {
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
pub(super) enum ReconcileStep {
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
/// silently freed and never double-detached.
#[derive(Clone, Debug)]
pub(super) struct PaneRecord {
    key: PaneKey,
    incarnation: u32,
    index: usize,
    panel_index: usize,
    site_id: SiteId,
    binding: NativeTerminalBinding,
    provenance: CreateTerminalProvenance,
    observed_geometry: Geometry,
    attached: bool,
    projected: bool,
    departing: bool,
    mode: Option<ControllerMode>,
    staged_frame: Option<FrameIdentity>,
    accepted_frame: Option<FrameIdentity>,
    last_admitted_geometry: Option<Geometry>,
    last_accepted_geometry: Option<Geometry>,
    fault: Option<PaneFault>,
}

impl PaneRecord {
    fn new(
        pane_ref: PaneRef,
        index: usize,
        panel_index: usize,
        site_id: SiteId,
        binding: NativeTerminalBinding,
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
            site_id,
            binding,
            provenance,
            observed_geometry,
            attached: false,
            projected: false,
            departing: false,
            mode: None,
            staged_frame: None,
            accepted_frame: None,
            last_admitted_geometry: None,
            last_accepted_geometry: None,
            fault: None,
        }
    }

    pub(super) const fn key(&self) -> &PaneKey {
        &self.key
    }

    pub(super) const fn incarnation(&self) -> u32 {
        self.incarnation
    }

    pub(super) fn pane_ref(&self) -> PaneRef {
        PaneRef::new(self.key.clone(), self.incarnation)
    }

    pub(super) const fn attachable(&self) -> bool {
        self.provenance.attachable()
    }

    /// Whether reconcile may still try to attach this pane: definite attach
    /// failures, a known lost viewer, and dead-episode parks do not become
    /// pending work again on an ordinary tick. A live read-only viewer stays
    /// in the target set; losing control is not losing the attachment.
    pub(super) const fn retryable(&self) -> bool {
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
    /// physical `projected`/`attached` facts survive so a leaf that reappears
    /// before its disposal proof resumes drawing without a re-install path.
    fn mark_departing(&mut self) {
        self.departing = true;
        self.staged_frame = None;
        self.accepted_frame = None;
    }

    #[cfg(test)]
    pub(super) const fn attached(&self) -> bool {
        self.attached
    }

    pub(super) const fn fault(&self) -> Option<PaneFault> {
        self.fault
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
    PanelProjectionUnavailable,
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
    PanelNotApplied,
    ConfirmationCancelled,
    DraftCancelled,
    /// A modal surface or armed confirmation owns the input layer — the
    /// Agent board refused to open rather than fight for capture.
    ModalOwnsInput,
    /// The viewport lost its facts/anchor and is returning to live —
    /// shown while the recovery demand pair is in flight.
    HistoryRecovering,
    /// Retained history could not be entered (demand refused or reset);
    /// the pane stayed Live.
    HistoryUnavailable,
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
    key: PaneKey,
    index: usize,
    panel_index: usize,
}

impl SelectedTab {
    pub(super) const fn new(key: PaneKey, index: usize) -> Self {
        Self::new_panel(key, index, 0)
    }

    pub(super) const fn new_panel(key: PaneKey, index: usize, panel_index: usize) -> Self {
        Self {
            key,
            index,
            panel_index,
        }
    }

    pub(super) const fn key(&self) -> &PaneKey {
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
                    .map_or(0, |ready| ready.selected.key.anchor().generation())
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
    incarnation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BootstrapTarget {
    selected: SelectedTab,
    site_id: SiteId,
    binding: NativeTerminalBinding,
    incarnation: u32,
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

    pub(super) fn pane_ref(&self) -> PaneRef {
        PaneRef::new(self.selected.key.clone(), self.incarnation)
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

    pub(super) fn pane_ref(&self) -> PaneRef {
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
    Superseded { desired: PaneKey },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CleanupContinuation {
    Stable,
    Superseded { desired: PaneKey },
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
pub(super) enum PublicationDebt {
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

pub(super) struct NativeSessionState {
    cache: NativeContextTabs,
    context_selection: Option<context_selection::ContextSelection>,
    retained_contexts: Vec<context_selection::RetainedContext>,
    provenance: Vec<CreateTerminalProvenance>,
    phase: SessionPhase,
    desired: Option<PaneKey>,
    panes: Vec<PaneRecord>,
    next_incarnation: u32,
    reconcile_deadlines: Option<OperationDeadlines>,
    latest_geometry: Geometry,
    notice: Option<SessionNotice>,
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
    pending_membership: Option<membership::PendingMembership>,
    settled_create: Option<CommittedTabCreate>,
    /// Volatile per-Tab Client-local zoom flags (`Ctrl+B z`): a flag lives
    /// while its Tab is known alive with more than one leaf — the current
    /// cache answers first (its silence about its own location is already
    /// that location's complete truth), then only the departed contexts'
    /// retained snapshots — and is collected at every membership install.
    /// Never wired, shared between Clients, or persisted; bounded by the
    /// same navigation retention the context snapshots already obey.
    zoomed_tabs: std::collections::BTreeSet<WorkbenchTabId>,
    split_ratios: split_ratios::SplitRatios,
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
        let anchor = AttachmentKey::new(generation, first.tab_id().clone());
        let panel = first.panel(0)?;
        let key = PaneKey::new(anchor, panel.panel_id().clone());
        let selected = SelectedTab::new(key.clone(), 0);
        let record = PaneRecord::new(
            PaneRef::new(key, 1),
            0,
            0,
            cache.site_id().clone(),
            panel.binding().clone(),
            CreateTerminalProvenance::BindingOnly,
            geometry,
        );
        let target = BootstrapTarget {
            selected: selected.clone(),
            site_id: cache.site_id().clone(),
            binding: panel.binding().clone(),
            incarnation: 1,
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
            desired: Some(record.key().clone()),
            panes: vec![record],
            next_incarnation: 2,
            reconcile_deadlines: None,
            latest_geometry: geometry,
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
            pending_membership: None,
            settled_create: None,
            zoomed_tabs: std::collections::BTreeSet::new(),
            split_ratios: split_ratios::SplitRatios::default(),
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
        if self.panel_recovery.is_some() && !matches!(&self.phase, SessionPhase::Retired(_)) {
            return SessionPhaseKind::PanelProjectionUnavailable;
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

    pub(super) const fn desired(&self) -> Option<&PaneKey> {
        self.desired.as_ref()
    }

    pub(super) fn pane_record(&self, pane: &PaneKey) -> Option<&PaneRecord> {
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
    pub(super) fn has_attachments(&self) -> bool {
        self.panes.iter().any(|record| record.attached)
    }

    /// The attachment key the single-attachment model exposed: the focus
    /// pane's key while it carries an attachment, and — when the focus pane is
    /// unattached (e.g. mid-switch before candidate setup) — the sole
    /// surviving attached record's key. `None` when nothing is attached or
    /// several background attachments make the answer ambiguous.
    #[cfg(test)]
    pub(super) fn attached_key(&self) -> Option<&PaneKey> {
        if let Some(record) = self.focus_record().filter(|record| record.attached) {
            return Some(record.key());
        }
        let mut attached = self.panes.iter().filter(|record| record.attached);
        match (attached.next(), attached.next()) {
            (Some(record), None) => Some(record.key()),
            _ => None,
        }
    }

    /// The focus pane's key while its projection is installed — the successor
    /// of the single `projection_key`.
    #[cfg(test)]
    pub(super) fn projection_key(&self) -> Option<&PaneKey> {
        self.focus_record()
            .filter(|record| record.projected)
            .map(PaneRecord::key)
    }

    /// The exact live incarnation of one logical pane — test code resolves a
    /// `PaneKey` into the `PaneRef` the current membership minted.
    #[cfg(test)]
    pub(super) fn pane_ref_for(&self, pane: &PaneKey) -> Option<PaneRef> {
        self.pane_record(pane).map(PaneRecord::pane_ref)
    }

    /// Legacy tests name panes by attachment anchor; resolve the live
    /// incarnation of the FIRST record carrying that anchor — the panel axis
    /// is ignored, so this is a weak claim that cannot distinguish sibling
    /// panes or stale incarnations. Departing records still resolve — settle
    /// paths (cleanup, superseded events) operate on them.
    #[cfg(test)]
    pub(super) fn pane_ref_for_anchor(&self, anchor: &AttachmentKey) -> Option<PaneRef> {
        self.panes
            .iter()
            .find(|record| record.key.anchor() == anchor)
            .map(PaneRecord::pane_ref)
    }

    /// The logical pane key an anchor resolves to: the live record's key, or
    /// the key installation would mint on the tab's first leaf panel — used to
    /// phrase expectations in `PaneKey` terms before the pane exists.
    #[cfg(test)]
    pub(super) fn pane_key_for_anchor(&self, anchor: &AttachmentKey) -> Option<PaneKey> {
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

    /// Compatibility variants of the pane-scoped state API for legacy
    /// single-pane tests: the key argument may be an `AttachmentKey`,
    /// `PaneKey`, or `PaneRef`; anchors and logical keys resolve to the live
    /// incarnation. An unresolvable key reports the same rejection the exact
    /// call would produce for a dead incarnation.
    #[cfg(test)]
    pub(super) fn install_projection_t<K: TestPaneRef + ?Sized>(&mut self, key: &K) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.install_projection(&resolved))
    }

    #[cfg(test)]
    pub(super) fn record_setup_configuration_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> bool {
        key.resolve_pane_ref(self).is_some_and(|resolved| {
            self.record_setup_configuration(&resolved, mode, applied_geometry)
        })
    }

    #[cfg(test)]
    pub(super) fn stage_drawn_frame_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.stage_drawn_frame(&resolved, frame))
    }

    #[cfg(test)]
    pub(super) fn mark_feedback_accepted_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.mark_feedback_accepted(&resolved, frame))
    }

    #[cfg(test)]
    pub(super) fn publish_ready_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.publish_ready(&resolved, frame, mode, accepted_geometry))
    }

    #[cfg(test)]
    pub(super) fn settle_ready_candidate_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> Result<ReadyDecision, CarrierFailure> {
        match key.resolve_pane_ref(self) {
            Some(resolved) => {
                self.settle_ready_candidate(&resolved, frame, mode, accepted_geometry)
            }
            None => Ok(ReadyDecision::Superseded),
        }
    }

    #[cfg(test)]
    pub(super) fn settle_attached_event_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
    ) -> Option<SetupEventDisposition> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.settle_attached_event(&resolved))
    }

    #[cfg(test)]
    pub(super) fn settle_feedback_event_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
    ) -> Option<SetupEventDisposition> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.settle_feedback_event(&resolved, frame))
    }

    #[cfg(test)]
    pub(super) fn settle_setup_configuration_event_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> Option<SetupEventDisposition> {
        key.resolve_pane_ref(self).and_then(|resolved| {
            self.settle_setup_configuration_event(&resolved, mode, applied_geometry)
        })
    }

    #[cfg(test)]
    pub(super) fn settle_switch_entry_expired_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
        settled_at: Instant,
    ) -> Option<CleanupContinuation> {
        key.resolve_pane_ref(self).and_then(|resolved| {
            self.settle_switch_entry_expired(&resolved, target_index, target_provenance, settled_at)
        })
    }

    #[cfg(test)]
    pub(super) fn settle_cleanup_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.settle_cleanup(&resolved, target_index, target_provenance))
    }

    #[cfg(test)]
    pub(super) fn settle_unattached_setup_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        key.resolve_pane_ref(self).and_then(|resolved| {
            self.settle_unattached_setup(&resolved, target_index, target_provenance)
        })
    }

    #[cfg(test)]
    pub(super) fn record_admitted_geometry_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        geometry: Geometry,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.record_admitted_geometry(&resolved, geometry))
    }

    #[cfg(test)]
    pub(super) fn expected_frame_geometry_t<K: TestPaneRef + ?Sized>(
        &self,
        key: &K,
    ) -> Option<Geometry> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.expected_frame_geometry(&resolved))
    }

    #[cfg(test)]
    pub(super) fn last_admitted_geometry_t<K: TestPaneRef + ?Sized>(
        &self,
        key: &K,
    ) -> Option<Geometry> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.last_admitted_geometry(&resolved))
    }

    #[cfg(test)]
    pub(super) fn last_accepted_geometry_t<K: TestPaneRef + ?Sized>(
        &self,
        key: &K,
    ) -> Option<Geometry> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.last_accepted_geometry(&resolved))
    }

    #[cfg(test)]
    pub(super) fn invalidate_projection_frame_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.invalidate_projection_frame(&resolved))
    }

    /// The focus pane's record when it is attached and live — the pane the
    /// session's input, ready checks, and status surface describe.
    pub(super) fn focus_record(&self) -> Option<&PaneRecord> {
        let desired = self.desired.as_ref()?;
        self.pane_record(desired).filter(|record| !record.departing)
    }

    /// Mutable access to the focus pane's record — legacy tests mutate
    /// attachment frame fields directly to build adversarial states.
    #[cfg(test)]
    pub(super) fn focus_record_mut(&mut self) -> Option<&mut PaneRecord> {
        let desired = self.desired.clone()?;
        self.panes
            .iter_mut()
            .find(|record| record.key == desired && !record.departing)
    }

    /// The focus pane's accepted frame as a `(anchor, frame)` pair — the shape
    /// the single-attachment `accepted_frame` field exposed.
    #[cfg(test)]
    pub(super) fn focus_accepted_frame(&self) -> Option<(AttachmentKey, FrameIdentity)> {
        self.focus_record()
            .and_then(|record| Some((record.key.anchor().clone(), record.accepted_frame?)))
    }

    #[cfg(test)]
    pub(super) fn focus_last_accepted_geometry(&self) -> Option<(AttachmentKey, Geometry)> {
        self.focus_record()
            .and_then(|record| Some((record.key.anchor().clone(), record.last_accepted_geometry?)))
    }

    /// The live (non-departing) record for one panel of the focus epoch — the
    /// render path asks for the pane's attachment facts by stable panel id.
    pub(super) fn pane_by_panel(&self, panel: &WorkbenchPanelId) -> Option<&PaneRecord> {
        self.panes
            .iter()
            .find(|record| record.key.panel() == panel && !record.departing)
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

    #[cfg(test)]
    pub(super) fn last_admitted_geometry(&self, key: &PaneRef) -> Option<Geometry> {
        self.live_pane_record(key)
            .and_then(|record| record.last_admitted_geometry)
    }

    #[cfg(test)]
    pub(super) fn last_accepted_geometry(&self, key: &PaneRef) -> Option<Geometry> {
        self.live_pane_record(key)
            .and_then(|record| record.last_accepted_geometry)
    }

    #[cfg(test)]
    pub(super) const fn notice(&self) -> Option<SessionNotice> {
        self.notice
    }

    pub(super) const fn pending_refresh_generation(&self) -> Option<u64> {
        match self.pending_refresh {
            Some(flight) => Some(flight.generation),
            None => None,
        }
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

    pub(super) fn pane_attached(&self, key: &PaneRef) -> bool {
        self.live_pane_record(key)
            .is_some_and(|record| record.attached)
    }

    /// Every pane record's live facts for the pointer probe — exact
    /// incarnation, attachment state, and controller mode. The probe
    /// caller pairs these with the view's per-incarnation projection
    /// status; positional and anchor spellings never enter the list.
    pub(super) fn pane_facts(
        &self,
    ) -> impl Iterator<Item = (PaneRef, bool, Option<&ControllerMode>)> + '_ {
        self.panes
            .iter()
            .map(|record| (record.pane_ref(), record.attached, record.mode.as_ref()))
    }

    /// The exact live incarnation of the focus pane — the key input, resize,
    /// and focus-scoped setup commands are tagged with. `None` when the focus
    /// pane departed or has no record.
    pub(super) fn focus_pane_ref(&self) -> Option<PaneRef> {
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
    pub(super) fn pollable_panes(&self) -> Vec<PaneRef> {
        self.panes
            .iter()
            .filter(|record| {
                record.attached && record.projected && !record.departing && record.mode.is_some()
            })
            .map(PaneRecord::pane_ref)
            .collect()
    }

    /// The focus pane's panel identity — used to match leaf geometry reports
    /// and projection lookups against the focus.
    pub(super) fn focus_panel_id(&self) -> Option<&WorkbenchPanelId> {
        self.desired.as_ref().map(PaneKey::panel)
    }

    /// Whether a context-selection admission is currently in flight —
    /// the Agent board's jump planner waits it out instead of stacking
    /// a second selection behind it.
    pub(super) fn context_selecting(&self) -> bool {
        self.context_selection.is_some()
    }

    pub(super) fn pane_projected(&self, key: &PaneRef) -> bool {
        self.live_pane_record(key)
            .is_some_and(|record| record.projected)
    }

    pub(super) fn expected_frame_geometry(&self, key: &PaneRef) -> Option<Geometry> {
        let record = self.live_pane_record(key)?;
        if !record.attached || !record.projected {
            return None;
        }
        match record.mode.as_ref() {
            Some(ControllerMode::Controlling(_)) => record.last_accepted_geometry,
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

    /// Per-pane leaf observation: the shell projects every visible leaf each
    /// viewport sync and reports `(panel_id, panel_index, content_geometry)`.
    /// Surviving records keep their identity; a leaf with no record gets one
    /// (fresh incarnation); a non-departing record whose panel vanished becomes
    /// `departing` and waits for canonical disposal. Returns whether the
    /// membership set diverged, so the caller may schedule reconciliation.
    pub(super) fn sync_pane_set(
        &mut self,
        leaves: &[(WorkbenchPanelId, usize, Geometry)],
        sampled_at: Instant,
    ) -> bool {
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
                    let provenance = self
                        .provenance
                        .get(tab_index)
                        .cloned()
                        .unwrap_or(CreateTerminalProvenance::BindingOnly);
                    let incarnation = self.next_incarnation;
                    self.next_incarnation += 1;
                    self.panes.push(PaneRecord::new(
                        PaneRef::new(pane, incarnation),
                        tab_index,
                        *panel_index,
                        self.cache.site_id().clone(),
                        panel.binding().clone(),
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
    pub(super) fn reconcile_pending(&self) -> bool {
        let Some(anchor) = self.desired.as_ref().map(|key| key.anchor().clone()) else {
            return false;
        };
        self.panes.iter().any(|record| {
            record.key.anchor == anchor
                && (record.departing
                    || (record.attachable() && !record.attached && record.retryable()))
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
    pub(super) fn reconcile_targets(
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
                    && !record.attached
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
    pub(super) fn settle_pane_detached(&mut self, key: &PaneRef) -> bool {
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
    pub(super) fn drop_anchor_panes(&mut self, anchor: &AttachmentKey) {
        self.panes.retain(|record| &record.key.anchor != anchor);
    }

    /// Marks every pane departing: used when a new anchor is installed (tab
    /// switch or committed create) so the old epoch's viewers converge to
    /// disposal while the new focus pane's record is minted by the caller.
    /// Unlike a same-anchor leaf departure, epoch teardown also voids the
    /// projection fact — old-anchor records never reappear in place, and the
    /// cleanup guards read `projected` as their uninstalled proof.
    pub(super) fn depart_all_panes(&mut self) {
        for record in &mut self.panes {
            record.mark_departing();
            record.projected = false;
            record.last_accepted_geometry = None;
        }
    }

    /// Installs the fresh focus record for a newly minted target. The caller
    /// owns incarnation minting (attach_target); this only registers the record.
    pub(super) fn install_pane_record(&mut self, target: &AttachTarget) {
        let pane = target.selected.key.clone();
        if self.pane_record(&pane).is_some() {
            return;
        }
        self.panes.push(PaneRecord::new(
            PaneRef::new(pane, target.incarnation),
            target.selected.index,
            target.selected.panel_index,
            target.site_id.clone(),
            target.binding.clone(),
            target.provenance.clone(),
            self.latest_geometry,
        ));
    }

    /// Installs the focus record for a selection that will never attach (the
    /// pane's provenance is not attachable): the record carries the leaf facts
    /// so status and reconcile see a parked, fault-free pane.
    pub(super) fn install_unattached_pane(&mut self, selected: &SelectedTab) {
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
        let incarnation = self.next_incarnation;
        self.next_incarnation += 1;
        self.panes.push(PaneRecord::new(
            PaneRef::new(pane, incarnation),
            selected.index,
            selected.panel_index,
            self.cache.site_id().clone(),
            panel.binding().clone(),
            self.provenance
                .get(selected.index)
                .cloned()
                .unwrap_or(CreateTerminalProvenance::BindingOnly),
            self.latest_geometry,
        ));
    }

    /// One pane's controller grant ended or was lost: the pane keeps its live
    /// viewer but is parked view-only until an explicit reconcile re-arms it.
    /// No automatic re-acquire — NotController and definite failures never
    /// silently retry.
    pub(super) fn settle_pane_fault(&mut self, key: &PaneRef, fault: PaneFault) -> bool {
        let eligible = self
            .live_pane_record(key)
            .is_some_and(|record| match fault {
                PaneFault::AttachRejected
                | PaneFault::AttachUnavailable
                | PaneFault::CapacityExceeded
                | PaneFault::ConvergenceExpired => !record.attached,
                PaneFault::ViewerLost | PaneFault::ControlLost => record.attached,
            });
        if !eligible {
            return false;
        }
        let pane = key.pane().clone();
        {
            let Some(record) = self.live_pane_record_mut(key) else {
                return false;
            };
            match fault {
                PaneFault::ViewerLost => {
                    record.attached = false;
                    record.projected = false;
                    record.mode = None;
                    record.staged_frame = None;
                    record.accepted_frame = None;
                    record.last_accepted_geometry = None;
                }
                PaneFault::ControlLost => {
                    record.mode = Some(ControllerMode::ReadOnly);
                }
                _ => {}
            }
            record.fault = Some(fault);
        }
        // A focus pane losing its grant downgrades the session's controller mode
        // to view-only as well; other panes are unaffected. A focus pane whose
        // viewer was lost drops to the unattached-selection phases, exactly like
        // a failed setup: parked until an explicit new selection retries.
        if self.desired.as_ref() == Some(&pane) {
            match fault {
                PaneFault::ControlLost => {
                    if let SessionPhase::Ready(ready) = &mut self.phase {
                        ready.mode = ControllerMode::ReadOnly;
                    }
                }
                PaneFault::ViewerLost => {
                    self.clear_pane_attachment(&pane);
                    if let Some(selected) = self.active().cloned() {
                        let attachable = self
                            .pane_record(&pane)
                            .is_some_and(|record| record.attachable());
                        self.phase = if attachable {
                            SessionPhase::RetryableAttachUnavailable(selected)
                        } else {
                            SessionPhase::SelectedWithoutAttachment(selected)
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
    pub(super) fn admit_observed_geometries(
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
                || !record.attached
                || !matches!(record.mode, Some(ControllerMode::Controlling(_)))
                || record.last_admitted_geometry == Some(*geometry)
            {
                continue;
            }
            resizes.push((record.pane_ref(), *geometry));
        }
        resizes
    }

    pub(super) fn record_admitted_geometry(&mut self, key: &PaneRef, geometry: Geometry) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        if record.departing
            || !record.attached
            || !matches!(record.mode, Some(ControllerMode::Controlling(_)))
        {
            return false;
        }
        record.last_admitted_geometry = Some(geometry);
        true
    }

    pub(super) fn record_setup_configuration(
        &mut self,
        key: &PaneRef,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> bool {
        if matches!(&mode, ControllerMode::Controlling(_)) != applied_geometry.is_some()
            || self.live_pane_record(key).is_none()
        {
            return false;
        }
        let pane_attached = self.pane_attached(key);
        let background_live = self.desired.as_ref() != Some(key.pane())
            && self
                .live_pane_record(key)
                .is_some_and(|record| record.attached && !record.departing);
        let accepted_geometry = match &mut self.phase {
            SessionPhase::Starting(setup) if setup.selected.key == *key.pane() && pane_attached => {
                setup.mode = Some(mode.clone());
                Some(&mut setup.accepted_geometry)
            }
            SessionPhase::Ready(ready)
                if ready.selected.key == *key.pane() && ready.mode == mode && pane_attached =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            SessionPhase::Creating(barrier)
                if barrier.anchor.ready().is_some_and(|ready| {
                    ready.selected.key == *key.pane() && ready.mode == mode
                }) && pane_attached =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            SessionPhase::CreateCommittedNeedDetach(committed)
                if committed.anchor.ready().is_some_and(|ready| {
                    ready.selected.key == *key.pane() && ready.mode == mode
                }) && pane_attached =>
            {
                if applied_geometry.is_none() {
                    return false;
                }
                None
            }
            SessionPhase::Switching(setup)
                if setup.target.selected.key == *key.pane()
                    && setup.membership.is_none()
                    && setup.previous.is_none()
                    && pane_attached =>
            {
                setup.mode = Some(mode.clone());
                Some(&mut setup.accepted_geometry)
            }
            SessionPhase::Switching(setup)
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
            SessionPhase::Ready(_) if background_live => None,
            _ => return false,
        };
        if let Some(accepted_geometry) = accepted_geometry {
            *accepted_geometry = applied_geometry;
        }
        if matches!(&self.phase, SessionPhase::Switching(setup)
            if setup.previous.as_ref().is_some_and(|previous| previous.selected.key == *key.pane()))
        {
            self.invalidate_switch_anchor();
        }
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        record.mode = Some(mode);
        record.last_accepted_geometry = applied_geometry;
        if record.projected {
            record.staged_frame = None;
            record.accepted_frame = None;
        }
        true
    }

    pub(super) fn settle_setup_configuration_event(
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
            || self.live_pane_record(key).is_none_or(|record| {
                record.projected || record.staged_frame.is_some() || record.accepted_frame.is_some()
            })
        {
            return None;
        }
        let SessionPhase::Switching(setup) = &mut self.phase else {
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
        if let Some(record) = self.live_pane_record_mut(key) {
            record.last_accepted_geometry = applied_geometry;
        }
        self.invalidate_switch_anchor();
        Some(SetupEventDisposition::Superseded { desired })
    }

    pub(super) fn stage_drawn_frame(&mut self, key: &PaneRef, frame: FrameIdentity) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        if !record.attached || !record.projected || record.departing {
            return false;
        }
        record.staged_frame = Some(frame);
        record.accepted_frame = None;
        true
    }

    pub(super) fn invalidate_projection_frame(&mut self, key: &PaneRef) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        if !record.attached || !record.projected {
            return false;
        }
        record.staged_frame = None;
        record.accepted_frame = None;
        true
    }

    pub(super) fn publish_ready(
        &mut self,
        key: &PaneRef,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> bool {
        let selected = match &self.phase {
            SessionPhase::Starting(setup) if setup.selected.key == *key.pane() => {
                setup.selected.clone()
            }
            SessionPhase::Switching(setup)
                if setup.target.selected.key == *key.pane() && setup.membership.is_none() =>
            {
                setup.target.selected.clone()
            }
            _ => return false,
        };
        let Some(record) = self.live_pane_record(key) else {
            return false;
        };
        if !record.attached
            || !record.projected
            || record.accepted_frame != Some(frame)
            || self.controller_mode() != Some(&mode)
            || matches!(&mode, ControllerMode::Controlling(_)) != accepted_geometry.is_some()
        {
            return false;
        }
        if matches!(&mode, ControllerMode::Controlling(_)) {
            let Some(geometry) = accepted_geometry else {
                return false;
            };
            if geometry != self.latest_geometry || record.last_accepted_geometry != Some(geometry) {
                return false;
            }
        }
        self.phase = SessionPhase::Ready(ReadyAttachment::new(selected, mode));
        true
    }

    pub(super) fn settle_ready_candidate(
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
            SessionPhase::Starting(setup) => setup.selected.key == *key.pane(),
            SessionPhase::Switching(setup) => {
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
        if !record.projected
            || record.accepted_frame != Some(frame)
            || self.controller_mode() != Some(&mode)
            || matches!(&mode, ControllerMode::Controlling(_)) != accepted_geometry.is_some()
        {
            return Err(CarrierFailure::Protocol);
        }
        if let ControllerMode::Controlling(_) = &mode {
            let Some(geometry) = accepted_geometry else {
                return Err(CarrierFailure::Protocol);
            };
            if record.last_accepted_geometry != Some(geometry) {
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
    pub(super) fn install_projection(&mut self, key: &PaneRef) -> bool {
        if self.desired.as_ref() != Some(key.pane())
            || self.pane_attached(key)
            || !matches!(&self.phase, SessionPhase::Starting(_))
                && !matches!(&self.phase, SessionPhase::Switching(setup)
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
        if record.attached || record.departing {
            return false;
        }
        self.mark_pane_attached(key)
    }

    fn mark_pane_attached(&mut self, key: &PaneRef) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        record.attached = true;
        record.projected = true;
        record.staged_frame = None;
        record.accepted_frame = None;
        record.fault = None;
        true
    }

    pub(super) fn settle_attached_event(&mut self, key: &PaneRef) -> Option<SetupEventDisposition> {
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
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (key.anchor().generation() < desired.anchor().generation()
            && setup.previous.is_none()
            && !self.pane_attached(key)
            && !self.pane_projected(key)
            && self.live_pane_record(key).is_none_or(|record| {
                record.staged_frame.is_none() && record.accepted_frame.is_none()
            })
            && self
                .cache
                .tabs()
                .iter()
                .any(|tab| tab.tab_id() == key.anchor().tab_id()))
        .then_some(SetupEventDisposition::Superseded { desired })
    }

    pub(super) fn mark_feedback_accepted(&mut self, key: &PaneRef, frame: FrameIdentity) -> bool {
        let Some(record) = self.live_pane_record_mut(key) else {
            return false;
        };
        if !record.attached || !record.projected || record.staged_frame != Some(frame) {
            return false;
        }
        record.accepted_frame = Some(frame);
        true
    }

    pub(super) fn settle_feedback_event(
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
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (key.anchor().generation() < desired.anchor().generation()
            && setup
                .previous
                .as_ref()
                .is_some_and(|previous| previous.selected.key == *key.pane())
            && self.pane_attached(key)
            && self.live_pane_record(key).is_some_and(|record| {
                !record.projected
                    && record.staged_frame.is_none()
                    && record.accepted_frame.is_none()
            }))
        .then_some(SetupEventDisposition::Superseded { desired })
    }

    #[cfg(test)]
    pub(super) fn feedback_accepted(&self) -> bool {
        self.focus_record()
            .is_some_and(|record| record.accepted_frame.is_some())
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
        let attachment_ok = match anchor.ready() {
            Some(ready) => {
                Some(&ready.selected.key) == self.desired.as_ref()
                    && self
                        .focus_record()
                        .is_some_and(|record| record.attached && record.projected)
            }
            None => self.desired.is_none() && self.panes.is_empty(),
        };
        if !attachment_ok
            || self.pending_refresh.is_some()
            || operation.site_id != *self.cache.site_id()
            || operation.expected_revision != self.cache.revision()
            || operation.terminal_context_id != *self.cache.terminal_context_id()
            || operation.geometry != self.latest_geometry
            || operation.key.reserved_generation
                <= anchor
                    .ready()
                    .map_or(0, |ready| ready.selected.key.anchor().generation())
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
                || self.desired.is_some()
                || !self.panes.is_empty()
                || !self.cache.tabs().is_empty()
            {
                return false;
            }
            self.phase = SessionPhase::Empty;
            self.notice = Some(SessionNotice::CreateNotCreated);
            return true;
        };
        let pane = old_ready.selected.key.clone();
        let Some(record) = self.pane_record(&pane) else {
            return false;
        };
        if !record.attached
            || !record.projected
            || matches!(&old_ready.mode, ControllerMode::Controlling(_))
                != applied_geometry.is_some()
        {
            return false;
        }
        if matches!(&old_ready.mode, ControllerMode::Controlling(_)) {
            let changed = record.last_accepted_geometry != applied_geometry;
            let Some(record) = self.pane_record_mut(&pane) else {
                return false;
            };
            record.last_accepted_geometry = applied_geometry;
            if changed {
                record.staged_frame = None;
                record.accepted_frame = None;
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
        key: &PaneRef,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
        settled_at: Instant,
    ) -> Option<CleanupContinuation> {
        let desired = self.desired.clone()?;
        if self.desired.as_ref() != Some(key.pane()) {
            return (key.anchor().generation() < desired.anchor().generation()
                && self.cache.tabs().get(target_index)?.tab_id() == key.anchor().tab_id()
                && self.provenance.get(target_index)? == target_provenance)
                .then_some(CleanupContinuation::Superseded { desired });
        }
        let SessionPhase::Switching(setup) = &self.phase else {
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
            || self.live_pane_record(key).is_none_or(|record| {
                record.staged_frame.is_some() || record.accepted_frame.is_some()
            })
        {
            return None;
        }
        let selected = setup.target.selected.clone();
        self.phase = if target_provenance.attachable() {
            self.park_focus_pane();
            SessionPhase::RetryableAttachUnavailable(selected)
        } else {
            SessionPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    pub(super) fn settle_cleanup(
        &mut self,
        key: &PaneRef,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        let SessionPhase::Switching(setup) = &self.phase else {
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
                    && record.attached
                    && record.incarnation != key.incarnation()
            })
            || self.live_pane_record(key).is_none_or(|record| {
                record.projected
                    && (record.staged_frame.is_some() || record.accepted_frame.is_some())
            })
            || self.cache.tabs().get(target_index)?.tab_id() != key.anchor().tab_id()
            || self.provenance.get(target_index)? != target_provenance
        {
            return None;
        }
        let selected = setup.target.selected.clone();
        self.clear_pane_attachment(key.pane());
        self.phase = if target_provenance.attachable() {
            self.park_focus_pane();
            SessionPhase::RetryableAttachUnavailable(selected)
        } else {
            SessionPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    pub(super) fn settle_unattached_setup(
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
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        if setup.previous.is_some()
            || setup.membership.is_some()
            || setup.target.selected.key != *key.pane()
            || self.pane_attached(key)
            || self.cache.tabs().get(target_index)?.tab_id() != key.anchor().tab_id()
            || self.provenance.get(target_index)? != target_provenance
        {
            return None;
        }
        let selected = setup.target.selected.clone();
        self.phase = if target_provenance.attachable() {
            self.park_focus_pane();
            SessionPhase::RetryableAttachUnavailable(selected)
        } else {
            SessionPhase::SelectedWithoutAttachment(selected)
        };
        Some(CleanupContinuation::Stable)
    }

    /// Clears one pane's attachment facts after a canonical disposal path (the
    /// record itself stays — it still owns the pane's leaf geometry and fault).
    fn clear_pane_attachment(&mut self, pane: &PaneKey) {
        if let Some(record) = self.pane_record_mut(pane) {
            record.attached = false;
            record.projected = false;
            record.mode = None;
            record.staged_frame = None;
            record.accepted_frame = None;
            record.last_accepted_geometry = None;
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
    pub(super) fn note_publication_hint(&mut self) {
        self.publication_debt = match self.publication_debt {
            PublicationDebt::Clean | PublicationDebt::Parked => PublicationDebt::Pending,
            PublicationDebt::Pending => PublicationDebt::Pending,
            PublicationDebt::InFlight { .. } => PublicationDebt::InFlight { late: true },
        };
    }

    /// Whether a captured publication demand is waiting for refresh admission.
    pub(super) const fn publication_debt_pending(&self) -> bool {
        matches!(self.publication_debt, PublicationDebt::Pending)
    }

    #[cfg(test)]
    pub(super) const fn publication_debt_kind(&self) -> PublicationDebt {
        self.publication_debt
    }

    /// Admits one pending publication demand into a fresh refresh flight. The flight
    /// adopts the demand, so its outcome settles exactly this debt and nothing else.
    pub(super) fn start_publication_drain(&mut self, generation: u64) -> bool {
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

    pub(super) fn start_pending_refresh(&mut self, generation: u64) -> bool {
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

    pub(super) fn prepare_refresh(
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
        let leaf_repair = active
            .as_ref()
            .and_then(|active| self.selected_leaf_repair_index(&candidate, active));
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
            SessionPhaseKind::Ready
            | SessionPhaseKind::MoveProjectionUnavailable
            | SessionPhaseKind::PanelProjectionUnavailable
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

    pub(super) fn apply_prepared_refresh(
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
                    self.move_recovery = None;
                    self.panel_recovery = None;
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
                && !record.attached
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
        candidate: &NativeContextTabs,
        active: &SelectedTab,
    ) -> Option<usize> {
        let index = candidate.tab_index(active.key.anchor().tab_id())?;
        let panel_id = self
            .cache
            .tabs()
            .get(active.index)
            .and_then(|tab| tab.panel(active.panel_index()))
            .map(|panel| panel.panel_id())?;
        let tab = candidate.tabs().get(index)?;
        tab.panel_index(panel_id).is_none().then(|| {
            active
                .panel_index()
                .min(tab.panel_count().saturating_sub(1))
        })
    }

    fn merge_projection(&mut self, candidate: NativeContextTabs) -> bool {
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
        self.prune_zoomed_tabs();
        true
    }

    fn attach_target(
        &mut self,
        selected: SelectedTab,
        provenance: CreateTerminalProvenance,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        let tab = &self.cache.tabs()[selected.index];
        let binding = tab.panel(selected.panel_index())?.binding().clone();
        let incarnation = self.next_incarnation;
        self.next_incarnation += 1;
        Some(AttachTarget {
            selected,
            site_id: self.cache.site_id().clone(),
            binding,
            provenance,
            deadlines,
            incarnation,
        })
    }

    pub(super) fn target_for_switch(
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
        let panel = self.cache.tabs().get(index)?.panel(0)?;
        let pane = PaneKey::new(key, panel.panel_id().clone());
        self.attach_target(
            SelectedTab::new_panel(pane, index, 0),
            provenance,
            deadlines,
        )
    }

    /// `target_for_switch` carrying the stable panel identity the caller
    /// wants focused after landing — the Agent board's jump variant. The
    /// identity is re-resolved to the at-that-moment `panel_index` here,
    /// so the switch can never fall back to `panel(0)` for a known target.
    /// `None` when the panel is not currently present in that tab.
    pub(super) fn target_for_switch_for_panel(
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

    /// A directional focus move is a pure local transition: the target pane's
    /// viewer is already live under the same anchor, so no detach/attach round
    /// trip is minted. Returns the pane to focus, or `None` when the move is
    /// not currently legal (no live viewer on the neighbour, or the session is
    /// not Ready).
    pub(super) fn plan_panel_focus_local(
        &self,
        direction: crate::client_view::PanelDirection,
    ) -> Option<PaneKey> {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.pending_refresh.is_some()
            || !matches!(&self.phase, SessionPhase::Ready(_))
        {
            return None;
        }
        let active = self.active()?;
        let tab = self.cache.tabs().get(active.index())?;
        let panel_index = tab.panel_focus_target(active.panel_index(), direction)?;
        if panel_index == active.panel_index() {
            return None;
        }
        let panel_id = tab.panel(panel_index)?.panel_id().clone();
        let pane = PaneKey::new(active.key().anchor().clone(), panel_id);
        let record = self.pane_record(&pane)?;
        (record.attached && record.projected && !record.departing && record.mode.is_some())
            .then_some(pane)
    }

    /// The pointer's sibling of [`plan_panel_focus_local`]: the hit test
    /// already resolved a stable panel identity, so the plan names that
    /// panel's pane under the active anchor instead of a direction. The
    /// same guards apply — a pure local focus transition over an attached,
    /// non-departing pane, never a fresh attach round trip.
    pub(super) fn plan_panel_focus_local_at(&self, panel: &WorkbenchPanelId) -> Option<PaneKey> {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.pending_refresh.is_some()
            || !matches!(&self.phase, SessionPhase::Ready(_))
        {
            return None;
        }
        let active = self.active()?;
        let pane = PaneKey::new(active.key().anchor().clone(), panel.clone());
        let record = self.pane_record(&pane)?;
        (record.attached && record.projected && !record.departing && record.mode.is_some())
            .then_some(pane)
    }

    /// Whether the named panel's viewer is live under the current desired
    /// anchor — attached, projected, not departing, and holding a
    /// controller mode. The Agent board's jump requires this for its
    /// `Landed` verdict: a resolved placement with a dead viewer record
    /// must reattach, never land.
    pub(super) fn panel_viewer_live(&self, panel: &WorkbenchPanelId) -> bool {
        let Some(anchor) = self.desired.as_ref().map(PaneKey::anchor) else {
            return false;
        };
        self.pane_record(&PaneKey::new(anchor.clone(), panel.clone()))
            .is_some_and(|record| {
                record.attached && record.projected && !record.departing && record.mode.is_some()
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
    pub(super) fn target_for_dead_panel_reattach(
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
                SessionPhase::Ready(_) | SessionPhase::SelectedWithoutAttachment(_)
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
            && record.attached
            && record.projected
            && !record.departing
            && record.mode.is_some()
        {
            return None;
        }
        let key = AttachmentKey::new(generation, tab.tab_id().clone());
        self.target_for_switch_for_panel(index, key, deadlines, panel_id)
    }

    /// Installs a local focus move approved by [`plan_panel_focus_local`]: the
    /// focus pane, its observed geometry, and its controller mode swap without
    /// touching any attachment. Queued input/resize commands keep their frozen
    /// `PaneRef` tags and remain valid — the membership set does not change.
    pub(super) fn move_panel_focus_local(&mut self, pane: PaneKey) -> bool {
        let Some(record) = self.pane_record(&pane) else {
            return false;
        };
        if !record.attached || !record.projected || record.departing {
            return false;
        }
        let Some(mode) = record.mode.clone() else {
            return false;
        };
        let geometry = record.observed_geometry;
        // Resolve the panel's current index from the cache: a refresh merge may
        // have reordered panels since the record last observed the leaf set.
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
        let SessionPhase::Ready(ready) = &mut self.phase else {
            return false;
        };
        ready.selected.key = pane.clone();
        ready.selected.panel_index = panel_index;
        ready.mode = mode;
        self.desired = Some(pane);
        self.latest_geometry = geometry;
        true
    }

    /// `Ctrl+B z`: a volatile per-Tab Client-local zoom flag, sharing the
    /// guards the local focus transition owns — a Ready phase with no
    /// structural mutation in flight, no armed rename draft, and no pending
    /// refresh. A single-leaf active Tab is an honest no-op. Returns whether
    /// the rendered pane set changed, which the dispatcher translates into
    /// a layout re-observation and pointer-map invalidation.
    pub(super) fn toggle_zoom(&mut self) -> bool {
        if self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.pending_refresh.is_some()
            || !matches!(&self.phase, SessionPhase::Ready(_))
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
        if !self.zoomed_tabs.remove(&tab_id) {
            self.zoomed_tabs.insert(tab_id);
        }
        true
    }

    /// Read one Client-local ratio only when both ordered slot identities still match.
    pub(super) fn split_ratio(
        &self,
        context: &WorkbenchTerminalContextId,
        tab: &WorkbenchTabId,
        split: &orchbee_workbench_protocol::WorkbenchSplitId,
        first: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
        second: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
    ) -> orchbee_workbench_protocol::ClientSplitRatio {
        self.split_ratios.get(context, tab, split, first, second)
    }

    pub(super) fn commit_layout_adjustment(
        &mut self,
        snapshot: &orchbee_workbench_protocol::Snapshot,
        adjustment: &orchbee_workbench_protocol::ClientLayoutAdjustment,
    ) -> Result<(), orchbee_workbench_protocol::ClientViewControlReason> {
        if snapshot.site_id != *self.cache.site_id() || snapshot.revision < self.cache.revision() {
            return Err(orchbee_workbench_protocol::ClientViewControlReason::StaleView);
        }
        self.split_ratios.commit(snapshot, adjustment)
    }

    /// Read-side adjudication: the active Tab draws zoomed exactly while its
    /// flag survives AND its current cache membership stays multi-leaf — a
    /// stale retained flag can never paint a zoom.
    /// Observe the retained flag against the queried Tab's authoritative membership, without GC.
    pub(super) fn layout_zoomed(&self, tab: &NativeSimpleTab) -> bool {
        tab.panel_count() > 1 && self.zoomed_tabs.contains(tab.tab_id())
    }

    pub(super) fn zoom_active(&self) -> bool {
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
        if let Some(tab) = self.cache.tabs().iter().find(|tab| tab.tab_id() == tab_id) {
            return tab.panel_count();
        }
        let current = self.cache.location();
        self.retained_contexts
            .iter()
            .filter(|retained| retained.location() != current)
            .find_map(|retained| retained.tabs().iter().find(|tab| tab.tab_id() == tab_id))
            .map_or(0, NativeSimpleTab::panel_count)
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
        self.plan_selection_index(index)
    }

    pub(super) fn plan_selection_index(&self, index: usize) -> SelectionPlan {
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
        self.focus_record().is_some_and(|record| record.attached)
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
        self.cancel_close_confirmation();
        self.cancel_panel_close_confirmation();
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
        self.panes.clear();
        self.reconcile_deadlines = None;
        self.pending_refresh = None;
        self.publication_debt = PublicationDebt::Clean;
        self.context_selection = None;
        self.pending_membership = None;
        self.refresh_status = None;
    }

    pub(super) fn view(&self) -> NativeSessionView<'_> {
        NativeSessionView { state: self }
    }

    pub(super) fn invariant_holds(&self) -> bool {
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
            SessionPhase::SelectedWithoutAttachment(_)
                | SessionPhase::RetryableAttachUnavailable(_)
                | SessionPhase::Retired(_)
        ) && self
            .panes
            .iter()
            .any(|record| record.projected && Some(&record.key) == self.desired.as_ref())
        {
            return false;
        }
        // Per-record shape: at most one record per pane; a projection implies an
        // attachment; an accepted frame implies the same staged frame; every
        // non-departing record belongs to the desired anchor; only an attached
        // record may carry frame or geometry state.
        let unique = !self.panes.iter().enumerate().any(|(index, record)| {
            self.panes[index + 1..]
                .iter()
                .any(|other| other.key == record.key)
        });
        let records_valid = unique
            && self.panes.iter().all(|record| {
                (!record.projected || record.attached)
                    && record
                        .accepted_frame
                        .is_none_or(|frame| record.staged_frame == Some(frame))
                    && (!record.attached
                        || (record.projected
                            || (record.staged_frame.is_none() && record.accepted_frame.is_none())))
                    && (record.departing
                        || self
                            .desired
                            .as_ref()
                            .is_some_and(|desired| record.key.anchor() == desired.anchor()))
                    && (record.attached
                        || (record.staged_frame.is_none()
                            && record.accepted_frame.is_none()
                            && record.mode.is_none()
                            && record.last_accepted_geometry.is_none()))
            });
        if !records_valid {
            return false;
        }
        let attachment_shape_valid = match &self.phase {
            SessionPhase::ReconcilingMembership(selected) => {
                Some(&selected.key) == self.desired.as_ref()
                    && self.focus_record().is_some_and(|record| record.attached)
            }
            SessionPhase::Empty => {
                self.cache.tabs().is_empty()
                    && self.provenance.is_empty()
                    && self.desired.is_none()
                    && self.panes.is_empty()
            }
            SessionPhase::Starting(setup) => {
                Some(&setup.selected.key) == self.desired.as_ref() && self.focus_record().is_some()
            }
            SessionPhase::Ready(ready) => {
                Some(&ready.selected.key) == self.desired.as_ref()
                    && self.focus_record().is_some_and(|record| {
                        record.attached
                            && record.projected
                            && record.mode.as_ref() == Some(&ready.mode)
                    })
            }
            SessionPhase::Creating(barrier) => {
                barrier.anchor.ready().map(|ready| &ready.selected.key) == self.desired.as_ref()
                    && match barrier.anchor.ready() {
                        Some(_) => self
                            .focus_record()
                            .is_some_and(|record| record.attached && record.projected),
                        None => self
                            .panes
                            .iter()
                            .all(|record| !record.attached && !record.projected),
                    }
            }
            SessionPhase::CreateCommittedNeedDetach(committed)
            | SessionPhase::ProjectionUnavailableBeforeCleanup(committed) => {
                committed.anchor.ready().map(|ready| &ready.selected.key) == self.desired.as_ref()
                    && match committed.anchor.ready() {
                        Some(_) => self
                            .focus_record()
                            .is_some_and(|record| record.attached && record.projected),
                        None => self
                            .panes
                            .iter()
                            .all(|record| !record.attached && !record.projected),
                    }
            }
            SessionPhase::Switching(setup) => {
                Some(&setup.target.selected.key) == self.desired.as_ref()
                    && match &setup.previous {
                        Some(previous) => self
                            .pane_record(&previous.selected.key)
                            .is_some_and(|record| record.attached),
                        None => self.focus_record().is_some(),
                    }
            }
            SessionPhase::SelectedWithoutAttachment(selected)
            | SessionPhase::RetryableAttachUnavailable(selected) => {
                Some(&selected.key) == self.desired.as_ref()
                    && self.focus_record().is_none_or(|record| !record.attached)
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
            ratios: &self.state.split_ratios,
            context: self.state.cache.terminal_context_id(),
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

    /// The active Tab renders zoomed — the volatile per-Tab flag read
    /// through current-cache adjudication only.
    pub(super) fn zoom_active(&self) -> bool {
        self.state.zoom_active()
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

    /// The live record for one panel of the focus epoch — the pane a leaf's
    /// projection and geometry belong to.
    pub(super) fn pane_by_panel(&self, panel: &WorkbenchPanelId) -> Option<&PaneRecord> {
        self.state.pane_by_panel(panel)
    }

    #[cfg(test)]
    pub(super) const fn retired_failure(&self) -> Option<CarrierFailure> {
        self.state.retired_failure()
    }
}

#[derive(Clone, Copy)]
pub(super) struct NativeTabView<'a> {
    ratios: &'a split_ratios::SplitRatios,
    context: &'a WorkbenchTerminalContextId,
    tab: &'a NativeSimpleTab,
    position: usize,
    active: bool,
    active_panel: Option<usize>,
}

impl<'a> NativeTabView<'a> {
    pub(super) fn split_ratio(
        &self,
        split: &orchbee_workbench_protocol::WorkbenchSplitId,
        first: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
        second: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
    ) -> orchbee_workbench_protocol::ClientSplitRatio {
        self.ratios
            .get(self.context, self.tab.tab_id(), split, first, second)
    }

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

    pub(super) fn tab_id(&self) -> &'a WorkbenchTabId {
        self.tab.tab_id()
    }

    #[cfg(test)]
    pub(super) fn presentation_label(&self) -> &'a str {
        self.tab.presentation_label().as_str()
    }
}

#[cfg(test)]
mod tests;
