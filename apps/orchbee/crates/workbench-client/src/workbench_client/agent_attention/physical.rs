//! Client-owned acknowledgement of complete, creation-qualified physical state checkpoints.
use std::collections::BTreeMap;

use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    ClientTerminalAgentAttentionEntry, ClientTerminalAgentAttentionSnapshot,
    ClientTerminalAgentAttentionVersion, PanelAgentState, TerminalAgentStateSnapshot,
    WorkbenchRevision,
};

pub(in crate::workbench_client) use crate::workbench_client::agent_read_lane::physical_attention::{
    MAXIMUM_PHYSICAL_BYTES as MAXIMUM_BYTES, MAXIMUM_PHYSICAL_ENTRIES as MAXIMUM_ENTRIES,
    PhysicalRound,
};

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

#[derive(Clone, Default)]
pub(super) struct PhysicalAttentionState {
    revision: Option<WorkbenchRevision>,
    serial: u64,
    readable: bool,
    entries: BTreeMap<String, Entry>,
}

impl PhysicalAttentionState {
    pub(super) fn current(
        &self,
        binding: &AgentLaunchBinding,
        sequence: WorkbenchRevision,
    ) -> bool {
        self.readable
            && self
                .entries
                .get(binding.presentation_id.as_str())
                .is_some_and(|entry| {
                    entry.current.binding == *binding && entry.current.sequence == sequence
                })
    }

    pub(super) fn agent_in_terminal(
        &self,
        presentation: &str,
        epoch: &str,
        terminal: &str,
    ) -> bool {
        self.readable
            && self.entries.get(presentation).is_some_and(|entry| {
                entry.current.agent.is_some()
                    && entry.current.binding.host_epoch.as_str() == epoch
                    && entry.current.binding.host_terminal_id.as_str() == terminal
            })
    }
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
                    || (current.sequence == old.current.sequence
                        && (old.current.agent != current.agent
                            || old.current.state != current.state))
                    || (self.revision == Some(round.report_revision)
                        && old.current.completion != current.completion))
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

    pub(super) fn notification_observations(
        &self,
    ) -> Option<Vec<crate::workbench_client::agent_notifications::AgentNotificationObservation>>
    {
        use crate::workbench_client::agent_notifications::{
            AgentNotificationIdentity as Identity, AgentNotificationObservation as Observation,
            AgentNotificationState as State,
        };
        self.readable.then(|| {
            self.entries
                .values()
                .map(|entry| Observation {
                    identity: Identity {
                        presentation_id: entry.current.binding.presentation_id.as_str().to_owned(),
                        generation: entry.current.binding.generation.as_str().to_owned(),
                        host_epoch: entry.current.binding.host_epoch.as_str().to_owned(),
                        host_terminal_id: entry
                            .current
                            .binding
                            .host_terminal_id
                            .as_str()
                            .to_owned(),
                    },
                    sequence: entry.current.sequence.value(),
                    agent: entry.current.agent.clone(),
                    state: match entry.current.state {
                        PanelAgentState::Idle => State::Idle,
                        PanelAgentState::Done => State::Done,
                        PanelAgentState::Error => State::Error,
                        PanelAgentState::Working => State::Working,
                        PanelAgentState::Blocked => State::Blocked,
                        // An opening Agent has no turn yet, so it raises no notification.
                        PanelAgentState::Unknown | PanelAgentState::Opening => State::Unknown,
                    },
                    seen: entry.seen,
                })
                .collect()
        })
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

    #[cfg(test)]
    pub(super) fn acknowledge(
        &mut self,
        paint: &PhysicalPaint,
        focused: Option<bool>,
        mut visible: impl FnMut(&AgentLaunchBinding) -> bool,
    ) -> bool {
        let versions = self.painted_versions(paint, &mut visible);
        !self
            .acknowledge_painted_versions(&versions, focused)
            .is_empty()
    }

    pub(super) fn painted_versions(
        &self,
        paint: &PhysicalPaint,
        mut visible: impl FnMut(&AgentLaunchBinding) -> bool,
    ) -> Vec<(AgentLaunchBinding, WorkbenchRevision)> {
        if !self.readable || self.serial != paint.serial {
            return Vec::new();
        }
        let mut painted = Vec::new();
        for (binding, sequence) in &paint.versions {
            if self
                .entries
                .get(binding.presentation_id.as_str())
                .is_some_and(|entry| {
                    entry.current.binding == *binding
                        && entry.current.sequence == *sequence
                        && visible(binding)
                })
            {
                painted.push((binding.clone(), *sequence));
            }
        }
        painted
    }

    pub(super) fn acknowledge_painted_versions(
        &mut self,
        versions: &[(AgentLaunchBinding, WorkbenchRevision)],
        focused: Option<bool>,
    ) -> Vec<(AgentLaunchBinding, WorkbenchRevision)> {
        if focused == Some(false) {
            return Vec::new();
        }
        let mut acknowledged = Vec::new();
        for (binding, sequence) in versions {
            if let Some(entry) = self.entries.get_mut(binding.presentation_id.as_str())
                && entry.current.binding == *binding
                && entry.current.sequence == *sequence
            {
                if !entry.seen {
                    acknowledged.push((binding.clone(), *sequence));
                }
                entry.seen = true;
            }
        }
        acknowledged
    }
}

#[cfg(test)]
mod tests;
