use std::{
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    LifecycleHostFailure, LifecycleInventoryCursor, LifecycleInventoryFailure,
    LifecycleInventoryPage, LifecycleRejection, LifecycleStepOrdinal, LifecycleTargetObservation,
    LifecycleTerminalPhase, LifecycleUnknownReason, TerminalLifecycleAction,
    TerminalLifecycleOutcome, TerminalLifecycleRequest, TerminalLifecycleResult,
};
use orchbee_workbench_protocol::{
    WorkbenchRevision, WorkbenchTabCloseBlockedReason as BlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason as PublicationReason, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnknownPhase as UnknownPhase, WorkbenchTabCloseUnknownReason as UnknownReason,
};

const ATTEMPT_DURATION: Duration = Duration::from_secs(30);
const QUERY_SPACING: Duration = Duration::from_millis(250);
const TERMINATE_GRACE: Duration = Duration::from_secs(5);
const MAXIMUM_STEPS: u8 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleStage {
    Observe,
    Terminate,
    Grace,
    Outcome,
    Dispose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalStage {
    TargetCleanup,
    Publication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Progress {
    Running(LifecycleStage),
    Blocked {
        resume: LifecycleStage,
        reason: BlockedReason,
        resumable: bool,
    },
    Unknown {
        phase: UnknownPhase,
        reason: UnknownReason,
    },
    Disposed {
        stage: LocalStage,
        parked: Option<(PublicationReason, bool)>,
    },
    Closed(WorkbenchRevision),
}

#[derive(Clone, Copy, Debug, Default)]
struct InventoryWalk {
    incomplete: bool,
    cursor: Option<LifecycleInventoryCursor>,
    found: Option<LifecycleTargetObservation>,
}

struct PendingStep {
    seal: Arc<TerminalLifecycleRequest>,
    admitted: bool,
}

/// An origin-bound prepared step; dropping it does not cancel retained work.
pub struct CloseStepToken {
    seal: Arc<TerminalLifecycleRequest>,
}

impl CloseStepToken {
    #[must_use]
    pub fn request(&self) -> TerminalLifecycleRequest {
        *self.seal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseStepTokenMismatch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseStepPrequeueFailure {
    Backpressure,
    Unavailable,
    IdentityConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseStepObservation<'result> {
    Result(&'result TerminalLifecycleResult),
    NotDelivered,
    OutcomeUnknown(LifecycleUnknownReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseContinuationNext {
    Outstanding,
    Parked,
    WaitUntil(Instant),
    Lifecycle {
        action: TerminalLifecycleAction,
        step_ordinal: LifecycleStepOrdinal,
    },
    TargetCleanup,
    Publication,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CloseContinuationResumeError {
    NotResumable,
    ClockUnavailable,
}

pub(super) struct PreparedCloseContinuationResume {
    deadline: Instant,
    progress: Progress,
}

/// Retained inside the one primary Workbench Close record. Generation, identity,
/// receipt and ledger capacity belong to that enclosing record, not this value.
pub(super) struct CloseContinuation {
    require_idle: bool,
    progress: Progress,
    deadline: Instant,
    submitted_steps: u8,
    last_query_at: Option<Instant>,
    backpressure_until: Option<Instant>,
    terminate_acknowledged: bool,
    grace_deadline: Option<Instant>,
    force_acknowledged: bool,
    settled_observed: bool,
    inventory: InventoryWalk,
    pending: Option<PendingStep>,
}

impl CloseContinuation {
    pub(super) fn confirmed(&mut self) {
        self.require_idle = false;
        if !self.settled_observed
            && matches!(self.progress, Progress::Running(LifecycleStage::Dispose))
        {
            self.progress = Progress::Running(LifecycleStage::Observe);
        }
    }
    pub(super) fn prepare_if_idle(
        now: Instant,
        phase: Option<LifecycleTerminalPhase>,
    ) -> Option<Self> {
        // AwaitingStart may become Running before the Close fence commits.
        // Re-observe it under the retained operation rather than freezing a
        // pre-admission no-process sample into unconditional disposal.
        let mut continuation = Self::prepare(
            now,
            phase.filter(|phase| !matches!(phase, LifecycleTerminalPhase::AwaitingStart)),
        )?;
        continuation.require_idle = true;
        Some(continuation)
    }
    pub(super) fn prepare_uncreated(now: Instant) -> Option<Self> {
        let mut continuation = Self::prepare(now, None)?;
        continuation.progress = Progress::Disposed {
            stage: LocalStage::Publication,
            parked: None,
        };
        Some(continuation)
    }

    pub(super) fn prepare(
        now: Instant,
        initial_phase: Option<LifecycleTerminalPhase>,
    ) -> Option<Self> {
        Some(Self {
            require_idle: false,
            progress: Progress::Running(
                initial_phase.map_or(LifecycleStage::Observe, initial_stage),
            ),
            deadline: now.checked_add(ATTEMPT_DURATION)?,
            submitted_steps: 0,
            last_query_at: None,
            backpressure_until: None,
            terminate_acknowledged: false,
            grace_deadline: None,
            force_acknowledged: false,
            settled_observed: initial_phase.is_some_and(is_settled),
            inventory: InventoryWalk::default(),
            pending: None,
        })
    }

    pub(super) fn status(&self) -> WorkbenchTabCloseStatus {
        match self.progress {
            Progress::Running(stage) => WorkbenchTabCloseStatus::Closing {
                phase: public_phase(stage),
            },
            Progress::Blocked {
                reason, resumable, ..
            } => WorkbenchTabCloseStatus::CleanupBlocked { reason, resumable },
            Progress::Unknown { phase, reason } => {
                WorkbenchTabCloseStatus::OutcomeUnknown { phase, reason }
            }
            Progress::Disposed { stage, parked } => {
                let (reason, resumable) = parked.unwrap_or((
                    match stage {
                        LocalStage::TargetCleanup => PublicationReason::TargetCleanupPending,
                        LocalStage::Publication => PublicationReason::Preparing,
                    },
                    false,
                ));
                WorkbenchTabCloseStatus::DisposedAwaitingPublication { reason, resumable }
            }
            Progress::Closed(revision) => WorkbenchTabCloseStatus::Closed { revision },
        }
    }

    pub(super) fn has_outstanding(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn is_unknown(&self) -> bool {
        matches!(self.progress, Progress::Unknown { .. })
    }

    pub(super) fn is_resumable(&self) -> bool {
        !self.has_outstanding()
            && matches!(
                self.progress,
                Progress::Blocked {
                    resumable: true,
                    ..
                } | Progress::Disposed {
                    parked: Some((_, true)),
                    ..
                }
            )
    }

    pub(super) fn prepare_resume(
        &self,
        now: Instant,
    ) -> Result<PreparedCloseContinuationResume, CloseContinuationResumeError> {
        if !self.is_resumable() {
            return Err(CloseContinuationResumeError::NotResumable);
        }
        let progress = match self.progress {
            Progress::Blocked { resume, .. } => Progress::Running(resume),
            Progress::Disposed { stage, .. } => Progress::Disposed {
                stage,
                parked: None,
            },
            _ => return Err(CloseContinuationResumeError::NotResumable),
        };
        Ok(PreparedCloseContinuationResume {
            deadline: now
                .checked_add(ATTEMPT_DURATION)
                .ok_or(CloseContinuationResumeError::ClockUnavailable)?,
            progress,
        })
    }

    // Parent prepares and commits under the same owner gate alongside generation/receipt.
    pub(super) fn commit_resume(&mut self, prepared: PreparedCloseContinuationResume) {
        self.deadline = prepared.deadline;
        self.progress = prepared.progress;
        self.submitted_steps = 0;
        self.backpressure_until = None;
    }

    pub(super) fn generation_exhausted(&mut self) {
        match &mut self.progress {
            Progress::Blocked {
                reason, resumable, ..
            } => {
                *reason = BlockedReason::ResumeGenerationExhausted;
                *resumable = false;
            }
            Progress::Disposed {
                parked: Some((_, resumable)),
                ..
            } => *resumable = false,
            _ => {}
        }
    }

    pub(super) fn next(&mut self, now: Instant) -> CloseContinuationNext {
        if self.pending.is_some() {
            return CloseContinuationNext::Outstanding;
        }
        if matches!(self.progress, Progress::Closed(_)) {
            return CloseContinuationNext::Closed;
        }
        if !matches!(
            self.progress,
            Progress::Running(_) | Progress::Disposed { parked: None, .. }
        ) {
            return CloseContinuationNext::Parked;
        }
        if now >= self.deadline || self.submitted_steps >= MAXIMUM_STEPS {
            self.park_budget();
            return CloseContinuationNext::Parked;
        }
        if let Some(until) = self.backpressure_until.filter(|until| now < *until) {
            return CloseContinuationNext::WaitUntil(until.min(self.deadline));
        }
        let stage = match self.progress {
            Progress::Disposed {
                stage: LocalStage::TargetCleanup,
                ..
            } => return CloseContinuationNext::TargetCleanup,
            Progress::Disposed {
                stage: LocalStage::Publication,
                ..
            } => return CloseContinuationNext::Publication,
            Progress::Running(stage) => stage,
            _ => return CloseContinuationNext::Parked,
        };
        let action = match stage {
            LifecycleStage::Terminate if self.require_idle => {
                TerminalLifecycleAction::TerminateIfIdle
            }
            LifecycleStage::Terminate => TerminalLifecycleAction::Terminate,
            LifecycleStage::Dispose => TerminalLifecycleAction::Dispose,
            LifecycleStage::Grace
                if !self.settled_observed
                    && self.grace_deadline.is_some_and(|deadline| now >= deadline) =>
            {
                if self.require_idle {
                    self.park_lifecycle(BlockedReason::HostRefused, true);
                    return CloseContinuationNext::Parked;
                }
                TerminalLifecycleAction::Force
            }
            LifecycleStage::Observe | LifecycleStage::Grace | LifecycleStage::Outcome => {
                if let Some(last) = self.last_query_at {
                    let Some(until) = last.checked_add(QUERY_SPACING) else {
                        self.park_lifecycle(BlockedReason::InventoryIncomplete, false);
                        return CloseContinuationNext::Parked;
                    };
                    if now < until {
                        return CloseContinuationNext::WaitUntil(until.min(self.deadline));
                    }
                }
                TerminalLifecycleAction::InventoryPage {
                    cursor: self.inventory.cursor,
                }
            }
        };
        let Ok(step_ordinal) = LifecycleStepOrdinal::new(self.submitted_steps + 1) else {
            self.park_budget();
            return CloseContinuationNext::Parked;
        };
        CloseContinuationNext::Lifecycle {
            action,
            step_ordinal,
        }
    }

    // Parent validates the complete target, binding fence and canonical operation first.
    pub(super) fn reserve_step(
        &mut self,
        now: Instant,
        generation: NonZeroU64,
        request: TerminalLifecycleRequest,
    ) -> Result<CloseStepToken, CloseStepTokenMismatch> {
        if request.attempt_generation != generation {
            return Err(CloseStepTokenMismatch);
        }
        if self.next(now)
            != (CloseContinuationNext::Lifecycle {
                action: request.action,
                step_ordinal: request.step_ordinal,
            })
        {
            return Err(CloseStepTokenMismatch);
        }
        let seal = Arc::new(request);
        self.pending = Some(PendingStep {
            seal: Arc::clone(&seal),
            admitted: false,
        });
        Ok(CloseStepToken { seal })
    }

    pub(super) fn confirm_admitted(
        &mut self,
        token: &CloseStepToken,
        admitted_observed_at: Instant,
        generation: NonZeroU64,
    ) -> Result<(), CloseStepTokenMismatch> {
        let Some(pending) = self.pending.as_mut() else {
            return Err(CloseStepTokenMismatch);
        };
        if !Arc::ptr_eq(&pending.seal, &token.seal)
            || pending.admitted
            || token.seal.attempt_generation != generation
        {
            return Err(CloseStepTokenMismatch);
        }
        // Prepared storage and allowance exist before transport. Late confirmation
        // must account for admission even when the attempt has since expired.
        pending.admitted = true;
        self.submitted_steps = token.seal.step_ordinal.get();
        if matches!(
            token.seal.action,
            TerminalLifecycleAction::InventoryPage { .. }
        ) {
            self.last_query_at = Some(admitted_observed_at);
            self.inventory.incomplete = true;
        }
        self.backpressure_until = None;
        Ok(())
    }

    pub(super) fn reject_prequeue(
        &mut self,
        token: CloseStepToken,
        now: Instant,
        failure: CloseStepPrequeueFailure,
    ) -> Result<(), CloseStepTokenMismatch> {
        self.take_pending(&token, false)?;
        match failure {
            CloseStepPrequeueFailure::Backpressure => {
                self.backpressure_until = now.checked_add(QUERY_SPACING);
                if self.backpressure_until.is_none() || now >= self.deadline {
                    self.park_budget();
                }
            }
            CloseStepPrequeueFailure::Unavailable => {
                self.park_lifecycle(BlockedReason::WorkerUnavailable, true)
            }
            CloseStepPrequeueFailure::IdentityConflict => {
                self.park_lifecycle(BlockedReason::IdentityConflict, false)
            }
        }
        Ok(())
    }

    pub(super) fn settle_step(
        &mut self,
        token: CloseStepToken,
        now: Instant,
        generation: NonZeroU64,
        observation: CloseStepObservation<'_>,
    ) -> Result<(), CloseStepTokenMismatch> {
        if token.seal.attempt_generation != generation {
            return Err(CloseStepTokenMismatch);
        }
        self.take_pending(&token, true)?;
        let request = *token.seal;
        match observation {
            CloseStepObservation::NotDelivered => {
                self.park_lifecycle(BlockedReason::WorkerUnavailable, true)
            }
            CloseStepObservation::OutcomeUnknown(reason) => self.unknown(request.action, reason),
            CloseStepObservation::Result(result) if result.request() != request => {
                self.unknown(request.action, LifecycleUnknownReason::InvalidResponse)
            }
            CloseStepObservation::Result(result) => {
                self.settle_outcome(now, request.action, result.outcome())
            }
        }
        // Settle known facts first. Expiry cannot turn a Disposed receipt into
        // a pre-effect refusal or leave room for a second termination.
        if now >= self.deadline || self.submitted_steps >= MAXIMUM_STEPS {
            self.park_budget();
        }
        Ok(())
    }

    fn take_pending(
        &mut self,
        token: &CloseStepToken,
        admitted: bool,
    ) -> Result<(), CloseStepTokenMismatch> {
        if !self.pending.as_ref().is_some_and(|pending| {
            pending.admitted == admitted && Arc::ptr_eq(&pending.seal, &token.seal)
        }) {
            return Err(CloseStepTokenMismatch);
        }
        self.pending = None;
        Ok(())
    }

    fn settle_outcome(
        &mut self,
        now: Instant,
        action: TerminalLifecycleAction,
        outcome: TerminalLifecycleOutcome,
    ) {
        match outcome {
            TerminalLifecycleOutcome::HostUnavailableBeforeHandoff => {
                self.park_lifecycle(BlockedReason::WorkerUnavailable, true);
            }
            TerminalLifecycleOutcome::TerminateAccepted => {
                self.terminate_acknowledged = true;
                self.grace_deadline = now.checked_add(TERMINATE_GRACE);
                self.progress = Progress::Running(LifecycleStage::Grace);
                if self.grace_deadline.is_none() {
                    self.park_lifecycle(BlockedReason::CleanupIncomplete, false);
                }
            }
            TerminalLifecycleOutcome::ForceAccepted => {
                self.force_acknowledged = true;
                self.inventory = InventoryWalk::default();
                self.progress = Progress::Running(LifecycleStage::Outcome);
            }
            TerminalLifecycleOutcome::Disposed => {
                self.progress = Progress::Disposed {
                    stage: LocalStage::TargetCleanup,
                    parked: None,
                }
            }
            TerminalLifecycleOutcome::InventoryPage(page) => self.inventory_page(action, page),
            TerminalLifecycleOutcome::Rejected(reason) => {
                let (blocked, resumable) = match reason {
                    LifecycleRejection::LeaseExpired | LifecycleRejection::BindingUnavailable => {
                        (BlockedReason::WorkerUnavailable, true)
                    }
                    LifecycleRejection::WrongWorker
                    | LifecycleRejection::StaleWorkerGeneration
                    | LifecycleRejection::UnsupportedCapability
                    | LifecycleRejection::BindingFenceMismatch
                    | LifecycleRejection::HostIdentityMismatch
                    | LifecycleRejection::LifecycleConflict => {
                        (BlockedReason::IdentityConflict, false)
                    }
                };
                self.park_lifecycle(blocked, resumable);
            }
            TerminalLifecycleOutcome::HostFailure(reason) => self.park_lifecycle(
                match reason {
                    LifecycleHostFailure::HostRefused => BlockedReason::HostRefused,
                    LifecycleHostFailure::ConfirmationRequired => BlockedReason::HostRefused,
                    LifecycleHostFailure::CleanupIncomplete => BlockedReason::CleanupIncomplete,
                },
                true,
            ),
            TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::StaleCursor) => {
                self.inventory = InventoryWalk {
                    incomplete: true,
                    ..InventoryWalk::default()
                }
            }
            TerminalLifecycleOutcome::InventoryFailure(reason) => self.park_lifecycle(
                match reason {
                    LifecycleInventoryFailure::IdentityConflict => BlockedReason::IdentityConflict,
                    LifecycleInventoryFailure::StaleCursor
                    | LifecycleInventoryFailure::PageRejected
                    | LifecycleInventoryFailure::InvalidPage => BlockedReason::InventoryIncomplete,
                },
                !matches!(reason, LifecycleInventoryFailure::IdentityConflict),
            ),
            TerminalLifecycleOutcome::OutcomeUnknown(reason) => self.unknown(action, reason),
        }
    }

    fn inventory_page(&mut self, action: TerminalLifecycleAction, page: LifecycleInventoryPage) {
        let TerminalLifecycleAction::InventoryPage { cursor } = action else {
            self.unknown(action, LifecycleUnknownReason::InvalidResponse);
            return;
        };
        if cursor != self.inventory.cursor
            || (self.inventory.found.is_some() && page.target.is_some())
        {
            self.park_lifecycle(BlockedReason::IdentityConflict, false);
            return;
        }
        self.inventory.found = self.inventory.found.or(page.target);
        // Exact Exited/Failed cannot restart under this retained Host identity.
        // Inventory membership churn may reset the walk, but never this fact.
        self.settled_observed |= page.target.is_some_and(|target| is_settled(target.phase));
        if let Some(next) = page.next_cursor {
            self.inventory.cursor = Some(next);
            return;
        }
        let found = self.inventory.found;
        self.inventory = InventoryWalk::default();
        let stage = match found {
            None => LifecycleStage::Dispose,
            Some(observed)
                if matches!(
                    observed.phase,
                    LifecycleTerminalPhase::AwaitingStart
                        | LifecycleTerminalPhase::Exited { .. }
                        | LifecycleTerminalPhase::Failed
                ) =>
            {
                LifecycleStage::Dispose
            }
            Some(_) if self.force_acknowledged => LifecycleStage::Outcome,
            Some(_) if self.terminate_acknowledged => LifecycleStage::Grace,
            Some(_) => LifecycleStage::Terminate,
        };
        self.progress = Progress::Running(stage);
    }

    fn unknown(&mut self, action: TerminalLifecycleAction, reason: LifecycleUnknownReason) {
        let phase = match action {
            TerminalLifecycleAction::InventoryPage { .. }
            | TerminalLifecycleAction::InspectInventoryPage { .. } => {
                self.park_lifecycle(BlockedReason::InventoryIncomplete, true);
                return;
            }
            TerminalLifecycleAction::Terminate | TerminalLifecycleAction::TerminateIfIdle => {
                UnknownPhase::Terminate
            }
            TerminalLifecycleAction::Force => UnknownPhase::Force,
            TerminalLifecycleAction::Dispose => UnknownPhase::Dispose,
        };
        self.progress = Progress::Unknown {
            phase,
            reason: match reason {
                LifecycleUnknownReason::TransportLost => UnknownReason::TransportLost,
                LifecycleUnknownReason::Deadline => UnknownReason::Deadline,
                LifecycleUnknownReason::InvalidResponse => UnknownReason::InvalidResponse,
            },
        };
    }

    fn park_lifecycle(&mut self, reason: BlockedReason, resumable: bool) {
        if let Progress::Running(resume) = self.progress {
            self.progress = Progress::Blocked {
                resume,
                reason,
                resumable,
            };
        }
    }

    fn park_budget(&mut self) {
        match self.progress {
            Progress::Running(resume) => {
                self.progress = Progress::Blocked {
                    resume,
                    reason: if self.inventory.incomplete || resume == LifecycleStage::Observe {
                        BlockedReason::InventoryIncomplete
                    } else {
                        BlockedReason::ContinuationBudgetExhausted
                    },
                    resumable: true,
                }
            }
            Progress::Disposed {
                stage,
                parked: None,
            } => {
                self.progress = Progress::Disposed {
                    stage,
                    parked: Some((
                        match stage {
                            LocalStage::TargetCleanup => PublicationReason::TargetCleanupPending,
                            LocalStage::Publication => PublicationReason::Preparing,
                        },
                        true,
                    )),
                }
            }
            _ => {}
        }
    }

    pub(super) fn target_cleanup_failed(&mut self) -> bool {
        if matches!(
            self.progress,
            Progress::Disposed {
                stage: LocalStage::TargetCleanup,
                parked: None
            }
        ) {
            self.progress = Progress::Disposed {
                stage: LocalStage::TargetCleanup,
                parked: Some((PublicationReason::TargetCleanupPending, true)),
            };
            true
        } else {
            false
        }
    }

    pub(super) fn target_cleanup_complete(&mut self) -> bool {
        if matches!(
            self.progress,
            Progress::Disposed {
                stage: LocalStage::TargetCleanup,
                parked: None
            }
        ) {
            self.progress = Progress::Disposed {
                stage: LocalStage::Publication,
                parked: None,
            };
            true
        } else {
            false
        }
    }

    pub(super) fn publication_ready(&self) -> bool {
        matches!(
            self.progress,
            Progress::Disposed {
                stage: LocalStage::Publication,
                parked: None
            }
        ) && self.pending.is_none()
    }

    pub(super) fn publication_failed(&mut self, reason: PublicationReason) {
        if let Progress::Disposed { stage, .. } = self.progress {
            self.progress = Progress::Disposed {
                stage,
                parked: Some((
                    reason,
                    !matches!(
                        reason,
                        PublicationReason::AuthorityRetired | PublicationReason::WorkspaceRetained
                    ),
                )),
            };
        }
    }

    // Parent holds the prepared projection/credit/status together at this no-fail cut.
    pub(super) fn commit_closed(&mut self, revision: WorkbenchRevision) {
        self.progress = Progress::Closed(revision);
    }
}

fn is_settled(phase: LifecycleTerminalPhase) -> bool {
    matches!(
        phase,
        LifecycleTerminalPhase::Exited { .. } | LifecycleTerminalPhase::Failed
    )
}

fn initial_stage(phase: LifecycleTerminalPhase) -> LifecycleStage {
    match phase {
        LifecycleTerminalPhase::AwaitingStart
        | LifecycleTerminalPhase::Exited { .. }
        | LifecycleTerminalPhase::Failed => LifecycleStage::Dispose,
        LifecycleTerminalPhase::Running { .. }
        | LifecycleTerminalPhase::Finishing { .. }
        | LifecycleTerminalPhase::ContainingFailure => LifecycleStage::Terminate,
    }
}

fn public_phase(stage: LifecycleStage) -> WorkbenchTabClosePhase {
    match stage {
        LifecycleStage::Observe | LifecycleStage::Terminate => WorkbenchTabClosePhase::Queued,
        LifecycleStage::Grace => WorkbenchTabClosePhase::WaitingGrace,
        LifecycleStage::Outcome => WorkbenchTabClosePhase::WaitingOutcome,
        LifecycleStage::Dispose => WorkbenchTabClosePhase::Disposing,
    }
}

#[cfg(test)]
mod tests;
