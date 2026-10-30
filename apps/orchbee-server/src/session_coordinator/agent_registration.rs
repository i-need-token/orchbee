//! Projects bounded Agent registration facts from committed presentations joined to exact
//! retained Worker and Host bindings. It does not infer Agent state, create registrations for
//! uncommitted membership, or consume physical observations.

use super::SessionCoordinator;
use crate::terminal_coordinator::{RetainedBindingResolution, TerminalPhase};
use orchbee_capability_node_protocol::{
    HostTerminalIdentity, TerminalId as CapabilityTerminalId, WorkerGeneration, WorkerId,
};
use orchbee_server::WorkbenchTerminalInvariantError;
use orchbee_workbench_protocol::{PresentationBinding, TerminalPresentationId};

/// Product bound carried by the Agent registration projection. The committed
/// membership model already bounds visible presentations; this second bound keeps the
/// snapshot shape explicit for the drain-read consumer.
const MAXIMUM_REGISTRATION_FACTS: usize = 1024;

/// One eligible registration: a visible committed Workbench presentation whose
/// binding resolves to a uniquely retained terminal authorization holding a Host
/// identity. Worker fields come straight from the typed authorization, never from
/// Workbench string fields; `identity` already carries creation, epoch, and terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegistrationFact {
    presentation_id: TerminalPresentationId,
    binding: PresentationBinding,
    worker_id: WorkerId,
    worker_generation: WorkerGeneration,
    identity: HostTerminalIdentity,
}

impl RegistrationFact {
    #[must_use]
    pub fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }

    #[must_use]
    pub fn binding(&self) -> &PresentationBinding {
        &self.binding
    }

    #[must_use]
    pub const fn worker_id(&self) -> WorkerId {
        self.worker_id
    }

    #[must_use]
    pub const fn worker_generation(&self) -> WorkerGeneration {
        self.worker_generation
    }

    #[must_use]
    pub const fn identity(&self) -> HostTerminalIdentity {
        self.identity
    }
}

fn capability_terminal_id(binding: &PresentationBinding) -> Option<CapabilityTerminalId> {
    u128::from_str_radix(binding.terminal_id.as_str(), 16)
        .ok()
        .and_then(|identity| CapabilityTerminalId::from_bytes(identity.to_be_bytes()).ok())
}

fn capability_worker_id(binding: &PresentationBinding) -> Option<WorkerId> {
    u128::from_str_radix(binding.worker_id.as_str(), 16)
        .ok()
        .and_then(|identity| WorkerId::from_bytes(identity.to_be_bytes()).ok())
}

fn retained_identity(phase: TerminalPhase) -> Option<HostTerminalIdentity> {
    match phase {
        TerminalPhase::Creating => None,
        TerminalPhase::AwaitingStart(identity)
        | TerminalPhase::Starting(identity)
        | TerminalPhase::Started { identity, .. } => Some(identity),
        TerminalPhase::Unknown(identity) | TerminalPhase::Unavailable(identity) => identity,
    }
}

impl SessionCoordinator {
    /// The Agent registration read the drain pump projects: one pure, bounded,
    /// all-or-nothing snapshot over the committed Workbench membership. It performs
    /// no I/O, holds no cache, and supplies both the terminal service's prepared-read
    /// provider and Windows process observation targets. An invariant failure aborts
    /// the whole snapshot rather than returning a partial vector.
    pub(crate) fn agent_registration_snapshot(
        &self,
    ) -> Result<Vec<RegistrationFact>, WorkbenchTerminalInvariantError> {
        self.agent_registration_filtered(|_| true, MAXIMUM_REGISTRATION_FACTS)
    }

    /// Checkpoint-owned bounded selection; ordinary canonical registration keeps its own bound.
    pub(crate) fn agent_registration_for_bindings(
        &self,
        bindings: &[PresentationBinding],
    ) -> Result<Vec<RegistrationFact>, WorkbenchTerminalInvariantError> {
        self.agent_registration_filtered(|binding| bindings.contains(binding), bindings.len())
    }

    fn agent_registration_filtered(
        &self,
        include: impl Fn(&PresentationBinding) -> bool,
        maximum: usize,
    ) -> Result<Vec<RegistrationFact>, WorkbenchTerminalInvariantError> {
        let mut facts = Vec::new();
        let Some(workbench) = &self.workbench else {
            return Ok(facts);
        };
        let snapshot = workbench.snapshot();
        for context in &snapshot.terminal_contexts {
            let membership = workbench.presentation_membership(&context.terminal_context_id)?;
            for member in membership.members() {
                let binding = member.binding();
                if !include(binding) {
                    continue;
                }
                let terminal =
                    capability_terminal_id(binding).ok_or(WorkbenchTerminalInvariantError)?;
                let worker =
                    capability_worker_id(binding).ok_or(WorkbenchTerminalInvariantError)?;
                let retained = match self.terminals.retained_binding_resolution(terminal, worker) {
                    RetainedBindingResolution::Absent => continue,
                    RetainedBindingResolution::Unique(retained) => retained,
                    RetainedBindingResolution::Ambiguous
                    | RetainedBindingResolution::Inconsistent => {
                        return Err(WorkbenchTerminalInvariantError);
                    }
                };
                let Some(identity) = retained_identity(retained.phase) else {
                    continue;
                };
                let authorization = self
                    .terminals
                    .retained_authorization(retained.reference)
                    .ok_or(WorkbenchTerminalInvariantError)?;
                if facts.len() == maximum {
                    return Err(WorkbenchTerminalInvariantError);
                }
                facts.push(RegistrationFact {
                    presentation_id: member.presentation_id().clone(),
                    binding: binding.clone(),
                    worker_id: authorization.worker_id(),
                    worker_generation: authorization.worker_generation(),
                    identity,
                });
            }
        }
        Ok(facts)
    }
}
