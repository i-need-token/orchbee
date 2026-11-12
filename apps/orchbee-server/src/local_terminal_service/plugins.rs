//! Authorization precedes manifest resolution; only the selected Worker owns subprocesses.
use super::LocalTerminalService;
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError,
};
use orchbee_capability_node_protocol as node;
use orchbee_local_runtime::{PluginRegistry, PluginRegistryEntry};
use orchbee_workbench_protocol::{
    PluginAction, PluginCommandLog, PluginFailure as Failure, PluginInvocationContext,
    PluginLogCursor, PluginOutcome as Outcome, PluginRequest, PluginResult, PluginWorker,
    ProtocolVersion, WorkbenchRevision, WorkerId,
};
#[cfg(windows)]
mod hooks;
#[cfg(windows)]
pub(super) use hooks::PluginHooks;
mod observation;
impl LocalTerminalService {
    pub(crate) fn execute_plugin(&self, request: PluginRequest) -> PluginResult {
        let outcome = self
            .plugin_operation(&request)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        PluginResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
    fn plugin_operation(&self, request: &PluginRequest) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidIntent)?;
        if self.workbench_session_matches(&request.session_id) != Some(true) {
            return Err(Failure::SessionMismatch);
        }
        if matches!(
            request.action,
            PluginAction::PopupOpen { .. }
                | PluginAction::PopupRead { .. }
                | PluginAction::PopupClose { .. }
        ) {
            return Err(Failure::Denied);
        }
        if let PluginAction::PanelObserve { pane } = &request.action {
            return self.observe_plugin_pane(request, pane);
        }
        if let PluginAction::PanelGet { panel_id } = &request.action {
            return self
                .coordinator
                .lock()
                .map_err(|_| Failure::Unavailable)?
                .plugin_panel(panel_id)
                .ok_or(Failure::Unavailable);
        }
        let handle = self
            .workers
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .request_handle()
            .ok_or(Failure::Unavailable)?;
        let worker = PluginWorker {
            worker_id: WorkerId::parse(&hex(handle.worker_id().as_bytes()))
                .map_err(|_| Failure::Unavailable)?,
            generation: WorkbenchRevision::new(handle.generation().get()),
        };
        let expected = match &request.action {
            PluginAction::Get { worker, .. } | PluginAction::Cancel { worker, .. } => Some(worker),
            PluginAction::Logs { cursor, .. } => cursor.as_ref().map(|c| &c.worker),
            PluginAction::PopupOpen { .. }
            | PluginAction::PopupRead { .. }
            | PluginAction::PopupClose { .. }
            | PluginAction::PanelObserve { .. }
            | PluginAction::PanelGet { .. }
            | PluginAction::Invoke { .. } => None,
        };
        if expected.is_some_and(|expected| *expected != worker) {
            return Err(Failure::WorkerChanged);
        }
        let (operation_id, action) = match &request.action {
            PluginAction::Invoke {
                operation_id,
                plugin_id,
                action_id,
                context,
                link_catalog,
            } => {
                let registry = PluginRegistry::from_environment(|name| std::env::var_os(name))
                    .map_err(|_| Failure::SourceUnavailable)?;
                let intent = resolve(
                    &registry,
                    plugin_id.as_deref(),
                    action_id,
                    context,
                    link_catalog.as_ref(),
                )?;
                (
                    identity(operation_id)?,
                    node::PluginJobAction::Start(intent),
                )
            }
            action => {
                let value = self
                    .coordinator
                    .lock()
                    .map_err(|_| Failure::Unavailable)?
                    .allocate_identity_block(1)
                    .ok_or(Failure::Unavailable)?;
                let id = node::OperationId::from_bytes(value.to_be_bytes())
                    .map_err(|_| Failure::Unavailable)?;
                let action = match action {
                    PluginAction::Logs {
                        plugin_id,
                        cursor,
                        limit,
                    } => node::PluginJobAction::List {
                        plugin_id: plugin_id.clone(),
                        before: cursor.as_ref().map(|c| c.before),
                        limit: *limit,
                    },
                    PluginAction::Get { job_id, .. } => node::PluginJobAction::Get {
                        job_id: identity(job_id)?,
                    },
                    PluginAction::Cancel { job_id, .. } => node::PluginJobAction::Cancel {
                        job_id: identity(job_id)?,
                    },
                    PluginAction::PopupOpen { .. }
                    | PluginAction::PopupRead { .. }
                    | PluginAction::PopupClose { .. }
                    | PluginAction::PanelObserve { .. }
                    | PluginAction::PanelGet { .. }
                    | PluginAction::Invoke { .. } => unreachable!(),
                };
                (id, action)
            }
        };
        let outcome = match request_plugin_job(&handle, operation_id, action) {
            Ok(outcome) => outcome,
            Err(JobRequestError::Busy) => return Err(Failure::Busy),
            Err(JobRequestError::Unknown) => {
                return if request.action.mutating() {
                    Ok(Outcome::Unknown { worker })
                } else {
                    Err(Failure::Unavailable)
                };
            }
            Err(JobRequestError::Unavailable) => return Err(Failure::Unavailable),
        };
        Ok(match outcome {
            node::PluginJobOutcome::ConfiguredStarted { .. } => return Err(Failure::InvalidIntent),
            node::PluginJobOutcome::Started { log, existing } => Outcome::Started {
                worker,
                log: Box::new(project(*log)),
                existing,
            },
            node::PluginJobOutcome::Logs { logs, next_before } => Outcome::Logs {
                next: next_before.map(|before| PluginLogCursor {
                    worker: worker.clone(),
                    before,
                }),
                worker,
                logs: logs.into_iter().map(project).collect(),
            },
            node::PluginJobOutcome::Cancellation { already_complete } => {
                let PluginAction::Cancel { job_id, .. } = &request.action else {
                    return Err(Failure::InvalidIntent);
                };
                Outcome::Cancellation {
                    worker,
                    job_id: job_id.clone(),
                    already_complete,
                }
            }
            node::PluginJobOutcome::Refused(reason) => Outcome::Failed {
                reason: match reason {
                    node::PluginJobRefusal::RejectedFencing => Failure::WorkerChanged,
                    node::PluginJobRefusal::InvalidIntent => Failure::InvalidIntent,
                    node::PluginJobRefusal::Busy => Failure::Busy,
                    node::PluginJobRefusal::Conflict => Failure::Conflict,
                    node::PluginJobRefusal::Unavailable => Failure::Unavailable,
                    node::PluginJobRefusal::PhysicalUnknown => Failure::PhysicalUnknown,
                    node::PluginJobRefusal::NotFound => Failure::NotFound,
                    node::PluginJobRefusal::Unsupported => Failure::Unsupported,
                },
            },
        })
    }
}
/// A Worker plugin-job exchange without a correlated answer.
enum JobRequestError {
    Busy,
    /// The request may have been delivered; it is never resubmitted.
    Unknown,
    Unavailable,
}
/// One exact-Worker plugin-job exchange, answered only by its own correlated result.
fn request_plugin_job(
    handle: &crate::worker_obcn_carrier::WorkerRequestHandle,
    operation_id: node::OperationId,
    action: node::PluginJobAction,
) -> Result<node::PluginJobOutcome, JobRequestError> {
    let dispatch = node::PluginJobRequest {
        operation_id,
        worker_id: node::WorkerId::from_bytes(handle.worker_id().as_bytes())
            .map_err(|_| JobRequestError::Unavailable)?,
        worker_generation: node::WorkerGeneration::new(handle.generation()),
        action,
    };
    match handle.request(WorkerOperationRequest::PluginJob(dispatch.clone())) {
        Ok(WorkerOperationResult::PluginJob(result)) if result.answers(&dispatch) => {
            Ok(result.outcome)
        }
        Err(WorkerRequestError::Backpressure) => Err(JobRequestError::Busy),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) | Ok(_) => {
            Err(JobRequestError::Unknown)
        }
        Err(_) => Err(JobRequestError::Unavailable),
    }
}
fn identity(value: &str) -> Result<node::OperationId, Failure> {
    u128::from_str_radix(value, 16)
        .ok()
        .and_then(|n| node::OperationId::from_bytes(n.to_be_bytes()).ok())
        .ok_or(Failure::InvalidIntent)
}
fn hex(value: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(value))
}
fn resolve(
    registry: &PluginRegistry,
    plugin_id: Option<&str>,
    action_id: &str,
    context: &PluginInvocationContext,
    link_catalog: Option<&orchbee_workbench_protocol::PluginLinkCatalogGuard>,
) -> Result<node::PluginCommandIntent, Failure> {
    let plugins = if let Some(expected) = link_catalog {
        let catalog = registry
            .list_registered()
            .map_err(|_| Failure::SourceUnavailable)?;
        if catalog.revision != expected.revision || catalog.fingerprint != expected.fingerprint {
            return Err(Failure::Conflict);
        }
        verify_link_selection(&catalog.plugins, plugin_id, action_id, context)?;
        catalog.plugins
    } else {
        registry
            .list(plugin_id)
            .map_err(|_| Failure::SourceUnavailable)?
            .plugins
    };
    let mut candidates = plugins
        .iter()
        .filter(|entry| plugin_id.is_none_or(|id| entry.manifest.id == id))
        .flat_map(|entry| {
            entry
                .manifest
                .actions
                .iter()
                .filter(move |action| {
                    action.id == action_id
                        || format!("{}.{}", entry.manifest.id, action.id) == action_id
                })
                .map(move |action| (entry, action))
        });
    let (entry, action) = candidates.next().ok_or(Failure::NotFound)?;
    if candidates.next().is_some() {
        return Err(Failure::Ambiguous);
    }
    if !entry.enabled {
        return Err(Failure::Disabled);
    }
    if entry.warning.is_some() {
        return Err(Failure::SourceUnavailable);
    }
    if !entry.manifest.supports_windows(&action.platforms) {
        return Err(Failure::Unsupported);
    }
    let mut environment = command_environment(registry, entry, context)?;
    environment.push(("ORCHBEE_PLUGIN_ACTION_ID".into(), action.id.clone()));
    let intent = node::PluginCommandIntent {
        plugin_id: entry.manifest.id.clone(),
        command_id: action.id.clone(),
        directory: entry.plugin_root.clone(),
        argv: action.command.clone(),
        environment,
    };
    intent.validate().map_err(|_| Failure::InvalidIntent)?;
    Ok(intent)
}

