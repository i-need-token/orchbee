//! Shows the Server's one-shot fact that a resume after a restart typed nothing. Each fact shows
//! once, as a 4-second status-line notice, and only while it is fresh: a Client that connects
//! later, or reads a snapshot the fact has long sat in, says nothing. An identical notice already
//! showing keeps its time. The GUI's notice shows the same facts the same way.

use super::WorkbenchClientUi;
use crate::localization::WorkbenchClientNotice;
use orchbee_workbench_protocol::{
    AgentNotResumedReason, PanelNode, PresentationAgentNotResumed, Snapshot, TerminalPresentationId,
};
use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// How long a notice shows: the status line's transient lifetime.
const NOTICE: Duration = Duration::from_secs(4);
/// How far a fact's time may lie from this Client's clock and still read as news.
const FRESH_MILLISECONDS: u64 = 60_000;

/// The facts this Client already decided on, by Presentation and time.
#[derive(Default)]
pub(super) struct AgentNotResumedNotices {
    seen: BTreeSet<(TerminalPresentationId, u64)>,
}

impl AgentNotResumedNotices {
    /// Records every fact in `snapshot` and returns the newest one that is new to this Client and
    /// fresh at `now` Unix milliseconds. Facts that left the snapshot are forgotten, since a fact
    /// never comes back with the same time.
    fn observe(
        &mut self,
        snapshot: &Snapshot,
        now: u64,
    ) -> Option<(String, AgentNotResumedReason)> {
        let mut present = BTreeSet::new();
        let mut newest: Option<&PresentationAgentNotResumed> = None;
        for (presentation, fact) in facts(snapshot) {
            let key = (presentation.clone(), fact.at);
            if !self.seen.contains(&key)
                && now.abs_diff(fact.at) <= FRESH_MILLISECONDS
                && newest.is_none_or(|newest| fact.at > newest.at)
            {
                newest = Some(fact);
            }
            present.insert(key);
        }
        self.seen = present;
        newest.map(|fact| (fact.provider_id.as_str().to_owned(), fact.reason))
    }
}

/// Every Presentation's fact in `snapshot`.
fn facts(
    snapshot: &Snapshot,
) -> impl Iterator<Item = (&TerminalPresentationId, &PresentationAgentNotResumed)> {
    fn walk<'a>(
        node: &'a PanelNode,
        out: &mut Vec<(&'a TerminalPresentationId, &'a PresentationAgentNotResumed)>,
    ) {
        match node {
            PanelNode::Panel {
                primary_presentation,
                child_presentations,
                ..
            } => out.extend(
                std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .filter_map(|presentation| {
                        Some((
                            &presentation.presentation_id,
                            presentation.agent_not_resumed.as_deref()?,
                        ))
                    }),
            ),
            PanelNode::Split { children, .. } => {
                for child in children.iter() {
                    walk(&child.node, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    for tab in snapshot
        .terminal_contexts
        .iter()
        .flat_map(|context| &context.tabs)
    {
        walk(&tab.root, &mut out);
    }
    out.into_iter()
}

impl WorkbenchClientUi {
    /// Shows the newest fresh resume fact `snapshot` brings, unless the same notice is showing.
    pub(super) fn observe_agent_not_resumed(&mut self, snapshot: &Snapshot) {
        let Some(now) = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        else {
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
