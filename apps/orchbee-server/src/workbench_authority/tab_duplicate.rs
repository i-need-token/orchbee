//! Retained Tab duplication admission and ordered copy facts. Every copy is an ordinary duplicate
//! Tab creation, which owns its membership commit and terminal launch; this owner freezes the
//! scope, keeps each member's copy monotone and proves completion before publishing it.
use super::{WorkbenchAuthority, WorkbenchGrant};
use orchbee_workbench_protocol::{
    MAXIMUM_TABS_PER_CONTEXT, ProtocolVersion, TabDuplicateAction as Action,
    TabDuplicateFailure as Failure, TabDuplicateMember, TabDuplicateOutcome as Outcome,
    TabDuplicateRejection as Rejection, TabDuplicateStatus as Status,
    TabDuplicateUnavailable as Unavailable, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabDuplicateRequest, WorkbenchTabDuplicateResult, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalGeometry,
};
use std::collections::BTreeMap;

/// Retained operations. At capacity the oldest settled one gives way; a pending one never does.
const MAXIMUM_OPERATIONS: usize = 64;

/// The owned driver's copy of one claimed operation; publication re-proves its frozen scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabDuplicatePlan {
    pub operation_id: WorkbenchRequestId,
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub geometry: WorkbenchTerminalGeometry,
    pub members: Vec<TabDuplicateMember>,
}

struct Record {
    plan: TabDuplicatePlan,
    expected_revision: WorkbenchRevision,
    status: Status,
    claimed: bool,
    sequence: u64,
}

#[derive(Default)]
pub(super) struct TabDuplicates {
    records: BTreeMap<WorkbenchRequestId, Record>,
    admitted: u64,
}

impl WorkbenchAuthority {
    /// Answers a Read, replays a known Start, or retains a new Start that passed every guard for
    /// the one owned driver. Nothing is created here.
    pub fn tab_duplicate(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchTabDuplicateRequest,
        owner_available: bool,
    ) -> WorkbenchTabDuplicateResult {
        let WorkbenchTabDuplicateRequest {
            request_id,
            session_id,
            action,
            ..
        } = request;
        let operation_id = action.operation_id().clone();
        let start = matches!(action, Action::Start { .. });
        let outcome = if !grant.is_some_and(|grant| {
            grant.session_id == *self.session_id()
                && session_id == *self.session_id()
                && (!start || (grant.mutate_navigation && grant.create_terminal))
        }) {
            Outcome::Denied {}
        } else {
            match self.plan_tab_duplicate(action, owner_available) {
                Ok((expected_revision, plan)) => self.admit_tab_duplicate(expected_revision, plan),
                Err(outcome) => outcome,
            }
        };
        result(request_id, operation_id, outcome)
    }

