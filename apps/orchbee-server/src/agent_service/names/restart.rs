//! Completed launch ownership crosses a restart only through fresh physical bindings.
use super::{Names, TerminalNameFact, launch::launch_binding, terminal::TerminalName};
use crate::{
    agent::native_observation::Binding,
    agent_service::{AgentService, PreparedUnavailable},
};
use orchbee_agent_observation_protocol::{
    AgentName, AgentNameTarget, AgentProviderId, AgentReportProvider, AgentResolveOutcome,
    AgentSessionSelector, MAXIMUM_NAME_BINDINGS,
};
use orchbee_server::ManagedAgentRestart;
use orchbee_workbench_protocol::PanelAgentSession;

#[derive(Clone)]
pub(super) struct RestoredManagement {
    pub(super) provider: AgentProviderId,
    owned_session: PanelAgentSession,
}

#[derive(Clone)]
pub(crate) struct ManagedNameClaim {
    pub(crate) name: AgentName,
    pub(crate) provider: AgentProviderId,
    pub(crate) binding: Binding,
    pub(crate) owned_session: Option<PanelAgentSession>,
}

impl ManagedNameClaim {
    pub(crate) fn matches_release(
        &self,
        name: &AgentName,
        binding: &orchbee_agent_observation_protocol::AgentLaunchBinding,
    ) -> bool {
        &self.name == name && launch_binding(&self.binding) == *binding
    }
}

impl Names {
    pub(super) fn managed_claims(&self) -> Vec<ManagedNameClaim> {
        let mut claims = self.canonical_managed_claims();
        claims.extend(self.terminals.values().filter_map(|entry| {
            entry.managed.as_ref().map(|managed| ManagedNameClaim {
                name: entry.name.clone(),
                provider: managed.provider.clone(),
                binding: entry.binding.clone(),
                owned_session: Some(managed.owned_session.clone()),
            })
        }));
        claims
    }

    pub(super) fn restore_managed(
        &mut self,
        seeds: &[(ManagedAgentRestart, PanelAgentSession)],
        facts: &[TerminalNameFact],
    ) -> Result<(), PreparedUnavailable> {
        // Validate the entire insertion before changing the shared namespace.
        let mut restored = Vec::new();
        let mut names = std::collections::BTreeSet::new();
        let mut presentations = std::collections::BTreeSet::new();
        for (claim, session) in seeds {
            let fact = facts
                .iter()
                .find(|fact| fact.binding.presentation.as_str() == claim.presentation_id.as_str())
                .ok_or(PreparedUnavailable)?;
            let id = launch_binding(&fact.binding).presentation_id;
            let provider = session.agent.as_str();
            if fact.owner.clear
                || fact.owner.agent.as_deref() != Some(provider)
                || AgentReportProvider::lookup(provider)
                    .is_none_or(|p| p.native_provider_id() != claim.provider_id.as_str())
                || !names.insert(claim.name.clone())
                || !presentations.insert(id.clone())
                || self.by_name.contains_key(&claim.name)
                || self.pending_name(claim.name.as_str())
                || self.terminal_claim(claim.name.as_str()).is_some()
                || self.terminals.contains_key(&id)
                || self.terminal_pending(&id)
            {
                return Err(PreparedUnavailable);
            }
            restored.push((
                id,
                TerminalName {
                    name: claim.name.clone(),
                    binding: fact.binding.clone(),
                    owner: fact.owner.clone(),
                    managed: Some(RestoredManagement {
                        provider: claim.provider_id.clone(),
                        owned_session: session.clone(),
                    }),
                },
            ));
        }
        if self.total_bindings() + restored.len() > MAXIMUM_NAME_BINDINGS {
            return Err(PreparedUnavailable);
        }
        self.terminals.extend(restored);
        Ok(())
    }
}

impl AgentService {
    pub(crate) fn managed_name_claims(&self) -> Result<Vec<ManagedNameClaim>, PreparedUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                Err(PreparedUnavailable)
            } else {
                Ok(state.names.managed_claims())
            }
        })
        .map_err(|_| PreparedUnavailable)?
    }

    #[cfg(windows)]
    pub(crate) fn restore_managed_names(
        &self,
        seeds: &[(ManagedAgentRestart, PanelAgentSession)],
        facts: &[TerminalNameFact],
    ) -> Result<(), PreparedUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                Err(PreparedUnavailable)
            } else {
                state.names.restore_managed(seeds, facts)
            }
        })
        .map_err(|_| PreparedUnavailable)?
    }

    #[cfg(windows)]
    pub(crate) fn managed_name_release(
        &self,
        target: &AgentSessionSelector,
        facts: &[TerminalNameFact],
    ) -> Result<Option<ManagedNameClaim>, PreparedUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                return Err(PreparedUnavailable);
            }
            state.names.reconcile_terminals(facts);
            let AgentResolveOutcome::ResolvedTarget {
                target: AgentNameTarget::Terminal { binding },
                ..
            } = state
                .names
                .resolve_named(state.runtime.snapshot(), target, facts)
            else {
                return Ok(None);
            };
            Ok(state.names.managed_claims().into_iter().find(|claim| {
                launch_binding(&claim.binding) == binding && claim.owned_session.is_some()
            }))
        })
        .map_err(|_| PreparedUnavailable)?
    }
}
