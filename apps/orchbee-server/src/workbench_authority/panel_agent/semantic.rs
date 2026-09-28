//! Prepared, bounded semantic history belongs to the physical report resource.
use super::{Failure, Reports, Resource, TerminalAgentBinding, retained_bytes};
use orchbee_workbench_protocol::{
    MAXIMUM_TERMINAL_AGENT_CHANGES, PanelAgentView, TerminalAgentChange, WorkbenchRevision,
};
use std::collections::VecDeque;

const MAXIMUM_HISTORY_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Default)]
pub(super) struct SemanticHistory {
    sequence: u64,
    floor: u64,
    changes: VecDeque<(TerminalAgentChange, usize)>,
    bytes: usize,
}
impl SemanticHistory {
    fn advance(&mut self, before: &PanelAgentView, after: &PanelAgentView) -> Result<(), Failure> {
        if before.agent == after.agent && before.state == after.state {
            return Ok(());
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        let change = TerminalAgentChange {
            sequence: WorkbenchRevision::new(sequence),
            agent: after.agent.clone(),
            state: after.state,
        };
        let bytes = serde_json::to_vec(&change)
            .map_err(|_| Failure::ResourceLimit)?
            .len();
        if bytes > MAXIMUM_HISTORY_BYTES {
            return Err(Failure::ResourceLimit);
        }
        self.changes.push_back((change, bytes));
        self.bytes += bytes;
        self.sequence = sequence;
        while self.changes.len() > MAXIMUM_TERMINAL_AGENT_CHANGES
            || self.bytes > MAXIMUM_HISTORY_BYTES
        {
            if let Some((old, bytes)) = self.changes.pop_front() {
                self.floor = old.sequence.value();
                self.bytes -= bytes;
            }
        }
        Ok(())
    }
    pub(super) fn sequence(&self) -> WorkbenchRevision {
        WorkbenchRevision::new(self.sequence)
    }
    pub(super) fn since(
        &self,
        after: Option<WorkbenchRevision>,
    ) -> Result<Vec<TerminalAgentChange>, Failure> {
        let Some(after) = after else {
            return Ok(vec![]);
        };
        if after.value() > self.sequence {
            return Err(Failure::InvalidRequest);
        }
        if after.value() < self.floor {
            return Err(Failure::HistoryLimit);
        }
        Ok(self
            .changes
            .iter()
            .filter(|(c, _)| c.sequence.value() > after.value())
            .map(|(c, _)| c.clone())
            .collect())
    }
}
impl Resource {
    pub(super) fn prepare(
        physical: TerminalAgentBinding,
        reports: Reports,
        explicit_revision: u64,
        prior: Option<&Self>,
    ) -> Result<Self, Failure> {
        let prior = prior.filter(|r| r.physical == physical);
        let mut semantic = prior.map_or_else(SemanticHistory::default, |r| r.semantic.clone());
        let before = prior.map_or_else(PanelAgentView::default, |r| r.reports.view());
        semantic.advance(&before, &reports.view())?;
        let bytes = retained_bytes(&reports)? + semantic.bytes;
        Ok(Self {
            physical,
            reports,
            explicit_revision,
            semantic,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests;
