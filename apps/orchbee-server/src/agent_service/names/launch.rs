//! Deterministic reservation transitions in the names owner. No I/O or clocks.

use std::collections::BTreeMap;

use orchbee_agent_observation_protocol::{
    AgentAttachmentGeneration, AgentHostEpoch, AgentHostTerminalId, AgentLaunchBinding,
    AgentLaunchOutcome as Outcome, AgentLaunchRejectionReason as Rejection, AgentLaunchRequest,
    AgentLaunchReservation, AgentName, AgentProviderId, AgentSafeInteger, AgentSessionId,
    LAUNCH_SETTLE_MS, MAXIMUM_LAUNCH_TTL_MS, ObapRequestId,
};

use super::Names;
use crate::agent::native_observation::Binding;
use crate::agent::{
    AgentFreshness, AgentLifecycle, AgentRuntimeSnapshot, AgentSemanticStatus,
    AgentSessionSnapshot, project_agent_session_summary,
};
use crate::agent_service::canonical_hex32;

#[derive(Default)]
pub(super) struct Launches {
    now_ms: u64,
    active: BTreeMap<AgentSessionId, Managed>,
    entries: BTreeMap<ObapRequestId, Entry>,
}

struct Managed {
    reservation_id: ObapRequestId,
    provider: AgentProviderId,
    binding: Binding,
}
struct Entry {
    id: ObapRequestId,
    name: AgentName,
    provider: AgentProviderId,
    binding: Binding,
    baseline: BTreeMap<String, (u64, u64)>,
    started_ms: u64,
    expires_ms: u64,
}

pub(super) fn launch_binding(binding: &Binding) -> AgentLaunchBinding {
    AgentLaunchBinding {
        presentation_id: orchbee_agent_observation_protocol::AgentPresentationId::parse(
            binding.presentation.as_str(),
        )
        .expect("native presentation has the nonzero hex identity profile"),
        generation: AgentAttachmentGeneration::parse(&canonical_hex32(
            binding.host.creation_id().as_bytes(),
        ))
        .expect("native creation identity fits the attachment profile"),
        host_epoch: AgentHostEpoch::parse(&canonical_hex32(binding.host.host_epoch().as_bytes()))
            .expect("native epoch is a nonzero hex identity"),
        host_terminal_id: AgentHostTerminalId::parse(&canonical_hex32(
            binding.host.terminal_id().as_bytes(),
        ))
        .expect("native Terminal is a nonzero hex identity"),
    }
}

pub(super) fn same_binding(left: &Binding, right: &Binding) -> bool {
    left.presentation == right.presentation
        && left.carrier == right.carrier
        && left.host == right.host
}

fn attached(session: &AgentSessionSnapshot, binding: &Binding) -> bool {
    let generation = canonical_hex32(binding.host.creation_id().as_bytes());
    session.presentation_attachments.iter().any(|attachment| {
        attachment.navigable
            && attachment.presentation_id.as_str() == binding.presentation.as_str()
            && attachment.generation.as_str() == generation
    })
}

fn reject(reason: Rejection) -> Outcome {
    Outcome::Rejected { reason }
}

impl Entry {
    fn view(&self, now: u64) -> AgentLaunchReservation {
        AgentLaunchReservation {
            reservation_id: self.id.clone(),
            name: self.name.clone(),
            provider_id: self.provider.clone(),
            binding: launch_binding(&self.binding),
            remaining_ms: AgentSafeInteger::new(self.expires_ms.saturating_sub(now)),
            settle_remaining_ms: AgentSafeInteger::new(
                self.started_ms
                    .saturating_add(LAUNCH_SETTLE_MS)
                    .saturating_sub(now),
            ),
        }
    }

    fn candidate<'a>(
        &self,
        snapshot: &'a AgentRuntimeSnapshot,
    ) -> Result<Option<&'a AgentSessionSnapshot>, Rejection> {
        let mut candidates = snapshot.sessions.iter().filter(|session| {
            session.lifecycle == AgentLifecycle::Live
                && session.parent_session_id.is_none()
                && session.provider_id.as_str() == self.provider.as_str()
                && attached(session, &self.binding)
                && self
                    .baseline
                    .get(session.session_id.as_str())
                    .is_none_or(|before| {
                        (session.observation.generation, session.observation.sequence) > *before
                    })
        });
        let first = candidates.next();
        if candidates.next().is_some() {
            return Err(Rejection::CandidateAmbiguous);
        }
        Ok(first)
    }
}

