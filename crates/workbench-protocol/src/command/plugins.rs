//! Plugin invocation context is display data; only an admitted Server resolves executable intent.
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision, WorkerId,
};
use serde::{Deserialize, Serialize};
pub const MAXIMUM_ENCODED_PLUGIN_REQUEST_BYTES: usize = 32 * 1024;
pub const MAXIMUM_ENCODED_PLUGIN_RESULT_BYTES: usize = 8 * 1024 * 1024;
const MAX_INTEGER: u64 = 9_007_199_254_740_991;
pub fn plugin_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
pub fn plugin_job_identifier(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && value.bytes().any(|b| b != b'0')
}
/// Display-only repository facts from a native read-only Worktree query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginWorktreeContext {
    pub worker_id: WorkerId,
    pub repository_root: crate::WorkbenchDirectoryPath,
    pub common_directory: crate::WorkbenchDirectoryPath,
    pub repository_name: String,
    pub checkout_path: crate::WorkbenchDirectoryPath,
    pub linked: bool,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginInvocationContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PluginWorktreeContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_pane_cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_pane_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_pane_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clicked_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_handler_id: Option<String>,
}

/// Registered-catalog identity carried only by a preselected link activation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginLinkCatalogGuard {
    pub revision: u64,
    pub fingerprint: String,
}

impl PluginLinkCatalogGuard {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.revision > MAX_INTEGER
            || self.fingerprint.len() != 64
            || !self
                .fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
impl PluginInvocationContext {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.worktree.as_ref().is_some_and(|worktree| {
            worktree.repository_name.is_empty()
                || worktree.repository_name.len() > 4096
                || worktree.repository_name.contains('\0')
        }) {
            return Err(ContractError::InvalidDocument);
        }
        for value in [
            &self.workspace_id,
            &self.workspace_label,
            &self.workspace_cwd,
            &self.tab_id,
            &self.tab_label,
            &self.focused_pane_id,
            &self.focused_pane_cwd,
            &self.focused_pane_agent,
            &self.focused_pane_status,
            &self.invocation_source,
            &self.correlation_id,
            &self.clicked_url,
            &self.link_handler_id,
        ]
        .into_iter()
        .flatten()
        {
            if value.len() > 4096 || value.contains('\0') {
                return Err(ContractError::InvalidDocument);
            }
        }
        if self
            .selected_text
            .as_ref()
            .is_some_and(|s| s.len() > 8192 || s.contains('\0'))
            || serde_json::to_vec(self)
                .map_err(|_| ContractError::InvalidDocument)?
                .len()
                > 16 * 1024
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }

