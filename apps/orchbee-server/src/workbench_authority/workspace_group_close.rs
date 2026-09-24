//! Retained group admission and ordered child observations; no runtime or Git effects occur here.
use super::{WorkbenchAuthority, WorkbenchGrant, WorktreeSourceTarget};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS, MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS, ProtocolVersion,
    WorkbenchProjectId, WorkbenchRequestId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceGroupCloseRequest, WorkbenchWorkspaceGroupCloseResult, WorkbenchWorkspaceId,
    WorkerId, WorkspaceGroupCloseAction as Action, WorkspaceGroupCloseMember,
    WorkspaceGroupCloseOutcome as Outcome, WorkspaceGroupCloseStatus as Status,
    WorkspaceGroupCloseUnavailable as Unavailable,
};
use std::{collections::BTreeMap, num::NonZeroU64};
const MAXIMUM_OPERATIONS: usize = 64;

/// Frozen canonical scope and compact child truth for the owned submitter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGroupCloseMemberPlan {
    pub member: WorkspaceGroupCloseMember,
    pub context_id: WorkbenchTerminalContextId,
    pub directory: Option<WorktreeSourceTarget>,
}
/// One attempt token; publication re-proves immutable membership and generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGroupClosePlan {
    pub operation_id: WorkbenchRequestId,
    pub project_id: WorkbenchProjectId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub worker: Option<(WorkerId, NonZeroU64)>,
    pub attempt: u32,
    pub members: Vec<WorkspaceGroupCloseMemberPlan>,
}
struct Record {
    plan: WorkspaceGroupClosePlan,
    status: Status,
    claimed: bool,
}
#[derive(Default)]
pub(super) struct WorkspaceGroupCloses {
    records: BTreeMap<WorkbenchRequestId, Record>,
}

