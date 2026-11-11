//! Shows the Server's one-shot fact that the Agent start a new Tab, split or Worktree carried did
//! not happen, as the start notice its reason names, whichever Panel has focus. Each fact shows
//! once and only while fresh, as a resume fact does; a fact found before this Client attached is
//! old, so attaching or reconnecting never repeats one. The Server marks a not-found provider's
//! catalog row itself, which lists it under not found in the Start Agent picker.

use super::{WorkbenchClientUi, presentation_facts};
use crate::workbench_state::{AgentStartNotice, AgentStartRefusal};
use orchbee_workbench_protocol::{AgentStartFailureReason, Snapshot};

/// The start facts this Client already decided on.
#[derive(Default)]
pub(super) struct AgentStartFailureNotices {
    facts: presentation_facts::FactNotices,
}

impl AgentStartFailureNotices {
    /// This Client attached at `now` Unix milliseconds: every fact found before then is old.
    pub(super) const fn attached(&mut self, now: u64) {
        self.facts.hold_back_before(now);
    }

    /// Records every fact in `snapshot` and returns the provider and reason of the newest one that
    /// is new to this Client, found since it attached and fresh at `now` Unix milliseconds.
    fn observe(
        &mut self,
        snapshot: &Snapshot,
        now: u64,
    ) -> Option<(&'static str, AgentStartFailureReason)> {
        self.facts
            .observe(snapshot, now, |presentation| {
                let fact = presentation.agent_start_failure.as_deref()?;
                Some((fact, fact.at))
            })
            .map(|fact| (fact.provider_id.agent().id(), fact.reason))
    }
}

/// The start notice a fact's reason names for `product`. A shell whose prompt was never confirmed
/// or whose Host predates the prompt gate reads like the typed start's own refusals.
const fn notice(reason: AgentStartFailureReason, product: &'static str) -> AgentStartNotice {
    match reason {
        AgentStartFailureReason::NotFound => AgentStartNotice::NotFound(product),
        AgentStartFailureReason::Exited => AgentStartNotice::Exited(product),
        AgentStartFailureReason::UnsupportedShell => AgentStartNotice::UnsupportedShell(product),
        AgentStartFailureReason::PromptUnconfirmed => {
            AgentStartNotice::Refused(AgentStartRefusal::NotAtPrompt)
        }
        AgentStartFailureReason::TerminalTooOld => {
            AgentStartNotice::Refused(AgentStartRefusal::TerminalTooOld)
        }
    }
}

impl WorkbenchClientUi {
    /// Notes this Client's attachment: the start facts already present are old.
    pub(super) fn attach_agent_start_failures(&mut self) {
        if let Some(now) = presentation_facts::unix_now() {
            self.agent_start_failure.attached(now);
        }
    }

    /// Shows the newest new start fact `snapshot` brings as the start notice.
    pub(super) fn observe_agent_start_failure(&mut self, snapshot: &Snapshot) {
        let Some(now) = presentation_facts::unix_now() else {
            return;
        };
        let Some((provider, reason)) = self.agent_start_failure.observe(snapshot, now) else {
            return;
        };
        let product =
            crate::agent_provider::choice(provider).map_or("Agent", |choice| choice.product);
        self.board.start_state().set_notice(notice(reason, product));
    }
}

#[cfg(test)]
mod tests;
