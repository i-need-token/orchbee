use super::*;

/// The fallback event ID is a call-time default: it advances the allocator
/// before the session lookup, matching the frozen translator evaluation order.
#[test]
fn dispatch_for_missing_session_event_id_allocation_parity() {
    let mut ingress =
        AgentHookObservationIngress::new(Some("fixture.sock".to_string()), JoinOptions::default());
    let mut runtime = AgentRuntime::new();
    // absent session + implicit id: default evaluated → burns `event:1`,
    // rejected, nothing dispatched or logged.
    let rejected = ingress.dispatch_for(
        &mut runtime,
        "desktop:agent:session:absent",
        SessionEventInput::SessionIdleObserved,
        None,
    );
    assert!(matches!(
        rejected,
        AgentTransition::Rejected {
            reason: AgentTransitionRejection::SessionNotFound,
            ..
        }
    ));
    // absent session + explicit id: the default is never evaluated → no burn.
    let rejected = ingress.dispatch_for(
        &mut runtime,
        "desktop:agent:session:absent",
        SessionEventInput::SessionIdleObserved,
        Some("desktop:agent:hook:codex:explicit".to_string()),
    );
    assert!(matches!(
        rejected,
        AgentTransition::Rejected {
            reason: AgentTransitionRejection::SessionNotFound,
            ..
        }
    ));
    // Following valid allocation must yield `event:2` — exactly one burn.
    assert_eq!(ingress.allocate_event_id(), "desktop:agent:event:2");
    // Missing-session rejections never reach dispatch or the trace log.
    assert!(ingress.take_dispatch_log().is_empty());
}

/// Production clock-adapter coverage: `sync`/`next_settlement_at`/
/// `tick` keep the corpus' two clocks separated exactly like
/// `advance_settlement` — scheduling lands on the synced scheduler base,
/// the injected `now` stays semantic. A settlement timer exists only
/// for a PARTIAL correlation match (join.rs :529-541): the fixture
/// seeds correlation-1's hint, then the envelope carries a second,
/// unmatched correlation — a fully matched single correlation applies
/// inside admit and schedules nothing.
#[test]
fn settlement_schedule_uses_the_synced_clock_base() {
    let mut runtime = AgentRuntime::new();
    let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
    seed_clock_adapter_hint(&mut ingress, &mut runtime, 0.0);
    // The hint alone produces no timer — only a partial match does.
    assert_eq!(ingress.next_settlement_at(), None);
    ingress.sync_settlement_clock(1_000.0);
    assert!(
        ingress
            .admit_envelope(&mut runtime, &clock_adapter_envelope_body(), 1_000.0)
            .is_some()
    );
    // fire_at = synced scheduler base + settle delay, not 25ms.
    assert_eq!(ingress.next_settlement_at(), Some(1_025.0));
}

#[test]
fn sync_settlement_clock_never_moves_backward() {
    let mut runtime = AgentRuntime::new();
    let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
    seed_clock_adapter_hint(&mut ingress, &mut runtime, 0.0);
    ingress.sync_settlement_clock(2_000.0);
    ingress.sync_settlement_clock(500.0);
    assert!(
        ingress
            .admit_envelope(&mut runtime, &clock_adapter_envelope_body(), 500.0)
            .is_some()
    );
    // The stale sync is clamped: the scheduler stays at 2000, so the
    // timer lands at 2025 — never at 525.
    assert_eq!(ingress.next_settlement_at(), Some(2_025.0));
}

#[test]
fn tick_fires_only_due_timers_and_applies_the_partial_match() {
    let mut runtime = AgentRuntime::new();
    let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
    seed_clock_adapter_hint(&mut ingress, &mut runtime, 0.0);
    ingress.sync_settlement_clock(0.0);
    assert!(
        ingress
            .admit_envelope(&mut runtime, &clock_adapter_envelope_body(), 0.0)
            .is_some()
    );
    ingress.tick(&mut runtime, 24.0);
    assert_eq!(ingress.next_settlement_at(), Some(25.0));
    assert!(runtime.snapshot().sessions.is_empty());
    ingress.tick(&mut runtime, 25.0);
    assert_eq!(ingress.next_settlement_at(), None);
    // The partial match applied at settle — the matched correlation's
    // attachment carried the event into canonical state.
    assert_eq!(runtime.snapshot().sessions.len(), 1);
}

fn seed_clock_adapter_hint(
    ingress: &mut AgentHookObservationIngress,
    runtime: &mut AgentRuntime,
    now: f64,
) {
    let generation = format!("{:032x}", 1u8);
    ingress.register_presentation_binding(
        runtime,
        "desktop:presentation:c1".to_owned(),
        generation.clone(),
    );
    ingress.observe_terminal_application_signal(
        runtime,
        "{\"correlationId\":\"correlation-0000000000000001\",\
         \"eventId\":\"event-0000000000000001\",\"kind\":\"correlation\",\"version\":1}",
        generation,
        "desktop:presentation:c1".to_owned(),
        now,
    );
}

