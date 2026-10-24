//! Conformance harness: replays the durable executed corpus
//! (`schemas/agent/v1/agent-canonical-conformance-v1.json`) through the current
//! Rust implementation and asserts every recorded step, complete final session maps, semantic-event ordering,
//! journal pages, and the recorded aggregate/location projections.
//!
//! The fixture is private parity evidence; decoding uses `serde_json` only.

use serde_json::{Map, Value, json};

use super::super::AgentObservationEvent;
use super::super::AgentRuntime;
use super::super::AgentTransition;
use super::super::changes::{
    AgentChangeReadInput, AgentRuntimeChange, AgentRuntimeChangeReadResult,
};
use super::super::events::AgentSemanticEvent;
use super::super::model::{RawAttachment, RawTurnOutcome, RawTurnState};
use super::super::projection::{
    ALL_AGENT_WAIT_CONDITIONS, AgentAggregatePresentation, AgentLocationPresentation,
    matching_agent_wait_conditions, project_agent_location, project_agent_session_group,
};
use super::super::{
    AgentRuntimeSnapshot, AgentSessionSnapshot, AgentTurnState, RawAttentionRequest,
    RawExecutionLocation, RawObservation, RawPresence,
};
use super::super::{
    AgentSemanticStatus, explain_agent_session, project_agent_session,
    project_agent_session_summary,
};

const CORPUS: &str =
    include_str!("../../../../../schemas/agent/v1/agent-canonical-conformance-v1.json");

#[test]
fn agent_canonical_conformance_all_vectors() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("corpus fixture must decode");
    let vectors = corpus["vectors"].as_array().expect("vectors array");
    assert_eq!(vectors.len(), 29, "corpus must carry all 29 vectors");
    for vector in vectors {
        run_vector(vector);
    }
}

fn run_vector(vector: &Value) {
    let vector_id = vector["id"].as_str().unwrap();
    let mut runtime = AgentRuntime::new();
    let events = vector["events"].as_array().unwrap();
    let steps = vector["expected"]["steps"].as_array().unwrap();
    assert_eq!(
        events.len(),
        steps.len(),
        "{vector_id}: every expanded event has an expected step"
    );
    for (index, entry) in events.iter().enumerate() {
        let event = decode_event(&entry["event"]);
        let transition = runtime.dispatch(&event);
        let expected = &steps[index];
        assert_eq!(
            expected["eventIndex"].as_u64().unwrap(),
            index as u64,
            "{vector_id} step {index}: eventIndex"
        );
        assert_eq!(
            transition.kind(),
            expected["kind"].as_str().unwrap(),
            "{vector_id} step {index} ({}): transition kind",
            event.event_id()
        );
        if let AgentTransition::Rejected { reason, .. } = &transition {
            assert_eq!(
                reason.as_str(),
                expected["reason"].as_str().unwrap(),
                "{vector_id} step {index}: rejection reason"
            );
        }
        assert_eq!(
            transition.snapshot().revision,
            expected["revision"].as_u64().unwrap(),
            "{vector_id} step {index}: post-transition revision"
        );
        assert_eq!(
            Value::Array(
                transition
                    .semantic_events()
                    .iter()
                    .map(semantic_event_json)
                    .collect()
            ),
            expected["semanticEvents"],
            "{vector_id} step {index}: semantic events in order"
        );
        if let Some(dispose_after) = vector["disposeAfterStep"].as_u64()
            && index as u64 == dispose_after - 1
        {
            runtime.dispose();
            assert!(runtime.is_disposed());
        }
    }

    assert_final_sessions(vector_id, &vector["expected"], runtime.snapshot());
    assert_journal_reads(vector_id, &vector["expected"], &runtime);
    assert_aggregate(vector_id, vector, runtime.snapshot());
    assert_location(vector_id, vector, runtime.snapshot());
}

// ---- final-state assertions ----

