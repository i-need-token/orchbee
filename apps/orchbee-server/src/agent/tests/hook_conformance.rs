//! `#[cfg(test)]` replay harnesses for the frozen Desktop-derived corpora and
//! the native joint-capacity checks. Comparison is on complete outputs —
//! command arrays, ACK bytes, every dispatch input/transition/reason, semantic
//! events, and per-step snapshot summaries — never counts.
//!
//! Two clocks are kept independent exactly as recorded: `step.t` is the
//! injected `now` passed to join operations, while `settleWait` advances only
//! the scheduler clock that fires settlement effects.

use serde_json::{Value, json};

use super::super::JoinOptions;
use super::super::hook_contract::{AdapterDefinition, parse_envelope};
use super::super::hook_ingress::ACCEPTED_ENVELOPE_ACK;
use super::super::join::{AgentObservationJoinStateMachine, JoinHint, MAX_JOINT_ADMISSIONS};
use super::conformance_support::{
    FixtureIngress, event_json, join_command_json, semantic_event_json,
};

const JOIN_CORPUS: &str =
    include_str!("../../../../../schemas/agent/v1/agent-observation-join-conformance-v1.json");
const HOOK_CORPUS: &str =
    include_str!("../../../../../schemas/agent/v1/agent-hook-observation-conformance-v1.json");

/// Corpus JSON stores integral numbers; the port serializes `f64`. Compare
/// numerically by lowering every `Number` to its `f64` value.
fn norm_numbers(value: &Value) -> Value {
    match value {
        Value::Array(entries) => Value::Array(entries.iter().map(norm_numbers).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, entry)| (key.clone(), norm_numbers(entry)))
                .collect(),
        ),
        Value::Number(number) => json!(number.as_f64().unwrap()),
        other => other.clone(),
    }
}

fn corpus_presentation(n: u64) -> String {
    format!("desktop:presentation:c{n}")
}

fn corpus_creation(n: u64) -> String {
    String::from_iter(std::iter::repeat_n(format!("{n:02}"), 16))
}

fn corpus_correlation(n: u64) -> String {
    format!("correlation-{n:016}")
}

fn corpus_event(n: u64) -> String {
    format!("event-{n:016}")
}

fn join_options(case: &Value) -> JoinOptions {
    let options = &case["options"];
    JoinOptions {
        join_settle_milliseconds: options["joinSettleMilliseconds"].as_f64().unwrap_or(25.0),
        join_ttl_milliseconds: options["joinTtlMilliseconds"].as_f64().unwrap_or(10_000.0),
        max_pending_events: options["maxPendingEvents"].as_u64().unwrap_or(512) as usize,
        max_pending_hints: options["maxPendingHints"].as_u64().unwrap_or(1024) as usize,
        recent_event_ttl_milliseconds: options["recentEventTtlMilliseconds"]
            .as_f64()
            .unwrap_or(60_000.0),
    }
}

#[test]
fn join_conformance_replays_frozen_corpus() {
    let corpus: Value = serde_json::from_str(JOIN_CORPUS).expect("join corpus decodes");
    let cases = corpus["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 17, "join corpus must carry all 17 cases");
    let mut total_steps = 0usize;
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let mut join = AgentObservationJoinStateMachine::new(join_options(case));
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            total_steps += 1;
            let now = step["t"].as_f64().unwrap();
            let action = step["action"].as_str().unwrap();
            let args = step["args"].as_array().unwrap();
            let (accepted, commands) = match action {
                "register" => (
                    None,
                    join.register_presentation_generation(
                        args[0].as_str().unwrap().to_string(),
                        args[1].as_str().unwrap().to_string(),
                    ),
                ),
                "retire" => (None, join.retire_presentation(args[0].as_str().unwrap())),
                "hint" => (
                    None,
                    join.receive_hint(
                        JoinHint {
                            correlation_id: args[0]["correlationId"].as_str().unwrap().to_string(),
                            event_id: args[0]["eventId"].as_str().unwrap().to_string(),
                            generation: args[0]["generation"].as_str().unwrap().to_string(),
                            presentation_id: args[0]["presentationId"]
                                .as_str()
                                .unwrap()
                                .to_string(),
                        },
                        now,
                    ),
                ),
                "admit" => {
                    let adapter_value = &args[0];
                    let adapter = AdapterDefinition {
                        adapter_id: adapter_value["adapterId"].as_str().unwrap().to_string(),
                        capabilities: adapter_value["capabilities"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|entry| entry.as_str().unwrap().to_string())
                            .collect(),
                        provider_id: adapter_value["providerId"].as_str().unwrap().to_string(),
                    };
                    let envelope = parse_envelope(&args[1])
                        .unwrap_or_else(|| panic!("{name}[{index}]: corpus envelope must parse"));
                    let admission = join.admit_event(adapter, envelope, now);
                    (Some(admission.accepted), admission.commands)
                }
                "settle" => (
                    None,
                    join.settle(args[0].as_str().unwrap(), args[1].as_f64().unwrap(), now),
                ),
                // A settle on a never-issued key: real API, expiry-only effect.
                "probe" => (None, join.settle("probe\0never-issued", -1.0, now)),
                other => panic!("{name}[{index}]: unknown join action {other}"),
            };
            let expected = &step["output"];
            if let Some(expected_accepted) = expected.get("accepted") {
                assert_eq!(
                    Some(expected_accepted.as_bool().unwrap()),
                    accepted,
                    "{name}[{index}]: admission mismatch"
                );
            }
            let actual_commands: Vec<Value> = commands.iter().map(join_command_json).collect();
            assert_eq!(
                norm_numbers(&json!(actual_commands)),
                norm_numbers(&expected["commands"]),
                "{name}[{index}]: command mismatch"
            );
        }
    }
    assert!(total_steps > 0);
}

