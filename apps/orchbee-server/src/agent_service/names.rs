//! Runtime-scoped aliases share the canonical cell and its lifecycle folds.
//! Preparation drains observations outside the leaf lock; resolution and any
//! mutation then inspect the live runtime together inside one fold.

mod launch;
mod terminal;
#[cfg(windows)]
pub(crate) use terminal::TerminalNameFact;

use std::collections::BTreeMap;

use orchbee_agent_observation_protocol::{
    AgentName, AgentNameAssignOutcome, AgentNameAssignOutcomeEnvelope,
    AgentNameAssignRejectionReason, AgentNameBinding, AgentNameReleaseOutcome,
    AgentNameReleaseOutcomeEnvelope, AgentNameReleaseRejectionReason, AgentNamesListOutcome,
    AgentNamesListOutcomeEnvelope, AgentNamesListRejectionReason, AgentNamesRequest,
    AgentNamesResult, AgentResolveOutcome, AgentResolveOutcomeEnvelope,
    AgentResolveRejectionReason, AgentResolvedBy, AgentSessionId, AgentSessionSelector,
    AgentUnavailableReason, MAXIMUM_NAME_BINDINGS,
};

use super::{AgentService, AgentState, PreparedUnavailable};
use crate::agent::{AgentLifecycle, AgentRuntime, AgentRuntimeSnapshot};

#[derive(Default)]
pub(super) struct Names {
    launches: launch::Launches,
    terminals:
        BTreeMap<orchbee_agent_observation_protocol::AgentPresentationId, terminal::TerminalName>,
    by_name: BTreeMap<AgentName, AgentSessionId>,
    by_session: BTreeMap<AgentSessionId, AgentName>,
}

impl Names {
    /// Every cell fold calls this before releasing its guard. End, removal,
    /// eviction, and disposal therefore cannot leave ghost names. A detached
    /// but live canonical session retains its alias until explicit release/end.
    pub(super) fn reconcile(&mut self, runtime: &AgentRuntime) {
        if runtime.is_disposed() {
            self.terminals.clear();
            self.by_name.clear();
            self.by_session.clear();
            self.reconcile_managed(runtime.snapshot());
            return;
        }
        let snapshot = runtime.snapshot();
        self.by_name.retain(|name, id| {
            let live = snapshot.sessions.iter().any(|session| {
                session.session_id.as_str() == id.as_str()
                    && session.lifecycle == AgentLifecycle::Live
            });
            if !live {
                self.by_session.remove(id);
            } else {
                debug_assert_eq!(self.by_session.get(id), Some(name));
            }
            live
        });
        self.reconcile_managed(snapshot);
        debug_assert_eq!(self.by_name.len(), self.by_session.len());
    }

    fn resolve(
        &self,
        snapshot: &AgentRuntimeSnapshot,
        target: &AgentSessionSelector,
    ) -> Result<(AgentSessionId, AgentResolvedBy), Missing> {
        if let AgentSessionSelector::Presentation { presentation_id } = target {
            let mut matches = snapshot.sessions.iter().filter(|session| {
                session.lifecycle == AgentLifecycle::Live
                    && session.presentation_attachments.iter().any(|attachment| {
                        attachment.navigable
                            && attachment.presentation_id.as_str() == presentation_id.as_str()
                    })
            });
            let found = matches.next().ok_or(Missing::Session)?;
            if matches.next().is_some() {
                return Err(Missing::Ambiguous);
            }
            let id = AgentSessionId::parse(found.session_id.as_str())
                .expect("canonical session identity");
            return Ok((id, AgentResolvedBy::Presentation));
        }
        let id = match target {
            AgentSessionSelector::Session { session_id } => Some(session_id),
            AgentSessionSelector::Auto { target } => Some(target),
            AgentSessionSelector::Name { .. }
            | AgentSessionSelector::Presentation { .. }
            | AgentSessionSelector::Terminal { .. } => None,
        };
        if let Some(id) = id
            && snapshot
                .sessions
                .iter()
                .any(|session| session.session_id.as_str() == id.as_str())
        {
            return Ok((id.clone(), AgentResolvedBy::SessionId));
        }
        let name = match target {
            AgentSessionSelector::Name { name } => name.as_str(),
            AgentSessionSelector::Auto { target } => target.as_str(),
            AgentSessionSelector::Session { .. }
            | AgentSessionSelector::Presentation { .. }
            | AgentSessionSelector::Terminal { .. } => {
                return Err(Missing::Session);
            }
        };
        if self.pending_name(name) {
            return Err(Missing::Pending);
        }
        self.by_name
            .get(name)
            .cloned()
            .map(|id| (id, AgentResolvedBy::Name))
            .ok_or(if matches!(target, AgentSessionSelector::Name { .. }) {
                Missing::Name
            } else {
                Missing::Session
            })
    }

