//! Fixture-shape JSON, per-step dispatch traces, and the frozen corpus owner pair.
//! Production settlement scheduling remains owned by hook ingress; corpus replay
//! advances its existing test seam with the recorded independent clock.

use serde_json::{Value, json};

use super::super::AgentObservationEvent;
use super::super::AgentTransition;
use super::super::events::AgentSemanticEvent;
use super::super::hook_contract::{AdapterDefinition, HookEnvelope};
use super::super::join::JoinCommand;
use super::super::model::RawAttachment;
use super::super::{ATTENTION_ACTION_PATH, AgentRuntimeSnapshot, AgentTurnState, RawPresence};

// ---- dispatch trace collector ----

/// A recorded dispatch for the conformance harness — the corpus sink's
/// `{event, transition:{kind, reason, semanticEvents}}` triple.
#[derive(Clone, Debug)]
pub struct DispatchRecord {
    pub event: AgentObservationEvent,
    pub kind: &'static str,
    pub reason: Option<&'static str>,
    pub semantic_events: Vec<AgentSemanticEvent>,
}

/// Per-step dispatch log. The translator calls `record` immediately after
/// each `runtime.dispatch`; the harness drains it with `take`.
#[derive(Default)]
pub struct DispatchTrace {
    records: Vec<DispatchRecord>,
}

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

// ---- corpus-shape serialization (fixture comparison only) ----

pub fn attachment_json(attachment: &RawAttachment) -> Value {
    json!({
        "generation": attachment.generation,
        "navigable": attachment.navigable,
        "presentationId": attachment.presentation_id,
        "visibility": attachment.visibility,
    })
}

pub fn adapter_json(adapter: &AdapterDefinition) -> Value {
    json!({
        "adapterId": adapter.adapter_id,
        "capabilities": adapter.capabilities,
        "providerId": adapter.provider_id,
    })
}

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

fn presence_json<T, F: Fn(&T) -> Value>(presence: &RawPresence<T>, map: F) -> Option<Value> {
    match presence {
        RawPresence::Absent => None,
        RawPresence::Null => Some(Value::Null),

        RawPresence::Value(value) => Some(map(value)),
    }
}

/// Serialize a dispatched event exactly like the recorded corpus `event`.
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
pub struct FixtureIngress {
    pub ingress: super::super::hook_ingress::AgentHookObservationIngress,
    pub runtime: super::super::runtime::AgentRuntime,
}

impl FixtureIngress {
    pub fn new(socket_path: Option<String>, join_options: super::super::join::JoinOptions) -> Self {
        Self {
            ingress: super::super::hook_ingress::AgentHookObservationIngress::new(
                socket_path,
                join_options,
            ),
            runtime: super::super::runtime::AgentRuntime::new(),
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
