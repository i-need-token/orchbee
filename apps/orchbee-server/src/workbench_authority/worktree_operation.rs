//! Retained Worktree intent, publication and canonical Workspace membership.
use super::{WorkbenchAuthority, WorkbenchGrant};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchProjectId, WorkbenchRequestId, WorkbenchWorkspaceId,
    WorkbenchWorktreeAction, WorkbenchWorktreeOutcome, WorkbenchWorktreeRequest,
    WorkbenchWorktreeResult, WorkerId, WorktreeAdmissionFailure, WorktreeGitEffect, WorktreeIntent,
    WorktreeOperationPhase, WorktreeOperationStatus, WorktreeProgress, WorktreeSource,
    WorktreeWorkspaceDisposition,
};
use std::{collections::BTreeMap, num::NonZeroU64};

const MAXIMUM_OPERATIONS: usize = 64;

mod restoration;
pub use restoration::{PreparedWorktreeRebind, WorktreeRebindError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeSourceTarget {
    pub directory_id: Option<orchbee_workbench_protocol::WorkbenchDirectoryId>,
    pub path: orchbee_workbench_protocol::WorkbenchDirectoryPath,
    pub worker_id: WorkerId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeOperationPlan {
    pub operation_id: WorkbenchRequestId,
    pub project_id: Option<WorkbenchProjectId>,
    pub intent: WorktreeIntent,
    pub source: WorktreeSourceTarget,
    pub source_workspace_id: Option<WorkbenchWorkspaceId>,
    pub worker_generation: NonZeroU64,
    pub attempt: u32,
    pub phase: WorktreeOperationPhase,
    pub progress: WorktreeProgress,
    /// Known Git deletion survives a later canonical publication failure.
    pub removal_complete: bool,
}
struct Record {
    plan: WorktreeOperationPlan,
    status: WorktreeOperationStatus,
    claimed: bool,
    close_hold: Option<WorkbenchRequestId>,
}
#[derive(Default)]
pub(super) struct WorktreeOperations {
    records: BTreeMap<WorkbenchRequestId, Record>,
    pub(super) associations: Vec<super::worktree_membership::Association>,
}

impl WorkbenchAuthority {
    pub(super) fn worktrees_pending(&self) -> bool {
        self.worktrees
            .records
            .values()
            .any(|record| matches!(record.status, WorktreeOperationStatus::Pending { .. }))
    }

    /// Every request is pure under this gate; the retained owner claims accepted work separately.
    pub fn execute_worktree_request(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchWorktreeRequest,
        worker: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
    ) -> WorkbenchWorktreeResult {
        let id = request.action.operation_id().clone();
        let permitted = grant.is_some_and(|grant| {
            grant.session_id == *self.session_id()
                && request.session_id == *self.session_id()
                && (matches!(request.action, WorkbenchWorktreeAction::Read { .. })
                    || (grant.mutate_navigation
                        && grant.associate_directory
                        && grant.create_terminal
                        && grant.close_terminal))
        });
        let outcome = if !permitted {
            WorkbenchWorktreeOutcome::Denied {}
        } else {
            self.worktree_action(request.action, worker, owner_available)
        };
        WorkbenchWorktreeResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            operation_id: id,
            outcome,
        }
    }

    fn worktree_action(
        &mut self,
        action: WorkbenchWorktreeAction,
        worker: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
    ) -> WorkbenchWorktreeOutcome {
        let unavailable = |reason| WorkbenchWorktreeOutcome::Unavailable { reason };
        let id = action.operation_id().clone();
        if let Some(record) = self.worktrees.records.get(&id) {
            match &action {
                WorkbenchWorktreeAction::Start {
                    project_id, intent, ..
                } if *project_id != record.plan.project_id || *intent != record.plan.intent => {
                    return WorkbenchWorktreeOutcome::Conflict {};
                }
                WorkbenchWorktreeAction::Read { .. } | WorkbenchWorktreeAction::Start { .. } => {
                    return record.outcome();
                }
                WorkbenchWorktreeAction::Resume {
                    expected_attempt, ..
                } => {
                    if *expected_attempt != record.plan.attempt {
                        return WorkbenchWorktreeOutcome::Conflict {};
                    }
                    if !matches!(
                        record.status,
                        WorktreeOperationStatus::Failed {
                            resumable: true,
                            ..
                        }
                    ) {
                        return record.outcome();
                    }
                }
            }
        } else if !matches!(action, WorkbenchWorktreeAction::Start { .. }) {
            return WorkbenchWorktreeOutcome::NotFound {};
        }
        if self.retired || !owner_available {
            return unavailable(WorktreeAdmissionFailure::ServiceUnavailable);
        }
        if self.worktrees_pending() || self.workspace_group_close_active() {
            return unavailable(WorktreeAdmissionFailure::Busy);
        }
        let Some((worker_id, generation)) = worker else {
            return unavailable(WorktreeAdmissionFailure::WorkerUnavailable);
        };
        match action {
            WorkbenchWorktreeAction::Start {
                project_id, intent, ..
            } => {
                if self.worktrees.records.len() >= MAXIMUM_OPERATIONS {
                    return unavailable(WorktreeAdmissionFailure::Capacity);
                }
                let Some((source, source_workspace_id)) =
                    self.worktree_source(&project_id, &intent, &worker_id)
                else {
                    return unavailable(WorktreeAdmissionFailure::SourceUnavailable);
                };
                let phase = initial_phase(&intent);
                let progress = WorktreeProgress {
                    path: match &intent {
                        WorktreeIntent::Create { path, .. } => path.clone(),
                        WorktreeIntent::Remove { .. } => Some(source.path.clone()),
                        WorktreeIntent::Open { .. } => None,
                    },
                    checkout: None,
                    workspace: match &intent {
                        WorktreeIntent::Remove { workspace_id, .. } => {
                            WorktreeWorkspaceDisposition::Open {
                                workspace_id: workspace_id.clone(),
                            }
                        }
                        WorktreeIntent::Create { .. } | WorktreeIntent::Open { .. } => {
                            WorktreeWorkspaceDisposition::Unassociated {}
                        }
                    },
                    git_effect: WorktreeGitEffect::None,
                    restoration: None,
                };
                let plan = WorktreeOperationPlan {
                    operation_id: id.clone(),
                    project_id,
                    intent,
                    source,
                    source_workspace_id,
                    worker_generation: generation,
                    attempt: 1,
                    phase,
                    progress: progress.clone(),
                    removal_complete: false,
                };
                let record = Record {
                    plan,
                    status: WorktreeOperationStatus::Pending { phase, progress },
                    claimed: false,
                    close_hold: None,
                };
                let outcome = record.outcome();
                self.worktrees.records.insert(id, record);
                outcome
            }
            WorkbenchWorktreeAction::Resume { .. } => {
                let Some(record) = self.worktrees.records.get_mut(&id) else {
                    return WorkbenchWorktreeOutcome::NotFound {};
                };
                if record.plan.source.worker_id != worker_id
                    || record.plan.worker_generation != generation
                {
                    return unavailable(WorktreeAdmissionFailure::WorkerUnavailable);
                }
                if record.plan.attempt
                    >= orchbee_workbench_protocol::MAXIMUM_WORKTREE_OPERATION_ATTEMPTS
                {
                    return unavailable(WorktreeAdmissionFailure::Capacity);
                }
                let WorktreeOperationStatus::Failed { progress, .. } = &record.status else {
                    return record.outcome();
                };
                record.plan.progress = progress.clone();
                record.plan.phase = if record.plan.removal_complete {
                    WorktreeOperationPhase::Removing
                } else {
                    match &progress.workspace {
                        WorktreeWorkspaceDisposition::Closed { .. } => {
                            WorktreeOperationPhase::InspectingRemove
                        }
                        WorktreeWorkspaceDisposition::Closing { .. }
                            if matches!(record.plan.intent, WorktreeIntent::Remove { .. }) =>
                        {
                            WorktreeOperationPhase::InspectingRemove
                        }
                        WorktreeWorkspaceDisposition::Closing { .. } => {
                            WorktreeOperationPhase::Closing
                        }
                        _ if matches!(record.plan.intent, WorktreeIntent::Remove { .. }) => {
                            WorktreeOperationPhase::InspectingRemove
                        }
                        _ if progress.git_effect != WorktreeGitEffect::None
                            && progress.path.is_some() =>
                        {
                            WorktreeOperationPhase::Opening
                        }
                        // This observer is scoped to one Worktree intent and its corresponding progress or retained receipt.
                        _ => initial_phase(&record.plan.intent),
                    }
                };
                record.plan.progress.restoration = None;
                record.plan.attempt += 1;
                record.claimed = false;
                record.status = WorktreeOperationStatus::Pending {
                    phase: record.plan.phase,
                    progress: record.plan.progress.clone(),
                };
                record.outcome()
            }
            WorkbenchWorktreeAction::Read { .. } => WorkbenchWorktreeOutcome::NotFound {},
        }
    }

    fn worktree_source(
        &self,
        project_id: &Option<WorkbenchProjectId>,
        intent: &WorktreeIntent,
        worker_id: &WorkerId,
    ) -> Option<(WorktreeSourceTarget, Option<WorkbenchWorkspaceId>)> {
        if project_id
            .as_ref()
            .is_some_and(|id| self.snapshot().navigation.project(id).is_none())
        {
            return None;
        }
        let workspace_id = match intent {
            WorktreeIntent::Create { source, .. } | WorktreeIntent::Open { source, .. } => {
                match source {
                    WorktreeSource::Workspace { workspace_id } => workspace_id,
                    WorktreeSource::Directory { path } => {
                        return Some((
                            WorktreeSourceTarget {
                                directory_id: None,
                                worker_id: worker_id.clone(),
                                path: path.clone(),
                            },
                            None,
                        ));
                    }
                }
            }
            WorktreeIntent::Remove { workspace_id, .. } => workspace_id,
        };
        let workspace = self.snapshot().navigation.workspace(workspace_id)?;
        if self.open_close_fences_context(&workspace.terminal_context_id) {
            return None;
        }
        let directory = self
            .snapshot()
            .navigation
            .workspace_directory(workspace_id)?;
        (directory.worker_id == *worker_id).then(|| {
            (
                WorktreeSourceTarget {
                    directory_id: Some(directory.directory_id.clone()),
                    path: directory.path.clone(),
                    worker_id: directory.worker_id.clone(),
                },
                Some(workspace_id.clone()),
            )
        })
    }

    pub fn claim_worktree_operation(&mut self) -> Option<WorktreeOperationPlan> {
        let record = self.worktrees.records.values_mut().find(|record| {
            !record.claimed && matches!(record.status, WorktreeOperationStatus::Pending { .. })
        })?;
        record.claimed = true;
        Some(record.plan.clone())
    }

    /// An attempt token cannot settle a newer retry or alter a terminal receipt.
    pub fn publish_worktree_operation(
        &mut self,
        plan: &WorktreeOperationPlan,
        status: WorktreeOperationStatus,
    ) -> bool {
        let Some(record) = self.worktrees.records.get_mut(&plan.operation_id) else {
            return false;
        };
        if record.plan.project_id != plan.project_id
            || record.plan.intent != plan.intent
            || record.plan.source != plan.source
            || record.plan.source_workspace_id != plan.source_workspace_id
            || record.plan.worker_generation != plan.worker_generation
        {
            return false;
        }
        if record.plan.attempt != plan.attempt
            || !record.claimed
            || !matches!(record.status, WorktreeOperationStatus::Pending { .. })
        {
            return false;
        }
        let WorktreeOperationStatus::Pending { phase, .. } = &record.status else {
            return false;
        };
        let valid_transition = match &status {
            WorktreeOperationStatus::Pending { phase: next, .. } => {
                phase == next
                    || matches!(
                        (phase, next),
                        (
                            WorktreeOperationPhase::ObservingSource,
                            WorktreeOperationPhase::Creating | WorktreeOperationPhase::Opening
                        ) | (
                            WorktreeOperationPhase::Creating | WorktreeOperationPhase::Opening,
                            WorktreeOperationPhase::Attaching
                        ) | (
                            WorktreeOperationPhase::InspectingRemove,
                            WorktreeOperationPhase::Closing | WorktreeOperationPhase::Removing
                        ) | (
                            WorktreeOperationPhase::Closing,
                            WorktreeOperationPhase::Removing
                        )
                    )
            }
            WorktreeOperationStatus::Opened { .. } => *phase == WorktreeOperationPhase::Attaching,
            WorktreeOperationStatus::Removed { .. } => *phase == WorktreeOperationPhase::Removing,
            WorktreeOperationStatus::Failed { .. } => true,
        };
        if !valid_transition {
            return false;
        }
        let result = WorkbenchWorktreeResult {
            protocol_version: ProtocolVersion,
            request_id: plan.operation_id.clone(),
            operation_id: plan.operation_id.clone(),
            outcome: WorkbenchWorktreeOutcome::Available {
                project_id: plan.project_id.clone(),
                attempt: plan.attempt,
                status: Box::new(status.clone()),
            },
        };
        if result.validate().is_err() {
            return false;
        }
        record.status = status;
        let event = match (&plan.intent, &record.status) {
            (
                WorktreeIntent::Create { .. },
                WorktreeOperationStatus::Opened { workspace_id, .. },
            ) => Some((
                orchbee_workbench_protocol::SemanticEventKind::WorktreeCreated,
                Some(workspace_id.clone()),
            )),
            (WorktreeIntent::Open { .. }, WorktreeOperationStatus::Opened { workspace_id, .. }) => {
                Some((
                    orchbee_workbench_protocol::SemanticEventKind::WorktreeOpened,
                    Some(workspace_id.clone()),
                ))
            }
            (_, WorktreeOperationStatus::Removed { workspace_id, .. }) => Some((
                orchbee_workbench_protocol::SemanticEventKind::WorktreeRemoved,
                Some(workspace_id.clone()),
            )),
            // This observer is scoped to one Worktree intent and its corresponding progress or retained receipt.
            _ => None,
        };
        if let Some((kind, workspace)) = event {
            self.event_journal().publish(
                self.snapshot().revision,
                workspace,
                kind,
                orchbee_workbench_protocol::SemanticEventData::Worktree {
                    result: Box::new(result),
                },
            );
        }
        true
    }

    /// Called before releasing the same Session guard that admitted this exact Close.
    pub fn hold_worktree_close(
        &mut self,
        plan: &WorktreeOperationPlan,
        close: &WorkbenchRequestId,
    ) -> bool {
        let WorktreeIntent::Remove { workspace_id, .. } = &plan.intent else {
            return false;
        };
        if !self.terminal_close_matches_workspace(close, workspace_id) {
            return false;
        }
        let Some(record) = self.worktrees.records.get_mut(&plan.operation_id) else {
            return false;
        };
        if !record.matches_attempt(plan)
            || record.plan.removal_complete
            || record.close_hold.as_ref().is_some_and(|held| held != close)
        {
            return false;
        }
        record.close_hold = Some(close.clone());
        true
    }

    pub(super) fn worktree_holds_close(&self, close: &WorkbenchRequestId) -> bool {
        self.worktrees
            .records
            .values()
            .any(|record| record.close_hold.as_ref() == Some(close))
    }

    pub(super) fn worktree_close_join_allowed(&self, close: &WorkbenchRequestId) -> bool {
        self.worktrees.records.values().all(|record| {
            record.close_hold.as_ref() != Some(close)
                || matches!(record.status, WorktreeOperationStatus::Failed { .. })
        })
    }

    /// A committed explicit Workspace Close can release a failed operation's hold without Git.
    pub(super) fn release_failed_worktree_close(&mut self, close: &WorkbenchRequestId) {
        for record in self.worktrees.records.values_mut() {
            if record.close_hold.as_ref() == Some(close)
                && matches!(record.status, WorktreeOperationStatus::Failed { .. })
            {
                record.close_hold = None;
            }
        }
    }

    pub fn worktree_close_quiesced(
        &self,
        plan: &WorktreeOperationPlan,
        close: &WorkbenchRequestId,
    ) -> bool {
        let Some(record) = self.worktrees.records.get(&plan.operation_id) else {
            return false;
        };
        record.matches_attempt(plan)
            && record.close_hold.as_ref() == Some(close)
            && self.terminal_close_publication_ready(close)
    }

    /// The retained driver supplies known successful Git completion; never inferred from a failed job.
    pub fn complete_worktree_removal(&mut self, plan: &mut WorktreeOperationPlan) -> bool {
        let Some(record) = self.worktrees.records.get_mut(&plan.operation_id) else {
            return false;
        };
        if !record.matches_attempt(plan)
            || !matches!(plan.intent, WorktreeIntent::Remove { .. })
            || !matches!(
                record.status,
                WorktreeOperationStatus::Pending {
                    phase: WorktreeOperationPhase::Removing,
                    ..
                }
            )
        {
            return false;
        }
        record.plan.removal_complete = true;
        record.close_hold = None;
        plan.removal_complete = true;
        true
    }

    pub fn worktree_source_matches(&self, plan: &WorktreeOperationPlan) -> bool {
        if self.retired {
            return false;
        }
        if matches!(
            plan.progress.workspace,
            WorktreeWorkspaceDisposition::Closed { .. }
        ) {
            return true;
        }
        self.worktree_source(&plan.project_id, &plan.intent, &plan.source.worker_id)
            .is_some_and(|(directory, workspace)| {
                directory == plan.source && workspace == plan.source_workspace_id
            })
    }

    pub fn worktree_workspace_available(&self, workspace_id: &WorkbenchWorkspaceId) -> bool {
        self.snapshot()
            .navigation
            .workspace(workspace_id)
            .is_some_and(|workspace| {
                !self.open_close_fences_context(&workspace.terminal_context_id)
            })
    }

    pub(super) fn worktree_removal_active(&self) -> bool {
        self.worktrees.records.values().any(|record| {
            matches!(record.plan.intent, WorktreeIntent::Remove { .. })
                && matches!(record.status, WorktreeOperationStatus::Pending { .. })
        })
    }
}
impl Record {
    fn matches_attempt(&self, plan: &WorktreeOperationPlan) -> bool {
        self.claimed
            && matches!(self.status, WorktreeOperationStatus::Pending { .. })
            && self.plan.operation_id == plan.operation_id
            && self.plan.attempt == plan.attempt
            && self.plan.project_id == plan.project_id
            && self.plan.intent == plan.intent
            && self.plan.source == plan.source
            && self.plan.source_workspace_id == plan.source_workspace_id
            && self.plan.worker_generation == plan.worker_generation
    }
    fn outcome(&self) -> WorkbenchWorktreeOutcome {
        WorkbenchWorktreeOutcome::Available {
            project_id: self.plan.project_id.clone(),
            attempt: self.plan.attempt,
            status: Box::new(self.status.clone()),
        }
    }
}
fn initial_phase(intent: &WorktreeIntent) -> WorktreeOperationPhase {
    match intent {
        WorktreeIntent::Create { .. } => WorktreeOperationPhase::ObservingSource,
        WorktreeIntent::Open { .. } => WorktreeOperationPhase::ObservingSource,
        WorktreeIntent::Remove { .. } => WorktreeOperationPhase::InspectingRemove,
    }
}
pub(super) fn same_path(left: &str, right: &str) -> bool {
    orchbee_workbench_protocol::same_windows_path_spelling(left, right)
}
