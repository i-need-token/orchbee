//! The Panels the attached Session's Agents hold, as the Start Agent picker reads them to decide
//! whether the focused Panel's shell is at an idle prompt.
//!
//! Hand-typed Agents count as much as started ones. Nothing is persisted; which providers ran
//! recently comes from the Session's Agent catalog instead.

use orchbee_workbench_protocol::WorkbenchPanelId;

/// One observation of the Session's Agents: the Panel of every Agent the sidebar shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SessionAgents {
    panels: Vec<WorkbenchPanelId>,
}

impl SessionAgents {
    /// The sidebar's Agent rows for the attached Session.
    pub(crate) fn from_rows(rows: &[crate::machine_agents::checkpoint::Row]) -> Self {
        Self {
            panels: rows.iter().map(|row| row.target.panel.clone()).collect(),
        }
    }

    /// Whether an Agent holds `panel`, so its shell is not at an idle prompt.
    pub(crate) fn holds(&self, panel: &WorkbenchPanelId) -> bool {
        self.panels.contains(panel)
    }
}
