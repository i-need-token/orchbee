//! Site-owned outer title publication, independent of pane metadata.
use crate::{ContractError, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowTitleContext {
    pub workspace: String,
    pub tab: String,
    pub pane: String,
    pub terminal_title: String,
}
impl WindowTitleContext {
    pub fn new(workspace: &str, tab: &str, pane: &str, terminal_title: &str) -> Self {
        let bounded = |value: &str| {
            value
                .chars()
                .filter(|ch| !ch.is_control())
                .take(200)
                .collect()
        };
        Self {
            workspace: bounded(workspace),
            tab: bounded(tab),
            pane: bounded(pane),
            terminal_title: bounded(terminal_title),
        }
    }
    fn validate(&self) -> Result<(), ContractError> {
        if [&self.workspace, &self.tab, &self.pane, &self.terminal_title]
            .iter()
            .all(|value| {
                value.len() <= 800
                    && value.chars().count() <= 200
                    && !value.chars().any(char::is_control)
            })
        {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowTitle(String);
impl WindowTitle {
    pub fn normalize(value: &str) -> Result<Self, ContractError> {
        let value = value
            .chars()
            .filter(|ch| !ch.is_control())
            .take(200)
            .collect::<String>();
        let value = value.trim();
        if value.is_empty() {
            Err(ContractError::InvalidDocument)
        } else {
            Ok(Self(value.into()))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.0.len() <= 800 && Self::normalize(&self.0).as_ref() == Ok(self) {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowTitleSnapshot {
    pub revision: WorkbenchRevision,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub title: Option<WindowTitle>,
}
impl WindowTitleSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.revision.value() == 0 {
            return Err(ContractError::InvalidDocument);
        }
        self.title.as_ref().map_or(Ok(()), WindowTitle::validate)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WindowTitleAction {
    Set {
        title: WindowTitle,
    },
    Clear {},
    Listen {
        focused: bool,
        context: WindowTitleContext,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowTitleRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub site_id: SiteId,
    pub action: WindowTitleAction,
}
impl WindowTitleRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
            WindowTitleAction::Set { title } => title.validate(),
            WindowTitleAction::Listen { context, .. } => context.validate(),
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowTitleReason {
    Set,
    Cleared,
    NoForegroundClient,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowTitleFailure {
    Denied,
    Unavailable,
    #[serde(rename = "session-mismatch")]
    SiteMismatch,
    Conflict,
    Busy,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WindowTitleOutcome {
    Published {
        snapshot: WindowTitleSnapshot,
        changed: bool,
        reason: WindowTitleReason,
    },
    Listening {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        snapshot: Option<WindowTitleSnapshot>,
    },
    Failed {
        reason: WindowTitleFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowTitleResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub site_id: SiteId,
    pub outcome: WindowTitleOutcome,
}
impl WindowTitleResult {
    pub fn answers(&self, request: &WindowTitleRequest) -> bool {
        self.request_id == request.request_id
            && self.site_id == request.site_id
            && match (&request.action, &self.outcome) {
                (_, WindowTitleOutcome::Failed { .. }) => true,
                (WindowTitleAction::Listen { .. }, WindowTitleOutcome::Listening { .. }) => true,
                (
                    WindowTitleAction::Set { title },
                    WindowTitleOutcome::Published { snapshot, .. },
                ) => snapshot.title.as_ref() == Some(title),
                (WindowTitleAction::Clear {}, WindowTitleOutcome::Published { snapshot, .. }) => {
                    snapshot.title.is_none()
                }
                _ => false,
            }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            WindowTitleOutcome::Published {
                snapshot,
                changed,
                reason,
            } => {
                snapshot.validate()?;
                let expected = if !changed {
                    WindowTitleReason::NoForegroundClient
                } else if snapshot.title.is_some() {
                    WindowTitleReason::Set
                } else {
                    WindowTitleReason::Cleared
                };
                if expected == *reason {
                    Ok(())
                } else {
                    Err(ContractError::InvalidDocument)
                }
            }
            WindowTitleOutcome::Listening {
                snapshot: Some(snapshot),
            } => snapshot.validate(),
            _ => Ok(()),
        }
    }
}
