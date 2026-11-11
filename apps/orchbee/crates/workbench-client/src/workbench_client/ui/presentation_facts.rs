//! The rule the Server's one-shot Presentation facts are shown by: a resume that typed nothing and
//! an Agent start that did not happen. Each fact shows once, keyed by its Presentation and time,
//! and only while it is news: fresh by this Client's clock, and not found before a time the
//! notice names. A later snapshot that still carries a fact never shows it again. The GUI shows
//! the same facts the same way.

use orchbee_workbench_protocol::{PanelNode, Presentation, Snapshot, TerminalPresentationId};
use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

/// How far a fact's time may lie from this Client's clock and still read as news.
const FRESH_MILLISECONDS: u64 = 60_000;

/// The facts of one kind this Client already decided on, by Presentation and time.
#[derive(Default)]
pub(super) struct FactNotices {
    seen: BTreeSet<(TerminalPresentationId, u64)>,
    /// A fact found before this Unix millisecond is old, however fresh.
    since: u64,
}

impl FactNotices {
    /// From now on a fact found before `since` Unix milliseconds is old.
    pub(super) const fn hold_back_before(&mut self, since: u64) {
        self.since = since;
    }

    /// Records the fact `read` finds on each of `snapshot`'s Presentations, with its time, and
    /// returns the newest one that is new to this Client and news at `now` Unix milliseconds.
    /// Facts that left the snapshot are forgotten, since a fact never comes back with the same
    /// time.
    pub(super) fn observe<'a, F: ?Sized>(
        &mut self,
        snapshot: &'a Snapshot,
        now: u64,
        read: impl Fn(&'a Presentation) -> Option<(&'a F, u64)>,
    ) -> Option<&'a F> {
        let mut present = BTreeSet::new();
        let mut newest: Option<(&F, u64)> = None;
        for presentation in presentations(snapshot) {
            let Some((fact, at)) = read(presentation) else {
                continue;
            };
            let key = (presentation.presentation_id.clone(), at);
            if !self.seen.contains(&key)
                && at >= self.since
                && now.abs_diff(at) <= FRESH_MILLISECONDS
                && newest.is_none_or(|(_, newest)| at > newest)
            {
                newest = Some((fact, at));
            }
            present.insert(key);
        }
        self.seen = present;
        newest.map(|(fact, _)| fact)
    }
}

/// This Client's clock in Unix milliseconds, the scale a fact's time uses.
pub(super) fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
}

/// Every Presentation in `snapshot`, primary and child.
fn presentations(snapshot: &Snapshot) -> Vec<&Presentation> {
    fn walk<'a>(node: &'a PanelNode, out: &mut Vec<&'a Presentation>) {
        match node {
            PanelNode::Panel {
                primary_presentation,
                child_presentations,
                ..
            } => out.extend(std::iter::once(primary_presentation).chain(child_presentations)),
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
    out
}
