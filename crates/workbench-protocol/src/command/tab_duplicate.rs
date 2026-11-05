//! Bounded retained duplication of several Tabs of one terminal context. Each copy is an ordinary
//! duplicate Tab creation; the operation records its Tab and terminal outcome in order.
use super::json_fields::deserialize_nullable;
use crate::{
    ContractError, MAXIMUM_TABS_PER_CONTEXT, ProtocolVersion, SessionId, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Independent aggregate response bound, checked before a frame body is allocated.
pub const MAXIMUM_ENCODED_TAB_DUPLICATE_RESULT_BYTES: usize = 256 * 1024;
/// An operation never names more Tabs than one terminal context holds.
pub const MAXIMUM_TAB_DUPLICATE_MEMBERS: usize = MAXIMUM_TABS_PER_CONTEXT;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TabDuplicateAction {
    /// `tab_ids` lists distinct Tabs in the context's current order; every copy starts its
    /// terminal at `geometry`.
    Start {
        operation_id: WorkbenchRequestId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_ids: Vec<WorkbenchTabId>,
        expected_revision: WorkbenchRevision,
        geometry: WorkbenchTerminalGeometry,
    },
    Read {
        operation_id: WorkbenchRequestId,
    },
}
impl TabDuplicateAction {
    /// Admits the operation receipt while preserving the frozen context and members.
    #[must_use]
    pub fn admits(&self, outcome: &TabDuplicateOutcome) -> bool {
        let failed = matches!(
            outcome,
            TabDuplicateOutcome::Denied {} | TabDuplicateOutcome::Unavailable { .. }
        );
        match self {
            Self::Start {
                terminal_context_id,
                tab_ids,
                ..
            } => {
                failed
                    || matches!(
                        outcome,
                        TabDuplicateOutcome::Rejected { .. } | TabDuplicateOutcome::Conflict {}
                    )
                    || matches!(outcome, TabDuplicateOutcome::Available {
                        terminal_context_id: actual,
                        members,
                        ..
                    } if actual == terminal_context_id
                        && members.iter().map(|member| &member.tab_id).eq(tab_ids))
            }
            Self::Read { .. } => {
                failed
                    || matches!(
                        outcome,
                        TabDuplicateOutcome::Available { .. } | TabDuplicateOutcome::NotFound {}
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
pub struct WorkbenchTabDuplicateRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: TabDuplicateAction,
}
impl WorkbenchTabDuplicateRequest {
    /// Rejects an empty, oversized or repeating member list.
    pub fn validate(&self) -> Result<(), ContractError> {
        if let TabDuplicateAction::Start { tab_ids, .. } = &self.action
            && (tab_ids.is_empty()
                || tab_ids.len() > MAXIMUM_TAB_DUPLICATE_MEMBERS
                || tab_ids.iter().collect::<BTreeSet<_>>().len() != tab_ids.len())
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
/// The Tab one member's duplicate creation committed and, once settled, its terminal outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabDuplicateCopy {
    pub tab_id: WorkbenchTabId,
    /// The revision that published this copy's latest fact.
    pub revision: WorkbenchRevision,
    /// Null while the copy's terminal launch is still being delivered.
    #[serde(deserialize_with = "deserialize_nullable")]
    pub terminal: Option<WorkbenchTerminalOutcome>,
}
impl TabDuplicateCopy {
    pub fn is_started(&self) -> bool {
        matches!(
            self.terminal,
            Some(WorkbenchTerminalOutcome::Started { .. })
        )
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabDuplicateMember {
    pub tab_id: WorkbenchTabId,
    /// Null until the operation commits this member's copy.
    #[serde(deserialize_with = "deserialize_nullable")]
    pub copy: Option<TabDuplicateCopy>,
}
impl TabDuplicateMember {
    /// Its copy exists and its terminal started.
    pub fn is_done(&self) -> bool {
        self.copy.as_ref().is_some_and(TabDuplicateCopy::is_started)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabDuplicateFailure {
    /// The member's Tab or its terminal context was gone at its turn.
    SourceUnavailable,
    /// The member's duplicate creation was refused; nothing was created for it.
    CopyRefused,
    /// The copy exists but its terminal did not start.
    TerminalNotStarted,
    /// The copy exists and its terminal launch has an unknown outcome.
    OutcomeUnknown,
    Deadline,
    SessionStopping,
    Internal,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TabDuplicateStatus {
    Pending {},
    Duplicated {},
    /// Earlier copies stay and later members are never copied; nothing is replayed.
    Failed {
        reason: TabDuplicateFailure,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabDuplicateUnavailable {
    ServiceUnavailable,
    /// A Close holds the terminal context, or its Workspace is closing.
    Busy,
    Capacity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabDuplicateRejection {
    RevisionMismatch,
    TerminalContextUnavailable,
    TabUnavailable,
    /// The members are not listed in the context's current Tab order.
    TabOrderMismatch,
    /// The context cannot hold one copy of every member.
    ResourceLimit,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TabDuplicateOutcome {
    Available {
        terminal_context_id: WorkbenchTerminalContextId,
        status: TabDuplicateStatus,
        members: Vec<TabDuplicateMember>,
    },
    /// Nothing was created and nothing is retained; `revision` is the current one.
    Rejected {
        revision: WorkbenchRevision,
        reason: TabDuplicateRejection,
    },
    NotFound {},
    Conflict {},
    Denied {},
    Unavailable {
        reason: TabDuplicateUnavailable,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTabDuplicateResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub operation_id: WorkbenchRequestId,
    pub outcome: TabDuplicateOutcome,
}
impl WorkbenchTabDuplicateResult {
    /// Enforces bounded unique membership, fresh copies in member order and truthful completion.
    pub fn validate(&self) -> Result<(), ContractError> {
        let TabDuplicateOutcome::Available {
            status, members, ..
        } = &self.outcome
        else {
            return Ok(());
        };
        if members.is_empty() || members.len() > MAXIMUM_TAB_DUPLICATE_MEMBERS {
            return Err(ContractError::InvalidDocument);
        }
        let sources = members
            .iter()
            .map(|member| &member.tab_id)
            .collect::<BTreeSet<_>>();
        if sources.len() != members.len() {
            return Err(ContractError::InvalidDocument);
        }
        let mut copies = BTreeSet::new();
        let mut unfinished = false;
        for member in members {
            if let Some(copy) = &member.copy
                && (unfinished || sources.contains(&copy.tab_id) || !copies.insert(&copy.tab_id))
            {
                return Err(ContractError::InvalidDocument);
            }
            unfinished = !member.is_done();
        }
        if matches!(status, TabDuplicateStatus::Duplicated {}) && unfinished {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
