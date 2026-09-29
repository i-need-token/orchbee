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
    if let orchbee_capability_node_protocol::PluginJobAction::Start(intent) = &mut action {
        if !std::path::Path::new(&intent.directory).is_absolute() {
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
            intent.environment.retain(|(k, _)| {
                !matches!(k.as_str(), "ORCHBEE_RUNTIME_BASE" | "ORCHBEE_RUNTIME_SCOPE")
            });
            intent.environment.extend([
                ("ORCHBEE_RUNTIME_BASE".into(), base.into()),
                ("ORCHBEE_RUNTIME_SCOPE".into(), scope.into()),
            ]);
        }
    }
    Ok(action)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use orchbee_capability_node_protocol::{PluginCommandIntent, PluginJobAction};
    use std::path::Path;

    #[test]
    fn plugin_route_uses_owned_site_and_rejects_invalid_paths_before_execution() {
        let intent = PluginCommandIntent {
            plugin_id: "tools".into(),
            command_id: "hello".into(),
            directory: "C:/plugins/tools".into(),
            argv: vec!["cmd.exe".into()],
            environment: vec![
                ("ORCHBEE_RUNTIME_BASE".into(), "C:/untrusted".into()),
                ("ORCHBEE_RUNTIME_SCOPE".into(), "other".into()),
                ("ORCHBEE_PLUGIN_ID".into(), "tools".into()),
            ],
        };
        let PluginJobAction::Start(routed) = route(
            PluginJobAction::Start(intent.clone()),
            Some(Path::new("C:/owned-runtime/default")),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            routed.environment,
            vec![
                ("ORCHBEE_PLUGIN_ID".into(), "tools".into()),
                ("ORCHBEE_RUNTIME_BASE".into(), "C:/owned-runtime".into()),
                ("ORCHBEE_RUNTIME_SCOPE".into(), "default".into()),
            ]
        );
        assert!(
            route(
                PluginJobAction::Start(intent.clone()),
                Some(Path::new("relative/default"))
            )
            .is_err()
        );
        let mut relative = intent;
        relative.directory = "relative".into();
        assert!(route(PluginJobAction::Start(relative), None).is_err());
    }
}