    fn assign(
        &mut self,
        snapshot: &AgentRuntimeSnapshot,
        target: &AgentSessionSelector,
        name: &AgentName,
    ) -> AgentNameAssignOutcome {
        let reject = |reason| AgentNameAssignOutcome::Rejected {
            reason,
            conflict_session_id: None,
        };
        let (id, _) = match self.resolve(snapshot, target) {
            Ok(found) => found,
            Err(Missing::Pending) => return reject(AgentNameAssignRejectionReason::LaunchPending),
            Err(Missing::Session) => {
                return reject(AgentNameAssignRejectionReason::SessionNotFound);
            }
            Err(Missing::Name) => return reject(AgentNameAssignRejectionReason::NameNotFound),
            Err(Missing::Ambiguous) => {
                return reject(AgentNameAssignRejectionReason::AmbiguousTarget);
            }
        };
        if !snapshot.sessions.iter().any(|session| {
            session.session_id.as_str() == id.as_str() && session.lifecycle == AgentLifecycle::Live
        }) {
            return reject(AgentNameAssignRejectionReason::SessionEnded);
        }
        if self.pending_name(name.as_str()) || self.pending_session(snapshot, &id) {
            return reject(AgentNameAssignRejectionReason::LaunchPending);
        }
        if let Some(binding) = self.terminal_claim(name.as_str()) {
            return AgentNameAssignOutcome::RejectedTarget {
                reason: AgentNameAssignRejectionReason::NameInUse,
                conflict_target: Some(
                    orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding },
                ),
            };
        }
        if let Some(incumbent) = self.by_name.get(name) {
            return if incumbent == &id {
                AgentNameAssignOutcome::Unchanged {
                    session_id: id,
                    name: name.clone(),
                }
            } else {
                AgentNameAssignOutcome::Rejected {
                    reason: AgentNameAssignRejectionReason::NameInUse,
                    conflict_session_id: Some(incumbent.clone()),
                }
            };
        }
        if !self.by_session.contains_key(&id) && self.total_bindings() >= MAXIMUM_NAME_BINDINGS {
            return reject(AgentNameAssignRejectionReason::CapacityExceeded);
        }
        let previous_name = self.by_session.insert(id.clone(), name.clone());
        if let Some(previous) = &previous_name {
            self.by_name.remove(previous);
        }
        self.by_name.insert(name.clone(), id.clone());
        AgentNameAssignOutcome::Assigned {
            session_id: id,
            name: name.clone(),
            previous_name,
        }
    }

    fn release(
        &mut self,
        snapshot: &AgentRuntimeSnapshot,
        target: &AgentSessionSelector,
    ) -> AgentNameReleaseOutcome {
        match self.resolve(snapshot, target) {
            Err(missing) => AgentNameReleaseOutcome::Rejected {
                reason: match missing {
                    Missing::Pending => AgentNameReleaseRejectionReason::LaunchPending,
                    Missing::Name => AgentNameReleaseRejectionReason::NameNotFound,
                    Missing::Ambiguous => AgentNameReleaseRejectionReason::AmbiguousTarget,
                    Missing::Session => AgentNameReleaseRejectionReason::SessionNotFound,
                },
            },
            Ok((session_id, _)) if self.pending_session(snapshot, &session_id) => {
                AgentNameReleaseOutcome::Rejected {
                    reason: AgentNameReleaseRejectionReason::LaunchPending,
                }
            }
            Ok((session_id, _)) => match self.by_session.remove(&session_id) {
                None => AgentNameReleaseOutcome::Unchanged { session_id },
                Some(name) => {
                    self.by_name.remove(&name);
                    AgentNameReleaseOutcome::Released { session_id, name }
                }
            },
        }
    }
}

enum Missing {
    Pending,
    Ambiguous,
    Session,
    Name,
}

#[derive(Clone, Copy)]
enum Refusal {
    Denied,
    Unavailable,
}

impl AgentService {
    #[cfg(any(not(windows), test))]
    pub(crate) fn execute_names_prepared(
        &self,
        request: AgentNamesRequest,
        prepare: impl FnOnce() -> Result<AgentRuntimeSnapshot, PreparedUnavailable>,
    ) -> AgentNamesResult {
        let allowed = match request {
            AgentNamesRequest::Resolve { .. } | AgentNamesRequest::NamesList { .. } => {
                self.policy.read
            }
            AgentNamesRequest::NameAssign { .. } | AgentNamesRequest::NameRelease { .. } => {
                self.policy.manage
            }
        };
        if !allowed {
            return answer(&request, Err(Refusal::Denied), &[]);
        }
        // The prepared clone is freshness evidence only. Liveness and name
        // ownership are re-read from the actual cell at the linearization point.
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(prepare))
            .unwrap_or(Err(PreparedUnavailable))
            .is_err()
        {
            return answer(&request, Err(Refusal::Unavailable), &[]);
        }
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                answer(&request, Err(Refusal::Unavailable), &[])
            } else {
                answer(&request, Ok(state), &[])
            }
        })
        .unwrap_or_else(|_| answer(&request, Err(Refusal::Unavailable), &[]))
    }
}

