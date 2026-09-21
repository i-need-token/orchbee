//! Conformance-harness support for the private pure ingress port — the
//! fixture-shape JSON emitters, the per-step dispatch trace collector, and
//! the settlement-clock queue that replaces Desktop `setTimeout` in replay.
//! The translator core (`hook_ingress.rs`) keeps only semantic behavior:
//! `runtime.dispatch` is called verbatim and every transition/order decision
//! is unchanged. The `SettlementQueue` timing primitives are shared with the
//! production `Instant` funnel; the JSON/corpus emitters stay `#[cfg(test)]`.

#[cfg(test)]
use serde_json::{Value, json};

#[cfg(test)]
use super::AgentObservationEvent;
#[cfg(test)]
use super::AgentTransition;
#[cfg(test)]
use super::events::AgentSemanticEvent;
#[cfg(test)]
use super::hook_contract::{AdapterDefinition, HookEnvelope};
#[cfg(test)]
use super::join::JoinCommand;
#[cfg(test)]
use super::model::RawAttachment;
#[cfg(test)]
use super::{ATTENTION_ACTION_PATH, AgentRuntimeSnapshot, AgentTurnState, RawPresence};

// ---- dispatch trace collector ----

/// A recorded dispatch for the conformance harness — the corpus sink's
/// `{event, transition:{kind, reason, semanticEvents}}` triple.
#[derive(Clone, Debug)]
#[cfg(test)]
pub struct DispatchRecord {
    pub event: AgentObservationEvent,
    pub kind: &'static str,
    pub reason: Option<&'static str>,
    pub semantic_events: Vec<AgentSemanticEvent>,
}

/// Per-step dispatch log. The translator calls `record` immediately after
/// each `runtime.dispatch`; the harness drains it with `take`.
#[derive(Default)]
#[cfg(test)]
pub struct DispatchTrace {
    records: Vec<DispatchRecord>,
}

#[cfg(test)]
impl DispatchTrace {
    pub fn record(&mut self, event: &AgentObservationEvent, transition: &AgentTransition) {
        self.records.push(DispatchRecord {
            event: event.clone(),
            kind: transition.kind(),
            reason: match transition {
                AgentTransition::Rejected { reason, .. } => Some(reason.as_str()),
                _ => None,
            },
            semantic_events: transition.semantic_events().to_vec(),
        });
    }

    pub fn take(&mut self) -> Vec<DispatchRecord> {
        std::mem::take(&mut self.records)
    }
}

// ---- settlement clock queue (the `setTimeout` stand-in, shared with the
// ---- production `Instant` funnel; corpus replay drives it in f64 domain) ----

#[derive(Clone, Debug)]
struct SettlementTimer {
    event_expires_at: f64,
    event_key: String,
    fire_at: f64,
}

/// The corpus scheduler clock. `advance` moves only this clock; `pop_due`
/// yields timers whose `fire_at` is reached — the caller fires them with the
/// injected semantic `now`, keeping the two clocks independent.
#[derive(Default)]
pub struct SettlementQueue {
    now: f64,
    timers: Vec<SettlementTimer>,
}

impl SettlementQueue {
    /// `schedule-settlement`: re-scheduling an existing key replaces it; a
    /// stale `event_expires_at` on the same key is ignored.
    pub fn schedule(&mut self, event_key: String, event_expires_at: f64, milliseconds: f64) {
        if let Some(existing) = self
            .timers
            .iter()
            .find(|timer| timer.event_key == event_key)
            && existing.event_expires_at == event_expires_at
        {
            return;
        }
        self.timers.retain(|timer| timer.event_key != event_key);
        self.timers.push(SettlementTimer {
            event_expires_at,
            event_key,
            fire_at: self.now + milliseconds,
        });
    }

    /// `cancel-settlement`: keyed on `(event_key, event_expires_at)`.
    pub fn cancel(&mut self, event_key: &str, event_expires_at: f64) {
        let Some(existing) = self
            .timers
            .iter()
            .find(|timer| timer.event_key == event_key)
        else {
            return;
        };
        if existing.event_expires_at != event_expires_at {
            return;
        }
        self.timers.retain(|timer| timer.event_key != event_key);
    }

