//! `#[cfg(test)]` pure port of the Desktop `AgentObservationJoinStateMachine`
//! (427 lines; fingerprint recorded in the join fixture). Maps preserve the
//! TypeScript `Map` insertion-order iteration contract — cleanup and attachment
//! emission order are observable in the recorded command arrays.
//!
//! Deliberate non-ports: unknown-generation hints drop (`currentGenerations`
//! has no tombstones — `None` means never-registered or retired alike), no
//! held-hint queue, and one accepted native delta — the joint admission bound
//! `pending + recent <= MAX_JOINT_ADMISSIONS` alongside the independent
//! pending/hint caps.

use super::AgentPresentationVisibility;
use super::hook_contract::{AdapterDefinition, HookEnvelope};
use super::model::RawAttachment;

const DEFAULT_JOIN_SETTLE_MILLISECONDS: f64 = 25.0;
const DEFAULT_JOIN_TTL_MILLISECONDS: f64 = 10_000.0;
const DEFAULT_MAX_PENDING_EVENTS: usize = 512;
const DEFAULT_MAX_PENDING_HINTS: usize = 1024;
const DEFAULT_RECENT_EVENT_TTL_MILLISECONDS: f64 = 60_000.0;

/// Native-only bounded-resource delta: `pending_events + recent_events <=
/// 4096`. A new key charges one joint slot; an applied expiry transfers its
/// slot to recent; unapplied and recent expiries release it; identical replay
/// of a pending key charges nothing.
pub const MAX_JOINT_ADMISSIONS: usize = 4096;

/// Insertion-ordered string-keyed map — the TypeScript `Map` contract this
/// port depends on (`set` updates in place, `delete` keeps relative order,
/// iteration is insertion order). Linear scans are acceptable: every consumer
/// is bounded (<= 16384 entries) and the port is test-compiled only.
#[derive(Clone, Debug)]
pub struct SeqMap<V> {
    entries: Vec<(String, V)>,
}

