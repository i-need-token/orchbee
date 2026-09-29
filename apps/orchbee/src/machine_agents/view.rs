//! Pure machine-qualified Agent list projection, independent of network and input ownership.
use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Scope {
    #[default]
    All,
    Machine,
    Context,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Order {
    #[default]
    Placement,
    Priority,
    Provider,
    Title,
    Sequence,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct View {
    pub(crate) state: Option<PanelAgentState>,
    pub(crate) scope: Scope,
    pub(crate) order: Order,
    pub(crate) query: String,
}
pub(crate) struct Entry<'a> {
    pub(crate) machine: &'a Choice,
    pub(crate) label: &'a str,
    pub(crate) row: &'a Row,
}
impl View {
    pub(crate) fn project<'a>(
        &self,
        candidates: impl IntoIterator<Item = Entry<'a>>,
        current: &Choice,
        context: Option<&NativeContextTarget>,
    ) -> Vec<Entry<'a>> {
        let mut rows: Vec<_> = candidates
            .into_iter()
            .filter(|e| self.includes(e.machine, e.label, e.row, current, context))
            .collect();
        rows.sort_by(|a, b| self.compare(a.row, b.row));
        rows
    }

    pub(crate) fn push(&mut self, c: char) {
        if self.query.chars().count() < 128 && !display_label(&c.to_string()).is_empty() {
            self.query.push(c);
        }
    }
    pub(crate) fn cycle_state(&mut self) {
        self.state = match self.state {
            None => Some(PanelAgentState::Blocked),
            Some(PanelAgentState::Blocked) => Some(PanelAgentState::Working),
            Some(PanelAgentState::Working) => Some(PanelAgentState::Idle),
            Some(PanelAgentState::Idle) => Some(PanelAgentState::Unknown),
            Some(PanelAgentState::Unknown) => None,
        };
    }
    pub(crate) fn cycle_scope(&mut self) {
        self.scope = match self.scope {
            Scope::All => Scope::Machine,
            Scope::Machine => Scope::Context,
            Scope::Context => Scope::All,
        };
    }
    pub(crate) fn cycle_order(&mut self) {
        self.order = match self.order {
            Order::Placement => Order::Priority,
            Order::Priority => Order::Provider,
            Order::Provider => Order::Title,
            Order::Title => Order::Sequence,
            Order::Sequence => Order::Placement,
        };
    }
    pub(crate) fn includes(
        &self,
        machine: &Choice,
        label: &str,
        row: &Row,
        current: &Choice,
        context: Option<&NativeContextTarget>,
    ) -> bool {
        if self.state.is_some_and(|state| state != row.state)
            || (self.scope != Scope::All && machine != current)
            || (self.scope == Scope::Context && context != Some(&row.target.context))
        {
            return false;
        }
        let query = self.query.trim().to_lowercase();
        query.is_empty()
            || [
                if machine == &Choice::Local {
                    "local"
                } else {
                    label
                },
                label,
                &row.workspace,
                &row.label,
                row.provider(),
                row.title(),
                row.target.member.as_str(),
            ]
            .into_iter()
            .any(|s| display_label(s).to_lowercase().contains(&query))
            || row.tokens.as_map().iter().any(|(k, v)| {
                format!(
                    "{}:{}",
                    display_label(k.as_str()),
                    display_label(v.as_str())
                )
                .to_lowercase()
                .contains(&query)
            })
    }
    pub(crate) fn compare(&self, a: &Row, b: &Row) -> std::cmp::Ordering {
        match self.order {
            Order::Placement => std::cmp::Ordering::Equal,
            Order::Priority => priority(b.state).cmp(&priority(a.state)),
            Order::Provider => display_label(a.provider()).cmp(&display_label(b.provider())),
            Order::Title => display_label(a.title()).cmp(&display_label(b.title())),
            Order::Sequence => b.sequence.cmp(&a.sequence),
        }
    }
}
fn priority(state: PanelAgentState) -> u8 {
    match state {
        PanelAgentState::Blocked => 4,
        PanelAgentState::Working => 2,
        PanelAgentState::Idle => 1,
        PanelAgentState::Unknown => 0,
    }
}
impl Row {
    pub(crate) fn title(&self) -> &str {
        self.metadata
            .title
            .as_ref()
            .map_or(&self.label, |v| v.as_str())
    }
    pub(crate) fn provider(&self) -> &str {
        self.metadata
            .display_agent
            .as_ref()
            .map_or(&self.target.agent, |v| v.as_str())
    }
    pub(crate) fn state_label(&self) -> Option<&str> {
        let state = match self.state {
            PanelAgentState::Blocked => MetadataState::Blocked,
            PanelAgentState::Working => MetadataState::Working,
            PanelAgentState::Idle => MetadataState::Idle,
            PanelAgentState::Unknown => MetadataState::Unknown,
        };
        self.metadata.state_labels.get(&state).map(|v| v.as_str())
    }
}
#[cfg(test)]
mod tests;