#[test]
fn hook_conformance_replays_frozen_corpus() {
    let corpus: Value = serde_json::from_str(HOOK_CORPUS).expect("hook corpus decodes");
    let cases = corpus["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 16, "hook corpus must carry all 16 cases");
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let mut ingress = FixtureIngress::new(Some("fixture.sock".to_string()), join_options(case));
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let now = step["t"].as_f64().unwrap();
            let action = step["action"].as_str().unwrap();
            let args = step["args"].as_array().unwrap();
            let mut actual_ack: Option<Value> = None;
            match action {
                "overlay" => {
                    ingress.environment_overlay_for(
                        corpus_presentation(args[0].as_u64().unwrap()),
                        corpus_creation(args[1].as_u64().unwrap()),
                    );
                }
                "signal" => {
                    let payload = json!({
                        "correlationId": corpus_correlation(args[0].as_u64().unwrap()),
                        "eventId": corpus_event(args[1].as_u64().unwrap()),
                        "kind": "correlation",
                        "version": 1,
                    })
                    .to_string();
                    ingress.observe_terminal_application_signal(
                        &payload,
                        corpus_creation(args[2].as_u64().unwrap()),
                        corpus_presentation(args[3].as_u64().unwrap()),
                        now,
                    );
                }
                "send" => {
                    let body = match &args[0] {
                        Value::String(text) => text.clone().into_bytes(),
                        other => serde_json::to_string(other).unwrap().into_bytes(),
                    };
                    let accepted = ingress.admit_envelope(&body, now);
                    actual_ack = Some(json!(if accepted { ACCEPTED_ENVELOPE_ACK } else { "" }));
                }
                "sendAll" => {
                    let mut acks = Vec::new();
                    for entry in args[0].as_array().unwrap() {
                        let body = match entry {
                            Value::String(text) => text.clone().into_bytes(),
                            other => serde_json::to_string(other).unwrap().into_bytes(),
                        };
                        let accepted = ingress.admit_envelope(&body, now);
                        acks.push(json!(if accepted { ACCEPTED_ENVELOPE_ACK } else { "" }));
                    }
                    actual_ack = Some(json!(acks));
                }
                "signalAll" => {
                    for entry in args[0].as_array().unwrap() {
                        let tuple = entry.as_array().unwrap();
                        let payload = json!({
                            "correlationId": corpus_correlation(tuple[0].as_u64().unwrap()),
                            "eventId": corpus_event(tuple[1].as_u64().unwrap()),
                            "kind": "correlation",
                            "version": 1,
                        })
                        .to_string();
                        ingress.observe_terminal_application_signal(
                            &payload,
                            corpus_creation(tuple[2].as_u64().unwrap()),
                            corpus_presentation(tuple[3].as_u64().unwrap()),
                            now,
                        );
                    }
                }
                "retire" => {
                    ingress.retire_presentation(&corpus_presentation(args[0].as_u64().unwrap()));
                }
                // The corpus waits real time; the port fires due settlement
                // effects with this step's injected `now`.
                "settleWait" => {
                    ingress.advance_settlement(args[0].as_f64().unwrap(), now);
                }
                other => panic!("{name}[{index}]: unknown hook action {other}"),
            }
            if let Some(expected_ack) = step.get("ack").filter(|ack| !ack.is_null()) {
                assert_eq!(
                    *expected_ack,
                    actual_ack.unwrap_or(Value::Null),
                    "{name}[{index}]: ack mismatch"
                );
            }
            let records = ingress.take_dispatch_log();
            let expected_dispatches = step["dispatches"].as_array().unwrap();
            assert_eq!(
                records.len(),
                expected_dispatches.len(),
                "{name}[{index}]: dispatch count mismatch"
            );
            let mut semantic = Vec::new();
            for (dispatch_index, (record, expected)) in
                records.iter().zip(expected_dispatches.iter()).enumerate()
            {
                assert_eq!(
                    norm_numbers(&event_json(&record.event)),
                    norm_numbers(&expected["event"]),
                    "{name}[{index}]: dispatch {dispatch_index} event mismatch"
                );
                let expected_transition = &expected["transition"];
                assert_eq!(
                    record.kind,
                    expected_transition["kind"].as_str().unwrap(),
                    "{name}[{index}]: dispatch {dispatch_index} transition kind"
                );
                let expected_reason = &expected_transition["reason"];
                assert_eq!(
                    record
                        .reason
                        .map(|reason| json!(reason))
                        .unwrap_or(Value::Null),
                    *expected_reason,
                    "{name}[{index}]: dispatch {dispatch_index} reason"
                );
                let actual_semantics: Vec<Value> = record
                    .semantic_events
                    .iter()
                    .map(semantic_event_json)
                    .collect();
                assert_eq!(
                    norm_numbers(&json!(actual_semantics)),
                    norm_numbers(&expected_transition["semanticEvents"]),
                    "{name}[{index}]: dispatch {dispatch_index} semantic events"
                );
                semantic.extend(actual_semantics);
            }
            assert_eq!(
                norm_numbers(&json!(semantic)),
                norm_numbers(&step["semanticEvents"]),
                "{name}[{index}]: step semantic events"
            );
            assert_eq!(
                norm_numbers(&ingress.snapshot_json()),
                norm_numbers(&step["snapshot"]),
                "{name}[{index}]: snapshot mismatch"
            );
        }
    }
}

