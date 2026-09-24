//! Ephemeral Client-owned selection and connection-scoped, acknowledged navigation.
use super::json_fields::deserialize_nullable;
use crate::identity::WorkbenchClientId;
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchPanelId, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{collections::BTreeSet, num::NonZeroU64};

pub const MAXIMUM_CLIENT_VIEWS: usize = 32;
pub const MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClientViewSequence(NonZeroU64);

impl ClientViewSequence {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(ContractError::InvalidDocument)
    }
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0.get()
    }
}
impl Serialize for ClientViewSequence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value().to_string())
    }
}
impl<'de> Deserialize<'de> for ClientViewSequence {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = WorkbenchRevision::deserialize(deserializer)?;
        Self::new(value.value()).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewPhase {
    Starting,
    Ready,
    Busy,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewSelection {
    pub terminal_context_id: WorkbenchTerminalContextId,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub tab_id: Option<WorkbenchTabId>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub panel_id: Option<WorkbenchPanelId>,
    pub zoomed: bool,
    pub panel_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewObservation {
    pub workbench_revision: WorkbenchRevision,
    pub phase: ClientViewPhase,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub selection: Option<ClientViewSelection>,
}
impl ClientViewObservation {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.phase == ClientViewPhase::Ready && self.selection.is_none() {
            return Err(ContractError::InvalidDocument);
        }
        if self.selection.as_ref().is_some_and(|selection| {
            (selection.tab_id.is_some() != (selection.panel_count > 0))
                || (selection.panel_id.is_some() && selection.tab_id.is_none())
                || (selection.zoomed && selection.panel_id.is_none())
        }) {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewAvailability {
    Current,
    Stale,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewRecord {
    pub client_id: WorkbenchClientId,
    pub heartbeat: ClientViewSequence,
    pub view_revision: ClientViewSequence,
    pub view: ClientViewObservation,
    pub availability: ClientViewAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewIntent {
    Context {
        terminal_context_id: WorkbenchTerminalContextId,
    },
    Tab {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    },
    Panel {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    },
    ToggleZoom {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    },
    Zoom {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        enabled: bool,
    },
}
impl ClientViewIntent {
    #[must_use]
    pub fn is_single_panel_zoom_observed(
        &self,
        view: &ClientViewObservation,
        minimum: WorkbenchRevision,
    ) -> bool {
        let (terminal_context_id, tab_id, panel_id) = match self {
            Self::Zoom {
                terminal_context_id,
                tab_id,
                panel_id,
                enabled: true,
            }
            | Self::ToggleZoom {
                terminal_context_id,
                tab_id,
                panel_id,
            } => (terminal_context_id, tab_id, panel_id),
            _ => return false,
        };
        view.selection
            .as_ref()
            .is_some_and(|selection| selection.panel_count == 1 && !selection.zoomed)
            && Self::Panel {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
            }
            .is_observed(view, minimum)
    }
    #[must_use]
    pub fn is_toggle_observed(
        &self,
        view: &ClientViewObservation,
        minimum: WorkbenchRevision,
        previously_zoomed: bool,
    ) -> bool {
        let Self::ToggleZoom {
            terminal_context_id,
            tab_id,
            panel_id,
        } = self
        else {
            return false;
        };
        Self::Zoom {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            enabled: !previously_zoomed,
        }
        .is_observed(view, minimum)
            && view
                .selection
                .as_ref()
                .is_some_and(|selection| selection.panel_count > 1)
    }
    #[must_use]
    pub fn is_observed(&self, view: &ClientViewObservation, minimum: WorkbenchRevision) -> bool {
        if view.workbench_revision.value() < minimum.value()
            || matches!(
                view.phase,
                ClientViewPhase::Starting | ClientViewPhase::Busy
            )
        {
            return false;
        }
        let Some(selection) = &view.selection else {
            return false;
        };
        match self {
            Self::ToggleZoom { .. } => false,
            Self::Context {
                terminal_context_id,
            } => selection.terminal_context_id == *terminal_context_id,
            Self::Tab {
                terminal_context_id,
                tab_id,
            } => {
                selection.terminal_context_id == *terminal_context_id
                    && selection.tab_id.as_ref() == Some(tab_id)
            }
            Self::Panel {
                terminal_context_id,
                tab_id,
                panel_id,
            } => {
                selection.terminal_context_id == *terminal_context_id
                    && selection.tab_id.as_ref() == Some(tab_id)
                    && selection.panel_id.as_ref() == Some(panel_id)
            }
            Self::Zoom {
                terminal_context_id,
                tab_id,
                panel_id,
                enabled,
            } => {
                selection.terminal_context_id == *terminal_context_id
                    && selection.tab_id.as_ref() == Some(tab_id)
                    && selection.panel_id.as_ref() == Some(panel_id)
                    && selection.zoomed == *enabled
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewControl {
    pub operation_id: WorkbenchRequestId,
    pub client_id: WorkbenchClientId,
    pub expected_view_revision: ClientViewSequence,
    pub minimum_workbench_revision: WorkbenchRevision,
    pub intent: ClientViewIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewControlReason {
    Busy,
    StaleView,
    TargetUnavailable,
    ProjectionUnavailable,
    Interrupted,
    ClientDisconnected,
    Expired,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewCompletionOutcome {
    Applied {},
    ZoomUnchanged {},
    ZoomToggled { previously_zoomed: bool },
    Rejected { reason: ClientViewControlReason },
    Incomplete { reason: ClientViewControlReason },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewCompletion {
    pub operation_id: WorkbenchRequestId,
    pub outcome: ClientViewCompletionOutcome,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewControlStatus {
    Queued {},
    Delivered {},
    Applied {
        view: ClientViewObservation,
    },
    ZoomUnchanged {
        view: ClientViewObservation,
    },
    ZoomToggled {
        previously_zoomed: bool,
        view: ClientViewObservation,
    },
    Rejected {
        reason: ClientViewControlReason,
    },
    Incomplete {
        reason: ClientViewControlReason,
        #[serde(deserialize_with = "deserialize_nullable")]
        view: Option<ClientViewObservation>,
    },
}
impl ClientViewControlStatus {
    #[must_use]
    pub fn pending(&self) -> bool {
        matches!(self, Self::Queued {} | Self::Delivered {})
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewOperation {
    pub command: ClientViewControl,
    pub status: ClientViewControlStatus,
}
impl ClientViewOperation {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.status {
            ClientViewControlStatus::ZoomToggled {
                previously_zoomed,
                view,
            } => {
                view.validate()?;
                if !self.command.intent.is_toggle_observed(
                    view,
                    self.command.minimum_workbench_revision,
                    *previously_zoomed,
                ) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::Applied { view } => {
                view.validate()?;
                if !self
                    .command
                    .intent
                    .is_observed(view, self.command.minimum_workbench_revision)
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::ZoomUnchanged { view } => {
                view.validate()?;
                if !self
                    .command
                    .intent
                    .is_single_panel_zoom_observed(view, self.command.minimum_workbench_revision)
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::Incomplete {
                view: Some(view), ..
            } => view.validate()?,
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewAction {
    Synchronize {
        heartbeat: ClientViewSequence,
        view_revision: ClientViewSequence,
        view: Box<ClientViewObservation>,
        #[serde(deserialize_with = "deserialize_nullable")]
        completion: Option<ClientViewCompletion>,
    },
    List {},
    Get {
        client_id: WorkbenchClientId,
    },
    Submit {
        command: Box<ClientViewControl>,
    },
    Read {
        operation_id: WorkbenchRequestId,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchClientViewRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: ClientViewAction,
}
impl WorkbenchClientViewRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let ClientViewAction::Synchronize { view, .. } = &self.action {
            view.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewUnavailable {
    ClientUnavailable,
    ServerBusy,
    ReceiptCapacity,
    Unsupported,
    SequenceExhausted,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewOutcome {
    Synchronized {
        client: Box<ClientViewRecord>,
        #[serde(deserialize_with = "deserialize_nullable")]
        control: Option<Box<ClientViewControl>>,
    },
    Listed {
        clients: Vec<ClientViewRecord>,
    },
    Found {
        client: Box<ClientViewRecord>,
    },
    Operation {
        operation: Box<ClientViewOperation>,
    },
    NotFound {},
    Conflict {},
    Denied {},
    Unavailable {
        reason: ClientViewUnavailable,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchClientViewResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: ClientViewOutcome,
}
impl WorkbenchClientViewResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            ClientViewOutcome::Synchronized { client, control } => {
                client.view.validate()?;
                if control.as_ref().is_some_and(|control| {
                    control.client_id != client.client_id
                        || control.expected_view_revision != client.view_revision
                        || client.availability != ClientViewAvailability::Current
                }) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewOutcome::Found { client } => client.view.validate()?,
            ClientViewOutcome::Listed { clients } => {
                if clients.len() > MAXIMUM_CLIENT_VIEWS {
                    return Err(ContractError::InvalidDocument);
                }
                let mut seen = BTreeSet::new();
                for client in clients {
                    client.view.validate()?;
                    if !seen.insert(&client.client_id) {
                        return Err(ContractError::InvalidDocument);
                    }
                }
            }
            ClientViewOutcome::Operation { operation } => operation.validate()?,
            _ => {}
        }
        Ok(())
    }
}
