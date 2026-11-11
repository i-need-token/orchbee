//! The Agents the attached Session currently knows, as the Agent board and the Start Agent picker
//! read them: which Agents the sidebar shows and which Panels an Agent holds.
//!
//! Hand-typed Agents count as much as started ones. Nothing is persisted; which providers ran
//! recently comes from the Session's Agent catalog instead.

use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{PanelAgentState, WorkbenchPanelId};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    panel: WorkbenchPanelId,
    shown: ShownAgent,
}

/// One Agent as the sidebar shows it: its report label, the terminal it runs in and its state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShownAgent {
    pub(crate) agent: String,
    pub(crate) binding: AgentLaunchBinding,
    pub(crate) state: PanelAgentState,
}

/// One observation of the Session's Agents.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SessionAgents {
    entries: Vec<Entry>,
}

impl SessionAgents {
    /// The sidebar's Agent rows for the attached Session.
    pub(crate) fn from_rows(rows: &[crate::machine_agents::checkpoint::Row]) -> Self {
        Self {
            entries: rows
                .iter()
                .map(|row| Entry {
                    panel: row.target.panel.clone(),
                    shown: ShownAgent {
                        agent: row.target.agent.clone(),
                        binding: row.target.physical.clone(),
                        state: row.state,
                    },
                })
                .collect(),
        }
    }

    /// Every Agent the sidebar shows, in its order.
    pub(crate) fn shown(&self) -> impl Iterator<Item = &ShownAgent> {
        self.entries.iter().map(|entry| &entry.shown)
    }

    /// Whether an Agent holds `panel`, so its shell is not at an idle prompt.
    pub(crate) fn holds(&self, panel: &WorkbenchPanelId) -> bool {
        self.entries.iter().any(|entry| &entry.panel == panel)
    }
}
