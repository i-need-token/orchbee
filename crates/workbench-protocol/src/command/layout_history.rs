//! Earlier layouts the Server keeps over time. `list` reads them newest first with how each
//! compares with the open Workbench; `reopen` opens from one entry only the Workspaces and Tabs
//! whose identity is not open now, without closing, moving or relabeling anything open.
use super::json_fields::deserialize_nullable;
use crate::{
    ContractError, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
use serde::{Deserialize, Serialize};

/// The Server keeps at most this many entries.
pub const MAXIMUM_LAYOUT_HISTORY_ENTRIES: usize = 48;
/// Save times are Unix milliseconds that every JSON reader holds exactly.
pub const MAXIMUM_LAYOUT_HISTORY_SAVED_AT: u64 = (1 << 53) - 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LayoutHistoryAction {
    List {},
    /// `saved_at` names the entry by the save time `list` reported.
    Reopen {
        saved_at: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchLayoutHistoryRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: LayoutHistoryAction,
}

impl WorkbenchLayoutHistoryRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self.action {
            LayoutHistoryAction::Reopen { saved_at } if !valid_saved_at(saved_at) => {
                Err(ContractError::InvalidDocument)
            }
            _ => Ok(()),
        }
    }
}

/// One kept layout and how it compares with the open Workbench by identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutHistoryEntry {
    pub saved_at: u64,
    pub workspaces: u32,
    pub tabs: u32,
    pub workspaces_not_open: u32,
    pub tabs_not_open: u32,
}

/// The first reopened Tab. An interactive Client may select it; nothing else moves focus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutHistoryFocus {
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutHistoryRejection {
    SessionMismatch,
    /// The reopened Workspaces or Tabs do not fit the Workbench bounds; nothing was opened.
    ResourceLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutHistoryUnavailable {
    ServiceUnavailable,
    /// A Close holds a Workspace the reopen would add to.
    Busy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LayoutHistoryOutcome {
    /// Newest first.
    Listed {
        entries: Vec<LayoutHistoryEntry>,
    },
    /// Counts of what was opened; terminals start in their saved directories afterwards.
    Reopened {
        revision: WorkbenchRevision,
        workspaces: u32,
        tabs: u32,
        #[serde(deserialize_with = "deserialize_nullable")]
        focus: Option<LayoutHistoryFocus>,
    },
    /// Everything in the entry is open now.
    NothingToReopen {},
    /// No kept entry has that save time.
    NotFound {},
    Rejected {
        reason: LayoutHistoryRejection,
    },
    Denied {},
    Unavailable {
        reason: LayoutHistoryUnavailable,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchLayoutHistoryResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: LayoutHistoryOutcome,
}

impl WorkbenchLayoutHistoryResult {
    /// Bounded, newest-first listing with consistent counts; a reopen names something opened.
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match &self.outcome {
            LayoutHistoryOutcome::Listed { entries } => {
                entries.len() <= MAXIMUM_LAYOUT_HISTORY_ENTRIES
                    && entries.iter().all(|entry| {
                        valid_saved_at(entry.saved_at)
                            && entry.workspaces_not_open <= entry.workspaces
                            && entry.tabs_not_open <= entry.tabs
                    })
                    && entries
                        .windows(2)
                        .all(|pair| pair[0].saved_at > pair[1].saved_at)
            }
            LayoutHistoryOutcome::Reopened {
                workspaces,
                tabs,
                focus,
                ..
            } => (*workspaces > 0 || *tabs > 0) && (focus.is_some() || *tabs == 0),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }

    /// Whether this result answers `request`: same identity, and an outcome its action admits.
    #[must_use]
    pub fn answers(&self, request: &WorkbenchLayoutHistoryRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && match (&request.action, &self.outcome) {
                (_, LayoutHistoryOutcome::Denied {} | LayoutHistoryOutcome::Unavailable { .. }) => {
                    true
                }
                (LayoutHistoryAction::List {}, outcome) => {
                    matches!(outcome, LayoutHistoryOutcome::Listed { .. })
                }
                (LayoutHistoryAction::Reopen { .. }, outcome) => {
                    !matches!(outcome, LayoutHistoryOutcome::Listed { .. })
                }
            }
    }
}

const fn valid_saved_at(saved_at: u64) -> bool {
    saved_at > 0 && saved_at <= MAXIMUM_LAYOUT_HISTORY_SAVED_AT
}