/// Native bounded-resource delta (accepted): `pending + recent <= 4096`
/// coexists with `pending <= 512`, `hints <= 1024`, cleanup-first admission,
/// no charge on exact replay, and reject-without-mutation at the joint cap.
#[test]
fn join_native_joint_capacity() {
    let mut join = AgentObservationJoinStateMachine::new(JoinOptions::default());
    let adapter = AdapterDefinition {
        adapter_id: "codex".to_string(),
        capabilities: Vec::new(),
        provider_id: "codex".to_string(),
    };

    // hints=1024 independent bound (unmatched hints against a live generation).
    join.register_presentation_generation("desktop:presentation:c9".to_string(), "09".repeat(16));
    for index in 0..1024u64 {
        join.receive_hint(
            JoinHint {
                correlation_id: corpus_correlation(100_000 + index),
                event_id: corpus_event(100_000 + index),
                generation: "09".repeat(16),
                presentation_id: "desktop:presentation:c9".to_string(),
            },
            0.0,
        );
    }
    let before = join.receive_hint(
        JoinHint {
            correlation_id: corpus_correlation(200_000),
            event_id: corpus_event(200_000),
            generation: "09".repeat(16),
            presentation_id: "desktop:presentation:c9".to_string(),
        },
        0.0,
    );
    assert!(before.is_empty(), "1025th pending hint must be refused");
    join.reset();

    // pending=512 independent bound.
    let mut now = 0.0;
    for index in 0..512u64 {
        let admission = join.admit_event(
            adapter.clone(),
            parse_envelope(&test_envelope(index, now)).unwrap(),
            now,
        );
        assert!(admission.accepted, "pending admission {index} must succeed");
    }
    assert!(
        !join
            .admit_event(
                adapter.clone(),
                parse_envelope(&test_envelope(999, now)).unwrap(),
                now
            )
            .accepted,
        "513th pending event must be rejected"
    );
    // Exact replay of a pending key at the cap: accepted, no new slot.
    assert!(
        join.admit_event(
            adapter.clone(),
            parse_envelope(&test_envelope(0, now)).unwrap(),
            now
        )
        .accepted,
        "identical replay must not charge a slot"
    );
    // Unapplied expiry releases slots entirely.
    now += 10_001.0;
    let admission = join.admit_event(
        adapter.clone(),
        parse_envelope(&test_envelope(10_000, now)).unwrap(),
        now,
    );
    assert!(admission.accepted, "cleanup frees unapplied occupancy");
    // Let the probe envelope expire too so the applied batches start clean.
    now += 10_001.0;

    // Fill recent to 3584 (7 applied batches of 512) then pending to 512 →
    // joint occupancy exactly 4096.
    join.register_presentation_generation("desktop:presentation:c1".to_string(), "01".repeat(16));
    let mut event_seq = 0u64;
    for _ in 0..7 {
        for _ in 0..512 {
            event_seq += 1;
            join.receive_hint(
                JoinHint {
                    correlation_id: corpus_correlation(event_seq),
                    event_id: corpus_event(event_seq),
                    generation: "01".repeat(16),
                    presentation_id: "desktop:presentation:c1".to_string(),
                },
                now,
            );
            let admission = join.admit_event(
                adapter.clone(),
                parse_envelope(&test_envelope(event_seq, now)).unwrap(),
                now,
            );
            assert!(admission.accepted);
        }
        now += 10_001.0; // applied events migrate pending→recent
    }
    for _ in 0..512 {
        event_seq += 1;
        let admission = join.admit_event(
            adapter.clone(),
            parse_envelope(&test_envelope(event_seq, now)).unwrap(),
            now,
        );
        assert!(admission.accepted);
    }
    // Joint full: recent 3584 + pending 512 = 4096. New admission rejected;
    // identical replay of a pending key still accepted (no charge).
    event_seq += 1;
    assert!(
        !join
            .admit_event(
                adapter.clone(),
                parse_envelope(&test_envelope(event_seq, now)).unwrap(),
                now
            )
            .accepted,
        "joint 4096 cap must reject a new key"
    );
    let replay_target = event_seq - 1;
    assert!(
        join.admit_event(
            adapter.clone(),
            parse_envelope(&test_envelope(replay_target, now)).unwrap(),
            now
        )
        .accepted,
        "identical pending replay must stay free at the cap"
    );
    // pending cap is independently reachable while recent holds space.
    let mut now_after = now + 10_001.0;
    let admission = join.admit_event(
        adapter.clone(),
        parse_envelope(&test_envelope(20_000, now_after)).unwrap(),
        now_after,
    );
    // The 512 pending expire unapplied (freed); 3584 recent remain.
    assert!(admission.accepted);
    // recent expiry frees slots after its own TTL.
    now_after += 60_001.0;
    assert!(
        join.admit_event(
            adapter,
            parse_envelope(&test_envelope(30_000, now_after)).unwrap(),
            now_after
        )
        .accepted,
        "recent expiry must release the joint slot"
    );
    assert_eq!(MAX_JOINT_ADMISSIONS, 4096);
}

