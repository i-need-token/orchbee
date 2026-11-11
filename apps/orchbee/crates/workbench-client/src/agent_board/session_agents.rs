//! The Agents the attached Session currently knows, as the Start Agent picker reads them: which
//! providers ran recently and which Panels an Agent holds.
//!
//! Hand-typed Agents count as much as started ones. Nothing is persisted; a later record of past
//! starts replaces [`recent`] without touching the picker.

use orchbee_workbench_protocol::WorkbenchPanelId;

/// At most this many providers lead the picker as recent.
pub(crate) const RECENT_LIMIT: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    provider: &'static str,
    panel: WorkbenchPanelId,
    /// The Session's sequence of the Agent's latest state; a larger one is more recent.
    sequence: u64,
}

/// One observation of the Session's Agents.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SessionAgents {
    entries: Vec<Entry>,
}

impl SessionAgents {
    /// The sidebar's Agent rows for the attached Session. A row whose label names no startable
    /// provider still holds its Panel but never counts as recent.
    pub(crate) fn from_rows(rows: &[crate::machine_agents::checkpoint::Row]) -> Self {
        Self {
            entries: rows
                .iter()
                .map(|row| Entry {
                    provider: crate::agent_provider::canonical_id(&row.target.agent).unwrap_or(""),
                    panel: row.target.panel.clone(),
                    sequence: row.sequence.value(),
                })
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with(entries: &[(&'static str, WorkbenchPanelId, u64)]) -> Self {
        Self {
            entries: entries
                .iter()
                .map(|(provider, panel, sequence)| Entry {
                    provider,
                    panel: panel.clone(),
                    sequence: *sequence,
                })
                .collect(),
        }
    }

    /// Whether an Agent holds `panel`, so its shell is not at an idle prompt.
    pub(crate) fn holds(&self, panel: &WorkbenchPanelId) -> bool {
        self.entries.iter().any(|entry| &entry.panel == panel)
    }
}

/// The recent providers, most recent first and one per provider: this Client's own starts lead,
/// newest first, then the Session's other Agents by their latest state.
pub(crate) fn recent(own: &[&'static str], agents: &SessionAgents) -> Vec<&'static str> {
    let mut observed: Vec<&Entry> = agents
        .entries
        .iter()
        .filter(|entry| !entry.provider.is_empty())
        .collect();
    observed.sort_by_key(|entry| std::cmp::Reverse(entry.sequence));
    let mut recent: Vec<&'static str> = Vec::new();
    for provider in own
        .iter()
        .copied()
        .chain(observed.iter().map(|entry| entry.provider))
    {
        if !recent.contains(&provider) {
            recent.push(provider);
        }
    }
    recent
}
