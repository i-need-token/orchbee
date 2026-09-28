//! Names finalize under the drain reader's existing registry/coordinator commit guards.
use super::{AGENT_DRAIN_READ_WINDOW, LocalTerminalService};
use crate::agent_service::AgentService;
use orchbee_agent_observation_protocol::{AgentNamesRequest, AgentNamesResult};
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

impl LocalTerminalService {
    pub(crate) fn bind_agent_names(&self, agents: &Arc<AgentService>) -> io::Result<()> {
        self.coordinator
            .lock()
            .map_err(|_| io::Error::other("Site coordinator unavailable"))?
            .bind_agent_names(agents)
    }
    pub(crate) fn execute_terminal_management(
        &self,
        agents: &AgentService,
        request: orchbee_agent_observation_protocol::AgentLaunchRequest,
    ) -> orchbee_agent_observation_protocol::AgentLaunchResult {
        agents.execute_launch_guarded(request, |request| {
            super::agent_drain_read::agent_drain_finish(
                &self.workers,
                &self.coordinator,
                agents,
                Instant::now() + AGENT_DRAIN_READ_WINDOW,
                Duration::from_millis(25),
                |_, _| agents.fold_launch(request),
            )
        })
    }

    pub(crate) fn execute_agent_names(
        &self,
        agents: &AgentService,
        request: AgentNamesRequest,
    ) -> AgentNamesResult {
        agents.execute_names_guarded(request, |request| {
            super::agent_drain_read::agent_drain_finish(
                &self.workers,
                &self.coordinator,
                agents,
                Instant::now() + AGENT_DRAIN_READ_WINDOW,
                Duration::from_millis(25),
                |coordinator, _| {
                    let facts = coordinator
                        .terminal_name_facts()
                        .map_err(|_| crate::agent_service::PreparedUnavailable)?;
                    let claim = match request {
                        AgentNamesRequest::NameRelease { target, .. } => agents.managed_name_release(target, &facts)?,
                        _ => None,
                    };
                    let prepared = claim.as_ref().map(|claim| coordinator.prepare_managed_name_clear(claim)).transpose()?;
                    let result = agents.fold_terminal_names(request, &facts)?;
                    if let (Some(claim), Some(prepared)) = (claim, prepared)
                        && let AgentNamesResult::NameRelease { result, .. } = &result
                        && let orchbee_agent_observation_protocol::AgentNameReleaseOutcome::ReleasedTarget {
                            target: orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding }, name,
                        } = &result.outcome
                        && claim.matches_release(name, binding)
                    {
                        prepared.commit();
                    }
                    Ok(result)
                },
            )
        })
    }
}
