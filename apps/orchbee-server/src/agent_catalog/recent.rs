//! The Session's Recent list: the providers started most recently, newest first, one each, at
//! most [`MAXIMUM_RECENT_AGENTS`]. It decides order only; which starts count is the caller's.
use orchbee_workbench_protocol::{AgentStartSelector, MAXIMUM_RECENT_AGENTS};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Recent(Vec<AgentStartSelector>);

impl Recent {
    /// Moves `agent` to the front, dropping its older entry and the oldest beyond the bound.
    pub(super) fn start(&mut self, agent: AgentStartSelector) {
        self.0.retain(|kept| *kept != agent);
        self.0.insert(0, agent);
        self.0.truncate(MAXIMUM_RECENT_AGENTS);
    }

    /// A saved list, newest first, under the same rules: a repeated provider keeps its newest
    /// place and the oldest beyond the bound drop.
    #[cfg(windows)]
    pub(super) fn restored(saved: &[AgentStartSelector]) -> Self {
        let mut recent = Self::default();
        for agent in saved.iter().rev() {
            recent.start(*agent);
        }
        recent
    }

    pub(super) fn agents(&self) -> &[AgentStartSelector] {
        &self.0
    }
}
