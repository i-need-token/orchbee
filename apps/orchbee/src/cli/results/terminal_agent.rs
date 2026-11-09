//! Physical Agent terminal result documents: exact semantic reads, matched waits, and one
//! Client's attention view corroborated against the physical report owner.

use orchbee_agent_observation_protocol::AgentSafeInteger;
use orchbee_workbench_protocol::{
    ClientViewObservation, TerminalAgentChange, TerminalAgentObservation, WorkbenchClientId,
    WorkbenchRequestId, WorkbenchRevision,
};
use serde_json::{Value, json};

use crate::invocation::AgentWaitCondition;

/// One exact semantic read of a physical Agent terminal at the report revision it came from.
#[derive(Debug)]
pub(crate) struct TerminalAgentRead {
    pub source: Value,
    pub report_revision: WorkbenchRevision,
    pub observation: Box<TerminalAgentObservation>,
}
impl TerminalAgentRead {
    pub(in crate::cli) fn document(&self) -> Value {
        json!({"source":self.source,"observation":self.observation})
    }
}

/// A physical match retains the latest read separately from a transient matched change.
#[derive(Debug)]
pub(crate) struct Matched {
    pub(in crate::cli) client_view: Option<ClientAttention>,
    /// The stable handle a person targets the matched terminal by, resolved
    /// from the snapshot of the matching read; absent when that read came
    /// from a caller without one. Text only: JSON keeps the binding.
    pub(in crate::cli) handle: Option<String>,
    pub(in crate::cli) read: TerminalAgentRead,
    pub(in crate::cli) conditions: Vec<AgentWaitCondition>,
    pub(in crate::cli) change: Option<TerminalAgentChange>,
    pub(in crate::cli) until: Vec<AgentWaitCondition>,
    pub(in crate::cli) after_revision: Option<AgentSafeInteger>,
    pub(in crate::cli) timeout_ms: Option<u32>,
    pub(in crate::cli) observations: u64,
    pub(in crate::cli) elapsed_ms: u64,
}
impl Matched {
    pub(in crate::cli) fn document(&self) -> Value {
        let wait = json!({"until":self.until.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "afterRevision":self.after_revision,"timeoutMs":self.timeout_ms,"observations":self.observations,"elapsedMs":self.elapsed_ms});
        let mut result = json!({"kind":"terminal-matched","target":{"kind":"terminal","binding":self.read.observation.binding},
            "semantic":self.read.document(),"matchedConditions":self.conditions.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "matchedChange":self.change,"wait":wait});
        result["clientView"] = self
            .client_view
            .as_ref()
            .map_or(Value::Null, |view| view.document());
        result
    }
}

/// One selected Client's view of an Agent terminal, corroborated against the physical report.
#[derive(Clone, Debug)]
pub(crate) struct ClientAttention {
    pub(in crate::cli) client_id: WorkbenchClientId,
    pub(in crate::cli) operation_id: WorkbenchRequestId,
    pub(in crate::cli) view: ClientViewObservation,
    pub(in crate::cli) report_revision: WorkbenchRevision,
    pub(in crate::cli) sequence: Option<WorkbenchRevision>,
    pub(in crate::cli) seen: Option<bool>,
    pub(in crate::cli) status: Option<&'static str>,
    pub(in crate::cli) outer_focused: Option<bool>,
}
impl ClientAttention {
    pub(in crate::cli) fn document(&self) -> Value {
        json!({"clientId":self.client_id,"operationId":self.operation_id,"view":self.view,
            "reportRevision":self.report_revision,"sequence":self.sequence,"seen":self.seen,
            "status":self.status,"outerFocused":self.outer_focused})
    }
}
