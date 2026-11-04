//! Bounded retained closure of several Tabs of one terminal context; child lifecycle truth remains
//! owned by Close.
use super::json_fields::deserialize_nullable;
use crate::{
    ContractError, MAXIMUM_TABS_PER_CONTEXT, ProtocolVersion, SessionId, WorkbenchLabel,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabCloseStatus, WorkbenchTabId, WorkbenchTerminalContextId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Independent aggregate response bound, checked before a frame body is allocated.
pub const MAXIMUM_ENCODED_TAB_SET_CLOSE_RESULT_BYTES: usize = 1024 * 1024;
/// A set never names more Tabs than one terminal context holds.
pub const MAXIMUM_TAB_SET_CLOSE_MEMBERS: usize = MAXIMUM_TABS_PER_CONTEXT;

/// Whether the Server first reads every member's terminals, closing unasked only when none runs
/// anything, or closes because the person already confirmed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSetCloseCondition {
    IfIdle,
    Confirmed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TabSetCloseAction {
    /// `tab_ids` lists distinct Tabs in the context's current order.
    Start {
        operation_id: WorkbenchRequestId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_ids: Vec<WorkbenchTabId>,
        expected_revision: WorkbenchRevision,
        condition: TabSetCloseCondition,
    },
    Read {
        operation_id: WorkbenchRequestId,
    },
}
impl TabSetCloseAction {
    /// Admits the operation receipt while preserving the frozen context, members and condition.
    #[must_use]
    pub fn admits(&self, outcome: &TabSetCloseOutcome) -> bool {
        let failed = matches!(
            outcome,
            TabSetCloseOutcome::Denied {} | TabSetCloseOutcome::Unavailable { .. }
        );
        match self {
            Self::Start {
                terminal_context_id,
                tab_ids,
                condition,
                ..
            } => {
                failed
                    || matches!(
                        outcome,
                        TabSetCloseOutcome::Rejected { .. } | TabSetCloseOutcome::Conflict {}
                    )
                    || (*condition == TabSetCloseCondition::IfIdle
                        && matches!(outcome, TabSetCloseOutcome::ConfirmationRequired {}))
                    || matches!(outcome, TabSetCloseOutcome::Available {
                        terminal_context_id: actual,
                        condition: actual_condition,
                        members,
                        ..
                    } if actual == terminal_context_id
                        && actual_condition == condition
                        && members.iter().map(|member| &member.tab_id).eq(tab_ids))
            }
            Self::Read { .. } => {
                failed
                    || matches!(
                        outcome,
                        TabSetCloseOutcome::Available { .. } | TabSetCloseOutcome::NotFound {}
                    )
            }
        }
    }

    /// Stable operation identity, independent of this exchange.
    pub fn operation_id(&self) -> &WorkbenchRequestId {
        match self {
            Self::Start { operation_id, .. } | Self::Read { operation_id } => operation_id,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTabSetCloseRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: TabSetCloseAction,
}
impl WorkbenchTabSetCloseRequest {
    /// Rejects an empty, oversized or repeating member list.
    pub fn validate(&self) -> Result<(), ContractError> {
        if let TabSetCloseAction::Start { tab_ids, .. } = &self.action
            && (tab_ids.is_empty()
                || tab_ids.len() > MAXIMUM_TAB_SET_CLOSE_MEMBERS
                || tab_ids.iter().collect::<BTreeSet<_>>().len() != tab_ids.len())
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabSetCloseChild {
    pub request_id: WorkbenchRequestId,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub observation: Option<TabSetCloseObservation>,
}
impl TabSetCloseChild {
    /// Closure is known only through an observed child tombstone.
    pub fn is_closed(&self) -> bool {
        self.observation.as_ref().is_some_and(|observation| {
            matches!(observation.status, WorkbenchTabCloseStatus::Closed { .. })
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabSetCloseObservation {
    pub attempt_generation: WorkbenchTabCloseAttemptGeneration,
    pub status: WorkbenchTabCloseStatus,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabSetCloseMember {
    pub tab_id: WorkbenchTabId,
    pub label: WorkbenchLabel,
    /// Null until the set submits this member's Close.
    #[serde(deserialize_with = "deserialize_nullable")]
    pub close: Option<TabSetCloseChild>,
    /// The Tab was already gone when the set reached it; it counts as done.
    pub absent: bool,
}
impl TabSetCloseMember {
    /// Gone before its turn, or its own Close observed Closed.
    pub fn is_done(&self) -> bool {
        self.absent || self.close.as_ref().is_some_and(TabSetCloseChild::is_closed)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSetCloseFailure {
    /// A member's terminals changed after admission.
    SourceChanged,
    WorkerUnavailable,
    /// A member of an if-idle set now runs something.
    ChildBusy,
    ChildAdmission,
    ChildBlocked,
    OutcomeUnknown,
    Deadline,
    SessionStopping,
    Internal,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TabSetCloseStatus {
    Pending {},
    Closed {},
    /// Earlier members stay closed and later ones are never submitted; the failed member's own
    /// Close stays retained for its Tab Resume.
    Failed {
        reason: TabSetCloseFailure,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSetCloseUnavailable {
    ServiceUnavailable,
    /// A member has an open Close or terminal creation, or belongs to another pending set.
    Busy,
    Capacity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSetCloseRejection {
    RevisionMismatch,
    TerminalContextUnavailable,
    TabUnavailable,
    /// The members are not listed in the context's current Tab order.
    TabOrderMismatch,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TabSetCloseOutcome {
    Available {
        terminal_context_id: WorkbenchTerminalContextId,
        condition: TabSetCloseCondition,
        status: TabSetCloseStatus,
        members: Vec<TabSetCloseMember>,
    },
    /// An if-idle Start found a member running something; nothing closed and nothing is retained.
    ConfirmationRequired {},
    /// Nothing closed and nothing is retained; `revision` is the current one.
    Rejected {
        revision: WorkbenchRevision,
        reason: TabSetCloseRejection,
    },
    NotFound {},
    Conflict {},
    Denied {},
    Unavailable {
        reason: TabSetCloseUnavailable,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTabSetCloseResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub operation_id: WorkbenchRequestId,
    pub outcome: TabSetCloseOutcome,
}
impl WorkbenchTabSetCloseResult {
    /// Enforces bounded unique membership, ordered children and truthful completion.
    pub fn validate(&self) -> Result<(), ContractError> {
        let TabSetCloseOutcome::Available {
            status, members, ..
        } = &self.outcome
        else {
            return Ok(());
        };
        if members.is_empty() || members.len() > MAXIMUM_TAB_SET_CLOSE_MEMBERS {
            return Err(ContractError::InvalidDocument);
        }
        let mut tabs = BTreeSet::new();
        let mut requests = BTreeSet::new();
        let mut unfinished = false;
        for member in members {
            if !tabs.insert(&member.tab_id) || (member.absent && member.close.is_some()) {
                return Err(ContractError::InvalidDocument);
            }
            if unfinished && (member.absent || member.close.is_some()) {
                return Err(ContractError::InvalidDocument);
            }
            if let Some(child) = &member.close
                && (child.request_id == self.operation_id || !requests.insert(&child.request_id))
            {
                return Err(ContractError::InvalidDocument);
            }
            unfinished = !member.is_done();
        }
        if matches!(status, TabSetCloseStatus::Closed {}) && unfinished {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
