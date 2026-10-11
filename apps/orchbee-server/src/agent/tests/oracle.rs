//! Named regression tests for the eleven second-half boundary observations
//! recorded by executing the frozen TypeScript `packages/agent` sources
//! (provider conformance extra-12). Each case asserts the full recorded fact
//! set of its `Q*` oracle entry: the aggregate attention-subset freshness fold,
//! provider dedup/ordering, equal-status freshness and activity-hint ties,
//! UTF-16 presentation ordering, per-location navigable/freshness/status
//! preference with the non-winning-generation group exclusion, invalid seeded
//! snapshots, the seeded-head empty-journal reset, and double-dispose
//! idempotence.
//!
//! Inputs are canonical-valid raw snapshots except the named invalid seeds in
//! `q9`; expected values are the recorded TypeScript outcomes, not restated
//! Rust-side preferences.

use super::super::AgentObservationEvent;
use super::super::AgentRuntime;
use super::super::AgentSemanticStatus;
use super::super::AgentTransition;
use super::super::changes::{AgentChangeReadInput, AgentRuntimeChangeReadResult};
use super::super::model::{MAX_SAFE_INTEGER, RawAttachment, RawSessionSnapshot, RawTurnState};
use super::super::projection::{
    AgentAggregatePresentation, project_agent_location, project_agent_locations,
    project_agent_presentation_group, project_agent_session_group,
};
use super::super::reducer::AgentTransitionRejection;
use super::super::{
    AgentFreshness, AgentLifecycle, AgentRuntimeSnapshot, AgentSessionSnapshot,
    RawAttentionRequest, RawExecutionLocation, RawObservation, RawPresence,
};

const ALL_CAPS: [&str; 8] = [
    "sessionLifecycle",
    "turnLifecycle",
    "questions",
    "approvals",
    "authentication",
    "activity",
    "subagents",
    "progress",
];

fn caps_full() -> Vec<String> {
    ALL_CAPS.iter().map(|cap| cap.to_string()).collect()
}

fn observation(freshness: &str) -> RawObservation {
    RawObservation {
        fidelity: "rich".to_string(),
        freshness: freshness.to_string(),
        generation: 0.0,
        sequence: 0.0,
        source: "hook".to_string(),
    }
}

fn turn_active(turn_id: &str, activity: &str, phase: &str) -> RawTurnState {
    RawTurnState {
        activity: RawPresence::Value(activity.to_string()),
        kind: "active".to_string(),
        phase: RawPresence::Value(phase.to_string()),
        turn_id: RawPresence::Value(turn_id.to_string()),
    }
}

fn turn_marker(kind: &str) -> RawTurnState {
    RawTurnState {
        activity: RawPresence::Absent,
        kind: kind.to_string(),
        phase: RawPresence::Absent,
        turn_id: RawPresence::Absent,
    }
}

fn attachment(presentation_id: &str, generation: &str, navigable: bool) -> RawAttachment {
    RawAttachment {
        generation: generation.to_string(),
        navigable,
        presentation_id: presentation_id.to_string(),
        visibility: "visible".to_string(),
    }
}

fn request(request_id: &str) -> RawAttentionRequest {
    RawAttentionRequest {
        action_path: "navigate-only".to_string(),
        kind: "question".to_string(),
        request_id: request_id.to_string(),
        turn_id: RawPresence::Null,
    }
}

fn session(session_id: &str) -> RawSessionSnapshot {
    RawSessionSnapshot {
        activity_hint: RawPresence::Null,
        capabilities: caps_full(),
        execution_location: RawExecutionLocation {
            execution_location_id: "exec-1".to_string(),
        },
        last_outcome: RawPresence::Null,
        lifecycle: "live".to_string(),
        observation: observation("fresh"),
        parent_session_id: RawPresence::Null,
        pending_requests: Vec::new(),
        presentation_attachments: Vec::new(),
        provider_id: "claude-code".to_string(),
        session_id: session_id.to_string(),
        turn: turn_marker("unknown"),
    }
}

fn typed(raw: RawSessionSnapshot) -> AgentSessionSnapshot {
    AgentSessionSnapshot::try_from_raw(raw).expect("seeded session must be canonically valid")
}

fn snapshot(revision: u64, sessions: Vec<AgentSessionSnapshot>) -> AgentRuntimeSnapshot {
    AgentRuntimeSnapshot { revision, sessions }
}

fn provider_ids(aggregate: &AgentAggregatePresentation) -> Vec<&str> {
    aggregate
        .provider_ids
        .iter()
        .map(|provider| provider.as_str())
        .collect()
}