impl<V> SeqMap<V> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries
            .iter_mut()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value)
    }

    /// `Map.set` — existing keys keep their position and update in place.
    pub fn set(&mut self, key: String, value: V) {
        if let Some((_, slot)) = self
            .entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            *slot = value;
            return;
        }
        self.entries.push((key, value));
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        let index = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        Some(self.entries.remove(index).1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(candidate, _)| candidate == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut V)> {
        self.entries.iter_mut().map(|(key, value)| (&*key, value))
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct JoinOptions {
    pub join_settle_milliseconds: f64,
    pub join_ttl_milliseconds: f64,
    pub max_pending_events: usize,
    pub max_pending_hints: usize,
    pub recent_event_ttl_milliseconds: f64,
}

impl Default for JoinOptions {
    fn default() -> Self {
        Self {
            join_settle_milliseconds: DEFAULT_JOIN_SETTLE_MILLISECONDS,
            join_ttl_milliseconds: DEFAULT_JOIN_TTL_MILLISECONDS,
            max_pending_events: DEFAULT_MAX_PENDING_EVENTS,
            max_pending_hints: DEFAULT_MAX_PENDING_HINTS,
            recent_event_ttl_milliseconds: DEFAULT_RECENT_EVENT_TTL_MILLISECONDS,
        }
    }
}

/// `AgentObservationJoinHint` after the consumer stamps channel identity:
/// `generation` is the signal's `creationId`, `presentationId` the channel's.
#[derive(Clone, Debug, PartialEq)]
pub struct JoinHint {
    pub correlation_id: String,
    pub event_id: String,
    pub generation: String,
    pub presentation_id: String,
}

#[derive(Clone, Debug)]
struct PendingHint {
    event_id: String,
    expires_at: f64,
    generation: String,
    presentation_id: String,
}

#[derive(Clone, Debug)]
struct PendingEvent {
    adapter: AdapterDefinition,
    applied: bool,
    envelope: HookEnvelope,
    expires_at: f64,
    matched_attachments: SeqMap<RawAttachment>,
    matched_correlation_ids: Vec<String>,
    matched_presentation_generations: SeqMap<String>,
    settlement_scheduled: bool,
}

/// `AgentObservationJoinCommand` — outward effects only; the consumer owns
/// scheduling, canonical dispatch, and presentation detaches.
#[derive(Clone, Debug)]
pub enum JoinCommand {
    CancelSettlement {
        event_expires_at: f64,
        event_key: String,
    },
    ApplyObservation {
        adapter: AdapterDefinition,
        attachments: Vec<RawAttachment>,
        envelope: HookEnvelope,
    },
    AttachPresentations {
        adapter_id: String,
        attachments: Vec<RawAttachment>,
        session_key: String,
    },
    ScheduleSettlement {
        event_expires_at: f64,
        event_key: String,
        milliseconds: f64,
    },
    RetirePresentationGeneration {
        generation: String,
        presentation_id: String,
    },
}

#[derive(Debug)]
pub struct JoinEventAdmission {
    pub accepted: bool,
    pub commands: Vec<JoinCommand>,
}

fn event_key(adapter_id: &str, external_event_id: &str) -> String {
    format!("{adapter_id}\0{external_event_id}")
}

fn attachment_key(presentation_id: &str, generation: &str) -> String {
    format!("{presentation_id}\0{generation}")
}

fn combine_visibility(
    left: AgentPresentationVisibility,
    right: AgentPresentationVisibility,
) -> AgentPresentationVisibility {
    if left == AgentPresentationVisibility::Hidden || right == AgentPresentationVisibility::Hidden {
        return AgentPresentationVisibility::Hidden;
    }
    if left == AgentPresentationVisibility::Visible && right == AgentPresentationVisibility::Visible
    {
        return AgentPresentationVisibility::Visible;
    }
    AgentPresentationVisibility::Unknown
}

fn attachment_for(
    generation: &str,
    presentation_id: &str,
    visibility: AgentPresentationVisibility,
) -> RawAttachment {
    RawAttachment {
        generation: generation.to_string(),
        navigable: true,
        presentation_id: presentation_id.to_string(),
        visibility: visibility.as_str().to_string(),
    }
}

fn pending_hint_equal(pending: &PendingHint, hint: &JoinHint) -> bool {
    pending.event_id == hint.event_id
        && pending.presentation_id == hint.presentation_id
        && pending.generation == hint.generation
}

fn apply_observation_command(pending: &PendingEvent) -> JoinCommand {
    JoinCommand::ApplyObservation {
        adapter: pending.adapter.clone(),
        attachments: pending
            .matched_attachments
            .iter()
            .map(|(_, attachment)| attachment.clone())
            .collect(),
        envelope: pending.envelope.clone(),
    }
}

pub struct AgentObservationJoinStateMachine {
    correlation_event_ids: SeqMap<String>,
    current_generations: SeqMap<String>,
    options: JoinOptions,
    pending_events: SeqMap<PendingEvent>,
    pending_hints: SeqMap<PendingHint>,
    recent_event_expirations: SeqMap<f64>,
}

impl AgentObservationJoinStateMachine {
    pub fn new(options: JoinOptions) -> Self {
        Self {
            correlation_event_ids: SeqMap::new(),
            current_generations: SeqMap::new(),
            options,
            pending_events: SeqMap::new(),
            pending_hints: SeqMap::new(),
            recent_event_expirations: SeqMap::new(),
        }
    }

    /// `registerPresentationGeneration` — no clock: registration is not a
    /// cleanup point in the source.
    pub fn register_presentation_generation(
        &mut self,
        presentation_id: String,
        generation: String,
    ) -> Vec<JoinCommand> {
        let previous_generation = self.current_generations.get(&presentation_id).cloned();
        let mut commands = Vec::new();
        if let Some(previous) = previous_generation
            && previous != generation
        {
            self.retire_generation(&presentation_id, &previous);
            commands.push(JoinCommand::RetirePresentationGeneration {
                generation: previous,
                presentation_id: presentation_id.clone(),
            });
        }
        self.current_generations.set(presentation_id, generation);
        commands
    }

    /// `retirePresentation` — no clock.
    pub fn retire_presentation(&mut self, presentation_id: &str) -> Vec<JoinCommand> {
        let generation = self.current_generations.remove(presentation_id);
        let Some(generation) = generation else {
            return Vec::new();
        };
        self.retire_generation(presentation_id, &generation);
        vec![JoinCommand::RetirePresentationGeneration {
            generation,
            presentation_id: presentation_id.to_string(),
        }]
    }

    /// `receiveHint` — cleanup first, then the generation fence. `None`
    /// registration (never seen or already retired) and a mismatched
    /// generation both drop the hint exactly like the Desktop.
    pub fn receive_hint(&mut self, hint: JoinHint, now: f64) -> Vec<JoinCommand> {
        let mut commands = self.cleanup_expired(now);
        if self.current_generations.get(&hint.presentation_id) != Some(&hint.generation) {
            return commands;
        }
        let pending_hint = PendingHint {
            event_id: hint.event_id.clone(),
            expires_at: now + self.options.join_ttl_milliseconds,
            generation: hint.generation.clone(),
            presentation_id: hint.presentation_id.clone(),
        };
        if let Some(existing) = self.pending_hints.get(&hint.correlation_id) {
            // Equal duplicates keep the original TTL; conflicting reuse drops.
            if !pending_hint_equal(existing, &hint) {
                return commands;
            }
        } else {
            if self.pending_hints.len() >= self.options.max_pending_hints {
                return commands;
            }
            self.pending_hints
                .set(hint.correlation_id.clone(), pending_hint);
        }
        commands.extend(self.join_correlation(&hint.correlation_id));
        commands
    }

    /// `admitEvent` — replay checks run before any cap accounting.
    pub fn admit_event(
        &mut self,
        adapter: AdapterDefinition,
        envelope: HookEnvelope,
        now: f64,
    ) -> JoinEventAdmission {
        let mut commands = self.cleanup_expired(now);
        let transport_event_key = event_key(&envelope.adapter_id, &envelope.event_id);
        if self
            .recent_event_expirations
            .contains_key(&transport_event_key)
        {
            return JoinEventAdmission {
                accepted: false,
                commands,
            };
        }
        if self.pending_events.contains_key(&transport_event_key) {
            let identical = self
                .pending_events
                .get(&transport_event_key)
                .is_some_and(|existing| existing.envelope == envelope);
            if !identical {
                return JoinEventAdmission {
                    accepted: false,
                    commands,
                };
            }
            for correlation in &envelope.correlations {
                commands.extend(self.join_correlation(&correlation.correlation_id));
            }
            return JoinEventAdmission {
                accepted: true,
                commands,
            };
        }
        if self.pending_events.len() >= self.options.max_pending_events {
            return JoinEventAdmission {
                accepted: false,
                commands,
            };
        }
        // Native delta: the joint pending+recent occupancy bound.
        if self.pending_events.len() + self.recent_event_expirations.len() >= MAX_JOINT_ADMISSIONS {
            return JoinEventAdmission {
                accepted: false,
                commands,
            };
        }
        if envelope.correlations.iter().any(|correlation| {
            self.correlation_event_ids
                .get(&correlation.correlation_id)
                .is_some_and(|claimed| *claimed != transport_event_key)
        }) {
            return JoinEventAdmission {
                accepted: false,
                commands,
            };
        }
        let pending_event = PendingEvent {
            adapter,
            applied: false,
            envelope: envelope.clone(),
            expires_at: now + self.options.join_ttl_milliseconds,
            matched_attachments: SeqMap::new(),
            matched_correlation_ids: Vec::new(),
            matched_presentation_generations: SeqMap::new(),
            settlement_scheduled: false,
        };
        self.pending_events
            .set(transport_event_key.clone(), pending_event);
        for correlation in &envelope.correlations {
            self.correlation_event_ids.set(
                correlation.correlation_id.clone(),
                transport_event_key.clone(),
            );
            commands.extend(self.join_correlation(&correlation.correlation_id));
        }
        JoinEventAdmission {
            accepted: true,
            commands,
        }
    }

    /// `settle` — fires only while the event is still pending, the recorded
    /// expiry matches, and a settlement was actually scheduled. `applied` or
    /// zero-attachment events clear the flag and emit nothing.
    pub fn settle(
        &mut self,
        transport_event_key: &str,
        event_expires_at: f64,
        now: f64,
    ) -> Vec<JoinCommand> {
        let mut commands = self.cleanup_expired(now);
        let Some(pending_event) = self.pending_events.get_mut(transport_event_key) else {
            return commands;
        };
        if pending_event.expires_at != event_expires_at || !pending_event.settlement_scheduled {
            return commands;
        }
        pending_event.settlement_scheduled = false;
        if pending_event.applied || pending_event.matched_attachments.is_empty() {
            return commands;
        }
        pending_event.applied = true;
        commands.push(apply_observation_command(pending_event));
        commands
    }

    /// `reset` — clears all five maps; no clock.
    #[cfg(test)]
    pub fn reset(&mut self) {
        self.pending_events.clear();
        self.pending_hints.clear();
        self.correlation_event_ids.clear();
        self.recent_event_expirations.clear();
        self.current_generations.clear();
    }

    fn join_correlation(&mut self, correlation_id: &str) -> Vec<JoinCommand> {
        let Some(transport_event_key) = self.correlation_event_ids.get(correlation_id).cloned()
        else {
            return Vec::new();
        };
        let Some(hint) = self.pending_hints.get(correlation_id) else {
            return Vec::new();
        };
        let hint = hint.clone();
        let Some(pending_event) = self.pending_events.get_mut(&transport_event_key) else {
            return Vec::new();
        };
        if hint.event_id != pending_event.envelope.event_id {
            return Vec::new();
        }
        let Some(correlation) = pending_event
            .envelope
            .correlations
            .iter()
            .find(|candidate| candidate.correlation_id == correlation_id)
            .cloned()
        else {
            return Vec::new();
        };
        if pending_event
            .matched_correlation_ids
            .iter()
            .any(|candidate| candidate == correlation_id)
        {
            self.pending_hints.remove(correlation_id);
            return Vec::new();
        }
        if let Some(matched_generation) = pending_event
            .matched_presentation_generations
            .get(&hint.presentation_id)
            && *matched_generation != hint.generation
        {
            self.pending_hints.remove(correlation_id);
            return Vec::new();
        }

        pending_event
            .matched_correlation_ids
            .push(correlation_id.to_string());
        pending_event
            .matched_presentation_generations
            .set(hint.presentation_id.clone(), hint.generation.clone());
        let key = attachment_key(&hint.presentation_id, &hint.generation);
        let next_attachment = attachment_for(
            &hint.generation,
            &hint.presentation_id,
            correlation.visibility,
        );
        let attachment = match pending_event.matched_attachments.get(&key) {
            None => next_attachment,
            Some(matched) => RawAttachment {
                visibility: combine_visibility(
                    AgentPresentationVisibility::parse(&matched.visibility)
                        .unwrap_or(AgentPresentationVisibility::Unknown),
                    correlation.visibility,
                )
                .as_str()
                .to_string(),
                ..next_attachment
            },
        };
        pending_event
            .matched_attachments
            .set(key, attachment.clone());
        self.pending_hints.remove(correlation_id);

        if pending_event.applied {
            return vec![JoinCommand::AttachPresentations {
                adapter_id: pending_event.envelope.adapter_id.clone(),
                attachments: vec![attachment],
                session_key: pending_event.envelope.observation.session_key.clone(),
            }];
        }
        if pending_event.matched_correlation_ids.len() == pending_event.envelope.correlations.len()
        {
            return self.apply_pending_event(&transport_event_key);
        }
        if pending_event.settlement_scheduled {
            return Vec::new();
        }
        let pending_event = self
            .pending_events
            .get_mut(&transport_event_key)
            .expect("pending event survives self-lookup");
        pending_event.settlement_scheduled = true;
        vec![JoinCommand::ScheduleSettlement {
            event_expires_at: pending_event.expires_at,
            event_key: transport_event_key,
            milliseconds: self.options.join_settle_milliseconds,
        }]
    }

    fn apply_pending_event(&mut self, transport_event_key: &str) -> Vec<JoinCommand> {
        let pending_event = self
            .pending_events
            .get_mut(transport_event_key)
            .expect("apply target exists");
        if pending_event.applied {
            return Vec::new();
        }
        let mut commands = Vec::new();
        if pending_event.settlement_scheduled {
            pending_event.settlement_scheduled = false;
            commands.push(JoinCommand::CancelSettlement {
                event_expires_at: pending_event.expires_at,
                event_key: transport_event_key.to_string(),
            });
        }
        if pending_event.matched_attachments.is_empty() {
            return commands;
        }
        pending_event.applied = true;
        commands.push(apply_observation_command(pending_event));
        commands
    }

    fn retire_generation(&mut self, presentation_id: &str, generation: &str) {
        let doomed: Vec<String> = self
            .pending_hints
            .iter()
            .filter(|(_, hint)| {
                hint.presentation_id == presentation_id && hint.generation == generation
            })
            .map(|(correlation_id, _)| correlation_id.clone())
            .collect();
        for correlation_id in doomed {
            self.pending_hints.remove(&correlation_id);
        }
        let key = attachment_key(presentation_id, generation);
        for (_, pending_event) in self.pending_events.iter_mut() {
            pending_event.matched_attachments.remove(&key);
        }
    }

    /// `#cleanupExpired` — runs at the head of `receive_hint`, `admit_event`,
    /// and `settle` only. Emits `cancel-settlement` for events that expire
    /// while a settlement was scheduled; applied events migrate to `recent`
    /// (occupancy transfer, not a new charge).
    fn cleanup_expired(&mut self, now: f64) -> Vec<JoinCommand> {
        let mut commands = Vec::new();
        let expired_hints: Vec<String> = self
            .pending_hints
            .iter()
            .filter(|(_, hint)| hint.expires_at <= now)
            .map(|(correlation_id, _)| correlation_id.clone())
            .collect();
        for correlation_id in expired_hints {
            self.pending_hints.remove(&correlation_id);
        }
        let expired_events: Vec<String> = self
            .pending_events
            .iter()
            .filter(|(_, pending)| pending.expires_at <= now)
            .map(|(key, _)| key.clone())
            .collect();
        for transport_event_key in expired_events {
            let Some(pending_event) = self.pending_events.remove(&transport_event_key) else {
                continue;
            };
            if pending_event.settlement_scheduled {
                commands.push(JoinCommand::CancelSettlement {
                    event_expires_at: pending_event.expires_at,
                    event_key: transport_event_key.clone(),
                });
            }
            for correlation in &pending_event.envelope.correlations {
                if self
                    .correlation_event_ids
                    .get(&correlation.correlation_id)
                    .is_some_and(|claimed| *claimed == transport_event_key)
                {
                    self.correlation_event_ids
                        .remove(&correlation.correlation_id);
                }
            }
            if pending_event.applied {
                self.recent_event_expirations.set(
                    transport_event_key,
                    now + self.options.recent_event_ttl_milliseconds,
                );
            }
        }
        let expired_recent: Vec<String> = self
            .recent_event_expirations
            .iter()
            .filter(|(_, expires_at)| **expires_at <= now)
            .map(|(key, _)| key.clone())
            .collect();
        for key in expired_recent {
            self.recent_event_expirations.remove(&key);
        }
        commands
    }
}
