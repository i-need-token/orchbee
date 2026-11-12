//! Retained Tab set admission and ordered child observations. Every member closes through its own
//! retained Tab Close, which owns all runtime effects; this owner freezes the scope, proves each
//! child submission and keeps the children's observations monotone.
use super::{WorkbenchAuthority, WorkbenchGrant, WorkbenchTerminalTab};
use orchbee_workbench_protocol::{
    ProtocolVersion, TabSetCloseAction as Action, TabSetCloseCondition as Condition,
    TabSetCloseFailure as Failure, TabSetCloseMember, TabSetCloseOutcome as Outcome,
    TabSetCloseRejection as Rejection, TabSetCloseStatus as Status,
    TabSetCloseUnavailable as Unavailable, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTabSetCloseRequest, WorkbenchTabSetCloseResult,
    WorkbenchTerminalContextId, WorkerId,
};
use std::{collections::BTreeMap, num::NonZeroU64};

/// Retained sets. At capacity the oldest settled set gives way; a pending one never does.
const MAXIMUM_OPERATIONS: usize = 64;

/// One frozen member: what the set reports about it and every bound Presentation its Tab held at
/// admission, in canonical membership order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabSetCloseMemberPlan {
    pub member: TabSetCloseMember,
    pub presentations: Vec<WorkbenchTerminalTab>,
}

/// The owned submitter's copy of one claimed set; publication re-proves its frozen scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabSetClosePlan {
    pub operation_id: WorkbenchRequestId,
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub condition: Condition,
    /// The Worker incarnation the set was admitted under.
    pub worker: Option<(WorkerId, NonZeroU64)>,
    pub members: Vec<TabSetCloseMemberPlan>,
}

/// Where one member stands when the set reaches it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabSetCloseMemberState {
    /// The Tab is already gone, which counts as done.
    Absent,
    /// The Tab's bound Presentations are no longer the ones frozen at admission.
    Changed,
    /// The Tab holds exactly its frozen Presentations.
    Unchanged,
}

pub enum TabSetCloseBegin<'a> {
    /// Answered without retaining anything new.
    Ready(WorkbenchTabSetCloseResult),
    /// A new Start that passed every guard; nothing is retained until it is admitted.
    Admitted(Box<TabSetCloseAdmission<'a>>),
}

pub struct TabSetCloseAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    request_id: WorkbenchRequestId,
    expected_revision: WorkbenchRevision,
    plan: TabSetClosePlan,
}

struct Record {
    plan: TabSetClosePlan,
    expected_revision: WorkbenchRevision,
    status: Status,
    claimed: bool,
    sequence: u64,
}

#[derive(Default)]
pub(super) struct TabSetCloses {
    records: BTreeMap<WorkbenchRequestId, Record>,
    admitted: u64,
}

impl TabSetCloseAdmission<'_> {
    #[must_use]
    pub fn condition(&self) -> Condition {
        self.plan.condition
    }

    /// Every frozen bound Presentation, member by member in set order.
    pub fn presentations(&self) -> impl Iterator<Item = &WorkbenchTerminalTab> {
        self.plan
            .members
            .iter()
            .flat_map(|member| &member.presentations)
    }

    /// The Server could not prove every member idle; nothing is retained.
    #[must_use]
    pub fn confirmation_required(self) -> WorkbenchTabSetCloseResult {
        result(
            self.request_id,
            self.plan.operation_id,
            Outcome::ConfirmationRequired {},
        )
    }

    /// Retains the set for its one submitter.
    #[must_use]
    pub fn admit(self) -> WorkbenchTabSetCloseResult {
        let sets = &mut self.authority.tab_sets;
        if sets.records.len() >= MAXIMUM_OPERATIONS {
            let oldest = sets
                .records
                .iter()
                .filter(|(_, record)| !matches!(record.status, Status::Pending {}))
                .min_by_key(|(_, record)| record.sequence)
                .map(|(id, _)| id.clone());
            let Some(oldest) = oldest else {
                return result(
                    self.request_id,
                    self.plan.operation_id,
                    Outcome::Unavailable {
                        reason: Unavailable::Capacity,
                    },
                );
            };
            sets.records.remove(&oldest);
        }
        sets.admitted += 1;
        let record = Record {
            plan: self.plan,
            expected_revision: self.expected_revision,
            status: Status::Pending {},
            claimed: false,
            sequence: sets.admitted,
        };
        let operation_id = record.plan.operation_id.clone();
        let outcome = record.outcome();
        sets.records.insert(operation_id.clone(), record);
        result(self.request_id, operation_id, outcome)
    }
}

