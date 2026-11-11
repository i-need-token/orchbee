//! Pure machine-qualified Agent list projection, independent of network and input ownership.
use super::checkpoint::Row;
use crate::client_context::{ClientContextLocation, ClientContextTarget};
use crate::machine_discovery::overview::display_label;
use crate::machines::Choice;
use crate::terminal_text::line_editor::{LengthLimit, LineEditor};
use crate::terminal_title::ChromeName;
use orchbee_workbench_protocol::{
    AgentViewSpec, MetadataState, PanelAgentState, TerminalTitleObservation,
};
mod source;

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
/// The Agent search query's scalar budget.
const MAXIMUM_QUERY_SCALARS: usize = 128;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct View {
    pub(crate) state: Option<MetadataState>,
    pub(crate) scope: Scope,
    pub(crate) order: Order,
    pub(crate) query: LineEditor,
}
impl Default for View {
    fn default() -> Self {
        Self {
            state: None,
            scope: Scope::default(),
            order: Order::default(),
            query: Self::query_editor(String::new()),
        }
    }
}
pub(crate) struct Entry<'a> {
    pub(crate) machine: &'a Choice,
    pub(crate) label: &'a str,
    pub(crate) row: &'a Row,
}
impl View {
    #[cfg(test)]
    pub(crate) fn project<'a>(
        &self,
        candidates: impl IntoIterator<Item = Entry<'a>>,
        current: &Choice,
        context: Option<&ClientContextTarget>,
    ) -> Vec<Entry<'a>> {
        self.project_source(candidates, current, context, None, None)
    }

    pub(crate) fn query_editor(query: String) -> LineEditor {
        LineEditor::new(query, LengthLimit::Scalars(MAXIMUM_QUERY_SCALARS))
    }

    /// Inserts one typed scalar at the query cursor; a scalar without a display label or one past
    /// the budget is dropped.
    pub(crate) fn push(&mut self, c: char) {
        let text = c.to_string();
        if !display_label(&text).is_empty() {
            let _ = self.query.insert(&text);
        }
    }
    pub(crate) fn cycle_state(&mut self) {
        self.state = match self.state {
            None => Some(MetadataState::Blocked),
            Some(MetadataState::Blocked) => Some(MetadataState::Error),
            Some(MetadataState::Error) => Some(MetadataState::Done),
            Some(MetadataState::Done) => Some(MetadataState::Working),
            Some(MetadataState::Working) => Some(MetadataState::Idle),
            Some(MetadataState::Idle) => Some(MetadataState::Unknown),
            Some(MetadataState::Unknown) => None,
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
        context: Option<&ClientContextTarget>,
    ) -> bool {
        if self.state.is_some_and(|state| state != row.display_state())
            || (self.scope != Scope::All && machine != current)
            || (self.scope == Scope::Context && context != Some(&row.target.context))
        {
            return false;
        }
        let query = self.query.text().trim().to_lowercase();
        query.is_empty()
            || [
                if machine == &Choice::Local {
                    "local"
                } else {
                    label
                },
                label,
                row.name.as_deref().unwrap_or_default(),
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
            Order::Priority => priority(b).cmp(&priority(a)),
            Order::Provider => display_label(a.provider()).cmp(&display_label(b.provider())),
            Order::Title => display_label(a.title()).cmp(&display_label(b.title())),
            Order::Sequence => b.sequence.cmp(&a.sequence),
        }
    }
}
fn priority(row: &Row) -> u8 {
    urgency(row)
}

pub(crate) fn urgency(row: &Row) -> u8 {
    match row.status() {
        "blocked" => 5,
        "error" => 4,
        "done" => 3,
        "working" | "opening" => 2,
        "idle" => 1,
        _ => 0,
    }
}