    fn plan_tab_duplicate(
        &self,
        action: Action,
        owner_available: bool,
    ) -> Result<(WorkbenchRevision, TabDuplicatePlan), Outcome> {
        let (operation_id, terminal_context_id, tab_ids, expected_revision, geometry) = match action
        {
            Action::Read { operation_id } => {
                return Err(self
                    .tab_duplicates
                    .records
                    .get(&operation_id)
                    .map_or(Outcome::NotFound {}, Record::outcome));
            }
            Action::Start {
                operation_id,
                terminal_context_id,
                tab_ids,
                expected_revision,
                geometry,
            } => (
                operation_id,
                terminal_context_id,
                tab_ids,
                expected_revision,
                geometry,
            ),
        };
        if let Some(record) = self.tab_duplicates.records.get(&operation_id) {
            let equal = record.plan.terminal_context_id == terminal_context_id
                && record.plan.geometry == geometry
                && record.expected_revision == expected_revision
                && record
                    .plan
                    .members
                    .iter()
                    .map(|member| &member.tab_id)
                    .eq(&tab_ids);
            return Err(if equal {
                record.outcome()
            } else {
                Outcome::Conflict {}
            });
        }
        let unavailable = |reason| Outcome::Unavailable { reason };
        if self.retired || !owner_available {
            return Err(unavailable(Unavailable::ServiceUnavailable));
        }
        let snapshot = self.snapshot();
        let rejected = |reason| Outcome::Rejected {
            revision: snapshot.revision,
            reason,
        };
        if expected_revision != snapshot.revision {
            return Err(rejected(Rejection::RevisionMismatch));
        }
        let workspace = snapshot
            .navigation
            .workspaces
            .iter()
            .find(|workspace| workspace.terminal_context_id == terminal_context_id);
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == terminal_context_id);
        let (Some(workspace), Some(context)) = (workspace, context) else {
            return Err(rejected(Rejection::TerminalContextUnavailable));
        };
        let mut positions = Vec::with_capacity(tab_ids.len());
        for tab_id in &tab_ids {
            let Some(position) = context.tabs.iter().position(|tab| tab.tab_id == *tab_id) else {
                return Err(rejected(Rejection::TabUnavailable));
            };
            positions.push(position);
        }
        if positions.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(rejected(Rejection::TabOrderMismatch));
        }
        if context.tabs.len() + tab_ids.len() > MAXIMUM_TABS_PER_CONTEXT {
            return Err(rejected(Rejection::ResourceLimit));
        }
        // Every copy would be refused while a Close fences the context or its Workspace closes.
        if self.workspace_group_close_contains(&workspace.workspace_id)
            || self.open_close_fences_context(&terminal_context_id)
        {
            return Err(unavailable(Unavailable::Busy));
        }
        if self.tab_duplicates.records.len() >= MAXIMUM_OPERATIONS
            && self
                .tab_duplicates
                .records
                .values()
                .all(|record| matches!(record.status, Status::Pending {}))
        {
            return Err(unavailable(Unavailable::Capacity));
        }
        Ok((
            expected_revision,
            TabDuplicatePlan {
                operation_id,
                terminal_context_id,
                geometry,
                members: tab_ids
                    .into_iter()
                    .map(|tab_id| TabDuplicateMember { tab_id, copy: None })
                    .collect(),
            },
        ))
    }

    fn admit_tab_duplicate(
        &mut self,
        expected_revision: WorkbenchRevision,
        plan: TabDuplicatePlan,
    ) -> Outcome {
        let duplicates = &mut self.tab_duplicates;
        if duplicates.records.len() >= MAXIMUM_OPERATIONS {
            let oldest = duplicates
                .records
                .iter()
                .filter(|(_, record)| !matches!(record.status, Status::Pending {}))
                .min_by_key(|(_, record)| record.sequence)
                .map(|(id, _)| id.clone());
            let Some(oldest) = oldest else {
                return Outcome::Unavailable {
                    reason: Unavailable::Capacity,
                };
            };
            duplicates.records.remove(&oldest);
        }
        duplicates.admitted += 1;
        let record = Record {
            plan,
            expected_revision,
            status: Status::Pending {},
            claimed: false,
            sequence: duplicates.admitted,
        };
        let outcome = record.outcome();
        duplicates
            .records
            .insert(record.plan.operation_id.clone(), record);
        outcome
    }

    /// Claims the oldest unclaimed pending operation for the one owned driver.
    pub fn claim_tab_duplicate(&mut self) -> Option<TabDuplicatePlan> {
        let record = self
            .tab_duplicates
            .records
            .values_mut()
            .filter(|record| !record.claimed && matches!(record.status, Status::Pending {}))
            .min_by_key(|record| record.sequence)?;
        record.claimed = true;
        Some(record.plan.clone())
    }

    /// Publishes a claimed pending operation's progress or end. A copy keeps its Tab once
    /// committed, a settled terminal outcome is final, and Duplicated requires every copy started.
    pub fn publish_tab_duplicate(&mut self, plan: &TabDuplicatePlan, status: Status) -> bool {
        let Some(record) = self.tab_duplicates.records.get_mut(&plan.operation_id) else {
            return false;
        };
        if !record.claimed
            || !matches!(record.status, Status::Pending {})
            || record.plan.terminal_context_id != plan.terminal_context_id
            || record.plan.geometry != plan.geometry
            || record.plan.members.len() != plan.members.len()
            || !record
                .plan
                .members
                .iter()
                .zip(&plan.members)
                .all(|(old, new)| advances(old, new))
        {
            return false;
        }
        let candidate = Record {
            plan: plan.clone(),
            expected_revision: record.expected_revision,
            status,
            claimed: true,
            sequence: record.sequence,
        };
        let published = result(
            plan.operation_id.clone(),
            plan.operation_id.clone(),
            candidate.outcome(),
        );
        if published.validate().is_err() {
            return false;
        }
        *record = candidate;
        true
    }

    /// Ends a claimed operation whose driver could not publish truthfully, keeping what it
    /// published.
    pub fn abandon_tab_duplicate(&mut self, operation_id: &WorkbenchRequestId) {
        if let Some(record) = self.tab_duplicates.records.get_mut(operation_id)
            && record.claimed
            && matches!(record.status, Status::Pending {})
        {
            record.status = Status::Failed {
                reason: Failure::Internal,
            };
        }
    }

    /// The label a copy of `tab_id` is requested under, while that Tab and its terminal context
    /// are still present. Duplicate creation copies the source's own label and naming mode.
    #[must_use]
    pub fn tab_duplicate_source(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> Option<WorkbenchLabel> {
        let snapshot = self.snapshot();
        if !snapshot
            .navigation
            .workspaces
            .iter()
            .any(|workspace| workspace.terminal_context_id == *context_id)
        {
            return None;
        }
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == *context_id)?;
        let position = context.tabs.iter().position(|tab| tab.tab_id == *tab_id)?;
        WorkbenchLabel::parse(&context.tabs[position].display_label(position)).ok()
    }
}

impl Record {
    fn outcome(&self) -> Outcome {
        Outcome::Available {
            terminal_context_id: self.plan.terminal_context_id.clone(),
            status: self.status.clone(),
            members: self.plan.members.clone(),
        }
    }
}

fn result(
    request_id: WorkbenchRequestId,
    operation_id: WorkbenchRequestId,
    outcome: Outcome,
) -> WorkbenchTabDuplicateResult {
    WorkbenchTabDuplicateResult {
        protocol_version: ProtocolVersion,
        request_id,
        operation_id,
        outcome,
    }
}

/// Whether `new` only moves `old` forward: the member stays, a committed copy keeps its Tab, its
/// revision never goes back, and a settled terminal outcome never changes.
fn advances(old: &TabDuplicateMember, new: &TabDuplicateMember) -> bool {
    if old.tab_id != new.tab_id {
        return false;
    }
    match (&old.copy, &new.copy) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(old), Some(new)) => {
            old.tab_id == new.tab_id
                && old.revision <= new.revision
                && (old.terminal.is_none() || old.terminal == new.terminal)
        }
    }
}

#[cfg(test)]
mod tests;