impl Names {
    pub(super) fn reconcile_managed(&mut self, snapshot: &AgentRuntimeSnapshot) {
        self.launches.active.retain(|id, managed| {
            self.by_session.contains_key(id)
                && snapshot.sessions.iter().any(|session| {
                    session.session_id.as_str() == id.as_str()
                        && session.lifecycle == AgentLifecycle::Live
                        && session.provider_id.as_str() == managed.provider.as_str()
                        && attached(session, &managed.binding)
                })
        });
    }
    fn management(&self, snapshot: &AgentRuntimeSnapshot, id: &AgentSessionId) -> Outcome {
        let Some(session) = snapshot
            .sessions
            .iter()
            .find(|session| session.session_id.as_str() == id.as_str())
        else {
            return reject(Rejection::SessionNotFound);
        };
        use orchbee_agent_observation_protocol::AgentManagement;
        let management = if let Some(managed) = self.launches.active.get(id) {
            AgentManagement::Active {
                reservation_id: managed.reservation_id.clone(),
                provider_id: managed.provider.clone(),
                binding: launch_binding(&managed.binding),
            }
        } else if let Some(pending) = self.launches.entries.values().find(|entry| {
            session.lifecycle == AgentLifecycle::Live && attached(session, &entry.binding)
        }) {
            AgentManagement::Pending {
                reservation: pending.view(self.launches.now_ms),
                blocked: !session.pending_requests.is_empty(),
            }
        } else {
            AgentManagement::Unmanaged {}
        };
        Outcome::Status {
            session_id: id.clone(),
            revision: AgentSafeInteger::new(snapshot.revision),
            management,
        }
    }

    pub(in crate::agent_service) fn reconcile_launches(
        &mut self,
        bindings: &[Binding],
        now_ms: u64,
        disposed: bool,
    ) {
        self.reconcile_terminal_bindings(bindings, disposed);
        self.launches.now_ms = self.launches.now_ms.max(now_ms);
        let now = self.launches.now_ms;
        self.launches.active.retain(|_, entry| {
            !disposed
                && bindings
                    .iter()
                    .any(|binding| same_binding(binding, &entry.binding))
        });
        self.launches.entries.retain(|_, entry| {
            !disposed
                && now < entry.expires_ms
                && bindings
                    .iter()
                    .any(|binding| same_binding(binding, &entry.binding))
        });
    }

    pub(in crate::agent_service) fn next_launch_deadline_ms(&self) -> Option<u64> {
        self.launches
            .entries
            .values()
            .map(|entry| entry.expires_ms)
            .min()
    }

    pub(super) fn pending_names(&self) -> Vec<AgentLaunchReservation> {
        let mut rows: Vec<_> = self
            .launches
            .entries
            .values()
            .map(|entry| entry.view(self.launches.now_ms))
            .collect();
        rows.sort_by(|left, right| left.name.cmp(&right.name));
        rows
    }

    pub(super) fn pending_name(&self, name: &str) -> bool {
        self.launches
            .entries
            .values()
            .any(|entry| entry.name.as_str() == name)
    }

    pub(super) fn pending_session(
        &self,
        snapshot: &AgentRuntimeSnapshot,
        id: &AgentSessionId,
    ) -> bool {
        snapshot
            .sessions
            .iter()
            .find(|session| session.session_id.as_str() == id.as_str())
            .is_some_and(|session| {
                self.launches
                    .entries
                    .values()
                    .any(|entry| attached(session, &entry.binding))
            })
    }

    pub(super) fn total_bindings(&self) -> usize {
        self.by_name.len() + self.terminals.len() + self.launches.entries.len()
    }

