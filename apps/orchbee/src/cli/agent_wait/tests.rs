use super::*;

/// A wire-shaped session detail with the caller's freshness and status:
/// the typed decoder the CLI trusts builds the same struct the loop
/// evaluates, so the predicate is tested on the real document shape.
fn detail(freshness: &str, status: serde_json::Value) -> AgentSessionDetail {
    serde_json::from_value(json!({
        "snapshot": {
            "sessionId": "sess-alpha",
            "providerId": "acme.agent",
            "lifecycle": "live",
            "executionLocation": {"executionLocationId": "host:7"},
            "observation": {
                "fidelity": "rich",
                "freshness": freshness,
                "generation": 3,
                "sequence": 11,
                "source": "native"
            },
            "parentSessionId": null,
            "activityHint": null,
            "capabilities": [],
            "lastOutcome": null,
            "pendingRequests": [],
            "presentationAttachments": [],
            "turn": {"kind": "idle"}
        },
        "status": status
    }))
    .expect("fixture decodes as an AgentSessionDetail")
}

/// The canonical predicate: the three freshness conditions match the
/// observation's freshness directly, every status condition requires
/// `fresh` freshness plus an equal status `kind`, and `matchedConditions`
/// preserves the caller's requested order rather than any canonical sort.
#[test]
fn condition_matching_is_freshness_gated_in_requested_order() {
    let fresh_idle = detail("fresh", json!({"kind": "idle"}));
    let stale_idle = detail("stale", json!({"kind": "idle"}));
    let gone_idle = detail("disconnected", json!({"kind": "idle"}));

    // Freshness conditions read the observation directly — no status gate.
    assert!(condition_matches(AgentWaitCondition::Fresh, &fresh_idle));
    assert!(!condition_matches(AgentWaitCondition::Fresh, &stale_idle));
    assert!(condition_matches(AgentWaitCondition::Stale, &stale_idle));
    assert!(!condition_matches(AgentWaitCondition::Stale, &fresh_idle));
    assert!(condition_matches(
        AgentWaitCondition::Disconnected,
        &gone_idle
    ));
    assert!(!condition_matches(
        AgentWaitCondition::Disconnected,
        &fresh_idle
    ));

    // Status conditions require `fresh`: the same `idle` status matches
    // on a fresh observation and fails on stale or disconnected.
    assert!(condition_matches(AgentWaitCondition::Idle, &fresh_idle));
    assert!(!condition_matches(AgentWaitCondition::Idle, &stale_idle));
    assert!(!condition_matches(AgentWaitCondition::Idle, &gone_idle));
    assert!(!condition_matches(AgentWaitCondition::Failed, &fresh_idle));

    // Every status kind answers its canonical condition name.
    for (status, condition) in [
        (
            json!({"kind": "needs-attention", "count": 2}),
            AgentWaitCondition::NeedsAttention,
        ),
        (json!({"kind": "failed"}), AgentWaitCondition::Failed),
        (json!({"kind": "succeeded"}), AgentWaitCondition::Succeeded),
        (json!({"kind": "canceled"}), AgentWaitCondition::Canceled),
        (
            json!({"kind": "interrupted"}),
            AgentWaitCondition::Interrupted,
        ),
        (
            json!({"kind": "working", "activity": null, "phase": "working"}),
            AgentWaitCondition::Working,
        ),
        (json!({"kind": "idle"}), AgentWaitCondition::Idle),
        (
            json!({"kind": "unknown", "activityHint": null}),
            AgentWaitCondition::Unknown,
        ),
        (json!({"kind": "ended"}), AgentWaitCondition::Ended),
    ] {
        let session = detail("fresh", status.clone());
        assert!(
            condition_matches(condition, &session),
            "{status} must match {}",
            condition.as_str()
        );
        let stale_session = detail("stale", status.clone());
        assert!(
            !condition_matches(condition, &stale_session),
            "{status} must not match under stale freshness"
        );
    }

    for kind in ["succeeded", "failed", "canceled", "interrupted"] {
        assert!(condition_matches(
            AgentWaitCondition::Done,
            &detail("fresh", json!({"kind":kind}))
        ));
        assert!(!condition_matches(
            AgentWaitCondition::Done,
            &detail("stale", json!({"kind":kind}))
        ));
    }
    for kind in ["idle", "ended"] {
        assert!(!condition_matches(
            AgentWaitCondition::Done,
            &detail("fresh", json!({"kind":kind}))
        ));
    }
    assert!(condition_matches(
        AgentWaitCondition::Blocked,
        &detail("fresh", json!({"kind":"needs-attention", "count":1}))
    ));
    assert!(!condition_matches(
        AgentWaitCondition::Blocked,
        &detail("stale", json!({"kind":"needs-attention", "count":1}))
    ));

    // A multi-condition request keeps the caller's order: `fresh` and
    // `idle` both hit a fresh idle document, in requested sequence.
    let requested = [
        AgentWaitCondition::Idle,
        AgentWaitCondition::Fresh,
        AgentWaitCondition::Failed,
    ];
    let matched: Vec<_> = requested
        .iter()
        .copied()
        .filter(|condition| condition_matches(*condition, &fresh_idle))
        .collect();
    assert_eq!(
        matched,
        vec![AgentWaitCondition::Idle, AgentWaitCondition::Fresh]
    );
}