impl WorkbenchAuthority {
    /// Answers a Read or a known Start purely, and returns a new Start that passed every guard for
    /// the Session to admit once its condition holds. `worker` is the incarnation it runs under.
    pub fn begin_tab_set_close(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchTabSetCloseRequest,
        worker: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
    ) -> TabSetCloseBegin<'_> {
        let WorkbenchTabSetCloseRequest {
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
                && (!start || grant.close_terminal)
        }) {
            Outcome::Denied {}
        } else {
            match self.plan_tab_set_close(action, worker, owner_available) {
                Ok((expected_revision, plan)) => {
                    return TabSetCloseBegin::Admitted(Box::new(TabSetCloseAdmission {
                        authority: self,
                        request_id,
                        expected_revision,
                        plan,
                    }));
                }
                Err(outcome) => outcome,
            }
        };
        TabSetCloseBegin::Ready(result(request_id, operation_id, outcome))
    }

    fn plan_tab_set_close(
        &self,
        action: Action,
        worker: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
    ) -> Result<(WorkbenchRevision, TabSetClosePlan), Outcome> {
        let (operation_id, terminal_context_id, tab_ids, expected_revision, condition) =
            match action {
                Action::Read { operation_id } => {
                    return Err(self
                        .tab_sets
                        .records
                        .get(&operation_id)
                        .map_or(Outcome::NotFound {}, Record::outcome));
                }
                Action::Start {
                    operation_id,
                    terminal_context_id,
                    tab_ids,
                    expected_revision,
                    condition,
                } => (
                    operation_id,
                    terminal_context_id,
                    tab_ids,
                    expected_revision,
                    condition,
                ),
            };
        if let Some(record) = self.tab_sets.records.get(&operation_id) {
            let equal = record.plan.terminal_context_id == terminal_context_id
                && record.plan.condition == condition
                && record.expected_revision == expected_revision
                && record
                    .plan
                    .members
                    .iter()
                    .map(|member| &member.member.tab_id)
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
        if self.workspace_group_close_contains(&workspace.workspace_id)
            || tab_ids
                .iter()
                .any(|tab_id| self.tab_set_member_busy(&terminal_context_id, tab_id))
        {
            return Err(unavailable(Unavailable::Busy));
        }
        if self.tab_sets.records.len() >= MAXIMUM_OPERATIONS
            && self
                .tab_sets
                .records
                .values()
                .all(|record| matches!(record.status, Status::Pending {}))
        {
            return Err(unavailable(Unavailable::Capacity));
        }
        let membership = self
            .presentation_membership(&terminal_context_id)
            .map_err(|_invariant| unavailable(Unavailable::ServiceUnavailable))?;
        let mut members = Vec::with_capacity(tab_ids.len());
        for (tab_id, position) in tab_ids.into_iter().zip(positions) {
            let label = context
                .tabs
                .get(position)
                .and_then(|tab| WorkbenchLabel::parse(&tab.display_label(position)).ok())
                .ok_or(unavailable(Unavailable::ServiceUnavailable))?;
            let presentations = membership.members_for_tab(&tab_id).cloned().collect();
            members.push(TabSetCloseMemberPlan {
                member: TabSetCloseMember {
                    tab_id,
                    label,
                    close: None,
                    absent: false,
                },
                presentations,
            });
        }
        Ok((
            expected_revision,
            TabSetClosePlan {
                operation_id,
                terminal_context_id,
                condition,
                worker,
                members,
            },
        ))
    }

    /// A member is busy while any Close covers part of its Tab, a terminal create or split into it
    /// is unsettled, or another pending set still owns it.
    fn tab_set_member_busy(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        self.open_close_covers_tab(context_id, tab_id)
            || self.pending_terminal_covers_tab(context_id, tab_id)
            || self.tab_sets.records.values().any(|record| {
                matches!(record.status, Status::Pending {})
                    && record.plan.terminal_context_id == *context_id
                    && record
                        .plan
                        .members
                        .iter()
                        .any(|member| member.member.tab_id == *tab_id)
            })
    }

    /// Every terminal a pending set froze at admission, its members not yet reached included. A set
    /// that stops gives back the members it did not close.
    pub(super) fn tab_set_removing_terminals(
        &self,
    ) -> impl Iterator<Item = &orchbee_workbench_protocol::PresentationBinding> {
        self.tab_sets
            .records
            .values()
            .filter(|record| matches!(record.status, Status::Pending {}))
            .flat_map(|record| &record.plan.members)
            .flat_map(|member| &member.presentations)
            .map(WorkbenchTerminalTab::binding)
    }

    /// Claims the oldest unclaimed pending set for the one owned submitter.
    pub fn claim_tab_set_close(&mut self) -> Option<TabSetClosePlan> {
        let record = self
            .tab_sets
            .records
            .values_mut()
            .filter(|record| !record.claimed && matches!(record.status, Status::Pending {}))
            .min_by_key(|record| record.sequence)?;
        record.claimed = true;
        Some(record.plan.clone())
    }

    /// Publishes a claimed pending set's progress or end. Child identities, observations and
    /// absences only advance, and Closed requires every member done.
    pub fn publish_tab_set_close(&mut self, plan: &TabSetClosePlan, status: Status) -> bool {
        let Some(record) = self.tab_sets.records.get_mut(&plan.operation_id) else {
            return false;
        };
        if !record.claimed
            || !matches!(record.status, Status::Pending {})
            || !same_scope(&record.plan, plan)
            || !record
                .plan
                .members
                .iter()
                .zip(&plan.members)
                .all(|(old, new)| advances(&old.member, &new.member))
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

    /// Ends a claimed set whose submitter could not publish truthfully, keeping what it published.
    pub fn abandon_tab_set_close(&mut self, operation_id: &WorkbenchRequestId) {
        if let Some(record) = self.tab_sets.records.get_mut(operation_id)
            && record.claimed
            && matches!(record.status, Status::Pending {})
        {
            record.status = Status::Failed {
                reason: Failure::Internal,
            };
        }
    }

    /// Where member `index` of a retained set stands now.
    #[must_use]
    pub fn tab_set_close_member_state(
        &self,
        operation_id: &WorkbenchRequestId,
        index: usize,
    ) -> Option<TabSetCloseMemberState> {
        let record = self.tab_sets.records.get(operation_id)?;
        let member = record.plan.members.get(index)?;
        Some(self.tab_set_member_state(&record.plan.terminal_context_id, member))
    }

    fn tab_set_member_state(
        &self,
        context_id: &WorkbenchTerminalContextId,
        member: &TabSetCloseMemberPlan,
    ) -> TabSetCloseMemberState {
        let snapshot = self.snapshot();
        let present = snapshot
            .navigation
            .workspaces
            .iter()
            .any(|workspace| workspace.terminal_context_id == *context_id)
            && snapshot.terminal_contexts.iter().any(|context| {
                context.terminal_context_id == *context_id
                    && context
                        .tabs
                        .iter()
                        .any(|tab| tab.tab_id == member.member.tab_id)
            });
        if !present {
            return TabSetCloseMemberState::Absent;
        }
        match self.presentation_membership(context_id) {
            Ok(membership)
                if membership
                    .members_for_tab(&member.member.tab_id)
                    .eq(member.presentations.iter()) =>
            {
                TabSetCloseMemberState::Unchanged
            }
            _ => TabSetCloseMemberState::Changed,
        }
    }

    /// Proves the claimed set's next unsubmitted member, unchanged since admission, before the
    /// set may submit that member's own Close.
    pub(crate) fn tab_set_member_matches(
        &self,
        operation_id: &WorkbenchRequestId,
        index: usize,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        let Some(record) = self.tab_sets.records.get(operation_id) else {
            return false;
        };
        if !record.claimed
            || !matches!(record.status, Status::Pending {})
            || record.plan.terminal_context_id != *context_id
        {
            return false;
        }
        let Some(member) = record.plan.members.get(index) else {
            return false;
        };
        member.member.tab_id == *tab_id
            && member.member.close.is_none()
            && !member.member.absent
            && record.plan.members[..index]
                .iter()
                .all(|member| member.member.is_done())
            && self.tab_set_member_state(context_id, member) == TabSetCloseMemberState::Unchanged
    }
}

impl Record {
    fn outcome(&self) -> Outcome {
        Outcome::Available {
            terminal_context_id: self.plan.terminal_context_id.clone(),
            condition: self.plan.condition,
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

fn result(
    request_id: WorkbenchRequestId,
    operation_id: WorkbenchRequestId,
    outcome: Outcome,
) -> WorkbenchTabSetCloseResult {
    WorkbenchTabSetCloseResult {
        protocol_version: ProtocolVersion,
        request_id,
        operation_id,
        outcome,
    }
}

fn same_scope(left: &TabSetClosePlan, right: &TabSetClosePlan) -> bool {
    left.operation_id == right.operation_id
        && left.terminal_context_id == right.terminal_context_id
        && left.condition == right.condition
        && left.worker == right.worker
        && left.members.len() == right.members.len()
        && left
            .members
            .iter()
            .zip(&right.members)
            .all(|(left, right)| {
                left.member.tab_id == right.member.tab_id
                    && left.member.label == right.member.label
                    && left.presentations == right.presentations
            })
}

/// Whether `new` only moves `old` forward: an absence or a closed child is final, a child keeps its
/// identity once submitted, and its attempt generation never goes back.
fn advances(old: &TabSetCloseMember, new: &TabSetCloseMember) -> bool {
    if old.absent {
        return new == old;
    }
    match (&old.close, &new.close) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(old_child), Some(new_child)) => {
            !new.absent
                && old_child.request_id == new_child.request_id
                && if old_child.is_closed() {
                    old_child == new_child
                } else {
                    match (&old_child.observation, &new_child.observation) {
                        (Some(_), None) => false,
                        (Some(old), Some(new)) => old.attempt_generation <= new.attempt_generation,
                        (None, _) => true,
                    }
                }
        }
    }
}

#[cfg(test)]
mod tests;
