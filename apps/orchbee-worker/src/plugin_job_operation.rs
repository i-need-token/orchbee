//! Fences plugin-job requests by Worker identity and generation before dispatching
//! retained-job actions or typed platform refusal. It maps protocol outcomes without executing
//! a process or owning job history.

use orchbee_capability_node_protocol::{
    PluginJobAction, PluginJobOutcome, PluginJobRefusal, PluginJobRequest, PluginJobResult,
};
use orchbee_local_worker_protocol::WorkerId;
use std::num::NonZeroU64;
pub(crate) fn execute(
    request: PluginJobRequest,
    worker: WorkerId,
    generation: NonZeroU64,
    jobs: &crate::plugin_jobs::PluginJobs,
    runtime: Option<&std::path::Path>,
    host: Option<&dyn crate::plugin_jobs::ProgramHost>,
) -> PluginJobResult {
    let outcome = if request.worker_id.as_bytes() != worker.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        PluginJobOutcome::Refused(PluginJobRefusal::RejectedFencing)
    } else if !served(&request.action) {
        PluginJobOutcome::Refused(PluginJobRefusal::Unsupported)
    } else {
        match route(request.action, runtime) {
            Ok(action) => jobs.execute_on(request.operation_id, action, host),
            Err(()) => PluginJobOutcome::Refused(PluginJobRefusal::InvalidIntent),
        }
    };
    PluginJobResult {
        operation_id: request.operation_id,
        worker_id: request.worker_id,
        worker_generation: request.worker_generation,
        outcome,
    }
}

/// Configured commands and the running count are served wherever the Worker runs. Plugin
/// processes stay Windows-only, as plugin stores are. A policy constant rather than a `cfg`, so
/// every branch compiles everywhere.
fn served(action: &PluginJobAction) -> bool {
    match action {
        PluginJobAction::StartConfigured(_) | PluginJobAction::Running => true,
        PluginJobAction::Start(_)
        | PluginJobAction::List { .. }
        | PluginJobAction::Get { .. }
        | PluginJobAction::Cancel { .. } => cfg!(windows),
    }
}

fn route(
    mut action: PluginJobAction,
    runtime: Option<&std::path::Path>,
) -> Result<PluginJobAction, ()> {
    let fields = match &mut action {
        PluginJobAction::Start(intent) => Some((&intent.directory, &mut intent.environment)),
        PluginJobAction::StartConfigured(intent) => {
            Some((&intent.directory, &mut intent.environment))
        }
        PluginJobAction::List { .. }
        | PluginJobAction::Get { .. }
        | PluginJobAction::Cancel { .. }
        | PluginJobAction::Running => None,
    };
    if let Some((directory, environment)) = fields {
        if !std::path::Path::new(directory).is_absolute() {
            return Err(());
        }
        if let Some(runtime) = runtime {
            let base = runtime
                .parent()
                .filter(|p| p.is_absolute())
                .and_then(|p| p.to_str())
                .ok_or(())?;
            let scope = runtime.file_name().and_then(|p| p.to_str()).ok_or(())?;
            orchbee_local_runtime::SessionScope::parse(scope).map_err(|_| ())?;
            let tag = orchbee_local_runtime::ENVIRONMENT_VARIABLE;
            environment.retain(|(k, _)| {
                !matches!(k.as_str(), "ORCHBEE_RUNTIME_BASE" | "ORCHBEE_RUNTIME_SCOPE") && k != tag
            });
            environment.extend([
                ("ORCHBEE_RUNTIME_BASE".into(), base.into()),
                ("ORCHBEE_RUNTIME_SCOPE".into(), scope.into()),
                (tag.into(), orchbee_local_runtime::environment_stem().into()),
            ]);
        }
    }
    Ok(action)
}

#[cfg(all(test, windows))]
mod tests;