impl Row {
    pub(crate) fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| self.provider())
    }

    pub(crate) fn title(&self) -> &str {
        self.metadata
            .title
            .as_ref()
            .map_or(&self.label, |v| v.as_str())
    }
    /// The Panel's display Agent, else the observed provider's product name, else its label.
    pub(crate) fn provider(&self) -> &str {
        self.metadata.display_agent.as_ref().map_or_else(
            || crate::agent_provider::product_name(&self.target.agent),
            |v| v.as_str(),
        )
    }

    /// What names this Agent ahead of its provider, sanitized: a person's name for it, then a Panel
    /// name the person chose, then an explicit title, then the title its terminal program set, then
    /// the Panel's creation label. Blank text, an automatic number, a console's launch path, and
    /// text that only names the provider or its program, as an Agent's idle title does, name
    /// nothing.
    pub(crate) fn identity_title(&self) -> Option<String> {
        // A Panel name the person chose outranks a title a program reported, as in the Panel header.
        let explicit = self
            .label_custom
            .then_some(self.label.as_str())
            .or_else(|| self.metadata.title.as_ref().map(|title| title.as_str()));
        let observed = match &self.terminal_title {
            TerminalTitleObservation::Observed {
                title: Some(title), ..
            } => match ChromeName::of_title(title) {
                Some(ChromeName::Text(title)) => Some(title),
                Some(ChromeName::Program { .. }) | None => None,
            },
            TerminalTitleObservation::Observed { title: None, .. }
            | TerminalTitleObservation::Unavailable {} => None,
        };
        let creation = (!self.label_custom).then_some(self.label.as_str());
        [
            (self.name.as_deref(), false),
            (explicit, true),
            (observed, false),
            (creation, true),
        ]
        .into_iter()
        .find_map(|(text, numbered)| {
            let text = display_label(text?);
            let text = text.trim();
            let automatic = numbered && text.chars().all(|ch| ch.is_ascii_digit());
            (!text.is_empty() && !automatic && !self.names_provider(text)).then(|| text.to_owned())
        })
    }

    /// Whether `text` only names this Agent's provider: its shown or reported label, canonical
    /// ID, or command, ignoring case, spacing and punctuation, so `Claude Code` names
    /// `claude-code`.
    fn names_provider(&self, text: &str) -> bool {
        let key = |name: &str| -> String {
            name.chars()
                .filter(|ch| ch.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect()
        };
        let text = key(text);
        let known =
            orchbee_agent_observation_protocol::AgentReportProvider::lookup(&self.target.agent);
        [self.provider(), self.target.agent.as_str()]
            .into_iter()
            .chain(known.into_iter().flat_map(|provider| {
                [
                    Some(provider.report_label()),
                    Some(provider.native_provider_id()),
                    crate::agent_provider::executable(provider.native_provider_id()),
                ]
                .into_iter()
                .flatten()
            }))
            .any(|name| key(name) == text)
    }
    pub(crate) fn display_state(&self) -> MetadataState {
        match self.status() {
            "blocked" => MetadataState::Blocked,
            "done" => MetadataState::Done,
            "error" => MetadataState::Error,
            // An opening Agent breathes with the working glyph until its screen is recognised.
            "working" | "opening" => MetadataState::Working,
            "idle" => MetadataState::Idle,
            _ => MetadataState::Unknown,
        }
    }
    /// The machine-picker text key of this row's state word.
    pub(crate) fn state_word_key(&self) -> &'static str {
        match self.status() {
            "opening" => "agent-starting",
            _ => match self.display_state() {
                MetadataState::Idle => "agent-idle",
                MetadataState::Done => "agent-done",
                MetadataState::Error => "agent-error",
                MetadataState::Working => "agent-working",
                MetadataState::Blocked => "agent-blocked",
                MetadataState::Unknown => "agent-unknown",
            },
        }
    }
    pub(crate) fn state_label(&self) -> Option<&str> {
        let state = match self.completion.display(self.state) {
            PanelAgentState::Done => MetadataState::Done,
            PanelAgentState::Error => MetadataState::Error,
            PanelAgentState::Blocked => MetadataState::Blocked,
            PanelAgentState::Working | PanelAgentState::Opening => MetadataState::Working,
            PanelAgentState::Idle => MetadataState::Idle,
            PanelAgentState::Unknown => MetadataState::Unknown,
        };
        self.metadata.state_labels.get(&state).map(|v| v.as_str())
    }
}
#[cfg(test)]
mod tests;