/// The environment every plugin command receives: identity, root, private directories, frozen
/// context and the CLI path. Each caller adds its own invocation fields.
fn command_environment(
    registry: &PluginRegistry,
    entry: &PluginRegistryEntry,
    context: &PluginInvocationContext,
) -> Result<Vec<(String, String)>, Failure> {
    let (config, state) = registry
        .user_directories(&entry.manifest.id)
        .map_err(|_| Failure::SourceUnavailable)?;
    let path = |p: &std::path::Path| p.to_str().map(String::from).ok_or(Failure::InvalidIntent);
    let executable = super::cli_executable().ok_or(Failure::Unavailable)?;
    let mut environment = vec![
        ("ORCHBEE_PLUGIN_ID".into(), entry.manifest.id.clone()),
        ("ORCHBEE_PLUGIN_ROOT".into(), entry.process_root()),
        ("ORCHBEE_PLUGIN_CONFIG_DIR".into(), path(&config)?),
        ("ORCHBEE_PLUGIN_STATE_DIR".into(), path(&state)?),
        (
            "ORCHBEE_PLUGIN_CONTEXT_JSON".into(),
            serde_json::to_string(context).map_err(|_| Failure::InvalidIntent)?,
        ),
        ("ORCHBEE_BIN_PATH".into(), path(&executable)?),
    ];
    for (name, value) in [
        ("ORCHBEE_WORKSPACE_ID", &context.workspace_id),
        ("ORCHBEE_TAB_ID", &context.tab_id),
        ("ORCHBEE_PANEL_ID", &context.focused_panel_id),
        ("ORCHBEE_PLUGIN_CLICKED_URL", &context.clicked_url),
        ("ORCHBEE_PLUGIN_LINK_HANDLER_ID", &context.link_handler_id),
    ] {
        if let Some(value) = value {
            environment.push((name.into(), value.clone()));
        }
    }
    Ok(environment)
}

