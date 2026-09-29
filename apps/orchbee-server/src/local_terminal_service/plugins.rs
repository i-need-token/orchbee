//! Authorization precedes manifest resolution; only the selected Worker owns subprocesses.
use super::LocalTerminalService;
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError,
};
use orchbee_capability_node_protocol as node;
use orchbee_local_site_runtime::PluginRegistry;
use orchbee_workbench_protocol::{
    PluginAction, PluginCommandLog, PluginFailure as Failure, PluginInvocationContext,
    PluginLogCursor, PluginOutcome as Outcome, PluginRequest, PluginResult, PluginWorker,
    ProtocolVersion, WorkbenchRevision, WorkerId,
};
impl LocalTerminalService {
    pub(crate) fn execute_plugin(&self, request: PluginRequest) -> PluginResult {
        let outcome = self
            .plugin_operation(&request)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        PluginResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
    fn plugin_operation(&self, request: &PluginRequest) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidIntent)?;
        if self.workbench_site_matches(&request.site_id) != Some(true) {
            return Err(Failure::SiteMismatch);
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
            _ => None,
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
            } => {
                let registry = PluginRegistry::from_environment(|name| std::env::var_os(name))
                    .map_err(|_| Failure::SourceUnavailable)?;
                let intent = resolve(&registry, plugin_id.as_deref(), action_id, context)?;
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
                    _ => unreachable!(),
                };
                (id, action)
            }
        };
        let dispatch = node::PluginJobRequest {
            operation_id,
            worker_id: node::WorkerId::from_bytes(handle.worker_id().as_bytes())
                .map_err(|_| Failure::Unavailable)?,
            worker_generation: node::WorkerGeneration::new(handle.generation()),
            action,
        };
        let result = match handle.request(WorkerOperationRequest::PluginJob(dispatch.clone())) {
            Ok(WorkerOperationResult::PluginJob(result)) if result.answers(&dispatch) => result,
            Err(WorkerRequestError::Backpressure) => return Err(Failure::Busy),
            Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) | Ok(_) => {
                return if request.action.mutating() {
                    Ok(Outcome::Unknown { worker })
                } else {
                    Err(Failure::Unavailable)
                };
            }
            Err(_) => return Err(Failure::Unavailable),
        };
        Ok(match result.outcome {
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
) -> Result<node::PluginCommandIntent, Failure> {
    let catalog = registry
        .list(plugin_id)
        .map_err(|_| Failure::SourceUnavailable)?;
    let mut candidates = catalog.plugins.iter().flat_map(|entry| {
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
    let (config, state) = registry
        .user_directories(&entry.manifest.id)
        .map_err(|_| Failure::SourceUnavailable)?;
    let path = |p: &std::path::Path| p.to_str().map(String::from).ok_or(Failure::InvalidIntent);
    let executable = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("orchbee.exe")))
        .ok_or(Failure::Unavailable)?;
    let mut environment = vec![
        ("ORCHBEE_PLUGIN_ID".into(), entry.manifest.id.clone()),
        ("ORCHBEE_PLUGIN_ROOT".into(), entry.plugin_root.clone()),
        ("ORCHBEE_PLUGIN_CONFIG_DIR".into(), path(&config)?),
        ("ORCHBEE_PLUGIN_STATE_DIR".into(), path(&state)?),
        ("ORCHBEE_PLUGIN_ACTION_ID".into(), action.id.clone()),
        (
            "ORCHBEE_PLUGIN_CONTEXT_JSON".into(),
            serde_json::to_string(context).map_err(|_| Failure::InvalidIntent)?,
        ),
        ("ORCHBEE_BIN_PATH".into(), path(&executable)?),
    ];
    for (name, value) in [
        ("ORCHBEE_WORKSPACE_ID", &context.workspace_id),
        ("ORCHBEE_TAB_ID", &context.tab_id),
        ("ORCHBEE_PANE_ID", &context.focused_pane_id),
        ("ORCHBEE_PLUGIN_CLICKED_URL", &context.clicked_url),
        ("ORCHBEE_PLUGIN_LINK_HANDLER_ID", &context.link_handler_id),
    ] {
        if let Some(value) = value {
            environment.push((name.into(), value.clone()));
        }
    }
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
