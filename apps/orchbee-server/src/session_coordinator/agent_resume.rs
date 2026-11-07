//! Selects the live Agent session reference each checkpoint retains for a Presentation: Agent
//! candidates joined to the exact Terminal generation, refuted by that generation's command and
//! process evidence, and admitted only when exactly one remains. Reports, process names and screen
//! detection never grant a reference. It does not persist, deduplicate across Presentations or
//! deliver a resume.

use crate::agent_service::AgentResumeCandidate;
use orchbee_agent_observation_protocol::AgentProcessIdentity;
use orchbee_server::RetainedAgentSession;
use std::collections::BTreeSet;

#[cfg(test)]
mod tests;

/// The shell command evidence of one exact Terminal generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandEvidence {
    /// The latest shell-echoed command, still running, with its command id.
    Open([u8; 16]),
    /// The latest shell-echoed command completed and none is open.
    Closed,
    /// The shell never echoed a command, as in an unmarked shell such as cmd.
    Unmarked,
}

impl CommandEvidence {
    /// The evidence a generation's latest shell-echoed command gives; none means unmarked.
    #[cfg(windows)]
    pub(crate) fn of(latest: Option<orchbee_terminal_protocol::TerminalObservedCommand>) -> Self {
        match latest {
            Some(command) if !command.completed => Self::Open(command.id),
            Some(_) => Self::Closed,
            None => Self::Unmarked,
        }
    }
}

/// One Presentation's reference, if any. An open command keeps the candidates its run token names,
/// else the tokenless ones; a completed command keeps only tokenless ones; an unmarked shell keeps
/// all. An Agent process identity keeps its own provider, `None` drops every candidate, and
/// `Unknown` or `Unsupported` refutes nothing. Exactly one distinct `(provider, native key)` that
/// is also a valid id gives the reference; zero or several give none.
pub(crate) fn select(
    candidates: &[AgentResumeCandidate],
    evidence: CommandEvidence,
    identity: &AgentProcessIdentity,
) -> Option<RetainedAgentSession> {
    let tokenless = || candidates.iter().filter(|c| c.run_token.is_none());
    let kept: Vec<&AgentResumeCandidate> = match evidence {
        CommandEvidence::Open(id) => {
            let open = format!("{:032x}", u128::from_be_bytes(id));
            let named: Vec<_> = candidates
                .iter()
                .filter(|c| c.run_token.as_ref().is_some_and(|t| t.as_str() == open))
                .collect();
            if named.is_empty() {
                tokenless().collect()
            } else {
                named
            }
        }
        CommandEvidence::Closed => tokenless().collect(),
        CommandEvidence::Unmarked => candidates.iter().collect(),
    };
    let provider = match identity {
        AgentProcessIdentity::Agent { provider, .. } => Some(provider.native_provider_id()),
        AgentProcessIdentity::None => return None,
        AgentProcessIdentity::Unknown(_) | AgentProcessIdentity::Unsupported(_) => None,
    };
    let mut distinct = kept
        .into_iter()
        .filter(|c| provider.is_none_or(|provider| c.provider.provider_id() == provider))
        .map(|c| (c.provider, c.native_key.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter();
    let (provider, key) = distinct.next()?;
    if distinct.next().is_some() {
        return None;
    }
    RetainedAgentSession::new(provider, key)
}

#[cfg(windows)]
impl super::SessionCoordinator {
    /// The live reference of every checkpoint target that has one, keyed by its binding. Targets
    /// join their Presentation through current registration at the exact generation they name;
    /// a stale target, an unreadable registration or an unavailable Agent cell contributes
    /// nothing, so pending restart references still carry over. Capture owns cross-Presentation
    /// deduplication in its own order.
    pub(crate) fn checkpoint_agent_sessions(
        &self,
        targets: &[super::ProcessObservationTarget],
    ) -> Vec<(
        orchbee_workbench_protocol::PresentationBinding,
        RetainedAgentSession,
    )> {
        let Some(agents) = self.agents.as_ref().and_then(std::sync::Weak::upgrade) else {
            return Vec::new();
        };
        let bindings: Vec<_> = targets.iter().map(|t| t.binding.clone()).collect();
        let Ok(facts) = self.agent_registration_for_bindings(&bindings) else {
            return Vec::new();
        };
        let physical: Vec<_> = facts
            .into_iter()
            .filter(|fact| {
                targets.iter().any(|t| {
                    t.binding == *fact.binding()
                        && t.target.worker.generation == fact.worker_generation()
                        && t.target.identity == fact.identity()
                })
            })
            .map(|fact| {
                (
                    fact.presentation_id().clone(),
                    orchbee_server::TerminalAgentBinding {
                        binding: fact.binding().clone(),
                        worker_generation: fact.worker_generation(),
                        identity: fact.identity(),
                    },
                )
            })
            .collect();
        let identities: Vec<_> = physical
            .iter()
            .map(|(presentation, terminal)| (presentation.clone(), terminal.identity))
            .collect();
        let Ok(candidates) = agents.resume_candidates(&identities) else {
            return Vec::new();
        };
        physical
            .into_iter()
            .filter_map(|(presentation, terminal)| {
                let own: Vec<_> = candidates
                    .iter()
                    .filter(|c| c.presentation == presentation)
                    .cloned()
                    .collect();
                let evidence = CommandEvidence::of(self.latest_command(&terminal));
                let session = select(&own, evidence, &self.process_identity(&terminal))?;
                Some((terminal.binding, session))
            })
            .collect()
    }
}