fn clock_adapter_envelope_body() -> Vec<u8> {
    serde_json::json!({
        "adapterId": "codex",
        "correlations": [
            {"correlationId": "correlation-0000000000000001", "visibility": "visible"},
            {"correlationId": "correlation-0000000000000002", "visibility": "visible"},
        ],
        "eventId": "event-0000000000000001",
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
    .to_string()
    .into_bytes()
}

/// Real joined delivery: a session keeps the first run token its hooks carried, a tokenless
/// envelope from an older integration still applies, and a malformed token is refused whole.
#[test]
fn joined_hooks_latch_the_first_run_token_and_admit_tokenless_envelopes() {
    let mut runtime = AgentRuntime::new();
    let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
    let generation = format!("{:032x}", 1u8);
    ingress.register_presentation_binding(
        &mut runtime,
        "desktop:presentation:c1".to_owned(),
        generation.clone(),
    );
    let mut deliver = |sequence: u64, session: &str, token: Option<&str>| {
        ingress.observe_terminal_application_signal(
            &mut runtime,
            &format!(
                "{{\"correlationId\":\"correlation-{sequence:016}\",\
                 \"eventId\":\"event-{sequence:016}\",\"kind\":\"correlation\",\"version\":1}}"
            ),
            generation.clone(),
            "desktop:presentation:c1".to_owned(),
            0.0,
        );
        let mut envelope: Value = serde_json::from_slice(&clock_adapter_envelope_body()).unwrap();
        envelope["correlations"] = serde_json::json!([
            {"correlationId": format!("correlation-{sequence:016}"), "visibility": "visible"}
        ]);
        envelope["eventId"] = Value::from(format!("event-{sequence:016}"));
        envelope["observation"]["sessionKey"] = Value::from(session);
        if let Some(token) = token {
            envelope["runToken"] = Value::from(token);
        }
        let accepted = ingress
            .admit_envelope(&mut runtime, &serde_json::to_vec(&envelope).unwrap(), 0.0)
            .is_some();
        let tokens: Vec<(String, String)> = ingress
            .run_tokens()
            .map(|(session, token)| (session.to_owned(), token.as_str().to_owned()))
            .collect();
        (accepted, tokens)
    };
    let first = "a".repeat(32);
    let bound = vec![("desktop:agent:session:2".to_owned(), first.clone())];
    assert_eq!(deliver(1, "session-1", Some(&first)), (true, bound.clone()));
    assert_eq!(
        deliver(2, "session-1", Some(&"b".repeat(32))),
        (true, bound.clone())
    );
    assert_eq!(deliver(3, "session-2", None), (true, bound.clone()));
    assert_eq!(
        deliver(4, "session-3", Some(&"0".repeat(32))),
        (false, bound.clone())
    );
    assert_eq!(
        deliver(5, "session-3", Some("A".repeat(32).as_str())),
        (false, bound)
    );
    assert_eq!(runtime.snapshot().sessions.len(), 2);
}

#[test]
fn cursor_identity_join_does_not_infer_idle_for_current_or_legacy_payload() {
    for (adapter, event) in [
        ("cursor", "session-identified"),
        ("cursor", "session-started"),
        ("qodercli", "session-identified"),
        ("antigravity-cli", "session-identified"),
        ("droid", "session-identified"),
        ("kimi", "session-identified"),
        ("letta", "session-identified"),
        ("devin", "session-identified"),
        ("hermes", "session-identified"),
    ] {
        let mut runtime = AgentRuntime::new();
        let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
        seed_clock_adapter_hint(&mut ingress, &mut runtime, 0.0);
        let mut envelope: Value = serde_json::from_slice(&clock_adapter_envelope_body()).unwrap();
        envelope["adapterId"] = Value::from(adapter);
        envelope["observation"]["event"] = Value::from(event);
        envelope["correlations"].as_array_mut().unwrap().truncate(1);
        let body = serde_json::to_vec(&envelope).unwrap();
        assert!(ingress.admit_envelope(&mut runtime, &body, 0.0).is_some());
        let snapshot = ingress.snapshot_json(&runtime);
        assert_eq!(snapshot["sessions"][0]["turn"]["kind"], "unknown");
        assert_eq!(runtime.snapshot().sessions.len(), 1);
        assert_eq!(
            runtime.snapshot().sessions[0]
                .presentation_attachments
                .len(),
            1
        );
        assert!(ingress.admit_envelope(&mut runtime, &body, 0.0).is_some());
        assert_eq!(ingress.snapshot_json(&runtime), snapshot);
        envelope["eventId"] = Value::from("event-0000000000000002");
        envelope["correlations"][0]["correlationId"] = Value::from("correlation-0000000000000002");
        assert!(
            ingress
                .admit_envelope(&mut runtime, &serde_json::to_vec(&envelope).unwrap(), 0.0)
                .is_some()
        );
        ingress.observe_terminal_application_signal(
            &mut runtime,
            r#"{"correlationId":"correlation-0000000000000002","eventId":"event-0000000000000002","kind":"correlation","version":1}"#,
            format!("{:032x}", 1u8),
            "desktop:presentation:c1".to_owned(),
            0.0,
        );
        assert_eq!(ingress.snapshot_json(&runtime), snapshot);
    }
}

// Translator-only seam: capabilities are synthetic. Public Cursor admission
// continues to reject turn/request events; the test does not widen that gate.
#[test]
fn identity_translation_preserves_active_blocked_and_completed_state() {
    use super::super::hook_contract::{HookObservation, resolve_adapter};
    use HookObservationEvent as Event;
    for identity in [Event::SessionIdentified, Event::SessionStarted] {
        let mut runtime = AgentRuntime::new();
        let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
        let mut adapter = resolve_adapter("codex", Event::TurnStarted).unwrap();
        adapter.adapter_id = "cursor".to_owned();
        adapter.provider_id = "cursor".to_owned();
        let observation = |event| HookObservation {
            activity: None,
            agent_key: None,
            event,
            operation_key: Some("approval-1".to_owned()),
            session_key: "identity-session".to_owned(),
            turn_key: Some("turn-1".to_owned()),
        };
        for (step, (event, kind)) in [
            (Event::TurnStarted, "active"),
            (Event::ApprovalRequested, "active"),
            (Event::TurnStopped, "idle"),
        ]
        .into_iter()
        .enumerate()
        {
            ingress.observe(
                &mut runtime,
                adapter.clone(),
                observation(event),
                &format!("step-{step}"),
                vec![],
                None,
            );
            let before = ingress.snapshot_json(&runtime);
            assert_eq!(before["sessions"][0]["turn"]["kind"], kind);
            if event == Event::ApprovalRequested {
                assert_eq!(
                    before["sessions"][0]["pendingRequests"]
                        .as_array()
                        .unwrap()
                        .len(),
                    1
                );
            }
            if event == Event::TurnStopped {
                assert_eq!(before["sessions"][0]["lastOutcome"]["kind"], "succeeded");
            }
            ingress.observe(
                &mut runtime,
                adapter.clone(),
                observation(identity),
                &format!("identity-{step}"),
                vec![],
                None,
            );
            assert_eq!(ingress.snapshot_json(&runtime), before);
        }
    }
}

#[test]
fn mastracode_joined_lifecycle_preserves_run_and_attention() {
    use serde_json::json;
    let mut runtime = AgentRuntime::new();
    let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
    seed_clock_adapter_hint(&mut ingress, &mut runtime, 0.0);
    for (step, (event, kind, requests)) in [
        ("session-identified", "unknown", 0),
        ("turn-started", "active", 0),
        ("approval-requested", "active", 1),
        ("session-identified", "active", 1),
        ("attention-resolved", "active", 0),
        ("turn-stopped", "idle", 0),
    ]
    .into_iter()
    .enumerate()
    {
        let event_id = format!("event-{:016}", step + 1);
        let correlation_id = format!("correlation-{:016}", step + 1);
        let mut envelope: Value = serde_json::from_slice(&clock_adapter_envelope_body()).unwrap();
        envelope["adapterId"] = json!("mastracode");
        envelope["eventId"] = json!(event_id);
        envelope["correlations"] = json!([{"correlationId":correlation_id,"visibility":"visible"}]);
        envelope["observation"]["event"] = json!(event);
        envelope["observation"]["turnKey"] = json!("run-1");
        envelope["observation"]["operationKey"] = json!("call-1");
        ingress.observe_terminal_application_signal(&mut runtime,
            &json!({"correlationId":correlation_id,"eventId":event_id,"kind":"correlation","version":1}).to_string(),
            format!("{:032x}",1u8),"desktop:presentation:c1".to_owned(),0.0);
        assert!(
            ingress
                .admit_envelope(&mut runtime, &serde_json::to_vec(&envelope).unwrap(), 0.0)
                .is_some()
        );
        let snapshot = ingress.snapshot_json(&runtime);
        let session = &snapshot["sessions"][0];
        assert_eq!(session["turn"]["kind"], kind, "{snapshot}");
        assert_eq!(
            session["pendingRequests"].as_array().unwrap().len(),
            requests
        );
        if event == "turn-stopped" {
            assert_eq!(session["lastOutcome"]["kind"], "succeeded");
        }
        assert_eq!(
            runtime.snapshot().sessions[0]
                .presentation_attachments
                .len(),
            1
        );
    }
}