fn assert_final_sessions(vector_id: &str, expected: &Value, snapshot: &AgentRuntimeSnapshot) {
    let expected_sessions = expected["sessions"].as_object().unwrap();
    let actual_ids: Vec<&str> = snapshot
        .sessions
        .iter()
        .map(|session| session.session_id.as_str())
        .collect();
    let mut expected_ids: Vec<&str> = expected_sessions.keys().map(String::as_str).collect();
    expected_ids.sort_by(|a, b| super::super::model::compare_agent_identity(a, b));
    assert_eq!(
        actual_ids, expected_ids,
        "{vector_id}: complete final session map (exact set, canonical order)"
    );
    for session in &snapshot.sessions {
        let id = session.session_id.as_str();
        let expected_entry = &expected_sessions[id];
        assert_eq!(
            session_json(session),
            expected_entry["snapshot"],
            "{vector_id} {id}: canonical snapshot"
        );
        assert_eq!(
            summary_json(session),
            expected_entry["summary"],
            "{vector_id} {id}: session summary"
        );
        assert_eq!(
            status_json(&project_agent_session(session)),
            expected_entry["status"],
            "{vector_id} {id}: semantic status"
        );
        assert_eq!(
            explanation_json(session),
            expected_entry["explanation"],
            "{vector_id} {id}: session explanation"
        );
        let matched: Vec<Value> =
            matching_agent_wait_conditions(session, &ALL_AGENT_WAIT_CONDITIONS)
                .iter()
                .map(|condition| json!(condition))
                .collect();
        assert_eq!(
            Value::Array(matched),
            expected_entry["matchedWaitConditions"],
            "{vector_id} {id}: matched wait conditions"
        );
    }
}

fn assert_journal_reads(vector_id: &str, expected: &Value, runtime: &AgentRuntime) {
    for read in expected["journalReads"].as_array().unwrap() {
        let input = AgentChangeReadInput {
            after_revision: read["input"]["afterRevision"]
                .as_f64()
                .expect("afterRevision"),
            limit: read["input"]["limit"].as_f64().expect("limit"),
        };
        let empty_runtime;
        let reader = if read["onEmptyRuntime"].as_bool() == Some(true) {
            empty_runtime = AgentRuntime::new();
            &empty_runtime
        } else {
            runtime
        };
        let result = reader
            .read_changes(input)
            .expect("corpus journal reads are always valid inputs");
        assert_eq!(
            read_result_json(&result),
            read["result"],
            "{vector_id} journal read {:?}: result",
            read["input"]
        );
    }
}

