//! One ephemeral Session override; validation and plugin inspection precede commit.
use super::WorkbenchService;
use orchbee_workbench_protocol::{
    AgentViewAction, AgentViewFailure as Failure, AgentViewOutcome, AgentViewRequest,
    AgentViewResult, AgentViewSnapshot, ProtocolVersion, WorkbenchRevision,
};

pub(super) struct Registry {
    snapshot: AgentViewSnapshot,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            snapshot: AgentViewSnapshot {
                revision: WorkbenchRevision::new(0),
                spec: None,
            },
        }
    }
}
impl Registry {
    fn apply(
        &mut self,
        action: &AgentViewAction,
        mut plugin: impl FnMut(&str) -> Result<(), Failure>,
    ) -> Result<AgentViewSnapshot, Failure> {
        action.validate().map_err(|_| Failure::InvalidRequest)?;
        let next = match action {
            AgentViewAction::Read {} => {
                if let Some(spec) = &self.snapshot.spec
                    && let Some(id) = spec.source.strip_prefix("plugin:")
                {
                    match plugin(id) {
                        Ok(()) => return Ok(self.snapshot.clone()),
                        Err(Failure::PluginNotFound | Failure::PluginDisabled) => None,
                        Err(e) => return Err(e),
                    }
                } else {
                    return Ok(self.snapshot.clone());
                }
            }
            AgentViewAction::Set { spec } => {
                let spec = spec.normalized().map_err(|_| Failure::InvalidRequest)?;
                if let Some(id) = spec.source.strip_prefix("plugin:") {
                    plugin(id)?;
                }
                Some(spec)
            }
            AgentViewAction::Clear { source } => {
                if source.as_ref().is_some_and(|s| {
                    self.snapshot
                        .spec
                        .as_ref()
                        .is_none_or(|view| view.source != s.trim())
                }) {
                    return Ok(self.snapshot.clone());
                }
                None
            }
        };
        if self.snapshot.spec != next {
            let revision = self
                .snapshot
                .revision
                .value()
                .checked_add(1)
                .filter(|n| *n <= 9_007_199_254_740_991)
                .ok_or(Failure::RevisionExhausted)?;
            self.snapshot = AgentViewSnapshot {
                revision: WorkbenchRevision::new(revision),
                spec: next,
            };
        }
        Ok(self.snapshot.clone())
    }
}
fn plugin(id: &str) -> Result<(), Failure> {
    use orchbee_local_runtime::{PluginRegistry, plugin_identifier};
    plugin_identifier(id).map_err(|_| Failure::InvalidRequest)?;
    let registry = PluginRegistry::from_environment(|name| std::env::var_os(name))
        .map_err(|_| Failure::SourceUnavailable)?;
    plugin_from_registry(&registry, id)
}
fn plugin_from_registry(
    registry: &orchbee_local_runtime::PluginRegistry,
    id: &str,
) -> Result<(), Failure> {
    use orchbee_local_runtime::PluginRegistryError;
    let catalog = registry.list(Some(id)).map_err(|error| match error {
        PluginRegistryError::PluginNotFound => Failure::PluginNotFound,
        _ => Failure::SourceUnavailable,
    })?;
    let entry = catalog
        .plugins
        .iter()
        .find(|p| p.manifest.id == id)
        .ok_or(Failure::PluginNotFound)?;
    if !entry.enabled {
        return Err(Failure::PluginDisabled);
    }
    if entry.warning.is_some() {
        return Err(Failure::SourceUnavailable);
    }
    Ok(())
}
impl WorkbenchService {
    pub(super) fn agent_view(&self, request: &AgentViewRequest) -> AgentViewResult {
        let outcome = match self
            .terminals
            .workbench_session_matches(&request.session_id)
        {
            Some(true) => self
                .agent_views
                .lock()
                .map_err(|_| Failure::Unavailable)
                .and_then(|mut views| views.apply(&request.action, plugin)),
            Some(false) => Err(Failure::SessionMismatch),
            None => Err(Failure::Unavailable),
        };
        AgentViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            outcome: match outcome {
                Ok(snapshot) => AgentViewOutcome::Available { snapshot },
                Err(reason) => AgentViewOutcome::Failed { reason },
            },
        }
    }
}
#[cfg(test)]
mod tests;