fn group(sessions: &[&AgentSessionSnapshot]) -> AgentAggregatePresentation {
    project_agent_session_group(sessions).expect("nonempty session group must project")
}

// ---------- Q1. attention-group freshness fold ----------

#[test]
fn q1_attention_freshness_fold() {
    let mut attn_fresh = session("s-attn-fresh");
    attn_fresh.pending_requests = vec![request("req-a1"), request("req-a2")];
    let mut attn_stale = session("s-attn-stale");
    attn_stale.observation.freshness = "stale".to_string();
    attn_stale.pending_requests = vec![request("req-b1")];
    let mut attn_disc = session("s-attn-disc");
    attn_disc.observation.freshness = "disconnected".to_string();
    attn_disc.pending_requests = vec![request("req-c1")];
    let mut work_fresh = session("s-work-fresh");
    work_fresh.turn = turn_active("t-w", "tool", "working");
    let sessions = [
        typed(attn_fresh),
        typed(attn_stale),
        typed(attn_disc),
        typed(work_fresh),
    ];
    let aggregate = group(&sessions.iter().collect::<Vec<_>>());
    assert_eq!(
        aggregate.status,
        AgentSemanticStatus::NeedsAttention { count: 4 }
    );
    assert_eq!(aggregate.freshness, AgentFreshness::Disconnected);
    assert_eq!(aggregate.attention_count, 4);
    assert_eq!(aggregate.active_count, 1);
    assert_eq!(aggregate.session_count, 4);
    assert_eq!(provider_ids(&aggregate), ["claude-code"]);
}

// ---------- Q2. multi-provider dedup + ordering ----------

#[test]
fn q2_provider_dedup_order() {
    let sessions = ["zeta", "alpha", "zeta", "mid"]
        .iter()
        .enumerate()
        .map(|(index, provider)| {
            let mut raw = session(&format!("s-{}", index + 1));
            raw.provider_id = provider.to_string();
            raw.turn = turn_active(&format!("t-{}", index + 1), "tool", "working");
            typed(raw)
        })
        .collect::<Vec<_>>();
    let aggregate = group(&sessions.iter().collect::<Vec<_>>());
    assert_eq!(provider_ids(&aggregate), ["alpha", "mid", "zeta"]);
    assert_eq!(
        aggregate.status,
        AgentSemanticStatus::Working {
            activity: Some("tool".to_string()),
            phase: "working".to_string(),
        }
    );
    assert_eq!(aggregate.freshness, AgentFreshness::Fresh);
    assert_eq!(aggregate.active_count, 4);
    assert_eq!(aggregate.attention_count, 0);
    assert_eq!(aggregate.session_count, 4);
}

// ---------- Q3. equal-status freshness tie ----------

#[test]
fn q3_equal_status_freshness_tie() {
    let mut stale_work = session("s-stale-work");
    stale_work.observation.freshness = "stale".to_string();
    stale_work.turn = turn_active("t-s", "tool", "working");
    let mut fresh_work = session("s-fresh-work");
    fresh_work.turn = turn_active("t-f", "editing", "working");
    let sessions = [typed(stale_work), typed(fresh_work)];
    let aggregate = group(&sessions.iter().collect::<Vec<_>>());
    assert_eq!(
        aggregate.status,
        AgentSemanticStatus::Working {
            activity: Some("editing".to_string()),
            phase: "working".to_string(),
        }
    );
    assert_eq!(aggregate.freshness, AgentFreshness::Fresh);
    assert_eq!(aggregate.active_count, 2);
    assert_eq!(aggregate.session_count, 2);
    assert_eq!(provider_ids(&aggregate), ["claude-code"]);
}

// ---------- Q4. unknown activityHint presence tie ----------

#[test]
fn q4_unknown_activity_hint_tie() {
    let nohint = session("s-nohint");
    let mut hint = session("s-hint");
    hint.activity_hint = RawPresence::Value("tool".to_string());
    hint.capabilities = vec!["activity".to_string()];
    hint.observation = RawObservation {
        fidelity: "basic".to_string(),
        freshness: "fresh".to_string(),
        generation: 0.0,
        sequence: 0.0,
        source: "terminal".to_string(),
    };
    let sessions = [typed(nohint), typed(hint)];
    let aggregate = group(&sessions.iter().collect::<Vec<_>>());
    assert_eq!(
        aggregate.status,
        AgentSemanticStatus::Unknown {
            activity_hint: Some("tool".to_string()),
        }
    );
    assert_eq!(aggregate.freshness, AgentFreshness::Fresh);
    assert_eq!(aggregate.active_count, 0);
    assert_eq!(aggregate.session_count, 2);
    assert_eq!(provider_ids(&aggregate), ["claude-code"]);
}