fn answer(
    request: &AgentNamesRequest,
    state: Result<&mut AgentState, Refusal>,
    facts: &[TerminalNameFact],
) -> AgentNamesResult {
    let unavailable = AgentUnavailableReason::RuntimeUnavailable;
    match request {
        AgentNamesRequest::Resolve { request_id, target } => AgentNamesResult::Resolve {
            request_id: request_id.clone(),
            result: AgentResolveOutcomeEnvelope {
                outcome: match state {
                    Err(Refusal::Denied) => AgentResolveOutcome::Rejected {
                        reason: AgentResolveRejectionReason::ReadDenied,
                    },
                    Err(Refusal::Unavailable) => AgentResolveOutcome::Unavailable {
                        reason: unavailable,
                    },
                    Ok(state) => state
                        .names
                        .resolve_named(state.runtime.snapshot(), target, facts),
                },
            },
        },
        AgentNamesRequest::NamesList { request_id } => AgentNamesResult::NamesList {
            request_id: request_id.clone(),
            result: AgentNamesListOutcomeEnvelope {
                outcome: match state {
                    Err(Refusal::Denied) => AgentNamesListOutcome::Rejected {
                        reason: AgentNamesListRejectionReason::ReadDenied,
                    },
                    Err(Refusal::Unavailable) => AgentNamesListOutcome::Unavailable {
                        reason: unavailable,
                    },
                    Ok(state) => state.names.names_list(),
                },
            },
        },
        AgentNamesRequest::NameAssign {
            request_id,
            target,
            name,
        } => AgentNamesResult::NameAssign {
            request_id: request_id.clone(),
            result: AgentNameAssignOutcomeEnvelope {
                outcome: match state {
                    Err(Refusal::Denied) => AgentNameAssignOutcome::Rejected {
                        reason: AgentNameAssignRejectionReason::ManageDenied,
                        conflict_session_id: None,
                    },
                    Err(Refusal::Unavailable) => AgentNameAssignOutcome::Unavailable {
                        reason: unavailable,
                    },
                    Ok(state) => {
                        state
                            .names
                            .assign_named(state.runtime.snapshot(), target, name, facts)
                    }
                },
            },
        },
        AgentNamesRequest::NameRelease { request_id, target } => AgentNamesResult::NameRelease {
            request_id: request_id.clone(),
            result: AgentNameReleaseOutcomeEnvelope {
                outcome: match state {
                    Err(Refusal::Denied) => AgentNameReleaseOutcome::Rejected {
                        reason: AgentNameReleaseRejectionReason::ManageDenied,
                    },
                    Err(Refusal::Unavailable) => AgentNameReleaseOutcome::Unavailable {
                        reason: unavailable,
                    },
                    Ok(state) => state
                        .names
                        .release_named(state.runtime.snapshot(), target, facts),
                },
            },
        },
    }
}

#[cfg(test)]
mod tests;

impl AgentService {
    /// Policy precedes preparation; the caller supplies the existing final commit guards.
    #[cfg(windows)]
    pub(crate) fn execute_names_guarded(
        &self,
        request: AgentNamesRequest,
        fold: impl FnOnce(&AgentNamesRequest) -> Result<AgentNamesResult, PreparedUnavailable>,
    ) -> AgentNamesResult {
        let allowed = match &request {
            AgentNamesRequest::Resolve { .. } | AgentNamesRequest::NamesList { .. } => {
                self.policy.read
            }
            AgentNamesRequest::NameAssign { .. } | AgentNamesRequest::NameRelease { .. } => {
                self.policy.manage
            }
        };
        if !allowed {
            return answer(&request, Err(Refusal::Denied), &[]);
        }
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fold(&request)))
            .ok()
            .and_then(Result::ok)
            .unwrap_or_else(|| answer(&request, Err(Refusal::Unavailable), &[]))
    }

    #[cfg(windows)]
    pub(crate) fn fold_terminal_names(
        &self,
        request: &AgentNamesRequest,
        facts: &[TerminalNameFact],
    ) -> Result<AgentNamesResult, PreparedUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                return answer(request, Err(Refusal::Unavailable), &[]);
            }
            state.names.reconcile_terminals(facts);
            answer(request, Ok(state), facts)
        })
        .map_err(|_| PreparedUnavailable)
    }

    #[cfg(windows)]
    pub(crate) fn observe_terminal_name(&self, fact: &TerminalNameFact) {
        // Poison/disposal is permanently unavailable to every names consumer, never stale success.
        let _ = self.with_state(|state| state.names.observe_terminal_owner(fact));
    }
}