    #[cfg(test)]
    pub fn advance(&mut self, milliseconds: f64) {
        self.now += milliseconds;
    }

    pub fn pop_due(&mut self) -> Option<(String, f64)> {
        let index = self
            .timers
            .iter()
            .position(|timer| timer.fire_at <= self.now)?;
        let timer = self.timers.remove(index);
        Some((timer.event_key, timer.event_expires_at))
    }

    /// Monotone sync toward the freshest known time — the production
    /// call-time clock adapter. Only the new Instant-domain seams reach
    /// this; the corpus `advance`/`pop_due` pair can never, so the
    /// recorded two-clock contract is preserved by non-reachability
    /// rather than by any claimed invariance. A stale or out-of-order
    /// `now` can never move the scheduler backward.
    pub fn sync(&mut self, now: f64) {
        self.now = self.now.max(now);
    }

    /// Earliest pending `fire_at` — the production poll deadline source.
    /// Returns `None` while no timer producer has scheduled anything.
    pub fn next_fire_at(&self) -> Option<f64> {
        self.timers
            .iter()
            .map(|timer| timer.fire_at)
            .reduce(f64::min)
    }
}

// ---- corpus-shape serialization (fixture comparison only) ----

#[cfg(test)]
pub fn attachment_json(attachment: &RawAttachment) -> Value {
    json!({
        "generation": attachment.generation,
        "navigable": attachment.navigable,
        "presentationId": attachment.presentation_id,
        "visibility": attachment.visibility,
    })
}

#[cfg(test)]
pub fn adapter_json(adapter: &AdapterDefinition) -> Value {
    json!({
        "adapterId": adapter.adapter_id,
        "capabilities": adapter.capabilities,
        "providerId": adapter.provider_id,
    })
}

#[cfg(test)]
pub fn envelope_json(envelope: &HookEnvelope) -> Value {
    json!({
        "adapterId": envelope.adapter_id,
        "correlations": envelope.correlations.iter().map(|correlation| json!({
            "correlationId": correlation.correlation_id,
            "visibility": correlation.visibility.as_str(),
        })).collect::<Vec<_>>(),
        "eventId": envelope.event_id,
        "observation": {
            "activity": envelope.observation.activity.map(|a| a.as_str()),
            "agentKey": envelope.observation.agent_key,
            "event": envelope.observation.event.as_str(),
            "operationKey": envelope.observation.operation_key,
            "sessionKey": envelope.observation.session_key,
            "turnKey": envelope.observation.turn_key,
            "version": 1,
        },
        "version": 3,
    })
}

#[cfg(test)]
pub fn join_command_json(command: &JoinCommand) -> Value {
    match command {
        JoinCommand::CancelSettlement {
            event_expires_at,
            event_key,
        } => json!({
            "eventExpiresAt": event_expires_at,
            "eventKey": event_key,
            "type": "cancel-settlement",
        }),
        JoinCommand::ApplyObservation {
            adapter,
            attachments,
            envelope,
        } => json!({
            "adapter": adapter_json(adapter),
            "attachments": attachments.iter().map(attachment_json).collect::<Vec<_>>(),
            "envelope": envelope_json(envelope),
            "type": "apply-observation",
        }),
        JoinCommand::AttachPresentations {
            adapter_id,
            attachments,
            session_key,
        } => json!({
            "adapterId": adapter_id,
            "attachments": attachments.iter().map(attachment_json).collect::<Vec<_>>(),
            "sessionKey": session_key,
            "type": "attach-presentations",
        }),
        JoinCommand::ScheduleSettlement {
            event_expires_at,
            event_key,
            milliseconds,
        } => json!({
            "eventExpiresAt": event_expires_at,
            "eventKey": event_key,
            "milliseconds": milliseconds,
            "type": "schedule-settlement",
        }),
        JoinCommand::RetirePresentationGeneration {
            generation,
            presentation_id,
        } => json!({
            "generation": generation,
            "presentationId": presentation_id,
            "type": "retire-presentation-generation",
        }),
    }
}