    pub(in crate::agent_service) fn execute_launch(
        &mut self,
        request: &AgentLaunchRequest,
        snapshot: &AgentRuntimeSnapshot,
        bindings: &[Binding],
        now_ms: u64,
    ) -> Outcome {
        self.reconcile_launches(bindings, now_ms, false);
        self.reconcile_managed(snapshot);
        let now = self.launches.now_ms;
        if let AgentLaunchRequest::Status { session_id, .. } = request {
            return self.management(snapshot, session_id);
        }
        if let AgentLaunchRequest::Reserve {
            request_id,
            name,
            provider_id,
            presentation_id,
            host_epoch,
            host_terminal_id,
            ttl_ms,
        } = request
        {
            if self.terminals.contains_key(presentation_id) {
                return reject(Rejection::NameInUse);
            }
            if self.by_name.contains_key(name)
                || self.terminal_claim(name.as_str()).is_some()
                || self.pending_name(name.as_str())
            {
                return reject(Rejection::NameInUse);
            }
            if self.launches.entries.contains_key(request_id)
                || self
                    .launches
                    .entries
                    .values()
                    .any(|entry| entry.binding.presentation.as_str() == presentation_id.as_str())
            {
                return reject(Rejection::PresentationBusy);
            }
            if self.total_bindings() >= super::MAXIMUM_NAME_BINDINGS {
                return reject(Rejection::CapacityExceeded);
            }
            let Some(binding) = bindings.iter().find(|binding| {
                binding.presentation.as_str() == presentation_id.as_str()
                    && canonical_hex32(binding.host.host_epoch().as_bytes()) == host_epoch.as_str()
                    && canonical_hex32(binding.host.terminal_id().as_bytes())
                        == host_terminal_id.as_str()
            }) else {
                return reject(Rejection::BindingChanged);
            };
            // Wire validation guarantees the TTL range. Retain the guard for
            // typed in-process callers, before any authoritative mutation.
            let Some(expires_ms) = now
                .checked_add(ttl_ms.value())
                .filter(|_| (1..=MAXIMUM_LAUNCH_TTL_MS).contains(&ttl_ms.value()))
            else {
                return reject(Rejection::NotReady);
            };
            let entry = Entry {
                id: request_id.clone(),
                name: name.clone(),
                provider: provider_id.clone(),
                binding: binding.clone(),
                started_ms: now,
                expires_ms,
                baseline: snapshot
                    .sessions
                    .iter()
                    .filter(|session| attached(session, binding))
                    .map(|session| {
                        (
                            session.session_id.as_str().to_owned(),
                            (session.observation.generation, session.observation.sequence),
                        )
                    })
                    .collect(),
            };
            let reservation = entry.view(now);
            self.launches.entries.insert(request_id.clone(), entry);
            return Outcome::Reserved { reservation };
        }
        let reservation_id = match request {
            AgentLaunchRequest::Inspect { reservation_id, .. }
            | AgentLaunchRequest::Complete { reservation_id, .. }
            | AgentLaunchRequest::Cancel { reservation_id, .. } => reservation_id,
            AgentLaunchRequest::Reserve { .. } | AgentLaunchRequest::Status { .. } => {
                unreachable!("handled above")
            }
        };
        let Some(entry) = self.launches.entries.get(reservation_id) else {
            return reject(Rejection::ReservationNotFound);
        };
        if matches!(request, AgentLaunchRequest::Cancel { .. }) {
            self.launches.entries.remove(reservation_id);
            return Outcome::Released {
                reservation_id: reservation_id.clone(),
            };
        }
        let candidate = match entry.candidate(snapshot) {
            Ok(candidate) => candidate,
            Err(reason) => return reject(reason),
        };
        if matches!(request, AgentLaunchRequest::Inspect { .. }) {
            return Outcome::Pending {
                reservation: entry.view(now),
                candidate_session_id: candidate.map(|session| {
                    AgentSessionId::parse(session.session_id.as_str())
                        .expect("canonical session uses the wire identity profile")
                }),
            };
        }
        let AgentLaunchRequest::Complete { session_id, .. } = request else {
            unreachable!()
        };
        let Some(session) =
            candidate.filter(|session| session.session_id.as_str() == session_id.as_str())
        else {
            return reject(Rejection::CandidateUnavailable);
        };
        let ready = matches!(
            project_agent_session_summary(session).status,
            AgentSemanticStatus::Idle
                | AgentSemanticStatus::Succeeded
                | AgentSemanticStatus::Failed
                | AgentSemanticStatus::Canceled
                | AgentSemanticStatus::Interrupted
        );
        if now.saturating_sub(entry.started_ms) < LAUNCH_SETTLE_MS
            || session.observation.freshness != AgentFreshness::Fresh
            || !session.pending_requests.is_empty()
            || !ready
        {
            return reject(Rejection::NotReady);
        }
        if self.by_session.contains_key(session_id) {
            return reject(Rejection::AlreadyNamed);
        }
        let name = entry.name.clone();
        self.by_name.insert(name.clone(), session_id.clone());
        self.by_session.insert(session_id.clone(), name.clone());
        self.launches.active.insert(
            session_id.clone(),
            Managed {
                reservation_id: reservation_id.clone(),
                provider: entry.provider.clone(),
                binding: entry.binding.clone(),
            },
        );
        self.launches.entries.remove(reservation_id);
        Outcome::Completed {
            reservation_id: reservation_id.clone(),
            name,
            session_id: session_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests;

impl Names {
    pub(super) fn terminal_pending(
        &self,
        presentation: &orchbee_agent_observation_protocol::AgentPresentationId,
    ) -> bool {
        self.launches
            .entries
            .values()
            .any(|entry| entry.binding.presentation.as_str() == presentation.as_str())
    }
}
