//! Physical aliases share Names capacity and claims; report events reconcile ownership synchronously.
use super::{
    Missing, Names,
    launch::{launch_binding, same_binding},
};
use crate::agent::{AgentRuntimeSnapshot, native_observation::Binding};
use orchbee_agent_observation_protocol::{
    AgentLaunchBinding, AgentName, AgentNameAssignOutcome as Assign,
    AgentNameAssignRejectionReason as AssignReason, AgentNameReleaseOutcome as Release,
    AgentNameReleaseRejectionReason as ReleaseReason, AgentNameTarget, AgentNamedTarget,
    AgentNamesListOutcome, AgentResolveOutcome, AgentResolveRejectionReason, AgentResolvedBy,
    AgentSessionId, AgentSessionSelector, MAXIMUM_NAME_BINDINGS,
};
use orchbee_server::TerminalAgentNameOwner;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(crate) struct TerminalNameFact {
    pub(crate) binding: Binding,
    pub(crate) owner: TerminalAgentNameOwner,
}
pub(super) struct TerminalName {
    pub(super) name: AgentName,
    pub(super) binding: Binding,
    pub(super) owner: TerminalAgentNameOwner,
}
enum Selected<'a> {
    Session(AgentSessionId, AgentResolvedBy),
    Terminal(&'a TerminalNameFact, AgentResolvedBy),
}
fn terminal(binding: &Binding) -> AgentNameTarget {
    AgentNameTarget::Terminal {
        binding: launch_binding(binding),
    }
}

impl Names {
    pub(super) fn terminal_claim(&self, name: &str) -> Option<AgentLaunchBinding> {
        self.terminals
            .values()
            .find(|entry| entry.name.as_str() == name)
            .map(|entry| launch_binding(&entry.binding))
    }

    #[cfg(any(windows, test))]
    pub(super) fn observe_terminal_owner(&mut self, fact: &TerminalNameFact) {
        let id = launch_binding(&fact.binding).presentation_id;
        let Some(entry) = self.terminals.get_mut(&id) else {
            return;
        };
        let incoming = &fact.owner;
        if !same_binding(&entry.binding, &fact.binding)
            || incoming.clear
            || incoming
                .agent
                .as_ref()
                .zip(entry.owner.agent.as_ref())
                .is_some_and(|(a, b)| a != b)
        {
            self.terminals.remove(&id);
            return;
        }
        if entry.owner.agent.is_none() {
            entry.owner.agent.clone_from(&incoming.agent);
        }
    }

    pub(super) fn reconcile_terminal_bindings(&mut self, bindings: &[Binding], disposed: bool) {
        self.terminals.retain(|_, entry| {
            !disposed
                && bindings
                    .iter()
                    .any(|binding| same_binding(&entry.binding, binding))
        });
    }

    #[cfg(any(windows, test))]
    pub(super) fn reconcile_terminals(&mut self, facts: &[TerminalNameFact]) {
        self.terminals.retain(|_, entry| {
            facts
                .iter()
                .any(|fact| same_binding(&entry.binding, &fact.binding))
        });
        for fact in facts {
            self.observe_terminal_owner(fact);
        }
    }