fn test_envelope(sequence: u64, _now: f64) -> Value {
    json!({
        "adapterId": "codex",
        "correlations": [{
            "correlationId": corpus_correlation(sequence),
            "visibility": "hidden",
        }],
        "eventId": corpus_event(sequence),
        "observation": {
            "activity": null,
            "agentKey": null,
            "event": "session-started",
            "operationKey": null,
            "sessionKey": "session-1",
            "turnKey": null,
            "version": 1,
        },
        "version": 1,
    })
}

#[test]
fn kilo_hook_boundary_matches_opencode_with_distinct_provider() {
    use super::super::hook_contract::{HookObservationEvent, resolve_adapter};
    let mut envelope = test_envelope(0, 0.0);
    envelope["adapterId"] = json!("kilo");
    assert!(parse_envelope(&envelope).is_some());
    for event in [
        HookObservationEvent::SessionStarted,
        HookObservationEvent::SessionIdle,
        HookObservationEvent::TurnStarted,
        HookObservationEvent::ToolStarted,
        HookObservationEvent::ToolFinished,
        HookObservationEvent::ToolFailed,
        HookObservationEvent::QuestionRequested,
        HookObservationEvent::ApprovalRequested,
        HookObservationEvent::AttentionResolved,
        HookObservationEvent::TurnFailed,
        HookObservationEvent::SessionEnded,
    ] {
        let kilo = resolve_adapter("kilo", event).unwrap();
        let opencode = resolve_adapter("opencode", event).unwrap();
        assert_eq!(kilo.provider_id, "kilo");
        assert_eq!(kilo.adapter_id, "kilo");
        assert_eq!(kilo.capabilities, opencode.capabilities);
    }
    assert!(resolve_adapter("kilo", HookObservationEvent::SubagentStarted).is_none());
}