#[cfg(test)]
fn presence_json<T, F: Fn(&T) -> Value>(presence: &RawPresence<T>, map: F) -> Option<Value> {
    match presence {
        RawPresence::Absent => None,
        RawPresence::Null => Some(Value::Null),
        RawPresence::Other => Some(json!("__malformed-presence__")),
        RawPresence::Value(value) => Some(map(value)),
    }
}

/// Serialize a dispatched event exactly like the recorded corpus `event`.
#[cfg(test)]
pub fn event_json(event: &AgentObservationEvent) -> Value {
    match event {
        AgentObservationEvent::SessionObserved {
            capabilities,
            event_id,
            execution_location,
            observation,
            parent_session_id,
            presentation_attachments,
            provider_id,
            session_id,
        } => {
            let mut value = json!({
                "capabilities": capabilities,
                "eventId": event_id,
                "executionLocation": {
                    "executionLocationId": execution_location.execution_location_id,
                },
                "observation": {
                    "fidelity": observation.fidelity,
                    "freshness": observation.freshness,
                    "generation": observation.generation,
                    "sequence": observation.sequence,
                    "source": observation.source,
                },
                "presentationAttachments": presentation_attachments
                    .iter()
                    .map(attachment_json)
                    .collect::<Vec<_>>(),
                "providerId": provider_id,
                "sessionId": session_id,
                "type": "session-observed",
            });
            if let Some(parent) = presence_json(parent_session_id, |id| json!(id)) {
                value["parentSessionId"] = parent;
            }
            value
        }
        AgentObservationEvent::TurnStarted {
            activity,
            event_id,
            generation,
            phase,
            sequence,
            session_id,
            turn_id,
        } => {
            let mut value = json!({
                "eventId": event_id,
                "generation": generation,
                "phase": phase,
                "sequence": sequence,
                "sessionId": session_id,
                "turnId": turn_id,
                "type": "turn-started",
            });
            if let Some(activity) = presence_json(activity, |a| json!(a)) {
                value["activity"] = activity;
            }
            value
        }
        AgentObservationEvent::TurnActivityChanged {
            activity,
            event_id,
            generation,
            phase,
            sequence,
            session_id,
            turn_id,
        } => {
            let mut value = json!({
                "eventId": event_id,
                "generation": generation,
                "phase": phase,
                "sequence": sequence,
                "sessionId": session_id,
                "turnId": turn_id,
                "type": "turn-activity-changed",
            });
            if let Some(activity) = presence_json(activity, |a| json!(a)) {
                value["activity"] = activity;
            }
            value
        }
        AgentObservationEvent::AttentionRequested {
            event_id,
            generation,
            request,
            sequence,
            session_id,
        } => {
            let mut request_json = json!({
                "actionPath": request.action_path,
                "kind": request.kind,
                "requestId": request.request_id,
            });
            if let Some(turn) = presence_json(&request.turn_id, |id| json!(id)) {
                request_json["turnId"] = turn;
            }
            json!({
                "eventId": event_id,
                "generation": generation,
                "request": request_json,
                "sequence": sequence,
                "sessionId": session_id,
                "type": "attention-requested",
            })
        }
        AgentObservationEvent::AttentionResolved {
            event_id,
            generation,
            request_id,
            sequence,
            session_id,
        } => json!({
            "eventId": event_id,
            "generation": generation,
            "requestId": request_id,
            "sequence": sequence,
            "sessionId": session_id,
            "type": "attention-resolved",
        }),
        AgentObservationEvent::TurnFinished {
            event_id,
            generation,
            outcome,
            sequence,
            session_id,
            turn_id,
        } => json!({
            "eventId": event_id,
            "generation": generation,
            "outcome": outcome,
            "sequence": sequence,
            "sessionId": session_id,
            "turnId": turn_id,
            "type": "turn-finished",
        }),
        AgentObservationEvent::SessionIdleObserved {
            event_id,
            generation,
            sequence,
            session_id,
        } => json!({
            "eventId": event_id,
            "generation": generation,
            "sequence": sequence,
            "sessionId": session_id,
            "type": "session-idle-observed",
        }),
        AgentObservationEvent::PresentationAttached {
            attachment,
            event_id,
            generation,
            sequence,
            session_id,
        } => json!({
            "attachment": attachment_json(attachment),
            "eventId": event_id,
            "generation": generation,
            "sequence": sequence,
            "sessionId": session_id,
            "type": "presentation-attached",
        }),
        AgentObservationEvent::PresentationVisibilityChanged {
            event_id,
            generation,
            presentation_generation,
            presentation_id,
            sequence,
            session_id,
            visibility,
        } => json!({
            "eventId": event_id,
            "generation": generation,
            "presentationGeneration": presentation_generation,
            "presentationId": presentation_id,
            "sequence": sequence,
            "sessionId": session_id,
            "type": "presentation-visibility-changed",
            "visibility": visibility,
        }),
        AgentObservationEvent::PresentationDetached {
            event_id,
            generation,
            presentation_generation,
            presentation_id,
            sequence,
            session_id,
        } => json!({
            "eventId": event_id,
            "generation": generation,
            "presentationGeneration": presentation_generation,
            "presentationId": presentation_id,
            "sequence": sequence,
            "sessionId": session_id,
            "type": "presentation-detached",
        }),
        AgentObservationEvent::SessionEnded {
            event_id,
            generation,
            sequence,
            session_id,
        } => json!({
            "eventId": event_id,
            "generation": generation,
            "sequence": sequence,
            "sessionId": session_id,
            "type": "session-ended",
        }),
        // The translator never emits these; unreachable in the corpus.
        other => json!({ "type": other.transition_kind() }),
    }
}

