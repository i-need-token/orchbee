//! Client-owned acknowledgement of complete, creation-qualified physical state checkpoints.
use std::collections::BTreeMap;

use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    ClientTerminalAgentAttentionEntry, ClientTerminalAgentAttentionSnapshot,
    ClientTerminalAgentAttentionVersion, TerminalAgentStateSnapshot, WorkbenchRevision,
};

pub(in crate::terminal_session) const MAXIMUM_ENTRIES: usize = 4096;
pub(in crate::terminal_session) const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct PhysicalRound {
    pub(in crate::terminal_session) report_revision: WorkbenchRevision,
    pub(in crate::terminal_session) states: Vec<TerminalAgentStateSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    current: TerminalAgentStateSnapshot,
    seen: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PhysicalPaint {
    serial: u64,
    versions: Vec<(AgentLaunchBinding, WorkbenchRevision)>,
}

#[derive(Default)]
pub(super) struct PhysicalAttentionState {
    revision: Option<WorkbenchRevision>,
    serial: u64,
    readable: bool,
    entries: BTreeMap<String, Entry>,
}

impl PhysicalAttentionState {
    pub(super) fn view_version(&self) -> Option<(u64, usize)> {
        self.readable.then(|| {
            (
                self.serial,
                self.entries.values().filter(|e| e.seen).count(),
            )
        })
    }

    pub(super) fn focus_matches(&self, binding: &AgentLaunchBinding) -> Option<bool> {
        self.readable.then(|| {
            self.entries
                .get(binding.presentation_id.as_str())
                .is_some_and(|entry| {
                    entry.current.binding == *binding && entry.current.agent.is_some()
                })
        })
    }
    pub(super) fn unavailable(&mut self) -> bool {
        std::mem::replace(&mut self.readable, false)
    }

    pub(super) fn publish(&mut self, round: &PhysicalRound) -> Result<bool, ()> {
        if round.states.len() > MAXIMUM_ENTRIES
            || self.revision.is_some_and(|old| old > round.report_revision)
            || serde_json::to_vec(&round.states).map_err(|_| ())?.len() > MAXIMUM_BYTES
        {
            return Err(());
        }
        let first = self.revision.is_none();
        let mut entries = BTreeMap::new();
        for current in &round.states {
            let id = current.binding.presentation_id.as_str();
            let previous = self
                .entries
                .get(id)
                .filter(|old| old.current.binding == current.binding);
            if let Some(old) = previous
                && (current.sequence < old.current.sequence
                    || (current.sequence == old.current.sequence && old.current != *current))
            {
                return Err(());
            }
            let seen = first
                || previous.is_some_and(|old| old.seen && old.current.sequence == current.sequence);
            if entries
                .insert(
                    id.to_owned(),
                    Entry {
                        current: current.clone(),
                        seen,
                    },
                )
                .is_some()
            {
                return Err(());
            }
        }
        let changed = !self.readable
            || self.revision != Some(round.report_revision)
            || entries != self.entries;
        let serial = if changed {
            self.serial.checked_add(1).ok_or(())?
        } else {
            self.serial
        };
        self.entries = entries;
        self.serial = serial;
        self.revision = Some(round.report_revision);
        self.readable = true;
        Ok(changed)
    }

    pub(super) fn snapshot(
        &self,
        bindings: &[AgentLaunchBinding],
        outer_focused: Option<bool>,
    ) -> Option<ClientTerminalAgentAttentionSnapshot> {
        if !self.readable {
            return None;
        }
        let snapshot = ClientTerminalAgentAttentionSnapshot {
            report_revision: self.revision?,
            outer_focused,
            entries: bindings
                .iter()
                .map(|binding| ClientTerminalAgentAttentionEntry {
                    binding: binding.clone(),
                    state: self
                        .entries
                        .get(binding.presentation_id.as_str())
                        .filter(|entry| entry.current.binding == *binding)
                        .map(|entry| ClientTerminalAgentAttentionVersion {
                            sequence: entry.current.sequence,
                            seen: entry.seen,
                        }),
                })
                .collect(),
        };
        snapshot.validate().ok()?;
        Some(snapshot)
    }

    pub(super) fn unseen_in(&self, presentation: &str) -> usize {
        usize::from(
            self.readable
                && self
                    .entries
                    .get(presentation)
                    .is_some_and(|entry| entry.current.agent.is_some() && !entry.seen),
        )
    }

    pub(super) fn paint(
        &self,
        mut visible: impl FnMut(&AgentLaunchBinding) -> bool,
    ) -> Option<PhysicalPaint> {
        self.readable.then(|| PhysicalPaint {
            serial: self.serial,
            versions: self
                .entries
                .values()
                .filter(|entry| !entry.seen && visible(&entry.current.binding))
                .map(|entry| (entry.current.binding.clone(), entry.current.sequence))
                .collect(),
        })
    }

    pub(super) fn acknowledge(
        &mut self,
        paint: &PhysicalPaint,
        focused: Option<bool>,
        mut visible: impl FnMut(&AgentLaunchBinding) -> bool,
    ) -> bool {
        if !self.readable || focused == Some(false) || self.serial != paint.serial {
            return false;
        }
        let mut changed = false;
        for (binding, sequence) in &paint.versions {
            if let Some(entry) = self.entries.get_mut(binding.presentation_id.as_str())
                && entry.current.binding == *binding
                && entry.current.sequence == *sequence
                && visible(binding)
            {
                changed |= !entry.seen;
                entry.seen = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests;