// ---------- Q5. UTF-16 presentationId ordering ----------

#[test]
fn q5_utf16_presentation_order() {
    let mut bmp = session("s-bmp");
    bmp.presentation_attachments = vec![attachment("\u{e000}", "g1", true)];
    let mut astral = session("s-astral");
    astral.presentation_attachments = vec![attachment("\u{1f600}", "g1", true)];
    let snapshot = snapshot(0, vec![typed(bmp), typed(astral)]);
    let locations = project_agent_locations(&snapshot);
    let order: Vec<&str> = locations
        .iter()
        .map(|location| location.location.presentation_id.as_str())
        .collect();
    assert_eq!(order, ["\u{1f600}", "\u{e000}"]);
    for location in &locations {
        assert_eq!(location.location.generation.as_str(), "g1");
        assert!(location.location.navigable);
        assert_eq!(location.aggregate.session_count, 1);
    }
}

// ---------- Q6. location preference: navigable arm ----------

#[test]
fn q6_location_navigable_preference() {
    let mut nav_stale = session("s-nav-stale");
    nav_stale.observation.freshness = "stale".to_string();
    nav_stale.presentation_attachments = vec![attachment("p", "g1", true)];
    let mut nonnav_fresh = session("s-nonnav-fresh");
    nonnav_fresh.presentation_attachments = vec![attachment("p", "g2", false)];
    let snapshot = snapshot(0, vec![typed(nav_stale), typed(nonnav_fresh)]);
    let location = project_agent_location(&snapshot, "p").expect("location p must project");
    assert!(location.location.navigable);
    assert_eq!(location.location.generation.as_str(), "g1");
    assert_eq!(location.location.presentation_id.as_str(), "p");
    assert_eq!(location.location.visibility.as_str(), "visible");
    assert_eq!(location.aggregate.freshness, AgentFreshness::Stale);
    assert_eq!(
        location.aggregate.status,
        AgentSemanticStatus::Unknown {
            activity_hint: None,
        }
    );
    assert_eq!(location.aggregate.session_count, 1);
    assert_eq!(location.aggregate.active_count, 0);
    assert_eq!(provider_ids(&location.aggregate), ["claude-code"]);
}

// ---------- Q7. location preference: freshness arm ----------

#[test]
fn q7_location_freshness_preference() {
    let mut nav_stale = session("s-nav-stale");
    nav_stale.observation.freshness = "stale".to_string();
    nav_stale.presentation_attachments = vec![attachment("p", "g1", true)];
    let mut nav_fresh = session("s-nav-fresh");
    nav_fresh.presentation_attachments = vec![attachment("p", "g2", true)];
    let snapshot = snapshot(0, vec![typed(nav_stale), typed(nav_fresh)]);
    let location = project_agent_location(&snapshot, "p").expect("location p must project");
    assert!(location.location.navigable);
    assert_eq!(location.location.generation.as_str(), "g2");
    assert_eq!(location.aggregate.freshness, AgentFreshness::Fresh);
    assert_eq!(
        location.aggregate.status,
        AgentSemanticStatus::Unknown {
            activity_hint: None,
        }
    );
    assert_eq!(location.aggregate.session_count, 1);
}

// ---------- Q8. location status arm + presentation-group exclusion ----------

#[test]
fn q8_location_status_and_group_exclusion() {
    let mut idle_g1 = session("s-idle-g1");
    idle_g1.provider_id = "alpha".to_string();
    idle_g1.turn = turn_marker("idle");
    idle_g1.presentation_attachments = vec![attachment("p", "g1", true)];
    let mut work_g2 = session("s-work-g2");
    work_g2.provider_id = "beta".to_string();
    work_g2.turn = turn_active("t-g2", "tool", "working");
    work_g2.presentation_attachments = vec![attachment("p", "g2", true)];
    let snapshot = snapshot(0, vec![typed(idle_g1), typed(work_g2)]);
    let location = project_agent_location(&snapshot, "p").expect("location p must project");
    assert_eq!(location.location.generation.as_str(), "g2");
    assert_eq!(
        location.aggregate.status,
        AgentSemanticStatus::Working {
            activity: Some("tool".to_string()),
            phase: "working".to_string(),
        }
    );
    assert_eq!(location.aggregate.session_count, 1);
    assert_eq!(location.aggregate.active_count, 1);
    assert_eq!(provider_ids(&location.aggregate), ["beta"]);
    let group = project_agent_presentation_group(&snapshot, &["p".to_string()])
        .expect("presentation group must project");
    assert_eq!(group.session_count, 1);
    assert_eq!(provider_ids(&group), ["beta"]);
    assert_eq!(
        group.status,
        AgentSemanticStatus::Working {
            activity: Some("tool".to_string()),
            phase: "working".to_string(),
        }
    );
    assert_eq!(group.freshness, AgentFreshness::Fresh);
    assert_eq!(group.active_count, 1);
    assert_eq!(group.attention_count, 0);
}