/// Serialize a semantic event exactly like the recorded corpus entries.
#[cfg(test)]
pub fn semantic_event_json(event: &AgentSemanticEvent) -> Value {
    match event {
        AgentSemanticEvent::AttentionRequested {
            event_id,
            request,
            session_id,
        } => {
            let mut request_json = json!({
                "actionPath": request.action_path,
                "kind": request.kind,
                "requestId": request.request_id,
            });
            if let Some(turn) = presence_json(&request.turn_id, |id| json!(id)) {
                request_json["turnId"] = turn;
            }
            json!({
                "eventId": event_id,
                "request": request_json,
                "sessionId": session_id,
                "type": "attention-requested",
            })
        }
        AgentSemanticEvent::AttentionResolved {
            event_id,
            request_id,
            session_id,
        } => json!({
            "eventId": event_id,
            "requestId": request_id,
            "sessionId": session_id,
            "type": "attention-resolved",
        }),
        AgentSemanticEvent::TurnOutcome {
            event_id,
            outcome,
            session_id,
            turn_id,
        } => json!({
            "eventId": event_id,
            "outcome": outcome,
            "sessionId": session_id,
            "turnId": turn_id,
            "type": "turn-outcome",
        }),
        AgentSemanticEvent::SessionEnded {
            event_id,
            session_id,
        } => json!({
            "eventId": event_id,
            "sessionId": session_id,
            "type": "session-ended",
        }),
    }
}