    /// Non-null explicit display values override observations; null means omitted.
    pub fn with_overrides(mut self, provided: Self) -> Self {
        macro_rules! overlay {
            ($($field:ident),+ $(,)?) => { $(self.$field = provided.$field.or(self.$field);)+ };
        }
        overlay!(
            workspace_id,
            workspace_label,
            workspace_cwd,
            worktree,
            tab_id,
            tab_label,
            focused_pane_id,
            focused_pane_cwd,
            focused_pane_agent,
            focused_pane_status,
            selected_text,
            invocation_source,
            correlation_id,
            clicked_url,
            link_handler_id
        );
        self
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginWorker {
    pub worker_id: WorkerId,
    pub generation: WorkbenchRevision,
}
impl PluginWorker {
    fn validate(&self) -> Result<(), ContractError> {
        if self.generation.value() == 0 {
            Err(ContractError::InvalidDocument)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginLogCursor {
    pub worker: PluginWorker,
    pub before: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PluginAction {
    PopupOpen {
        popup_id: WorkbenchRequestId,
        spec: Box<crate::PluginPopupSpec>,
        geometry: crate::WorkbenchTerminalGeometry,
    },
    PopupRead {
        popup_id: WorkbenchRequestId,
    },
    PopupClose {
        popup_id: WorkbenchRequestId,
    },
    PaneObserve {
        pane: Box<PluginPaneInfo>,
    },
    PaneGet {
        panel_id: crate::WorkbenchPanelId,
    },
    Invoke {
        operation_id: String,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        plugin_id: Option<String>,
        action_id: String,
        context: Box<PluginInvocationContext>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_link_catalog",
            skip_serializing_if = "Option::is_none"
        )]
        link_catalog: Option<PluginLinkCatalogGuard>,
    },
    Logs {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        plugin_id: Option<String>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        cursor: Option<PluginLogCursor>,
        limit: u8,
    },
    Get {
        worker: PluginWorker,
        job_id: String,
    },
    Cancel {
        worker: PluginWorker,
        job_id: String,
    },
}

fn deserialize_optional_link_catalog<'de, D>(
    deserializer: D,
) -> Result<Option<PluginLinkCatalogGuard>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    PluginLinkCatalogGuard::deserialize(deserializer).map(Some)
}

impl PluginAction {
    pub fn mutating(&self) -> bool {
        matches!(
            self,
            Self::Invoke { .. }
                | Self::Cancel { .. }
                | Self::PopupOpen { .. }
                | Self::PopupClose { .. }
        )
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::PopupOpen { spec, .. } => spec.validate(),
            Self::PopupRead { .. } | Self::PopupClose { .. } => Ok(()),
            Self::PaneGet { .. } => Ok(()),
            Self::PaneObserve { pane } => pane.validate(),
            Self::Invoke {
                operation_id,
                plugin_id,
                action_id,
                context,
                link_catalog,
            } => {
                if !plugin_job_identifier(operation_id)
                    || !plugin_identifier(action_id, 241)
                    || plugin_id
                        .as_ref()
                        .is_some_and(|id| !plugin_identifier(id, 120))
                {
                    return Err(ContractError::InvalidDocument);
                }
                context.validate()?;
                if let Some(guard) = link_catalog {
                    guard.validate()?;
                    if plugin_id.is_none()
                        || context.invocation_source.as_deref() != Some("link_click")
                        || context.clicked_url.is_none()
                        || context.link_handler_id.is_none()
                    {
                        return Err(ContractError::InvalidDocument);
                    }
                }
                Ok(())
            }
            Self::Logs {
                plugin_id,
                cursor,
                limit,
            } => {
                if !(1..=4).contains(limit)
                    || plugin_id
                        .as_ref()
                        .is_some_and(|id| !plugin_identifier(id, 120))
                {
                    return Err(ContractError::InvalidDocument);
                }
                if let Some(cursor) = cursor {
                    cursor.worker.validate()?;
                    if cursor.before == 0 || cursor.before > MAX_INTEGER {
                        return Err(ContractError::InvalidDocument);
                    }
                }
                Ok(())
            }
            Self::Get { worker, job_id } | Self::Cancel { worker, job_id } => {
                worker.validate()?;
                if !plugin_job_identifier(job_id) {
                    return Err(ContractError::InvalidDocument);
                }
                Ok(())
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: PluginAction,
}
impl PluginRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.action.validate()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginCommandFailure {
    ExitFailure,
    SpawnUnavailable,
    SpawnFailed,
    Io,
    Cancelled,
    PhysicalUnknown,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCommandLog {
    pub job_id: String,
    pub sequence: u64,
    pub plugin_id: String,
    pub command_id: String,
    pub command: Vec<String>,
    pub started_ms: u64,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub finished_ms: Option<u64>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub exit_code: Option<i32>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub failure: Option<PluginCommandFailure>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}
impl PluginCommandLog {
    pub fn validate(&self) -> Result<(), ContractError> {
        if !plugin_job_identifier(&self.job_id)
            || !plugin_identifier(&self.plugin_id, 120)
            || !plugin_identifier(&self.command_id, 241)
            || self.sequence == 0
            || self.sequence > MAX_INTEGER
            || self.started_ms > MAX_INTEGER
            || self
                .finished_ms
                .is_some_and(|n| n < self.started_ms || n > MAX_INTEGER)
            || self.command.is_empty()
            || self.command.len() > 128
            || self
                .command
                .iter()
                .any(|s| s.is_empty() || s.len() > 4096 || s.contains('\0'))
            || self.command.iter().map(String::len).sum::<usize>() > 128 * 1024
            || self.stdout.len() > 3 * 64 * 1024
            || self.stderr.len() > 3 * 64 * 1024
        {
            return Err(ContractError::InvalidDocument);
        }
        let valid = match self.finished_ms {
            None => {
                self.exit_code.is_none()
                    && self.failure.is_none()
                    && self.stdout.is_empty()
                    && self.stderr.is_empty()
                    && !self.stdout_truncated
                    && !self.stderr_truncated
            }
            Some(_) => match self.failure {
                None => self.exit_code == Some(0),
                Some(PluginCommandFailure::ExitFailure) => self.exit_code != Some(0),
                Some(_) => self.exit_code.is_none(),
            },
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginFailure {
    Denied,
    Unavailable,
    SiteMismatch,
    WorkerChanged,
    InvalidIntent,
    NotFound,
    Disabled,
    Unsupported,
    SourceUnavailable,
    Ambiguous,
    Busy,
    Conflict,
    PhysicalUnknown,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PluginOutcome {
    PopupOpened {
        popup_id: WorkbenchRequestId,
        title: String,
    },
    PopupState {
        popup_id: WorkbenchRequestId,
        settled: bool,
    },
    PopupClosed {
        popup_id: WorkbenchRequestId,
    },
    PopupClosing {
        popup_id: WorkbenchRequestId,
    },
    PopupUnknown {
        popup_id: WorkbenchRequestId,
    },
    PaneObserved {
        revision: WorkbenchRevision,
        pane: Box<PluginPaneInfo>,
        settled: bool,
    },
    PaneFound {
        revision: WorkbenchRevision,
        pane: Box<PluginPaneInfo>,
    },
    Started {
        worker: PluginWorker,
        log: Box<PluginCommandLog>,
        existing: bool,
    },
    Logs {
        worker: PluginWorker,
        logs: Vec<PluginCommandLog>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        next: Option<PluginLogCursor>,
    },
    Cancellation {
        worker: PluginWorker,
        job_id: String,
        already_complete: bool,
    },
    Failed {
        reason: PluginFailure,
    },
    Unknown {
        worker: PluginWorker,
    },
}
/// Current canonical ownership, distinct from caller-provided plugin display data.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPaneInfo {
    pub selector: crate::PluginPaneSelector,
    pub terminal_context_id: crate::WorkbenchTerminalContextId,
    pub tab_id: crate::WorkbenchTabId,
    pub panel_id: crate::WorkbenchPanelId,
    pub presentation_id: crate::TerminalPresentationId,
    pub binding: crate::PresentationBinding,
}
impl PluginPaneInfo {
    fn validate(&self) -> Result<(), ContractError> {
        crate::PluginPaneSelector::new(
            self.selector.plugin_id.clone(),
            self.selector.entrypoint_id.clone(),
        )
        .map_err(|_| ContractError::InvalidDocument)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: PluginOutcome,
}
impl PluginResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            PluginOutcome::PopupOpened { title, .. } => {
                crate::WorkbenchLabel::parse(title).map_err(|_| ContractError::InvalidDocument)?;
            }
            PluginOutcome::PopupState { .. }
            | PluginOutcome::PopupClosed { .. }
            | PluginOutcome::PopupUnknown { .. }
            | PluginOutcome::PopupClosing { .. } => {}
            PluginOutcome::PaneFound { revision, pane }
            | PluginOutcome::PaneObserved { revision, pane, .. } => {
                if revision.value() == 0 {
                    return Err(ContractError::InvalidDocument);
                }
                pane.validate()?;
            }
            PluginOutcome::Started { worker, log, .. } => {
                worker.validate()?;
                log.validate()?;
            }
            PluginOutcome::Logs { worker, logs, next } => {
                worker.validate()?;
                if logs.len() > 4
                    || logs
                        .windows(2)
                        .any(|pair| pair[0].sequence <= pair[1].sequence)
                    || next.as_ref().is_some_and(|next| {
                        next.worker != *worker
                            || logs.last().is_none_or(|log| log.sequence != next.before)
                    })
                {
                    return Err(ContractError::InvalidDocument);
                }
                for log in logs {
                    log.validate()?;
                }
            }
            PluginOutcome::Cancellation { worker, job_id, .. } => {
                worker.validate()?;
                if !plugin_job_identifier(job_id) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            PluginOutcome::Unknown { worker } => worker.validate()?,
            PluginOutcome::Failed { .. } => {}
        }
        Ok(())
    }
    pub fn answers(&self, request: &PluginRequest) -> bool {
        self.request_id == request.request_id
            && self.site_id == request.site_id
            && self.validate().is_ok()
            && match (&request.action, &self.outcome) {
                (
                    PluginAction::PopupOpen { popup_id, .. },
                    PluginOutcome::PopupOpened {
                        popup_id: actual, ..
                    },
                )
                | (
                    PluginAction::PopupRead { popup_id },
                    PluginOutcome::PopupState {
                        popup_id: actual, ..
                    },
                )
                | (
                    PluginAction::PopupClose { popup_id },
                    PluginOutcome::PopupClosing { popup_id: actual },
                )
                | (
                    PluginAction::PopupRead { popup_id },
                    PluginOutcome::PopupClosed { popup_id: actual },
                )
                | (
                    PluginAction::PopupClose { popup_id },
                    PluginOutcome::PopupClosed { popup_id: actual },
                )
                | (
                    PluginAction::PopupOpen { popup_id, .. }
                    | PluginAction::PopupClose { popup_id }
                    | PluginAction::PopupRead { popup_id },
                    PluginOutcome::PopupUnknown { popup_id: actual },
                ) => popup_id == actual,
                (
                    PluginAction::PaneObserve { pane },
                    PluginOutcome::PaneObserved { pane: actual, .. },
                ) => pane == actual,
                (PluginAction::PaneGet { panel_id }, PluginOutcome::PaneFound { pane, .. }) => {
                    *panel_id == pane.panel_id
                }
                (_, PluginOutcome::Failed { .. }) => true,
                (PluginAction::Invoke { .. }, PluginOutcome::Unknown { .. }) => true,
                (
                    PluginAction::Cancel { worker, .. },
                    PluginOutcome::Unknown { worker: actual },
                ) => worker == actual,
                (
                    PluginAction::Invoke {
                        operation_id,
                        plugin_id,
                        action_id,
                        ..
                    },
                    PluginOutcome::Started { log, .. },
                ) => {
                    log.job_id == *operation_id
                        && plugin_id.as_ref().is_none_or(|id| *id == log.plugin_id)
                        && (*action_id == log.command_id
                            || *action_id == format!("{}.{}", log.plugin_id, log.command_id))
                }
                (
                    PluginAction::Logs {
                        plugin_id,
                        cursor,
                        limit,
                    },
                    PluginOutcome::Logs { worker, logs, .. },
                ) => {
                    cursor.as_ref().is_none_or(|c| c.worker == *worker)
                        && logs.len() <= usize::from(*limit)
                        && logs.iter().all(|log| {
                            plugin_id.as_ref().is_none_or(|id| *id == log.plugin_id)
                                && cursor.as_ref().is_none_or(|c| log.sequence < c.before)
                        })
                }
                (
                    PluginAction::Get { worker, job_id },
                    PluginOutcome::Logs {
                        worker: actual,
                        logs,
                        next,
                    },
                ) => {
                    worker == actual
                        && logs.len() <= 1
                        && next.is_none()
                        && logs.iter().all(|log| log.job_id == *job_id)
                }
                (
                    PluginAction::Cancel { worker, job_id },
                    PluginOutcome::Cancellation {
                        worker: actual,
                        job_id: actual_id,
                        ..
                    },
                ) => worker == actual && job_id == actual_id,
                _ => false,
            }
    }
}
