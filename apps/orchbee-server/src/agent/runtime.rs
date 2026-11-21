//! `AgentRuntime` — the stateful owner of the canonical snapshot and its
//! bounded change journal.
//!
//! The production surface is deliberately read-mostly: empty-snapshot
//! construction, the rebuilt runtime a recovering owner starts from,
//! `snapshot`, `dispose`, `is_disposed`, `dispatch`, `rejected`, and
//! `take_refused` — semantic events ride the returned `AgentTransition` like the
//! TypeScript runtime's `transition.semanticEvents`. The seed constructor
//! stays under `#[cfg(test)]`; bounded change reads serve native observers.

use super::AgentObservationEvent;
use super::AgentRuntimeSnapshot;
use super::AgentTransition;
use super::changes::{
    AgentChangeReadError, AgentChangeReadInput, AgentRuntimeChangeReadResult,
    read_agent_runtime_changes,
};
use super::changes::{
    AgentRuntimeChange, append_agent_runtime_change, create_agent_runtime_change,
};
use super::model::MAX_SAFE_INTEGER;
use super::reducer::{AgentTransitionRejection, transition_agent_runtime};
use std::sync::Arc;

pub struct AgentRuntime {
    changes: Vec<AgentRuntimeChange>,
    disposed: bool,
    /// The `invalid-event` refusals not yet taken: the first one's transition kind and the count.
    refused: Option<(&'static str, u64)>,
    /// Shared with the transitions that report it, so no dispatch copies the whole snapshot.
    snapshot: Arc<AgentRuntimeSnapshot>,
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            disposed: false,
            refused: None,
            snapshot: Arc::new(AgentRuntimeSnapshot::create()),
        }
    }

    /// The runtime a recovering owner starts from: no Sessions and no journal, one revision past
    /// the last committed one, so every list cursor and change read based on earlier state reads
    /// stale or reset. At the revision bound it stays disposed rather than reuse a revision.
    pub fn rebuilt(committed_revision: u64, disposed: bool) -> Self {
        let (revision, disposed) = if committed_revision < MAX_SAFE_INTEGER {
            (committed_revision + 1, disposed)
        } else {
            (MAX_SAFE_INTEGER, true)
        };
        Self {
            changes: Vec::new(),
            disposed,
            refused: None,
            snapshot: Arc::new(AgentRuntimeSnapshot {
                revision,
                sessions: Vec::new(),
            }),
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
            refused: None,
            snapshot: Arc::new(snapshot),
        })
    }

    pub fn dispatch(&mut self, event: &AgentObservationEvent) -> AgentTransition {
        if self.disposed {
            return self.rejected(AgentTransitionRejection::RuntimeDisposed);
        }
        let transition = transition_agent_runtime(&self.snapshot, event);
        match &transition {
            AgentTransition::Applied { snapshot, .. } => {
                // An applied snapshot without its target Session has no journal record: refused.
                let Some(change) = create_agent_runtime_change(snapshot, event) else {
                    self.count_refusal(event);
                    return self.rejected(AgentTransitionRejection::InvalidEvent);
                };
                append_agent_runtime_change(&mut self.changes, change);
                self.snapshot = Arc::clone(snapshot);
            }
            AgentTransition::Rejected {
                reason: AgentTransitionRejection::InvalidEvent,
                ..
            } => self.count_refusal(event),
            AgentTransition::Unchanged { .. } | AgentTransition::Rejected { .. } => {}
        }
        transition
    }

    fn count_refusal(&mut self, event: &AgentObservationEvent) {
        let (_, count) = self.refused.get_or_insert((event.transition_kind(), 0));
        *count = count.saturating_add(1);
    }

    /// The `invalid-event` refusals since the last call — the first one's transition kind and the
    /// count — for the owner's diagnostic record.
    pub fn take_refused(&mut self) -> Option<(&'static str, u64)> {
        self.refused.take()
    }

    /// A rejection that reports the current snapshot without copying it.
    pub fn rejected(&self, reason: AgentTransitionRejection) -> AgentTransition {
        AgentTransition::Rejected {
            reason,
            snapshot: Arc::clone(&self.snapshot),
        }
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

    pub fn read_changes(
        &self,
        input: AgentChangeReadInput,
    ) -> Result<AgentRuntimeChangeReadResult, AgentChangeReadError> {
        read_agent_runtime_changes(self.snapshot.revision, &self.changes, input)
    }
}