impl WorkbenchAuthority {
    /// Admits only exact authorized scope; reads and equal starts remain pure under exhausted capacity.
    pub fn execute_workspace_group_close(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchWorkspaceGroupCloseRequest,
        worker: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
    ) -> WorkbenchWorkspaceGroupCloseResult {
        let operation_id = request.action.operation_id().clone();
        let outcome = if !grant.is_some_and(|grant| {
            grant.site_id == *self.site_id()
                && request.site_id == *self.site_id()
                && (matches!(request.action, Action::Read { .. }) || grant.close_terminal)
        }) {
            Outcome::Denied {}
        } else {
            self.workspace_group_action(request.action, worker, owner_available)
        };
        WorkbenchWorkspaceGroupCloseResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            operation_id,
            outcome,
        }
    }
    fn workspace_group_action(
        &mut self,
        action: Action,
        worker: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
    ) -> Outcome {
        let id = action.operation_id().clone();
        if matches!(action, Action::Read { .. }) {
            return self
                .workspace_groups
                .records
                .get(&id)
                .map_or(Outcome::NotFound {}, Record::outcome);
        }
        if let Action::Start {
            project_id,
            workspace_id,
            ..
        } = &action
            && let Some(record) = self.workspace_groups.records.get(&id)
        {
            return if record.plan.project_id == *project_id
                && record.plan.workspace_id == *workspace_id
            {
                record.outcome()
            } else {
                Outcome::Conflict {}
            };
        }
        let unavailable = |reason| Outcome::Unavailable { reason };
        if self.retired || !owner_available {
            return unavailable(Unavailable::ServiceUnavailable);
        }
        if self.workspace_group_close_active() || self.worktrees_pending() {
            return unavailable(Unavailable::Busy);
        }
        match action {
            Action::Start {
                operation_id,
                project_id,
                workspace_id,
            } => {
                if self.workspace_groups.records.len() >= MAXIMUM_OPERATIONS {
                    return unavailable(Unavailable::Capacity);
                }
                let selection = match self.worktree_group_members(&project_id, &workspace_id) {
                    Ok(selection) => selection,
                    Err(reason) => return unavailable(reason),
                };
                if selection.members.len() > MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS {
                    return unavailable(Unavailable::Capacity);
                }
                if selection.worker.is_some() && selection.worker != worker {
                    return unavailable(Unavailable::SourceChanged);
                }
                let mut members = vec![];
                for (project_id, workspace_id) in selection.members {
                    let Some(project) = self
                        .snapshot()
                        .navigation
                        .projects
                        .iter()
                        .find(|project| project.project_id == project_id)
                    else {
                        return unavailable(Unavailable::SourceChanged);
                    };
                    let Some(workspace) = project
                        .workspaces
                        .iter()
                        .find(|workspace| workspace.workspace_id == workspace_id)
                    else {
                        return unavailable(Unavailable::SourceChanged);
                    };
                    if self.open_close_fences_context(&workspace.terminal_context_id) {
                        return unavailable(Unavailable::Busy);
                    }
                    let directory = self.workspace_directory_target(&project_id, &workspace_id);
                    if directory.as_ref().is_some_and(|directory| {
                        !worker
                            .as_ref()
                            .is_some_and(|(id, _)| *id == directory.worker_id)
                    }) {
                        return unavailable(Unavailable::WorkerUnavailable);
                    }
                    members.push(WorkspaceGroupCloseMemberPlan {
                        member: WorkspaceGroupCloseMember {
                            project_id,
                            workspace_id,
                            label: match orchbee_workbench_protocol::WorkbenchLabel::parse(
                                &workspace.label,
                            ) {
                                Ok(label) => label,
                                Err(_) => return unavailable(Unavailable::ServiceUnavailable),
                            },
                            close: None,
                        },
                        context_id: workspace.terminal_context_id.clone(),
                        directory,
                    });
                }
                let plan = WorkspaceGroupClosePlan {
                    operation_id: operation_id.clone(),
                    project_id,
                    workspace_id,
                    worker,
                    attempt: 1,
                    members,
                };
                let record = Record {
                    plan,
                    status: Status::Pending {},
                    claimed: false,
                };
                let outcome = record.outcome();
                self.workspace_groups.records.insert(operation_id, record);
                outcome
            }
            Action::Resume {
                expected_attempt, ..
            } => {
                let Some(record) = self.workspace_groups.records.get_mut(&id) else {
                    return Outcome::NotFound {};
                };
                if record.plan.attempt != expected_attempt {
                    return unavailable(Unavailable::StaleAttempt);
                }
                if !matches!(
                    record.status,
                    Status::Failed {
                        resumable: true,
                        ..
                    }
                ) {
                    return unavailable(Unavailable::NotResumable);
                }
                if record.plan.attempt >= MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS {
                    return unavailable(Unavailable::AttemptExhausted);
                }
                if record.plan.worker != worker {
                    return unavailable(Unavailable::SourceChanged);
                }
                record.plan.attempt += 1;
                record.claimed = false;
                record.status = Status::Pending {};
                record.outcome()
            }
            Action::Read { .. } => unreachable!("read handled before admission"),
        }
    }
    /// Claims a single generation for the owned continuation; callers cannot start two submitters.
    pub fn claim_workspace_group_close(&mut self) -> Option<WorkspaceGroupClosePlan> {
        let record = self
            .workspace_groups
            .records
            .values_mut()
            .find(|record| !record.claimed && matches!(record.status, Status::Pending {}))?;
        record.claimed = true;
        Some(record.plan.clone())
    }
    /// Publishes only monotone child identity and known Closed facts for the current claimed attempt.
    pub fn publish_workspace_group_close(
        &mut self,
        plan: &WorkspaceGroupClosePlan,
        status: Status,
    ) -> bool {
        let Some(record) = self.workspace_groups.records.get_mut(&plan.operation_id) else {
            return false;
        };
        if !record.claimed
            || !matches!(record.status, Status::Pending {})
            || !same_plan(&record.plan, plan)
        {
            return false;
        }
        for (old, new) in record.plan.members.iter().zip(&plan.members) {
            if let Some(old) = &old.member.close {
                let Some(new) = &new.member.close else {
                    return false;
                };
                if old.request_id != new.request_id
                    || old.observation.as_ref().is_some_and(|old_observation| {
                        new.observation.as_ref().is_none_or(|new_observation| {
                            old_observation.attempt_generation > new_observation.attempt_generation
                        })
                    })
                    || (old.is_closed() && old != new)
                {
                    return false;
                }
            }
        }
        let candidate = Record {
            plan: plan.clone(),
            status,
            claimed: true,
        };
        let result = WorkbenchWorkspaceGroupCloseResult {
            protocol_version: ProtocolVersion,
            request_id: plan.operation_id.clone(),
            operation_id: plan.operation_id.clone(),
            outcome: candidate.outcome(),
        };
        if result.validate().is_err() {
            return false;
        }
        *record = candidate;
        true
    }
    /// Proves the pending member token and canonical scope before the group may submit a child Close.
    pub(crate) fn workspace_group_member_matches(
        &self,
        operation_id: &WorkbenchRequestId,
        attempt: u32,
        index: usize,
        project_id: &WorkbenchProjectId,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> bool {
        let Some(record) = self.workspace_groups.records.get(operation_id) else {
            return false;
        };
        if !record.claimed
            || record.plan.attempt != attempt
            || !matches!(record.status, Status::Pending {})
        {
            return false;
        }
        let Some(member) = record.plan.members.get(index) else {
            return false;
        };
        if member.member.project_id != *project_id
            || member.member.workspace_id != *workspace_id
            || member.member.close.is_some()
            || record.plan.members.iter().take(index).any(|member| {
                !member
                    .member
                    .close
                    .as_ref()
                    .is_some_and(|child| child.is_closed())
            })
        {
            return false;
        }
        self.snapshot()
            .navigation
            .projects
            .iter()
            .find(|project| project.project_id == *project_id)
            .and_then(|project| {
                project
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.workspace_id == *workspace_id)
            })
            .is_some_and(|workspace| workspace.terminal_context_id == member.context_id)
            && self.workspace_directory_target(project_id, workspace_id) == member.directory
    }
    pub(super) fn workspace_group_close_active(&self) -> bool {
        self.workspace_groups
            .records
            .values()
            .any(|record| matches!(record.status, Status::Pending {}))
    }
}
impl Record {
    fn outcome(&self) -> Outcome {
        Outcome::Available {
            project_id: self.plan.project_id.clone(),
            workspace_id: self.plan.workspace_id.clone(),
            attempt: self.plan.attempt,
            status: self.status.clone(),
            members: self
                .plan
                .members
                .iter()
                .map(|member| member.member.clone())
                .collect(),
        }
    }
}
fn same_plan(left: &WorkspaceGroupClosePlan, right: &WorkspaceGroupClosePlan) -> bool {
    left.operation_id == right.operation_id
        && left.project_id == right.project_id
        && left.workspace_id == right.workspace_id
        && left.worker == right.worker
        && left.attempt == right.attempt
        && left.members.len() == right.members.len()
        && left
            .members
            .iter()
            .zip(&right.members)
            .all(|(left, right)| {
                left.context_id == right.context_id
                    && left.directory == right.directory
                    && left.member.project_id == right.member.project_id
                    && left.member.workspace_id == right.member.workspace_id
                    && left.member.label == right.member.label
            })
}