/// `summarize(runtime.getSnapshot())` — the recorded per-step snapshot shape.
#[cfg(test)]
pub fn summarize_snapshot(snapshot: &AgentRuntimeSnapshot) -> Value {
    let sessions: Vec<Value> = snapshot
        .sessions
        .iter()
        .map(|session| {
            let turn = match &session.turn {
                AgentTurnState::Unknown => json!({ "kind": "unknown" }),
                AgentTurnState::Idle => json!({ "kind": "idle" }),
                AgentTurnState::Active {
                    activity,
                    phase,
                    turn_id,
                } => json!({
                    "activity": activity.map(|a| a.as_str()),
                    "kind": "active",
                    "phase": phase.as_str(),
                    "turnId": turn_id.as_str(),
                }),
            };
            json!({
                "activityHint": session.activity_hint.map(|a| a.as_str()),
                "lifecycle": session.lifecycle.as_str(),
                "lastOutcome": session.last_outcome.as_ref().map(|outcome| json!({
                    "eventId": outcome.event_id.as_str(),
                    "kind": outcome.kind.as_str(),
                    "turnId": outcome.turn_id.as_str(),
                })),
                "observation": {
                    "fidelity": session.observation.fidelity.as_str(),
                    "freshness": session.observation.freshness.as_str(),
                    "generation": session.observation.generation,
                    "sequence": session.observation.sequence,
                    "source": session.observation.source.as_str(),
                },
                "parentSessionId": session.parent_session_id.as_ref().map(|id| id.as_str()),
                "pendingRequests": session
                    .pending_requests
                    .iter()
                    .map(|request| json!({
                        "actionPath": ATTENTION_ACTION_PATH,
                        "kind": request.kind.as_str(),
                        "requestId": request.request_id.as_str(),
                        "turnId": request.turn_id.as_ref().map(|id| id.as_str()),
                    }))
                    .collect::<Vec<_>>(),
                "presentationAttachments": session
                    .presentation_attachments
                    .iter()
                    .map(|attachment| json!({
                        "generation": attachment.generation.as_str(),
                        "navigable": attachment.navigable,
                        "presentationId": attachment.presentation_id.as_str(),
                        "visibility": attachment.visibility.as_str(),
                    }))
                    .collect::<Vec<_>>(),
                "providerId": session.provider_id.as_str(),
                "sessionId": session.session_id.as_str(),
                "turn": turn,
            })
        })
        .collect();
    json!({
        "revision": snapshot.revision,
        "sessions": sessions,
    })
}

// ---- corpus driver owner (fixture runtime + runtime-free translator) ----

/// The corpus-facing owner pair: the fixture owns the one canonical
/// `AgentRuntime` and lends it mutably into the runtime-free translator,
/// mirroring the production `AgentState` cell where the service owns the
/// runtime. Method signatures reproduce the pre-split corpus surface so
/// the frozen 16-case hook corpus records identical expected values.
#[cfg(test)]
pub struct FixtureIngress {
    pub ingress: super::hook_ingress::AgentHookObservationIngress,
    pub runtime: super::runtime::AgentRuntime,
}

#[cfg(test)]
impl FixtureIngress {
    pub fn new(socket_path: Option<String>, join_options: super::join::JoinOptions) -> Self {
        Self {
            ingress: super::hook_ingress::AgentHookObservationIngress::new(
                socket_path,
                join_options,
            ),
            runtime: super::runtime::AgentRuntime::new(),
        }
    }

    pub fn environment_overlay_for(
        &mut self,
        presentation_id: String,
        creation_id: String,
    ) -> Vec<(String, String)> {
        self.ingress
            .environment_overlay_for(&mut self.runtime, presentation_id, creation_id)
    }

    pub fn observe_terminal_application_signal(
        &mut self,
        payload: &str,
        creation_id: String,
        presentation_id: String,
        now: f64,
    ) {
        self.ingress.observe_terminal_application_signal(
            &mut self.runtime,
            payload,
            creation_id,
            presentation_id,
            now,
        );
    }

    pub fn retire_presentation(&mut self, presentation_id: &str) {
        self.ingress
            .retire_presentation(&mut self.runtime, presentation_id);
    }

    pub fn admit_envelope(&mut self, body: &[u8], now: f64) -> bool {
        self.ingress.admit_envelope(&mut self.runtime, body, now)
    }

    pub fn advance_settlement(&mut self, milliseconds: f64, now: f64) {
        self.ingress
            .advance_settlement(&mut self.runtime, milliseconds, now);
    }

    pub fn take_dispatch_log(&mut self) -> Vec<DispatchRecord> {
        self.ingress.take_dispatch_log()
    }

    pub fn snapshot_json(&self) -> Value {
        self.ingress.snapshot_json(&self.runtime)
    }
}
