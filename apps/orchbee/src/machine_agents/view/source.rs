//! Raw canonical fields, separate from display metadata and terminal routing authority.
use super::{
    AgentViewSpec, Choice, Entry, NativeContextLocation, NativeContextTarget, PanelAgentState, Row,
    View,
};
use orchbee_workbench_protocol::agent_view::evaluation::{self, Context, Entry as ViewEntry};

pub(super) fn scope(choice: &Choice) -> &str {
    match choice {
        Choice::Local => "local",
        Choice::Remote(id) => id.as_str(),
    }
}
impl ViewEntry for Entry<'_> {
    fn scope(&self) -> &str {
        scope(self.machine)
    }
    fn status(&self) -> &str {
        self.row.status()
    }
    fn workspace_id(&self) -> Option<&str> {
        match &self.row.target.context.location {
            NativeContextLocation::NoWorkspace => None,
            NativeContextLocation::Workspace { workspace_id, .. } => Some(workspace_id.as_str()),
        }
    }
    fn tab_id(&self) -> Option<&str> {
        Some(self.row.target.tab.as_str())
    }
    fn pane_id(&self) -> Option<&str> {
        Some(self.row.target.member.as_str())
    }
    fn agent(&self) -> Option<&str> {
        Some(&self.row.target.agent)
    }
    fn seen(&self) -> bool {
        self.row.seen
    }
    fn state_change_seq(&self) -> Option<u64> {
        Some(self.row.sequence.value())
    }
    fn token(&self, name: &str) -> Option<&str> {
        self.row
            .tokens
            .as_map()
            .iter()
            .find(|(k, _)| k.as_str() == name)
            .map(|(_, v)| v.as_str())
    }
    fn workspace_order(&self) -> Option<u64> {
        self.row.order.workspace
    }
    fn tab_order(&self) -> Option<u64> {
        Some(self.row.order.tab)
    }
    fn pane_order(&self) -> Option<u64> {
        Some(self.row.order.pane)
    }
    fn attention(&self) -> u64 {
        match self.row.status() {
            "blocked" => 5,
            "error" => 4,
            "done" => 3,
            "working" => 2,
            "idle" => 1,
            _ => 0,
        }
    }
}
impl View {
    pub(crate) fn project_source<'a>(
        &self,
        candidates: impl IntoIterator<Item = Entry<'a>>,
        current: &Choice,
        context: Option<&NativeContextTarget>,
        member: Option<&crate::workbench_client::NativeMemberTarget>,
        spec: Option<&AgentViewSpec>,
    ) -> Vec<Entry<'a>> {
        let evaluation_context = Context {
            scope: scope(current),
            workspace_id: context.and_then(|c| match &c.location {
                NativeContextLocation::Workspace { workspace_id, .. } => {
                    Some(workspace_id.as_str())
                }
                _ => None,
            }),
            tab_id: member.map(|m| m.tab.as_str()),
        };
        let mut rows: Vec<_> = candidates
            .into_iter()
            .filter(|e| self.includes(e.machine, e.label, e.row, current, context))
            .filter(|e| {
                spec.and_then(|s| s.filter.as_ref())
                    .is_none_or(|f| evaluation::matches_filter(&evaluation_context, e, f))
            })
            .collect();
        if let Some(spec) = spec.filter(|s| !s.sort.is_empty()) {
            rows.sort_by(|a, b| evaluation::compare_entries(a, b, &spec.sort));
        } else {
            rows.sort_by(|a, b| self.compare(a.row, b.row));
        }
        rows
    }
}
impl Row {
    pub(crate) fn status(&self) -> &'static str {
        match self.state {
            PanelAgentState::Done => "done",
            PanelAgentState::Error => "error",
            PanelAgentState::Idle => "idle",
            PanelAgentState::Blocked => "blocked",
            PanelAgentState::Working => "working",
            PanelAgentState::Unknown => "unknown",
        }
    }
}
