//! `AgentRuntime` — the stateful owner of the canonical snapshot and its
//! bounded change journal.
//!
//! The production surface is deliberately read-mostly: empty-snapshot
//! construction, `snapshot`, `dispose`, `is_disposed`, and `dispatch` —
//! semantic events ride the returned `AgentTransition` exactly like the
//! TypeScript runtime's `transition.semanticEvents`. The seed constructor
//! and change reads stay under `#[cfg(test)]` where only the corpus reaches
//! them.

use super::AgentObservationEvent;
use super::AgentRuntimeSnapshot;
use super::AgentTransition;
#[cfg(test)]
#[cfg(test)]
use super::changes::{
    AgentChangeReadError, AgentChangeReadInput, AgentRuntimeChangeReadResult,
    read_agent_runtime_changes,
};
use super::changes::{
    AgentRuntimeChange, append_agent_runtime_change, create_agent_runtime_change,
};
use super::reducer::{AgentTransitionRejection, transition_agent_runtime};

pub struct AgentRuntime {
    changes: Vec<AgentRuntimeChange>,
    disposed: bool,
    snapshot: AgentRuntimeSnapshot,
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            disposed: false,
            snapshot: AgentRuntimeSnapshot::create(),
        }
    }

    /// `constructor(snapshot)` — validates the seed snapshot like the
    /// TypeScript `AgentRuntimeSnapshotSchema.parse`.
    #[cfg(test)]
    pub fn from_snapshot(
        snapshot: AgentRuntimeSnapshot,
    ) -> Result<Self, super::model::AgentModelError> {
        snapshot.validate_runtime()?;
        Ok(Self {
            changes: Vec::new(),
            disposed: false,
            snapshot,
        })
    }

    pub fn dispatch(&mut self, event: &AgentObservationEvent) -> AgentTransition {
        if self.disposed {
            return AgentTransition::Rejected {
                reason: AgentTransitionRejection::RuntimeDisposed,
                snapshot: self.snapshot.clone(),
            };
        }
        let transition = transition_agent_runtime(&self.snapshot, event);
        let AgentTransition::Applied { snapshot, .. } = &transition else {
            return transition;
        };
        let change = create_agent_runtime_change(snapshot, event);
        append_agent_runtime_change(&mut self.changes, change);
        self.snapshot = snapshot.clone();
        transition
    }

    pub fn dispose(&mut self) {
        self.disposed = true;
    }

    /// Whether `dispose` has run. The single disposed flag stays inside the
    /// canonical runtime — no service-side duplicate exists.
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }

    pub fn snapshot(&self) -> &AgentRuntimeSnapshot {
        &self.snapshot
    }

    #[cfg(test)]
    pub fn read_changes(
        &self,
        input: AgentChangeReadInput,
    ) -> Result<AgentRuntimeChangeReadResult, AgentChangeReadError> {
        read_agent_runtime_changes(self.snapshot.revision, &self.changes, input)
    }
}
