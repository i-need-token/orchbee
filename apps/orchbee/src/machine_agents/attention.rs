//! Endpoint-qualified Client acknowledgements; complete reads never mark changed versions seen.
use super::{
    AgentLaunchBinding, Arc, Checkpoint, Choice, ClientTerminalAgentAttentionSnapshot,
    MAXIMUM_ROWS, PanelAgentState, SiteId, TerminalPresentationId, WorkbenchRevision,
};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Eq, PartialEq)]
struct Version {
    binding: AgentLaunchBinding,
    sequence: WorkbenchRevision,
    agent: String,
    state: PanelAgentState,
    seen: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Endpoint {
    choice: Choice,
    site: SiteId,
    report: WorkbenchRevision,
    versions: BTreeMap<TerminalPresentationId, Version>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Attention {
    endpoints: Vec<Endpoint>,
}
impl Attention {
    pub(crate) fn retain(&mut self, mut exists: impl FnMut(&Choice) -> bool) {
        self.endpoints.retain(|e| exists(&e.choice));
    }
    pub(crate) fn project(
        &mut self,
        choice: &Choice,
        round: &Arc<Checkpoint>,
    ) -> Result<Arc<Checkpoint>, ()> {
        let previous = self
            .endpoints
            .iter()
            .find(|e| &e.choice == choice && e.site == round.site_id);
        if previous.is_some_and(|e| e.report > round.report_revision)
            || round.rows.len() > MAXIMUM_ROWS
        {
            return Err(());
        }
        let mut versions = BTreeMap::new();
        let mut rows = Vec::with_capacity(round.rows.len());
        for row in round.rows.iter() {
            if row.target.physical.presentation_id.as_str() != row.target.member.as_str()
                || row.target.context.site_id != round.site_id
            {
                return Err(());
            }
            let old = previous
                .and_then(|e| e.versions.get(&row.target.member))
                .filter(|v| v.binding == row.target.physical);
            if old.is_some_and(|v| {
                v.sequence > row.sequence
                    || (v.sequence == row.sequence
                        && (v.agent != row.target.agent || v.state != row.state))
            }) {
                return Err(());
            }
            let seen =
                previous.is_none() || old.is_some_and(|v| v.sequence == row.sequence && v.seen);
            if versions
                .insert(
                    row.target.member.clone(),
                    Version {
                        binding: row.target.physical.clone(),
                        sequence: row.sequence,
                        agent: row.target.agent.clone(),
                        state: row.state,
                        seen,
                    },
                )
                .is_some()
            {
                return Err(());
            }
            let mut projected = row.clone();
            projected.seen = seen;
            rows.push(projected);
        }
        let next = Endpoint {
            choice: choice.clone(),
            site: round.site_id.clone(),
            report: round.report_revision,
            versions,
        };
        if let Some(index) = self.endpoints.iter().position(|e| &e.choice == choice) {
            self.endpoints[index] = next;
        } else {
            if self.endpoints.len() >= 65 {
                return Err(());
            }
            self.endpoints.push(next);
        }
        if rows.as_slice() == round.rows.as_ref() {
            Ok(round.clone())
        } else {
            Ok(Arc::new(Checkpoint {
                rows: rows.into(),
                ..round.as_ref().clone()
            }))
        }
    }
    pub(crate) fn acknowledge(
        &mut self,
        choice: &Choice,
        snapshot: &ClientTerminalAgentAttentionSnapshot,
    ) -> bool {
        if snapshot.validate().is_err() {
            return false;
        }
        let Some(endpoint) = self.endpoints.iter_mut().find(|e| &e.choice == choice) else {
            return false;
        };
        if snapshot.report_revision < endpoint.report {
            return false;
        }
        let mut changed = false;
        for entry in &snapshot.entries {
            let Some(state) = &entry.state else {
                continue;
            };
            if let Some(version) = endpoint
                .versions
                .values_mut()
                .find(|v| v.binding == entry.binding && v.sequence == state.sequence)
            {
                changed |= version.seen != state.seen;
                version.seen = state.seen;
            }
        }
        changed
    }
}
#[cfg(test)]
mod tests;
