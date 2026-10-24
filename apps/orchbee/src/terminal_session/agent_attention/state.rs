//! Client-local acknowledgement over admitted canonical observations.
use std::collections::{BTreeMap, BTreeSet};

use orchbee_agent_observation_protocol::{
    AgentAttachmentGeneration, AgentChangeSession, AgentPresentationId, AgentSafeInteger,
    AgentSessionId, AgentSessionSummary,
};

use super::super::agent_read_lane::AttentionRound;

const MAXIMUM_SESSIONS: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Entry {
    pub(super) summary: AgentSessionSummary,
    pub(super) version: u64,
    acknowledged: Option<u64>,
    edited_completion: Option<AgentSafeInteger>,
}

impl Entry {
    pub(super) fn seen(&self) -> bool {
        self.acknowledged == Some(self.version)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PaintVersion {
    session: AgentSessionId,
    generation: AgentSafeInteger,
    version: u64,
    presentation: AgentPresentationId,
    attachment_generation: AgentAttachmentGeneration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct AttentionPaint {
    serial: u64,
    versions: Vec<PaintVersion>,
}

#[derive(Default)]
pub(super) struct AttentionState {
    serial: u64,
    revision: Option<u64>,
    readable: bool,
    entries: BTreeMap<AgentSessionId, Entry>,
}

impl AttentionState {
    pub(super) fn edit_completed(&mut self, presentation: &str) -> bool {
        if !self.readable {
            return false;
        }
        let mut changed = false;
        for entry in self.entries.values_mut() {
            if matches!(
                entry.summary.status,
                orchbee_agent_observation_protocol::AgentSemanticStatus::Succeeded {}
            ) && entry.summary.lifecycle
                == orchbee_agent_observation_protocol::AgentLifecycle::Live
                && entry
                    .summary
                    .presentation_attachments
                    .iter()
                    .any(|attachment| {
                        attachment.navigable && attachment.presentation_id.as_str() == presentation
                    })
                && entry.edited_completion != Some(entry.summary.observation.sequence)
            {
                entry.edited_completion = Some(entry.summary.observation.sequence);
                changed = true;
            }
        }
        changed
    }

    #[cfg(any(windows, test))]
    pub(super) fn completion_edited(
        &self,
        session: &AgentSessionId,
        generation: AgentSafeInteger,
        sequence: AgentSafeInteger,
    ) -> bool {
        self.readable
            && self.entries.get(session).is_some_and(|entry| {
                entry.summary.observation.generation == generation
                    && entry.summary.observation.sequence == sequence
                    && entry.edited_completion == Some(sequence)
            })
    }

    pub(super) fn focus_matches(
        &self,
        session: &AgentSessionId,
        presentation: &str,
    ) -> Option<bool> {
        self.readable.then(|| {
            self.entries.get(session).is_some_and(|entry| {
                entry.summary.lifecycle == orchbee_agent_observation_protocol::AgentLifecycle::Live
                    && entry
                        .summary
                        .presentation_attachments
                        .iter()
                        .any(|attachment| {
                            attachment.navigable
                                && attachment.presentation_id.as_str() == presentation
                        })
            })
        })
    }
    pub(super) fn snapshot(
        &self,
        cursor: orchbee_agent_observation_protocol::AgentChangesCursor,
        outer_focused: Option<bool>,
        sessions: &[AgentSessionId],
    ) -> Option<orchbee_workbench_protocol::ClientAgentAttentionSnapshot> {
        use orchbee_workbench_protocol::{
            ClientAgentAttentionEntry, ClientAgentAttentionSnapshot, ClientAgentAttentionVersion,
        };
        if !self.readable {
            return None;
        }
        let snapshot = ClientAgentAttentionSnapshot {
            cursor,
            revision: AgentSafeInteger::new(self.revision?),
            outer_focused,
            entries: sessions
                .iter()
                .map(|session_id| ClientAgentAttentionEntry {
                    session_id: session_id.clone(),
                    state: self
                        .entries
                        .get(session_id)
                        .map(|entry| ClientAgentAttentionVersion {
                            generation: entry.summary.observation.generation,
                            sequence: entry.summary.observation.sequence,
                            state_change_revision: AgentSafeInteger::new(entry.version),
                            seen: entry.seen(),
                        }),
                })
                .collect(),
        };
        snapshot.validate().ok()?;
        Some(snapshot)
    }

    pub(super) fn unavailable(&mut self) -> bool {
        std::mem::replace(&mut self.readable, false)
    }

    /// Validate and build a candidate before replacing the displayed round.
    /// A failed round cannot partially advance an acknowledgement version.
    pub(super) fn publish(&mut self, round: &AttentionRound) -> Result<bool, ()> {
        if round.items.len() > MAXIMUM_SESSIONS || round.changes.len() > MAXIMUM_SESSIONS {
            return Err(());
        }
        let first = self.revision.is_none();
        let baseline = first || round.reset;
        if !baseline && round.prior_revision != self.revision {
            return Err(());
        }
        let summaries: BTreeMap<_, _> = round
            .items
            .iter()
            .map(|row| (row.session_id.clone(), row))
            .collect();
        if summaries.len() != round.items.len() {
            return Err(());
        }
        let mut entries = if baseline {
            BTreeMap::new()
        } else {
            self.entries.clone()
        };
        if baseline {
            for row in &round.items {
                entries.insert(
                    row.session_id.clone(),
                    Entry {
                        summary: row.clone(),
                        version: round.revision,
                        acknowledged: (first && !round.reset).then_some(round.revision),
                        edited_completion: None,
                    },
                );
            }
        } else {
            let mut revision = self.revision.ok_or(())?;
            for change in &round.changes {
                revision = revision.checked_add(1).ok_or(())?;
                if change.revision.value() != revision {
                    return Err(());
                }
                let row = summaries.get(&change.session.session_id).ok_or(())?;
                let entry = entries
                    .entry(change.session.session_id.clone())
                    .or_insert_with(|| Entry {
                        summary: (*row).clone(),
                        version: revision,
                        acknowledged: None,
                        edited_completion: None,
                    });
                if meaningful_change(&entry.summary, &change.session) {
                    entry.version = revision;
                    entry.edited_completion = None;
                }
                apply_change(&mut entry.summary, &change.session);
            }
            if revision != round.revision {
                return Err(());
            }
            // A metadata snapshot cannot smuggle a state transition past the
            // journal. It must exactly agree with the last observed record.
            for row in &round.items {
                let entry = entries.get_mut(&row.session_id).ok_or(())?;
                if projection(&entry.summary) != projection(row) {
                    return Err(());
                }
                entry.summary = row.clone();
            }
            entries.retain(|id, _| summaries.contains_key(id));
        }
        let changed =
            !self.readable || self.revision != Some(round.revision) || self.entries != entries;
        if changed {
            self.serial = self.serial.checked_add(1).ok_or(())?;
        }
        self.entries = entries;
        self.revision = Some(round.revision);
        self.readable = true;
        Ok(changed)
    }

    pub(super) fn session_unread(&self, session: &AgentSessionId) -> bool {
        self.readable && self.entries.get(session).is_some_and(|entry| !entry.seen())
    }
    pub(super) fn unseen_in(&self, presentation: &str) -> usize {
        if !self.readable {
            return 0;
        }
        self.entries
            .values()
            .filter(|entry| {
                !entry.seen()
                    && entry
                        .summary
                        .presentation_attachments
                        .iter()
                        .any(|attachment| {
                            attachment.navigable
                                && attachment.presentation_id.as_str() == presentation
                        })
            })
            .count()
    }

    pub(super) fn paint(&self, presentations: &BTreeSet<&str>) -> Option<AttentionPaint> {
        if !self.readable {
            return None;
        }
        let versions = self
            .entries
            .values()
            .filter(|entry| !entry.seen())
            .filter_map(|entry| {
                let attachment =
                    entry
                        .summary
                        .presentation_attachments
                        .iter()
                        .find(|attachment| {
                            attachment.navigable
                                && presentations.contains(attachment.presentation_id.as_str())
                        })?;
                Some(PaintVersion {
                    session: entry.summary.session_id.clone(),
                    generation: entry.summary.observation.generation,
                    version: entry.version,
                    presentation: attachment.presentation_id.clone(),
                    attachment_generation: attachment.generation.clone(),
                })
            })
            .collect();
        Some(AttentionPaint {
            serial: self.serial,
            versions,
        })
    }

    pub(super) fn acknowledge(
        &mut self,
        paint: &AttentionPaint,
        outer_focused: Option<bool>,
        mut still_visible: impl FnMut(&str) -> bool,
    ) -> bool {
        if !self.readable || outer_focused == Some(false) || paint.serial != self.serial {
            return false;
        }
        let mut changed = false;
        for proof in &paint.versions {
            let Some(entry) = self.entries.get_mut(&proof.session) else {
                continue;
            };
            if entry.version != proof.version
                || entry.summary.observation.generation != proof.generation
                || !still_visible(proof.presentation.as_str())
                || !entry
                    .summary
                    .presentation_attachments
                    .iter()
                    .any(|attachment| {
                        attachment.navigable
                            && attachment.presentation_id == proof.presentation
                            && attachment.generation == proof.attachment_generation
                    })
            {
                continue;
            }
            changed |= !entry.seen();
            entry.acknowledged = Some(entry.version);
        }
        changed
    }
}

fn meaningful_change(previous: &AgentSessionSummary, next: &AgentChangeSession) -> bool {
    previous.status != next.status
        || previous.lifecycle != next.lifecycle
        || previous.observation.generation != next.observation.generation
        || previous.observation.freshness != next.observation.freshness
}

fn apply_change(summary: &mut AgentSessionSummary, change: &AgentChangeSession) {
    summary.lifecycle = change.lifecycle;
    summary.observation = change.observation.clone();
    summary.parent_session_id = change.parent_session_id.clone();
    summary.pending_request_count = change.pending_request_count;
    summary.provider_id = change.provider_id.clone();
    summary.status = change.status.clone();
}

fn projection(summary: &AgentSessionSummary) -> AgentChangeSession {
    AgentChangeSession {
        lifecycle: summary.lifecycle,
        observation: summary.observation.clone(),
        parent_session_id: summary.parent_session_id.clone(),
        pending_request_count: summary.pending_request_count,
        provider_id: summary.provider_id.clone(),
        session_id: summary.session_id.clone(),
        status: summary.status.clone(),
    }
}

#[cfg(test)]
mod tests;
