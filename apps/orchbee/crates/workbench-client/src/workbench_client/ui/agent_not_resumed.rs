//! Shows the Server's one-shot fact that a resume after a restart typed nothing. Each fact shows
//! once, as a 4-second status-line notice, and only while it is fresh: a Client that connects
//! later, or reads a snapshot the fact has long sat in, says nothing. An identical notice already
//! showing keeps its time. The GUI's notice shows the same facts the same way.

use super::{WorkbenchClientUi, presentation_facts};
use crate::localization::WorkbenchClientNotice;
use orchbee_workbench_protocol::{AgentNotResumedReason, Snapshot};
use std::time::Duration;

/// How long a notice shows: the status line's transient lifetime.
const NOTICE: Duration = Duration::from_secs(4);

/// The resume facts this Client already decided on.
#[derive(Default)]
pub(super) struct AgentNotResumedNotices {
    facts: presentation_facts::FactNotices,
}

impl AgentNotResumedNotices {
    /// Records every fact in `snapshot` and returns the newest one that is new to this Client and
    /// fresh at `now` Unix milliseconds.
    fn observe(
        &mut self,
        snapshot: &Snapshot,
        now: u64,
    ) -> Option<(String, AgentNotResumedReason)> {
        self.facts
            .observe(snapshot, now, |presentation| {
                let fact = presentation.agent_not_resumed.as_deref()?;
                Some((fact, fact.at))
            })
            .map(|fact| (fact.provider_id.as_str().to_owned(), fact.reason))
    }
}

impl WorkbenchClientUi {
    /// Shows the newest fresh resume fact `snapshot` brings, unless the same notice is showing.
    pub(super) fn observe_agent_not_resumed(&mut self, snapshot: &Snapshot) {
        let Some(now) = presentation_facts::unix_now() else {
            return;
        };
        let Some((provider, reason)) = self.agent_not_resumed.observe(snapshot, now) else {
            return;
        };
        let agent = crate::agent_provider::integration_id(&provider)
            .and_then(crate::agent_provider::IntegrationId::parse);
        let notice = WorkbenchClientNotice::AgentNotResumed(agent, reason);
        let now = self.clock.now();
        if self.command_notice.current(now) != Some(notice) {
            self.command_notice
                .set(now.checked_add(NOTICE).map(|until| (notice, until)));
        }
    }
}

#[cfg(test)]
mod tests;