fn verify_link_selection(
    plugins: &[orchbee_local_runtime::PluginRegistryEntry],
    plugin_id: Option<&str>,
    action_id: &str,
    context: &PluginInvocationContext,
) -> Result<(), Failure> {
    let clicked = context
        .clicked_url
        .as_deref()
        .ok_or(Failure::InvalidIntent)?;
    let handler_id = context
        .link_handler_id
        .as_deref()
        .ok_or(Failure::InvalidIntent)?;
    let selected = plugins
        .iter()
        .filter(|entry| entry.enabled && entry.warning.is_none())
        .find_map(|entry| {
            entry.manifest.link_handlers.iter().find_map(|handler| {
                let action = entry
                    .manifest
                    .actions
                    .iter()
                    .find(|action| action.id == handler.action)?;
                if !entry.manifest.supports_windows(&handler.platforms)
                    || !entry.manifest.supports_windows(&action.platforms)
                {
                    return None;
                }
                let pattern = regex::RegexBuilder::new(&handler.pattern)
                    .size_limit(1024 * 1024)
                    .build()
                    .ok()?;
                pattern
                    .is_match(clicked)
                    .then_some((entry, handler, action))
            })
        })
        .ok_or(Failure::Conflict)?;
    if plugin_id != Some(selected.0.manifest.id.as_str())
        || handler_id != selected.1.id
        || action_id != selected.2.id
    {
        return Err(Failure::Conflict);
    }
    Ok(())
}
fn project(log: node::PluginCommandLog) -> PluginCommandLog {
    use orchbee_workbench_protocol::PluginCommandFailure as Failure;
    PluginCommandLog {
        job_id: hex(log.job_id.as_bytes()),
        sequence: log.sequence,
        plugin_id: log.plugin_id,
        command_id: log.command_id,
        command: log.command,
        started_ms: log.started_ms,
        finished_ms: log.finished_ms,
        exit_code: log.exit_code,
        failure: log.failure.map(|failure| match failure {
            node::PluginCommandFailure::ExitFailure => Failure::ExitFailure,
            node::PluginCommandFailure::SpawnUnavailable => Failure::SpawnUnavailable,
            node::PluginCommandFailure::SpawnFailed => Failure::SpawnFailed,
            node::PluginCommandFailure::Io => Failure::Io,
            node::PluginCommandFailure::Cancelled => Failure::Cancelled,
            node::PluginCommandFailure::PhysicalUnknown => Failure::PhysicalUnknown,
        }),
        stdout: String::from_utf8_lossy(&log.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&log.stderr).into_owned(),
        stdout_truncated: log.stdout_truncated,
        stderr_truncated: log.stderr_truncated,
    }
}

#[cfg(all(test, windows))]
mod tests;