fn assert_aggregate(vector_id: &str, vector: &Value, snapshot: &AgentRuntimeSnapshot) {
    let expected = &vector["expected"]["aggregate"];
    if expected.is_null() {
        return;
    }
    let ids: Vec<&str> = vector["aggregateSessionIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect();
    assert_eq!(
        Value::Array(ids.iter().map(|id| json!(id)).collect()),
        expected["input"]["sessionIds"],
        "{vector_id}: aggregate input ids"
    );
    let sessions: Vec<&AgentSessionSnapshot> = snapshot
        .sessions
        .iter()
        .filter(|session| ids.contains(&session.session_id.as_str()))
        .collect();
    let aggregate = project_agent_session_group(&sessions);
    assert_eq!(
        aggregate
            .as_ref()
            .map(aggregate_json)
            .unwrap_or(Value::Null),
        expected["result"],
        "{vector_id}: aggregate projection"
    );
}

fn assert_location(vector_id: &str, vector: &Value, snapshot: &AgentRuntimeSnapshot) {
    let expected = &vector["expected"]["location"];
    if expected.is_null() {
        return;
    }
    let presentation_id = vector["locationPresentationId"].as_str().unwrap();
    assert_eq!(
        presentation_id,
        expected["input"]["presentationId"].as_str().unwrap(),
        "{vector_id}: location input"
    );
    let location = project_agent_location(snapshot, presentation_id);
    assert_eq!(
        location.as_ref().map(location_json).unwrap_or(Value::Null),
        expected["result"],
        "{vector_id}: location projection"
    );
}

// ---- event decoding (raw: numbers stay f64, strings stay unvalidated) ----

fn decode_event(value: &Value) -> AgentObservationEvent {
    let get = |key: &str| -> &Value { &value[key] };
    let number = |key: &str| -> f64 { get(key).as_f64().expect("numeric event field") };
    let string = |key: &str| -> String { get(key).as_str().unwrap_or_default().to_string() };
    let strings = |key: &str| -> Vec<String> {
        get(key)
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| item.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    match get("type").as_str().unwrap_or("") {
        "session-observed" => AgentObservationEvent::SessionObserved {
            capabilities: strings("capabilities"),
            event_id: string("eventId"),
            execution_location: RawExecutionLocation {
                execution_location_id: get("executionLocation")["executionLocationId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            },
            observation: decode_observation(get("observation")),
            parent_session_id: presence_string(value, "parentSessionId"),
            presentation_attachments: get("presentationAttachments")
                .as_array()
                .map(|items| items.iter().map(decode_attachment).collect())
                .unwrap_or_default(),
            provider_id: string("providerId"),
            session_id: string("sessionId"),
        },
        "turn-started" => AgentObservationEvent::TurnStarted {
            activity: presence_string(value, "activity"),
            event_id: string("eventId"),
            generation: number("generation"),
            phase: string("phase"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
            turn_id: string("turnId"),
        },
        "turn-activity-changed" => AgentObservationEvent::TurnActivityChanged {
            activity: presence_string(value, "activity"),
            event_id: string("eventId"),
            generation: number("generation"),
            phase: string("phase"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
            turn_id: string("turnId"),
        },
        "attention-requested" => AgentObservationEvent::AttentionRequested {
            event_id: string("eventId"),
            generation: number("generation"),
            request: decode_request(get("request")),
            sequence: number("sequence"),
            session_id: string("sessionId"),
        },
        "attention-resolved" => AgentObservationEvent::AttentionResolved {
            event_id: string("eventId"),
            generation: number("generation"),
            request_id: string("requestId"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
        },
        "turn-finished" => AgentObservationEvent::TurnFinished {
            event_id: string("eventId"),
            generation: number("generation"),
            outcome: string("outcome"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
            turn_id: string("turnId"),
        },
        "session-idle-observed" => AgentObservationEvent::SessionIdleObserved {
            event_id: string("eventId"),
            generation: number("generation"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
        },
        "observation-became-stale" => AgentObservationEvent::ObservationBecameStale {
            event_id: string("eventId"),
            generation: number("generation"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
        },
        "observation-reconnected" => AgentObservationEvent::ObservationReconnected {
            activity_hint: presence_string(value, "activityHint"),
            capabilities: strings("capabilities"),
            event_id: string("eventId"),
            generation: number("generation"),
            last_outcome: match presence(value, "lastOutcome") {
                RawPresence::Value(outcome @ Value::Object(_)) => {
                    RawPresence::Value(decode_turn_outcome(outcome))
                }
                RawPresence::Value(_) => RawPresence::Value(RawTurnOutcome {
                    event_id: String::new(),
                    kind: String::new(),
                    turn_id: String::new(),
                }),
                RawPresence::Null => RawPresence::Null,
                RawPresence::Absent => RawPresence::Absent,
            },
            pending_requests: get("pendingRequests")
                .as_array()
                .map(|items| items.iter().map(decode_request).collect())
                .unwrap_or_default(),
            sequence: number("sequence"),
            session_id: string("sessionId"),
            turn: decode_raw_turn(get("turn")),
        },
        "presentation-attached" => AgentObservationEvent::PresentationAttached {
            attachment: decode_attachment(get("attachment")),
            event_id: string("eventId"),
            generation: number("generation"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
        },
        "presentation-visibility-changed" => AgentObservationEvent::PresentationVisibilityChanged {
            event_id: string("eventId"),
            generation: number("generation"),
            presentation_generation: string("presentationGeneration"),
            presentation_id: string("presentationId"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
            visibility: string("visibility"),
        },
        "presentation-detached" => AgentObservationEvent::PresentationDetached {
            event_id: string("eventId"),
            generation: number("generation"),
            presentation_generation: string("presentationGeneration"),
            presentation_id: string("presentationId"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
        },
        "session-ended" => AgentObservationEvent::SessionEnded {
            event_id: string("eventId"),
            generation: number("generation"),
            sequence: number("sequence"),
            session_id: string("sessionId"),
        },
        other => panic!("unknown Agent observation event type: {other}"),
    }
}

fn decode_observation(value: &Value) -> RawObservation {
    RawObservation {
        fidelity: value["fidelity"].as_str().unwrap_or_default().to_string(),
        freshness: value["freshness"].as_str().unwrap_or_default().to_string(),
        generation: value["generation"].as_f64().unwrap_or_default(),
        sequence: value["sequence"].as_f64().unwrap_or_default(),
        source: value["source"].as_str().unwrap_or_default().to_string(),
    }
}

fn decode_attachment(value: &Value) -> RawAttachment {
    RawAttachment {
        generation: value["generation"].as_str().unwrap_or_default().to_string(),
        navigable: value["navigable"].as_bool().unwrap_or_default(),
        presentation_id: value["presentationId"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        visibility: value["visibility"].as_str().unwrap_or_default().to_string(),
    }
}

fn decode_request(value: &Value) -> RawAttentionRequest {
    RawAttentionRequest {
        action_path: value["actionPath"].as_str().unwrap_or_default().to_string(),
        kind: value["kind"].as_str().unwrap_or_default().to_string(),
        request_id: value["requestId"].as_str().unwrap_or_default().to_string(),
        turn_id: presence_string(value, "turnId"),
    }
}

/// Key-presence decode: `Absent` (missing key), `Null` (explicit JSON null),
/// or `Value`. Mistyped values decode to invalid typed contents. Required
/// non-nullable keys keep the lossy `as_str` collapse — the typed domain
/// cannot express their absence, and the corpus never carries such shapes.
fn presence<'a>(object: &'a Value, key: &str) -> RawPresence<&'a Value> {
    match object.get(key) {
        None => RawPresence::Absent,
        Some(Value::Null) => RawPresence::Null,
        Some(other) => RawPresence::Value(other),
    }
}

fn presence_string(object: &Value, key: &str) -> RawPresence<String> {
    match presence(object, key) {
        RawPresence::Value(Value::String(text)) => RawPresence::Value(text.clone()),
        RawPresence::Value(_) => RawPresence::Value(String::new()),
        RawPresence::Null => RawPresence::Null,
        RawPresence::Absent => RawPresence::Absent,
    }
}

fn decode_turn_outcome(value: &Value) -> RawTurnOutcome {
    RawTurnOutcome {
        event_id: value["eventId"].as_str().unwrap_or_default().to_string(),
        kind: value["kind"].as_str().unwrap_or_default().to_string(),
        turn_id: value["turnId"].as_str().unwrap_or_default().to_string(),
    }
}

fn decode_raw_turn(value: &Value) -> RawTurnState {
    RawTurnState {
        activity: presence_string(value, "activity"),
        kind: value["kind"].as_str().unwrap_or_default().to_string(),
        phase: presence_string(value, "phase"),
        turn_id: presence_string(value, "turnId"),
    }
}

// ---- serializers (canonical wire shapes, camelCase) ----

fn semantic_event_json(event: &AgentSemanticEvent) -> Value {
    match event {
        AgentSemanticEvent::AttentionRequested {
            event_id,
            request,
            session_id,
        } => json!({
            "eventId": event_id,
            "request": raw_request_json(request),
            "sessionId": session_id,
            "type": "attention-requested",
        }),
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

fn session_json(session: &AgentSessionSnapshot) -> Value {
    json!({
        "activityHint": session.activity_hint.map(|activity| activity.as_str()),
        "capabilities": session
            .capabilities
            .iter()
            .map(|capability| capability.as_str())
            .collect::<Vec<_>>(),
        "executionLocation": {
            "executionLocationId": session.execution_location.execution_location_id.as_str(),
        },
        "lastOutcome": session.last_outcome.as_ref().map(|outcome| json!({
            "eventId": outcome.event_id.as_str(),
            "kind": outcome.kind.as_str(),
            "turnId": outcome.turn_id.as_str(),
        })),
        "lifecycle": session.lifecycle.as_str(),
        "observation": observation_json(&session.observation),
        "parentSessionId": session.parent_session_id.as_ref().map(|id| id.as_str()),
        "pendingRequests": session
            .pending_requests
            .iter()
            .map(|request| json!({
                "actionPath": super::super::model::ATTENTION_ACTION_PATH,
                "kind": request.kind.as_str(),
                "requestId": request.request_id.as_str(),
                "turnId": request.turn_id.as_ref().map(|id| id.as_str()),
            }))
            .collect::<Vec<_>>(),
        "presentationAttachments": session
            .presentation_attachments
            .iter()
            .map(attachment_json)
            .collect::<Vec<_>>(),
        "providerId": session.provider_id.as_str(),
        "sessionId": session.session_id.as_str(),
        "turn": turn_json(&session.turn),
    })
}

fn observation_json(observation: &super::super::model::AgentObservation) -> Value {
    json!({
        "fidelity": observation.fidelity.as_str(),
        "freshness": observation.freshness.as_str(),
        "generation": observation.generation,
        "sequence": observation.sequence,
        "source": observation.source.as_str(),
    })
}

fn attachment_json(attachment: &super::super::model::AgentPresentationAttachment) -> Value {
    json!({
        "generation": attachment.generation.as_str(),
        "navigable": attachment.navigable,
        "presentationId": attachment.presentation_id.as_str(),
        "visibility": attachment.visibility.as_str(),
    })
}

fn raw_request_json(request: &RawAttentionRequest) -> Value {
    // Emitted semantic events only carry requests that passed the attention
    // guards, so `turnId` is always `null` or a string here.
    let turn_id = match &request.turn_id {
        RawPresence::Value(turn_id) => json!(turn_id),
        RawPresence::Null => Value::Null,
        RawPresence::Absent => {
            unreachable!("an emitted request always carries its turnId key")
        }
    };
    json!({
        "actionPath": request.action_path,
        "kind": request.kind,
        "requestId": request.request_id,
        "turnId": turn_id,
    })
}

fn turn_json(turn: &AgentTurnState) -> Value {
    match turn {
        AgentTurnState::Active {
            activity,
            phase,
            turn_id,
        } => json!({
            "activity": activity.map(|activity| activity.as_str()),
            "kind": "active",
            "phase": phase.as_str(),
            "turnId": turn_id.as_str(),
        }),
        AgentTurnState::Idle => json!({ "kind": "idle" }),
        AgentTurnState::Unknown => json!({ "kind": "unknown" }),
    }
}

fn status_json(status: &AgentSemanticStatus) -> Value {
    match status {
        AgentSemanticStatus::NeedsAttention { count } => {
            json!({ "count": count, "kind": "needs-attention" })
        }
        AgentSemanticStatus::Failed => json!({ "kind": "failed" }),
        AgentSemanticStatus::Succeeded => json!({ "kind": "succeeded" }),
        AgentSemanticStatus::Canceled => json!({ "kind": "canceled" }),
        AgentSemanticStatus::Interrupted => json!({ "kind": "interrupted" }),
        AgentSemanticStatus::Working { activity, phase } => json!({
            "activity": activity,
            "kind": "working",
            "phase": phase,
        }),
        AgentSemanticStatus::Idle => json!({ "kind": "idle" }),
        AgentSemanticStatus::Unknown { activity_hint } => {
            json!({ "activityHint": activity_hint, "kind": "unknown" })
        }
        AgentSemanticStatus::Ended => json!({ "kind": "ended" }),
    }
}

fn summary_json(session: &AgentSessionSnapshot) -> Value {
    let summary = project_agent_session_summary(session);
    json!({
        "executionLocation": {
            "executionLocationId": summary.execution_location_id.as_str(),
        },
        "lifecycle": summary.lifecycle.as_str(),
        "observation": observation_json(&summary.observation),
        "parentSessionId": summary.parent_session_id.as_ref().map(|id| id.as_str()),
        "pendingRequestCount": summary.pending_request_count,
        "presentationAttachments": summary
            .presentation_attachments
            .iter()
            .map(attachment_json)
            .collect::<Vec<_>>(),
        "providerId": summary.provider_id.as_str(),
        "sessionId": summary.session_id.as_str(),
        "status": status_json(&summary.status),
    })
}

fn explanation_json(session: &AgentSessionSnapshot) -> Value {
    let explanation = explain_agent_session(session);
    json!({
        "actionPath": explanation.action_path,
        "capabilityCeiling": explanation
            .capability_ceiling
            .iter()
            .map(|capability| capability.as_str())
            .collect::<Vec<_>>(),
        "contentAvailable": explanation.content_available,
        "fidelity": explanation.fidelity.as_str(),
        "freshness": explanation.freshness.as_str(),
        "freshnessMeaning": explanation.freshness_meaning,
        "generation": explanation.generation,
        "sequence": explanation.sequence,
        "source": explanation.source.as_str(),
        "status": status_json(&explanation.status),
        "statusBasis": explanation.status_basis,
    })
}

fn change_json(change: &AgentRuntimeChange) -> Value {
    json!({
        "revision": change.revision,
        "session": {
            "lifecycle": change.session.lifecycle.as_str(),
            "observation": observation_json(&change.session.observation),
            "parentSessionId": change
                .session
                .parent_session_id
                .as_ref()
                .map(|id| id.as_str()),
            "pendingRequestCount": change.session.pending_request_count,
            "providerId": change.session.provider_id.as_str(),
            "sessionId": change.session.session_id.as_str(),
            "status": status_json(&change.session.status),
        },
        "transition": change.transition,
    })
}

fn read_result_json(result: &AgentRuntimeChangeReadResult) -> Value {
    let (kind, changes, has_more, observed, runtime) = match result {
        AgentRuntimeChangeReadResult::Changes {
            changes,
            has_more,
            observed_through_revision,
            runtime_revision,
        } => (
            "changes",
            changes.iter().map(change_json).collect::<Vec<_>>(),
            *has_more,
            *observed_through_revision,
            *runtime_revision,
        ),
        AgentRuntimeChangeReadResult::Current {
            observed_through_revision,
            runtime_revision,
        } => (
            "current",
            Vec::new(),
            false,
            *observed_through_revision,
            *runtime_revision,
        ),
        AgentRuntimeChangeReadResult::ResetRequired {
            observed_through_revision,
            runtime_revision,
        } => (
            "reset-required",
            Vec::new(),
            false,
            *observed_through_revision,
            *runtime_revision,
        ),
        AgentRuntimeChangeReadResult::RevisionAhead {
            observed_through_revision,
            runtime_revision,
        } => (
            "revision-ahead",
            Vec::new(),
            false,
            *observed_through_revision,
            *runtime_revision,
        ),
    };
    json!({
        "changes": changes,
        "hasMore": has_more,
        "kind": kind,
        "observedThroughRevision": observed,
        "runtimeRevision": runtime,
    })
}

fn aggregate_json(aggregate: &AgentAggregatePresentation) -> Value {
    json!({
        "activeCount": aggregate.active_count,
        "attentionCount": aggregate.attention_count,
        "freshness": aggregate.freshness.as_str(),
        "providerIds": aggregate
            .provider_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        "sessionCount": aggregate.session_count,
        "status": status_json(&aggregate.status),
    })
}

fn location_json(location: &AgentLocationPresentation) -> Value {
    let mut value = match aggregate_json(&location.aggregate) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    value.insert("location".to_string(), attachment_json(&location.location));
    Value::Object(value)
}
