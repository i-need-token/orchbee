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

/// Where a pending restart reference's Presentation stands when its delivery is attempted.
#[cfg(windows)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResumeTarget {
    /// The Presentation is no longer a bound member of the Workbench.
    Gone,
    /// Bound as this, but no started, unfenced physical terminal of the current Worker answers.
    Unavailable(orchbee_workbench_protocol::PresentationBinding),
    /// The started physical terminal its current binding names.
    Started(super::ProcessObservationTarget),
}

#[cfg(windows)]
impl super::SessionCoordinator {
    /// Whether a restart reference still waits for delivery in this Presentation.
    pub(crate) fn has_pending_agent_session(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> bool {
        self.pending_agent_session(presentation).is_some()
    }

    /// The restart reference still pending for this Presentation, if any.
    pub(crate) fn pending_agent_session(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> Option<RetainedAgentSession> {
        self.workbench
            .as_ref()?
            .pending_agent_session(presentation)
            .cloned()
    }

    /// Consumes the pending restart reference, so no later attempt or capture carries it.
    pub(crate) fn take_pending_agent_session(
        &mut self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> Option<RetainedAgentSession> {
        self.workbench
            .as_mut()?
            .take_pending_agent_session(presentation)
    }

    /// The Presentation's current binding and, when the current Worker has started it and no
    /// lifecycle fence covers it, the exact physical terminal a delivery may reach.
    pub(crate) fn agent_resume_target(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
        current_worker: Option<crate::terminal_coordinator::WorkerPlacement>,
    ) -> ResumeTarget {
        let Some(workbench) = self.workbench.as_ref() else {
            return ResumeTarget::Gone;
        };
        let binding = workbench
            .snapshot()
            .terminal_contexts
            .iter()
            .filter_map(|context| {
                workbench
                    .presentation_membership(&context.terminal_context_id)
                    .ok()
            })
            .find_map(|membership| {
                membership
                    .members()
                    .iter()
                    .find(|member| member.presentation_id() == presentation)
                    .map(|member| member.binding().clone())
            });
        let Some(binding) = binding else {
            return ResumeTarget::Gone;
        };
        let parse = |text: &str| u128::from_str_radix(text, 16).ok().map(u128::to_be_bytes);
        let target = parse(binding.terminal_id.as_str())
            .zip(parse(binding.worker_id.as_str()))
            .and_then(|(terminal, worker)| {
                Some((
                    orchbee_capability_node_protocol::TerminalId::from_bytes(terminal).ok()?,
                    orchbee_capability_node_protocol::WorkerId::from_bytes(worker).ok()?,
                ))
            })
            .and_then(|(terminal, worker)| {
                self.terminals
                    .binding_send_target(terminal, worker, current_worker)
                    .ok()
            })
            .filter(|target| self.target_lifecycle(*target) == super::TargetLifecycle::Unfenced);
        match target {
            Some(target) => ResumeTarget::Started(super::ProcessObservationTarget {
                session_id: workbench.session_id().clone(),
                binding,
                target,
            }),
            None => ResumeTarget::Unavailable(binding),
        }
    }

    /// Whether the shell of this exact Terminal generation has echoed any command.
    pub(crate) fn agent_resume_command_observed(
        &self,
        target: &super::ProcessObservationTarget,
    ) -> bool {
        self.latest_command(&orchbee_server::TerminalAgentBinding {
            binding: target.binding.clone(),
            worker_generation: target.target.worker.generation,
            identity: target.target.identity,
        })
        .is_some()
    }
}