#[test]
fn cursor_hook_boundary_accepts_only_session_identity() {
    use super::super::hook_contract::{HookObservationEvent, resolve_adapter};
    let mut envelope = test_envelope(0, 0.0);
    envelope["adapterId"] = json!("cursor");
    assert!(parse_envelope(&envelope).is_some());
    assert_eq!(
        resolve_adapter("cursor", HookObservationEvent::SessionIdentified)
            .unwrap()
            .provider_id,
        "cursor"
    );
    envelope["observation"]["event"] = json!("session-identified");
    assert!(parse_envelope(&envelope).is_some());
    assert_eq!(
        resolve_adapter("cursor", HookObservationEvent::SessionStarted),
        resolve_adapter("cursor", HookObservationEvent::SessionIdentified)
    );
    assert!(resolve_adapter("codex", HookObservationEvent::SessionIdentified).is_none());
    for event in [
        HookObservationEvent::TurnStarted,
        HookObservationEvent::SessionIdle,
        HookObservationEvent::ApprovalRequested,
        HookObservationEvent::SessionEnded,
    ] {
        assert!(resolve_adapter("cursor", event).is_none());
    }
}

#[test]
fn qoder_hook_boundary_accepts_only_identified_session() {
    use super::super::hook_contract::{HookObservationEvent, resolve_adapter};
    for (id, provider) in [
        ("qodercli", "qodercli"),
        ("antigravity-cli", "antigravity"),
        ("droid", "droid"),
        ("letta", "letta"),
        ("devin", "devin"),
        ("hermes", "hermes"),
    ] {
        let mut envelope = test_envelope(0, 0.0);
        envelope["adapterId"] = json!(id);
        envelope["observation"]["event"] = json!("session-identified");
        assert!(parse_envelope(&envelope).is_some());
        let adapter = resolve_adapter(id, HookObservationEvent::SessionIdentified).unwrap();
        assert_eq!(adapter.provider_id, provider);
        assert_eq!(adapter.capabilities, ["sessionLifecycle"]);
        for event in [
            HookObservationEvent::SessionStarted,
            HookObservationEvent::TurnStarted,
            HookObservationEvent::SessionIdle,
            HookObservationEvent::ApprovalRequested,
            HookObservationEvent::SessionEnded,
        ] {
            assert!(resolve_adapter(id, event).is_none());
        }
    }
}

#[test]
fn mastracode_hook_boundary_keeps_a_closed_lifecycle_registry() {
    use super::super::hook_contract::{HookObservationEvent as E, resolve_adapter};
    for event in [
        E::SessionIdentified,
        E::SessionEnded,
        E::TurnStarted,
        E::TurnStopped,
        E::TurnCanceled,
        E::TurnFailed,
        E::ToolStarted,
        E::ToolFinished,
        E::ToolFailed,
        E::ApprovalRequested,
        E::AttentionResolved,
        E::SubagentStarted,
        E::SubagentStopped,
    ] {
        let adapter = resolve_adapter("mastracode", event).unwrap();
        assert_eq!(adapter.provider_id, "mastracode");
        assert_eq!(
            adapter.capabilities,
            [
                "sessionLifecycle",
                "turnLifecycle",
                "activity",
                "approvals",
                "subagents"
            ]
        );
        let mut envelope = test_envelope(0, 0.0);
        envelope["adapterId"] = json!("mastracode");
        envelope["observation"]["event"] = json!(event.as_str());
        assert!(parse_envelope(&envelope).is_some());
    }
    for event in [
        E::SessionStarted,
        E::SessionIdle,
        E::QuestionRequested,
        E::CompactionStarted,
        E::TurnInterrupted,
    ] {
        assert!(resolve_adapter("mastracode", event).is_none());
    }
}

#[test]
fn kimi_hook_boundary_accepts_session_lifecycle_only() {
    use super::super::hook_contract::{HookObservationEvent as E, resolve_adapter};
    for (event, name) in [
        (E::SessionIdentified, "session-identified"),
        (E::SessionEnded, "session-ended"),
    ] {
        let mut envelope = test_envelope(0, 0.0);
        envelope["adapterId"] = json!("kimi");
        envelope["observation"]["event"] = json!(name);
        assert!(parse_envelope(&envelope).is_some());
        let adapter = resolve_adapter("kimi", event).unwrap();
        assert_eq!(adapter.provider_id, "kimi");
        assert_eq!(adapter.capabilities, ["sessionLifecycle"]);
    }
    for event in [
        E::SessionStarted,
        E::SessionIdle,
        E::TurnStarted,
        E::TurnStopped,
        E::ApprovalRequested,
    ] {
        assert!(resolve_adapter("kimi", event).is_none());
    }
}
