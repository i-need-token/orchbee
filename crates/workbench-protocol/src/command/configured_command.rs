//! Installed command selectors, source fences and distinct execution receipts.
use crate::{
    ClientPopupSource, ClientViewSequence, ContractError, PluginInvocationContext, PluginPopupSize,
    PluginResult, PluginWorker, ProtocolVersion, SessionId, WorkbenchClientId,
    WorkbenchClientViewResult, WorkbenchCommandResult, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTerminalGeometry,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_CONFIGURED_COMMAND_BYTES: usize = 256 * 1024;
/// UTF-8 bytes a descriptor's `display` text may hold.
pub const MAXIMUM_CONFIGURED_COMMAND_DISPLAY_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfiguredCommandKind {
    Shell,
    Pane,
    Popup,
    PluginAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandDescriptor {
    pub id: String,
    pub action: ConfiguredCommandKind,
    pub bindings: Vec<String>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub description: Option<String>,
    /// The command line or plugin target as the TUI's key help shows it: display text only, never
    /// an executable, bounded and free of control characters.
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub display: Option<String>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub width: Option<PluginPopupSize>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub height: Option<PluginPopupSize>,
}
fn identifier(id: &str) -> Result<(), ContractError> {
    if id.len() > 120
        || !id.starts_with("cmd_")
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}
fn dimensions(
    width: Option<PluginPopupSize>,
    height: Option<PluginPopupSize>,
) -> Result<(), ContractError> {
    for size in [width, height].into_iter().flatten() {
        size.validate()?;
        if matches!(size, PluginPopupSize::Cells(cells) if !(1..=4096).contains(&cells)) {
            return Err(ContractError::InvalidDocument);
        }
    }
    Ok(())
}
impl ConfiguredCommandDescriptor {
    pub fn validate(&self) -> Result<(), ContractError> {
        identifier(&self.id)?;
        dimensions(self.width, self.height)?;
        if self.bindings.len() > 16
            || self.bindings.iter().any(|value| {
                value.is_empty() || value.len() > 128 || value.chars().any(char::is_control)
            })
            || self.description.as_ref().is_some_and(|value| {
                value.trim().is_empty() || value.len() > 512 || value.contains('\0')
            })
            || self.display.as_ref().is_some_and(|value| {
                value.trim().is_empty()
                    || value.len() > MAXIMUM_CONFIGURED_COMMAND_DISPLAY_BYTES
                    || value.chars().any(char::is_control)
            })
            || self.action != ConfiguredCommandKind::Popup
                && (self.width.is_some() || self.height.is_some())
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandSource {
    pub client_id: WorkbenchClientId,
    pub view_revision: ClientViewSequence,
    pub presentation: ClientPopupSource,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandInvocation {
    pub command_id: String,
    pub expected_revision: WorkbenchRevision,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub source: Option<ConfiguredCommandSource>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub selection: Option<ConfiguredCommandSelection>,
    pub context: Box<PluginInvocationContext>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandTextPoint {
    pub row: WorkbenchRevision,
    pub col: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandSelection {
    pub anchor: ConfiguredCommandTextPoint,
    pub cursor: ConfiguredCommandTextPoint,
    pub revision: WorkbenchRevision,
}
impl ConfiguredCommandSelection {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.anchor.col >= 4096
            || self.cursor.col >= 4096
            || self.anchor.row.value().abs_diff(self.cursor.row.value()) >= 4096
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
impl ConfiguredCommandInvocation {
    pub fn validate(&self) -> Result<(), ContractError> {
        identifier(&self.command_id)?;
        self.context.validate()?;
        if let Some(selection) = self.selection {
            selection.validate()?;
            if self.source.is_none() {
                return Err(ContractError::InvalidDocument);
            }
        }
        if let Some(source) = &self.source {
            source.presentation.member.validate()?;
            if source.presentation.member.binding().is_none()
                || self.context.tab_id.as_deref() != Some(source.presentation.tab_id.as_str())
                || self.context.focused_pane_id.as_deref()
                    != Some(source.presentation.panel_id.as_str())
            {
                return Err(ContractError::InvalidDocument);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandPopupSpec {
    pub command_id: String,
    pub source: ClientPopupSource,
    pub context: Box<PluginInvocationContext>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub width: Option<PluginPopupSize>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub height: Option<PluginPopupSize>,
}
impl ConfiguredCommandPopupSpec {
    pub fn validate(&self) -> Result<(), ContractError> {
        identifier(&self.command_id)?;
        self.source.member.validate()?;
        self.context.validate()?;
        dimensions(self.width, self.height)?;
        if self.source.member.binding().is_none()
            || self.context.tab_id.as_deref() != Some(self.source.tab_id.as_str())
            || self.context.focused_pane_id.as_deref() != Some(self.source.panel_id.as_str())
        {
            return Err(ContractError::InvalidDocument);
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
pub enum ConfiguredCommandAction {
    List {},
    Invoke {
        invocation: Box<ConfiguredCommandInvocation>,
    },
    PopupOpen {
        popup_id: WorkbenchRequestId,
        spec: Box<ConfiguredCommandPopupSpec>,
        geometry: WorkbenchTerminalGeometry,
    },
    PopupRead {
        popup_id: WorkbenchRequestId,
    },
    PopupClose {
        popup_id: WorkbenchRequestId,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: ConfiguredCommandAction,
}
impl ConfiguredCommandRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
            ConfiguredCommandAction::Invoke { invocation } => invocation.validate(),
            ConfiguredCommandAction::PopupOpen { spec, .. } => spec.validate(),
            ConfiguredCommandAction::List {}
            | ConfiguredCommandAction::PopupRead { .. }
            | ConfiguredCommandAction::PopupClose { .. } => Ok(()),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfiguredCommandFailure {
    Denied,
    InvalidIntent,
    #[serde(rename = "session-mismatch")]
    SessionMismatch,
    Unavailable,
    Busy,
    Conflict,
    NotFound,
    StaleSource,
    PhysicalUnknown,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConfiguredCommandOutcome {
    Catalog {
        revision: WorkbenchRevision,
        entries: Vec<ConfiguredCommandDescriptor>,
    },
    ShellStarted {
        command_id: String,
        worker: PluginWorker,
        job_id: WorkbenchRequestId,
        existing: bool,
    },
    Pane {
        command_id: String,
        result: Box<WorkbenchCommandResult>,
    },
    Plugin {
        command_id: String,
        result: Box<PluginResult>,
    },
    PopupControl {
        command_id: String,
        result: Box<WorkbenchClientViewResult>,
    },
    PopupOpened {
        popup_id: WorkbenchRequestId,
        title: String,
    },
    PopupState {
        popup_id: WorkbenchRequestId,
        settled: bool,
    },
    PopupClosing {
        popup_id: WorkbenchRequestId,
    },
    PopupClosed {
        popup_id: WorkbenchRequestId,
    },
    PopupUnknown {
        popup_id: WorkbenchRequestId,
    },
    Failed {
        reason: ConfiguredCommandFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredCommandResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub outcome: ConfiguredCommandOutcome,
}
impl ConfiguredCommandResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        use ConfiguredCommandOutcome as O;
        match &self.outcome {
            O::Catalog { entries, .. } => {
                if entries.len() > 64 {
                    return Err(ContractError::InvalidDocument);
                }
                let mut ids = std::collections::BTreeSet::new();
                for entry in entries {
                    entry.validate()?;
                    if !ids.insert(&entry.id) {
                        return Err(ContractError::InvalidDocument);
                    }
                }
            }
            O::ShellStarted {
                command_id,
                job_id,
                worker,
                ..
            } => {
                identifier(command_id)?;
                if worker.generation.value() == 0 {
                    return Err(ContractError::InvalidDocument);
                }
                if job_id != &self.request_id {
                    return Err(ContractError::InvalidDocument);
                }
            }
            O::Pane { command_id, result } => {
                identifier(command_id)?;
                if result.request_id != self.request_id
                    || !matches!(
                        result.outcome,
                        crate::WorkbenchCommandOutcome::PanelSplitPending { .. }
                            | crate::WorkbenchCommandOutcome::PanelSplitCreated { .. }
                            | crate::WorkbenchCommandOutcome::Rejected { .. }
                            | crate::WorkbenchCommandOutcome::Denied {}
                            | crate::WorkbenchCommandOutcome::Unavailable { .. }
                    )
                {
                    return Err(ContractError::InvalidDocument);
                }
                crate::serialize_workbench_response(&crate::WorkbenchResponse::Command(
                    (**result).clone(),
                ))?;
            }
            O::Plugin { command_id, result } => {
                identifier(command_id)?;
                result.validate()?;
                if result.request_id != self.request_id
                    || result.session_id != self.session_id
                    || !matches!(
                        result.outcome,
                        crate::PluginOutcome::Started { .. }
                            | crate::PluginOutcome::Failed { .. }
                            | crate::PluginOutcome::Unknown { .. }
                    )
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            O::PopupControl { command_id, result } => {
                identifier(command_id)?;
                result.validate()?;
                if result.request_id != self.request_id || result.session_id != self.session_id {
                    return Err(ContractError::InvalidDocument);
                }
                match &result.outcome {
                    crate::ClientViewOutcome::Operation { operation } => {
                        if operation.command.operation_id != self.request_id
                            || !matches!(&operation.command.intent,
                                crate::ClientViewIntent::CommandPopupOpen { spec }
                                if spec.command_id == *command_id)
                        {
                            return Err(ContractError::InvalidDocument);
                        }
                    }
                    crate::ClientViewOutcome::NotFound {}
                    | crate::ClientViewOutcome::Conflict {}
                    | crate::ClientViewOutcome::Denied {}
                    | crate::ClientViewOutcome::Unavailable { .. } => {}
                    crate::ClientViewOutcome::Synchronized { .. }
                    | crate::ClientViewOutcome::Listed { .. }
                    | crate::ClientViewOutcome::Found { .. } => {
                        return Err(ContractError::InvalidDocument);
                    }
                }
            }
            O::PopupOpened { title, .. }
                if title.is_empty() || title.len() > 4096 || title.contains('\0') =>
            {
                return Err(ContractError::InvalidDocument);
            }
            O::PopupOpened { .. }
            | O::PopupState { .. }
            | O::PopupClosing { .. }
            | O::PopupClosed { .. }
            | O::PopupUnknown { .. }
            | O::Failed { .. } => {}
        }
        Ok(())
    }
    #[must_use]
    pub fn answers(&self, request: &ConfiguredCommandRequest) -> bool {
        use ConfiguredCommandAction as A;
        use ConfiguredCommandOutcome as O;
        self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.validate().is_ok()
            && {
                let shared_failure = matches!(&self.outcome, O::Failed { .. });
                match &request.action {
                    A::List {} => matches!(&self.outcome, O::Catalog { .. }) || shared_failure,
                    A::Invoke { invocation } => {
                        matches!(&self.outcome, O::ShellStarted { command_id, .. }
                    | O::Pane { command_id, .. }
                    | O::Plugin { command_id, .. } if command_id == &invocation.command_id)
                            || matches!(&self.outcome, O::PopupControl { command_id, result } if {
                                command_id == &invocation.command_id
                                    && match &result.outcome {
                                        crate::ClientViewOutcome::Operation { operation } => {
                                            invocation.source.as_ref().is_some_and(|source| {
                                                operation.command.client_id == source.client_id
                                                    && operation.command.expected_view_revision
                                                        == source.view_revision
                                                    && operation.command.minimum_workbench_revision
                                                        == invocation.expected_revision
                                                    && matches!(&operation.command.intent,
                                                crate::ClientViewIntent::CommandPopupOpen { spec }
                                                if spec.source == source.presentation)
                                            })
                                        }
                                        // Only an operation receipt echoes the popup source; refusals carry none.
                                        crate::ClientViewOutcome::Synchronized { .. } | crate::ClientViewOutcome::Listed { .. } | crate::ClientViewOutcome::Found { .. } | crate::ClientViewOutcome::NotFound { .. } | crate::ClientViewOutcome::Conflict { .. } | crate::ClientViewOutcome::Denied { .. } | crate::ClientViewOutcome::Unavailable { .. } => true,
                                    }
                            })
                            || shared_failure
                    }
                    A::PopupOpen { popup_id, .. } => {
                        matches!(&self.outcome, O::PopupOpened {
                        popup_id: actual, ..
                    } if popup_id == actual)
                            || matches!(&self.outcome, O::PopupClosed { popup_id: actual } | O::PopupUnknown { popup_id: actual } if popup_id == actual)
                            || shared_failure
                    }
                    A::PopupRead { popup_id } => {
                        matches!(&self.outcome, O::PopupState {
                        popup_id: actual, ..
                    } if popup_id == actual)
                            || matches!(&self.outcome, O::PopupClosing { popup_id: actual } if popup_id == actual)
                            || matches!(&self.outcome, O::PopupClosed { popup_id: actual } | O::PopupUnknown { popup_id: actual } if popup_id == actual)
                            || shared_failure
                    }
                    A::PopupClose { popup_id } => {
                        matches!(&self.outcome, O::PopupClosing { popup_id: actual } if popup_id == actual)
                            || matches!(&self.outcome, O::PopupClosed { popup_id: actual } | O::PopupUnknown { popup_id: actual } if popup_id == actual)
                            || shared_failure
                    }
                }
            }
    }
}