    fn select_named<'a>(
        &self,
        snapshot: &AgentRuntimeSnapshot,
        target: &AgentSessionSelector,
        facts: &'a [TerminalNameFact],
    ) -> Result<Selected<'a>, Missing> {
        let by_name = match target {
            AgentSessionSelector::Name { name } => Some(name.as_str()),
            AgentSessionSelector::Auto { target }
                if !snapshot
                    .sessions
                    .iter()
                    .any(|s| s.session_id.as_str() == target.as_str()) =>
            {
                Some(target.as_str())
            }
            AgentSessionSelector::Session { .. }
            | AgentSessionSelector::Auto { .. }
            | AgentSessionSelector::Terminal { .. }
            | AgentSessionSelector::Presentation { .. } => None,
        };
        let named = by_name.and_then(|name| {
            self.terminals
                .values()
                .find(|entry| entry.name.as_str() == name)
        });
        let (presentation, exact, by) = if let Some(entry) = named {
            (
                Some(entry.binding.presentation.as_str()),
                Some(launch_binding(&entry.binding)),
                AgentResolvedBy::Name,
            )
        } else {
            match target {
                AgentSessionSelector::Presentation { presentation_id } => (
                    Some(presentation_id.as_str()),
                    None,
                    AgentResolvedBy::Presentation,
                ),
                AgentSessionSelector::Terminal { binding } => (
                    Some(binding.presentation_id.as_str()),
                    Some(binding.clone()),
                    AgentResolvedBy::Presentation,
                ),
                AgentSessionSelector::Session { .. }
                | AgentSessionSelector::Name { .. }
                | AgentSessionSelector::Auto { .. } => (None, None, AgentResolvedBy::Name),
            }
        };
        // Once named as a terminal, a later hook does not silently move its alias to a different identity domain.
        let already_terminal = named.is_some()
            || matches!(target, AgentSessionSelector::Terminal { .. })
            || presentation.is_some_and(|id| self.terminals.keys().any(|key| key.as_str() == id));
        if !already_terminal {
            match self.resolve(snapshot, target) {
                Ok((id, by)) => return Ok(Selected::Session(id, by)),
                Err(Missing::Session) if presentation.is_some() => {}
                Err(missing) => return Err(missing),
            }
        }
        let fact = facts
            .iter()
            .find(|fact| {
                Some(fact.binding.presentation.as_str()) == presentation
                    && exact
                        .as_ref()
                        .is_none_or(|expected| *expected == launch_binding(&fact.binding))
            })
            .ok_or(Missing::Session)?;
        let id = launch_binding(&fact.binding).presentation_id;
        if self.terminal_pending(&id) {
            return Err(Missing::Pending);
        }
        if fact.owner.clear || (fact.owner.agent.is_none() && !self.terminals.contains_key(&id)) {
            return Err(Missing::Session);
        }
        Ok(Selected::Terminal(fact, by))
    }

    pub(super) fn resolve_named(
        &self,
        snapshot: &AgentRuntimeSnapshot,
        target: &AgentSessionSelector,
        facts: &[TerminalNameFact],
    ) -> AgentResolveOutcome {
        match self.select_named(snapshot, target, facts) {
            Ok(Selected::Session(session_id, resolved_by)) => AgentResolveOutcome::Resolved {
                session_id,
                resolved_by,
            },
            Ok(Selected::Terminal(fact, resolved_by)) => AgentResolveOutcome::ResolvedTarget {
                target: terminal(&fact.binding),
                resolved_by,
            },
            Err(missing) => AgentResolveOutcome::Rejected {
                reason: match missing {
                    Missing::Pending => AgentResolveRejectionReason::LaunchPending,
                    Missing::Ambiguous => AgentResolveRejectionReason::AmbiguousTarget,
                    Missing::Name | Missing::Session => AgentResolveRejectionReason::NotFound,
                },
            },
        }
    }

    pub(super) fn assign_named(
        &mut self,
        snapshot: &AgentRuntimeSnapshot,
        target: &AgentSessionSelector,
        name: &AgentName,
        facts: &[TerminalNameFact],
    ) -> Assign {
        let reject = |reason| Assign::Rejected {
            reason,
            conflict_session_id: None,
        };
        let fact = match self.select_named(snapshot, target, facts) {
            Ok(Selected::Session(_, _)) => return self.assign(snapshot, target, name),
            Ok(Selected::Terminal(fact, _)) => fact,
            Err(missing) => {
                return reject(match missing {
                    Missing::Pending => AssignReason::LaunchPending,
                    Missing::Ambiguous => AssignReason::AmbiguousTarget,
                    Missing::Name => AssignReason::NameNotFound,
                    Missing::Session => AssignReason::SessionNotFound,
                });
            }
        };
        if self.pending_name(name.as_str()) {
            return reject(AssignReason::LaunchPending);
        }
        let target = terminal(&fact.binding);
        if let Some(id) = self.by_name.get(name) {
            return Assign::RejectedTarget {
                reason: AssignReason::NameInUse,
                conflict_target: Some(AgentNameTarget::Session {
                    session_id: id.clone(),
                }),
            };
        }
        if let Some(binding) = self.terminal_claim(name.as_str()) {
            return if binding == launch_binding(&fact.binding) {
                Assign::UnchangedTarget {
                    target,
                    name: name.clone(),
                }
            } else {
                Assign::RejectedTarget {
                    reason: AssignReason::NameInUse,
                    conflict_target: Some(AgentNameTarget::Terminal { binding }),
                }
            };
        }
        let id = launch_binding(&fact.binding).presentation_id;
        if !self.terminals.contains_key(&id) && self.total_bindings() >= MAXIMUM_NAME_BINDINGS {
            return reject(AssignReason::CapacityExceeded);
        }
        let mut owner = fact.owner.clone();
        if let Some(existing) = self.terminals.get(&id) {
            owner = existing.owner.clone();
        }
        let previous_name = self
            .terminals
            .insert(
                id,
                TerminalName {
                    name: name.clone(),
                    binding: fact.binding.clone(),
                    owner,
                },
            )
            .map(|entry| entry.name);
        Assign::AssignedTarget {
            target,
            name: name.clone(),
            previous_name,
        }
    }

    pub(super) fn release_named(
        &mut self,
        snapshot: &AgentRuntimeSnapshot,
        target: &AgentSessionSelector,
        facts: &[TerminalNameFact],
    ) -> Release {
        match self.select_named(snapshot, target, facts) {
            Ok(Selected::Session(_, _)) => self.release(snapshot, target),
            Ok(Selected::Terminal(fact, _)) => {
                let target = terminal(&fact.binding);
                match self
                    .terminals
                    .remove(&launch_binding(&fact.binding).presentation_id)
                {
                    Some(entry) => Release::ReleasedTarget {
                        target,
                        name: entry.name,
                    },
                    None => Release::UnchangedTarget { target },
                }
            }
            Err(missing) => Release::Rejected {
                reason: match missing {
                    Missing::Pending => ReleaseReason::LaunchPending,
                    Missing::Ambiguous => ReleaseReason::AmbiguousTarget,
                    Missing::Name => ReleaseReason::NameNotFound,
                    Missing::Session => ReleaseReason::SessionNotFound,
                },
            },
        }
    }

    pub(super) fn names_list(&self) -> AgentNamesListOutcome {
        if self.terminals.is_empty() {
            return AgentNamesListOutcome::Listed {
                bindings: self
                    .by_name
                    .iter()
                    .map(
                        |(name, session_id)| crate::agent_service::names::AgentNameBinding {
                            name: name.clone(),
                            session_id: session_id.clone(),
                            lifecycle: orchbee_agent_observation_protocol::AgentLifecycle::Live,
                        },
                    )
                    .collect(),
                pending: self.pending_names(),
            };
        }
        let mut bindings: Vec<_> = self
            .by_name
            .iter()
            .map(|(name, session_id)| AgentNamedTarget {
                name: name.clone(),
                target: AgentNameTarget::Session {
                    session_id: session_id.clone(),
                },
            })
            .chain(self.terminals.values().map(|entry| AgentNamedTarget {
                name: entry.name.clone(),
                target: terminal(&entry.binding),
            }))
            .collect();
        bindings.sort_by(|a, b| a.name.cmp(&b.name));
        AgentNamesListOutcome::ListedTargets {
            bindings,
            pending: self.pending_names(),
        }
    }
}