// ---------- Q9. invalid seeded snapshots rejected ----------

#[test]
fn q9_invalid_seed_rejected() {
    let over_revision = AgentRuntime::from_snapshot(snapshot(MAX_SAFE_INTEGER + 1, Vec::new()))
        .err()
        .expect("revision beyond MAX_SAFE_INTEGER must reject");
    assert_eq!(
        over_revision.0,
        "Agent runtime revision exceeds safe integer"
    );
    let duplicate = AgentRuntime::from_snapshot(snapshot(
        0,
        vec![typed(session("s-dup")), typed(session("s-dup"))],
    ))
    .err()
    .expect("duplicate session identities must reject");
    assert_eq!(duplicate.0, "Agent Session identities must be unique");
}

// ---------- Q10. seeded reset + double dispose ----------

#[test]
fn q10a_seeded_empty_journal_reset() {
    // A seeded head with nothing retained forces re-baseline — distinct from
    // the rev-0 empty runtime, which answers `current`.
    let runtime = AgentRuntime::from_snapshot(snapshot(5, vec![typed(session("s-seed"))]))
        .expect("seeded runtime must be valid");
    let read = runtime
        .read_changes(AgentChangeReadInput {
            after_revision: 0.0,
            limit: 100.0,
        })
        .expect("well-formed read input");
    match read {
        AgentRuntimeChangeReadResult::ResetRequired {
            observed_through_revision,
            runtime_revision,
        } => {
            assert_eq!(observed_through_revision, 5);
            assert_eq!(runtime_revision, 5);
        }
        other => panic!("expected reset-required, got {other:?}"),
    }
}

#[test]
fn q10b_double_dispose_idempotent() {
    let mut runtime = AgentRuntime::new();
    let first = runtime.dispatch(&AgentObservationEvent::SessionObserved {
        capabilities: caps_full(),
        event_id: "evt-x1".to_string(),
        execution_location: RawExecutionLocation {
            execution_location_id: "exec-1".to_string(),
        },
        observation: observation("fresh"),
        parent_session_id: RawPresence::Null,
        presentation_attachments: Vec::new(),
        provider_id: "claude-code".to_string(),
        session_id: "s-d".to_string(),
    });
    assert_eq!(first.kind(), "applied");
    assert_eq!(first.snapshot().revision, 1);
    assert!(!runtime.is_disposed());
    runtime.dispose();
    runtime.dispose();
    assert!(runtime.is_disposed());
    let after = runtime.dispatch(&AgentObservationEvent::SessionIdleObserved {
        event_id: "evt-x2".to_string(),
        generation: 0.0,
        sequence: 1.0,
        session_id: "s-d".to_string(),
    });
    match &after {
        AgentTransition::Rejected {
            reason: AgentTransitionRejection::RuntimeDisposed,
            snapshot,
        } => assert_eq!(snapshot.revision, 1),
        other => panic!("expected rejected(runtime-disposed), got {}", other.kind()),
    }
    let read = runtime
        .read_changes(AgentChangeReadInput {
            after_revision: 0.0,
            limit: 100.0,
        })
        .expect("well-formed read input");
    match read {
        AgentRuntimeChangeReadResult::Changes {
            changes,
            has_more,
            observed_through_revision,
            runtime_revision,
        } => {
            assert!(!has_more);
            assert_eq!(observed_through_revision, 1);
            assert_eq!(runtime_revision, 1);
            assert_eq!(changes.len(), 1);
            let change = &changes[0];
            assert_eq!(change.revision, 1);
            assert_eq!(change.transition, "session-observed");
            assert_eq!(change.session.session_id.as_str(), "s-d");
            assert_eq!(change.session.lifecycle, AgentLifecycle::Live);
            assert_eq!(change.session.provider_id.as_str(), "claude-code");
            assert_eq!(change.session.pending_request_count, 0);
            assert_eq!(
                change.session.status,
                AgentSemanticStatus::Unknown {
                    activity_hint: None,
                }
            );
        }
        other => panic!("expected changes, got {other:?}"),
    }
    assert_eq!(runtime.snapshot().revision, 1);
    assert_eq!(runtime.snapshot().sessions.len(), 1);
}
