use orchbee_capability_node_protocol::{
    PluginJobOutcome, PluginJobRefusal, PluginJobRequest, PluginJobResult,
};
use orchbee_local_site_protocol::WorkerId;
use std::num::NonZeroU64;
pub(crate) fn execute(
    request: PluginJobRequest,
    worker: WorkerId,
    generation: NonZeroU64,
    #[cfg(windows)] jobs: &crate::plugin_jobs::PluginJobs,
    #[cfg(windows)] runtime: Option<&std::path::Path>,
) -> PluginJobResult {
    let outcome = if request.worker_id.as_bytes() != worker.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        PluginJobOutcome::Refused(PluginJobRefusal::RejectedFencing)
    } else {
        #[cfg(windows)]
        {
            match route(request.action, runtime) {
                Ok(action) => jobs.execute(request.operation_id, action),
                Err(()) => PluginJobOutcome::Refused(PluginJobRefusal::InvalidIntent),
            }
        }
        #[cfg(not(windows))]
        {
            PluginJobOutcome::Refused(PluginJobRefusal::Unsupported)
        }
    };
    PluginJobResult {
        operation_id: request.operation_id,
        worker_id: request.worker_id,
        worker_generation: request.worker_generation,
        outcome,
    }
}

#[cfg(windows)]
fn route(
    mut action: orchbee_capability_node_protocol::PluginJobAction,
    runtime: Option<&std::path::Path>,
) -> Result<orchbee_capability_node_protocol::PluginJobAction, ()> {
    let fields = match &mut action {
        orchbee_capability_node_protocol::PluginJobAction::Start(intent) => {
            Some((&intent.directory, &mut intent.environment))
        }
        orchbee_capability_node_protocol::PluginJobAction::StartConfigured(intent) => {
            Some((&intent.directory, &mut intent.environment))
        }
        _ => None,
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
            orchbee_local_site_runtime::SiteScope::parse(scope).map_err(|_| ())?;
            environment.retain(|(k, _)| {
                !matches!(k.as_str(), "ORCHBEE_RUNTIME_BASE" | "ORCHBEE_RUNTIME_SCOPE")
            });
            environment.extend([
                ("ORCHBEE_RUNTIME_BASE".into(), base.into()),
                ("ORCHBEE_RUNTIME_SCOPE".into(), scope.into()),
            ]);
        }
    }
    Ok(action)
}

#[cfg(all(test, windows))]
mod tests;
