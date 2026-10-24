//! Named B2 boundary tests plus the 39 supplemental executed observations
//! (provider conformance extra-05/06). Each case names the recorded TypeScript
//! outcome it asserts; inputs are built directly on the raw event mirrors so
//! every reducer guard stays reachable.
//!
//! `P2`/`E3`-class cases assert a panic: the TypeScript `applied()`/`ZodError`
//! throw maps to `replace_session`'s panic, not to a transition rejection.

use super::super::AgentObservationEvent;
use super::super::AgentRuntime;
use super::super::AgentTransition;
use super::super::changes::AgentChangeReadInput;
use super::super::model::{
    MAX_SAFE_INTEGER, RawAttachment, RawSessionSnapshot, RawTurnOutcome, RawTurnState,
};
use super::super::projection::{
    project_agent_location, project_agent_locations, project_agent_presentation_group,
};
use super::super::reducer::AgentTransitionRejection;
use super::super::{
    AgentRuntimeSnapshot, AgentSessionSnapshot, RawAttentionRequest, RawExecutionLocation,
    RawObservation, RawPresence,
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

fn base_observation() -> RawObservation {
    RawObservation {
        fidelity: "rich".to_string(),
        freshness: "fresh".to_string(),
        generation: 0.0,
        sequence: 0.0,
        source: "hook".to_string(),
    }
}

fn observe(session_id: &str) -> AgentObservationEvent {
    observe_over(session_id, |_| {})
}

fn observe_over(
    session_id: &str,
    over: impl FnOnce(&mut ObservedOverrides),
) -> AgentObservationEvent {
    let mut overrides = ObservedOverrides {
        capabilities: caps_full(),
        execution_location_id: "exec-1".to_string(),
        observation: base_observation(),
        parent_session_id: None,
        presentation_attachments: Vec::new(),
        provider_id: "claude-code".to_string(),
    };
    over(&mut overrides);
    AgentObservationEvent::SessionObserved {
        capabilities: overrides.capabilities,
        event_id: "evt-t".to_string(),
        execution_location: RawExecutionLocation {
            execution_location_id: overrides.execution_location_id,
        },
        observation: overrides.observation,
        parent_session_id: match overrides.parent_session_id {
            Some(parent) => RawPresence::Value(parent),
            None => RawPresence::Null,
        },
        presentation_attachments: overrides.presentation_attachments,
        provider_id: overrides.provider_id,
        session_id: session_id.to_string(),
    }
}

struct ObservedOverrides {
    capabilities: Vec<String>,
    execution_location_id: String,
    observation: RawObservation,
    parent_session_id: Option<String>,
    presentation_attachments: Vec<RawAttachment>,
    provider_id: String,
}

fn sess_ev(session_id: &str, sequence: f64) -> SessEv {
    SessEv {
        event_id: "evt-t".to_string(),
        generation: 0.0,
        sequence,
        session_id: session_id.to_string(),
    }
}

struct SessEv {
    event_id: String,
    generation: f64,
    sequence: f64,
    session_id: String,
}

impl SessEv {
    fn generation(mut self, generation: f64) -> Self {
        self.generation = generation;
        self
    }

    fn turn_started(
        &self,
        turn_id: &str,
        activity: Option<&str>,
        phase: &str,
    ) -> AgentObservationEvent {
        AgentObservationEvent::TurnStarted {
            activity: activity.map_or(RawPresence::Null, |a| RawPresence::Value(a.to_string())),
            event_id: self.event_id.clone(),
            generation: self.generation,
            phase: phase.to_string(),
            sequence: self.sequence,
            session_id: self.session_id.clone(),
            turn_id: turn_id.to_string(),
        }
    }

    fn turn_activity_changed(
        &self,
        turn_id: &str,
        activity: Option<&str>,
        phase: &str,
    ) -> AgentObservationEvent {
        AgentObservationEvent::TurnActivityChanged {
            activity: activity.map_or(RawPresence::Null, |a| RawPresence::Value(a.to_string())),
            event_id: self.event_id.clone(),
            generation: self.generation,
            phase: phase.to_string(),
            sequence: self.sequence,
            session_id: self.session_id.clone(),
            turn_id: turn_id.to_string(),
        }
    }

    fn attention_requested(&self, request: RawAttentionRequest) -> AgentObservationEvent {
        AgentObservationEvent::AttentionRequested {
            event_id: self.event_id.clone(),
            generation: self.generation,
            request,
            sequence: self.sequence,
            session_id: self.session_id.clone(),
        }
    }

    fn idle(&self) -> AgentObservationEvent {
        AgentObservationEvent::SessionIdleObserved {
            event_id: self.event_id.clone(),
            generation: self.generation,
            sequence: self.sequence,
            session_id: self.session_id.clone(),
        }
    }

    fn stale(&self) -> AgentObservationEvent {
        AgentObservationEvent::ObservationBecameStale {
            event_id: self.event_id.clone(),
            generation: self.generation,
            sequence: self.sequence,
            session_id: self.session_id.clone(),
        }
    }

    fn ended(&self) -> AgentObservationEvent {
        AgentObservationEvent::SessionEnded {
            event_id: self.event_id.clone(),
            generation: self.generation,
            sequence: self.sequence,
            session_id: self.session_id.clone(),
        }
    }

    fn attach(&self, attachment: RawAttachment) -> AgentObservationEvent {
        AgentObservationEvent::PresentationAttached {
            attachment,
            event_id: self.event_id.clone(),
            generation: self.generation,
            sequence: self.sequence,
            session_id: self.session_id.clone(),
        }
    }

    fn visibility(
        &self,
        presentation_id: &str,
        generation: &str,
        visibility: &str,
    ) -> AgentObservationEvent {
        AgentObservationEvent::PresentationVisibilityChanged {
            event_id: self.event_id.clone(),
            generation: self.generation,
            presentation_generation: generation.to_string(),
            presentation_id: presentation_id.to_string(),
            sequence: self.sequence,
            session_id: self.session_id.clone(),
            visibility: visibility.to_string(),
        }
    }

    fn detach(&self, presentation_id: &str, generation: &str) -> AgentObservationEvent {
        AgentObservationEvent::PresentationDetached {
            event_id: self.event_id.clone(),
            generation: self.generation,
            presentation_generation: generation.to_string(),
            presentation_id: presentation_id.to_string(),
            sequence: self.sequence,
            session_id: self.session_id.clone(),
        }
    }

    fn reconnect(&self, over: impl FnOnce(&mut ReconnectOverrides)) -> AgentObservationEvent {
        let mut overrides = ReconnectOverrides {
            activity_hint: RawPresence::Null,
            capabilities: caps_full(),
            last_outcome: RawPresence::Null,
            pending_requests: Vec::new(),
            turn: RawTurnState {
                activity: RawPresence::Absent,
                kind: "unknown".to_string(),
                phase: RawPresence::Absent,
                turn_id: RawPresence::Absent,
            },
        };
        over(&mut overrides);
        AgentObservationEvent::ObservationReconnected {
            activity_hint: overrides.activity_hint,
            capabilities: overrides.capabilities,
            event_id: self.event_id.clone(),
            generation: self.generation,
            last_outcome: overrides.last_outcome,
            pending_requests: overrides.pending_requests,
            sequence: self.sequence,
            session_id: self.session_id.clone(),
            turn: overrides.turn,
        }
    }
}

struct ReconnectOverrides {
    activity_hint: RawPresence<String>,
    capabilities: Vec<String>,
    last_outcome: RawPresence<RawTurnOutcome>,
    pending_requests: Vec<RawAttentionRequest>,
    turn: RawTurnState,
}

fn attach(presentation_id: &str, generation: &str) -> RawAttachment {
    RawAttachment {
        generation: generation.to_string(),
        navigable: true,
        presentation_id: presentation_id.to_string(),
        visibility: "visible".to_string(),
    }
}

fn request(request_id: &str, kind: &str, turn_id: Option<&str>) -> RawAttentionRequest {
    RawAttentionRequest {
        action_path: "navigate-only".to_string(),
        kind: kind.to_string(),
        request_id: request_id.to_string(),
        turn_id: turn_id.map_or(RawPresence::Null, |id| RawPresence::Value(id.to_string())),
    }
}

fn make_session(session_id: &str) -> RawSessionSnapshot {
    RawSessionSnapshot {
        activity_hint: RawPresence::Null,
        capabilities: caps_full(),
        execution_location: RawExecutionLocation {
            execution_location_id: "exec-1".to_string(),
        },
        last_outcome: RawPresence::Null,
        lifecycle: "live".to_string(),
        observation: base_observation(),
        parent_session_id: RawPresence::Null,
        pending_requests: Vec::new(),
        presentation_attachments: Vec::new(),
        provider_id: "claude-code".to_string(),
        session_id: session_id.to_string(),
        turn: RawTurnState {
            activity: RawPresence::Absent,
            kind: "unknown".to_string(),
            phase: RawPresence::Absent,
            turn_id: RawPresence::Absent,
        },
    }
}

fn seed_runtime(revision: u64, sessions: Vec<RawSessionSnapshot>) -> AgentRuntime {
    let sessions: Vec<AgentSessionSnapshot> = sessions
        .into_iter()
        .map(|raw| AgentSessionSnapshot::try_from_raw(raw).expect("seeded session must be valid"))
        .collect();
    AgentRuntime::from_snapshot(AgentRuntimeSnapshot { revision, sessions })
        .expect("seeded runtime must be valid")
}

fn assert_kind(transition: &AgentTransition, kind: &str) {
    assert_eq!(transition.kind(), kind);
}

fn assert_rejected(transition: &AgentTransition, reason: AgentTransitionRejection) {
    match transition {
        AgentTransition::Rejected { reason: actual, .. } => assert_eq!(*actual, reason),
        other => panic!("expected rejected({reason:?}), got {}", other.kind()),
    }
}

fn assert_semantic_event_types(transition: &AgentTransition, expected: &[&str]) {
    let actual: Vec<&str> = transition
        .semantic_events()
        .iter()
        .map(|event| match event {
            super::super::events::AgentSemanticEvent::AttentionRequested { .. } => {
                "attention-requested"
            }
            super::super::events::AgentSemanticEvent::AttentionResolved { .. } => {
                "attention-resolved"
            }
            super::super::events::AgentSemanticEvent::TurnOutcome { .. } => "turn-outcome",
            super::super::events::AgentSemanticEvent::SessionEnded { .. } => "session-ended",
        })
        .collect();
    assert_eq!(actual, expected);
}

// ---------- A. request-conflict vs identical re-attention ----------

#[test]
fn a1_identical_reattention_is_unchanged() {
    let mut runtime = AgentRuntime::new();
    assert_kind(&runtime.dispatch(&observe("s-a")), "applied");
    assert_kind(
        &runtime.dispatch(&sess_ev("s-a", 1.0).turn_started("turn-1", Some("tool"), "working")),
        "applied",
    );
    let req = || request("req-1", "question", Some("turn-1"));
    let first = runtime.dispatch(&sess_ev("s-a", 2.0).attention_requested(req()));
    assert_kind(&first, "applied");
    assert_semantic_event_types(&first, &["attention-requested"]);
    let repeated = runtime.dispatch(&sess_ev("s-a", 3.0).attention_requested(req()));
    assert_kind(&repeated, "unchanged");
    assert_eq!(runtime.snapshot().revision, 3);
}

#[test]
fn a2_request_conflict_turn_id() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-a"));
    runtime.dispatch(&sess_ev("s-a", 1.0).turn_started("turn-1", Some("tool"), "working"));
    runtime.dispatch(&sess_ev("s-a", 2.0).attention_requested(request(
        "req-1",
        "question",
        Some("turn-1"),
    )));
    assert_rejected(
        &runtime
            .dispatch(&sess_ev("s-a", 3.0).attention_requested(request("req-1", "question", None))),
        AgentTransitionRejection::RequestConflict,
    );
}

#[test]
fn a3_request_conflict_kind() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-a"));
    runtime.dispatch(&sess_ev("s-a", 1.0).turn_started("turn-1", Some("tool"), "working"));
    runtime.dispatch(&sess_ev("s-a", 2.0).attention_requested(request(
        "req-1",
        "question",
        Some("turn-1"),
    )));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-a", 3.0).attention_requested(request(
            "req-1",
            "approval",
            Some("turn-1"),
        ))),
        AgentTransitionRejection::RequestConflict,
    );
}

// ---------- B. post-end visibility change ----------

#[test]
fn b1_post_end_visibility_applies() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-b", |over| {
        over.presentation_attachments = vec![attach("p1", "g1")];
    }));
    runtime.dispatch(&sess_ev("s-b", 1.0).ended());
    let transition = runtime.dispatch(&sess_ev("s-b", 2.0).visibility("p1", "g1", "hidden"));
    assert_kind(&transition, "applied");
    let session = &runtime.snapshot().sessions[0];
    assert_eq!(
        session.presentation_attachments[0].visibility.as_str(),
        "hidden"
    );
}

#[test]
fn b2_post_end_same_visibility_unchanged() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-b", |over| {
        over.presentation_attachments = vec![attach("p1", "g1")];
    }));
    runtime.dispatch(&sess_ev("s-b", 1.0).ended());
    assert_kind(
        &runtime.dispatch(&sess_ev("s-b", 2.0).visibility("p1", "g1", "visible")),
        "unchanged",
    );
}

#[test]
fn b3_post_end_visibility_non_navigable() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-b", |over| {
        let mut attachment = attach("p1", "g1");
        attachment.navigable = false;
        attachment.visibility = "unknown".to_string();
        over.presentation_attachments = vec![attachment];
    }));
    runtime.dispatch(&sess_ev("s-b", 1.0).ended());
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-b", 2.0).visibility("p1", "g1", "visible")),
        AgentTransitionRejection::PresentationNotFound,
    );
}

// ---------- C. observe guard precedence ----------

#[test]
fn c1_identical_reobserve_unchanged() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-c", |over| {
        over.presentation_attachments = vec![attach("p1", "g1")];
    }));
    assert_kind(
        &runtime.dispatch(&observe_over("s-c", |over| {
            over.presentation_attachments = vec![attach("p1", "g1")];
        })),
        "unchanged",
    );
}

#[test]
fn c2_source_fidelity_precedes_identity() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-c", |over| {
        over.presentation_attachments = vec![attach("p1", "g1")];
    }));
    assert_rejected(
        &runtime.dispatch(&observe_over("s-c", |over| {
            over.observation.source = "terminal".to_string();
            over.observation.fidelity = "rich".to_string();
            over.presentation_attachments = vec![attach("p1", "g1")];
        })),
        AgentTransitionRejection::SourceCapabilityExceeded,
    );
}

#[test]
fn c3_heuristic_with_capabilities() {
    let mut runtime = AgentRuntime::new();
    assert_rejected(
        &runtime.dispatch(&observe_over("s-c2", |over| {
            over.observation.source = "heuristic".to_string();
            over.observation.fidelity = "heuristic".to_string();
        })),
        AgentTransitionRejection::SourceCapabilityExceeded,
    );
}

#[test]
fn c4_observe_negative_sequence() {
    let mut runtime = AgentRuntime::new();
    assert_rejected(
        &runtime.dispatch(&observe_over("s-c3", |over| {
            over.observation.sequence = -1.0;
        })),
        AgentTransitionRejection::SourceCapabilityExceeded,
    );
}

#[test]
fn c5_observe_fractional_generation() {
    let mut runtime = AgentRuntime::new();
    assert_rejected(
        &runtime.dispatch(&observe_over("s-c4", |over| {
            over.observation.generation = 1.5;
        })),
        AgentTransitionRejection::SourceCapabilityExceeded,
    );
}

#[test]
fn c6_nonobserve_invalid_numeric_identities() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-c5"));
    for sequence in [1.5, -1.0, 9_007_199_254_740_992.0] {
        assert_rejected(
            &runtime.dispatch(&sess_ev("s-c5", sequence).idle()),
            AgentTransitionRejection::InvalidEvent,
        );
        assert_eq!(runtime.snapshot().revision, 1);
    }
}

#[test]
fn c7_sequence_zero_is_valid() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-c6"));
    assert_kind(&runtime.dispatch(&sess_ev("s-c6", 0.0).idle()), "unchanged");
}

#[test]
fn c8_duplicate_capability_deduped() {
    let mut runtime = AgentRuntime::new();
    let transition = runtime.dispatch(&observe_over("s-c7", |over| {
        over.capabilities.push("progress".to_string());
        over.capabilities.push("progress".to_string());
    }));
    assert_kind(&transition, "applied");
    assert_eq!(runtime.snapshot().sessions[0].capabilities.len(), 8);
}

#[test]
fn c9_unknown_capability_rejected() {
    let mut runtime = AgentRuntime::new();
    assert_rejected(
        &runtime.dispatch(&observe_over("s-c8", |over| {
            over.capabilities.push("bogus-cap".to_string());
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn c10_duplicate_attachment_key() {
    let mut runtime = AgentRuntime::new();
    assert_rejected(
        &runtime.dispatch(&observe_over("s-c9", |over| {
            let mut second = attach("p1", "g1");
            second.visibility = "hidden".to_string();
            over.presentation_attachments = vec![attach("p1", "g1"), second];
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn c11_two_navigable_same_presentation() {
    let mut runtime = AgentRuntime::new();
    assert_rejected(
        &runtime.dispatch(&observe_over("s-c10", |over| {
            over.presentation_attachments = vec![attach("p1", "g1"), attach("p1", "g2")];
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

// ---------- D. reconnect cross-invariant ----------

#[test]
fn d1_reconnect_turn_scoped_request_on_idle_turn() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-d"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-d", 1.0).generation(1.0).reconnect(|over| {
            over.pending_requests = vec![request("req-1", "question", Some("turn-1"))];
            over.turn = RawTurnState {
                activity: RawPresence::Absent,
                kind: "idle".to_string(),
                phase: RawPresence::Absent,
                turn_id: RawPresence::Absent,
            };
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn d2_reconnect_source_constrained_capabilities() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-d2", |over| {
        over.capabilities = vec!["activity".to_string()];
        over.observation.source = "terminal".to_string();
        over.observation.fidelity = "basic".to_string();
    }));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-d2", 1.0).generation(1.0).reconnect(|over| {
            over.capabilities = vec!["activity".to_string(), "questions".to_string()];
        })),
        AgentTransitionRejection::SourceCapabilityExceeded,
    );
}

#[test]
fn d3_reconnect_activity_hint_nonterminal() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-d3"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-d3", 1.0).generation(1.0).reconnect(|over| {
            over.activity_hint = RawPresence::Value("tool".to_string());
        })),
        AgentTransitionRejection::CapabilityNotDeclared,
    );
}

// ---------- F1. detach freshness preservation (extra-oracle-07) ----------

#[test]
fn f1_stale_partial_detach_preserves_freshness() {
    // extra-oracle-07 f1_stale_partial_detach: applied rev3, stored "stale"
    // freshness preserved while p-b stays navigable.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-f1", |over| {
        over.presentation_attachments = vec![attach("p-a", "g1"), attach("p-b", "g2")];
    }));
    runtime.dispatch(&sess_ev("s-f1", 1.0).stale());
    let transition = runtime.dispatch(&sess_ev("s-f1", 2.0).detach("p-a", "g1"));
    assert_kind(&transition, "applied");
    let session = &runtime.snapshot().sessions[0];
    assert_eq!(session.observation.freshness.as_str(), "stale");
    let p_a = &session.presentation_attachments[0];
    assert_eq!(p_a.presentation_id.as_str(), "p-a");
    assert!(!p_a.navigable);
    assert_eq!(p_a.visibility.as_str(), "unknown");
    let p_b = &session.presentation_attachments[1];
    assert!(p_b.navigable);
    assert_eq!(p_b.visibility.as_str(), "visible");
}

#[test]
fn f1b_stale_detach_last_navigable_disconnects() {
    // extra-oracle-07 f1b: losing the last navigable attachment while stale
    // forces disconnected, not stale.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-f1b", |over| {
        over.presentation_attachments = vec![attach("p-a", "g1"), attach("p-b", "g2")];
    }));
    runtime.dispatch(&sess_ev("s-f1b", 1.0).stale());
    runtime.dispatch(&sess_ev("s-f1b", 2.0).detach("p-a", "g1"));
    let transition = runtime.dispatch(&sess_ev("s-f1b", 3.0).detach("p-b", "g2"));
    assert_kind(&transition, "applied");
    assert_eq!(
        runtime.snapshot().sessions[0]
            .observation
            .freshness
            .as_str(),
        "disconnected"
    );
}

// ---------- F2. strict-object presence parity (extra-oracle-07) ----------

#[test]
fn f2a_reconnect_unknown_turn_string_activity() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2a"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2a", 1.0).generation(1.0).reconnect(|over| {
            over.turn = RawTurnState {
                activity: RawPresence::Value("tool".to_string()),
                kind: "unknown".to_string(),
                phase: RawPresence::Absent,
                turn_id: RawPresence::Absent,
            };
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn f2b_reconnect_unknown_turn_null_activity() {
    // Present-null residue: `activity: null` is still a key the strict
    // `unknown` member forbids.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2b"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2b", 1.0).generation(1.0).reconnect(|over| {
            over.turn = RawTurnState {
                activity: RawPresence::Null,
                kind: "unknown".to_string(),
                phase: RawPresence::Absent,
                turn_id: RawPresence::Absent,
            };
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn f2c_reconnect_idle_turn_null_phase() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2c"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2c", 1.0).generation(1.0).reconnect(|over| {
            over.turn = RawTurnState {
                activity: RawPresence::Absent,
                kind: "idle".to_string(),
                phase: RawPresence::Null,
                turn_id: RawPresence::Absent,
            };
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn f2d_reconnect_active_turn_missing_activity() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2d"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2d", 1.0).generation(1.0).reconnect(|over| {
            over.turn = RawTurnState {
                activity: RawPresence::Absent,
                kind: "active".to_string(),
                phase: RawPresence::Value("working".to_string()),
                turn_id: RawPresence::Value("t-1".to_string()),
            };
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn f2e_reconnect_active_turn_null_activity_control() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2e"));
    assert_kind(
        &runtime.dispatch(&sess_ev("s-f2e", 1.0).generation(1.0).reconnect(|over| {
            over.turn = RawTurnState {
                activity: RawPresence::Null,
                kind: "active".to_string(),
                phase: RawPresence::Value("working".to_string()),
                turn_id: RawPresence::Value("t-1".to_string()),
            };
        })),
        "applied",
    );
}

#[test]
fn f2f_observe_parent_session_id_absent_is_invalid_parent() {
    // `parentSessionId !== null` treats a missing key as a value: the parent
    // guard runs and cannot match any session.
    let mut runtime = AgentRuntime::new();
    let event = AgentObservationEvent::SessionObserved {
        capabilities: caps_full(),
        event_id: "evt-t".to_string(),
        execution_location: RawExecutionLocation {
            execution_location_id: "exec-1".to_string(),
        },
        observation: base_observation(),
        parent_session_id: RawPresence::Absent,
        presentation_attachments: Vec::new(),
        provider_id: "claude-code".to_string(),
        session_id: "s-f2f".to_string(),
    };
    assert_rejected(
        &runtime.dispatch(&event),
        AgentTransitionRejection::InvalidParent,
    );
}

#[test]
fn f2g_request_turn_id_absent_is_turn_not_found() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2g"));
    runtime.dispatch(&sess_ev("s-f2g", 1.0).turn_started("t-1", Some("tool"), "working"));
    let request = RawAttentionRequest {
        action_path: "navigate-only".to_string(),
        kind: "question".to_string(),
        request_id: "req-1".to_string(),
        turn_id: RawPresence::Absent,
    };
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2g", 2.0).attention_requested(request)),
        AgentTransitionRejection::TurnNotFound,
    );
}

#[test]
#[should_panic]
fn f2h_turn_started_missing_activity_panics() {
    // The candidate `active` Turn loses its required `activity` key — the
    // TypeScript `applied()` parse throws, which maps to a panic.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2h"));
    let event = AgentObservationEvent::TurnStarted {
        activity: RawPresence::Absent,
        event_id: "evt-t".to_string(),
        generation: 0.0,
        phase: "working".to_string(),
        sequence: 1.0,
        session_id: "s-f2h".to_string(),
        turn_id: "t-1".to_string(),
    };
    runtime.dispatch(&event);
}

#[test]
fn f2i_reconnect_activity_hint_absent_hook_is_capability_not_declared() {
    // An absent `activityHint` reads as `!== null` in claims-fit, so a
    // hook-source session fails the activity-capability gate first.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2i"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2i", 1.0).generation(1.0).reconnect(|over| {
            over.activity_hint = RawPresence::Absent;
        })),
        AgentTransitionRejection::CapabilityNotDeclared,
    );
}

#[test]
fn f2i2_reconnect_activity_hint_absent_terminal_is_invalid_event() {
    // Terminal+activity clears claims-fit; the missing required key then
    // fails the candidate parse.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-f2i2", |over| {
        over.capabilities = vec!["activity".to_string()];
        over.observation.source = "terminal".to_string();
        over.observation.fidelity = "basic".to_string();
    }));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2i2", 1.0).generation(1.0).reconnect(|over| {
            over.capabilities = vec!["activity".to_string()];
            over.activity_hint = RawPresence::Absent;
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn f2j_reconnect_last_outcome_absent_is_invalid_event() {
    // `lastOutcome !== null` is truthy for a missing key; turnLifecycle is
    // declared, so claims-fit passes and the parse reports the missing key.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2j"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2j", 1.0).generation(1.0).reconnect(|over| {
            over.last_outcome = RawPresence::Absent;
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn f2m_reconnect_request_turn_id_absent_is_invalid_event() {
    // Reconnect claims-fit does not read `turnId`; the missing required key
    // fails at the candidate parse.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-f2m"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-f2m", 1.0).generation(1.0).reconnect(|over| {
            over.pending_requests = vec![RawAttentionRequest {
                action_path: "navigate-only".to_string(),
                kind: "question".to_string(),
                request_id: "req-1".to_string(),
                turn_id: RawPresence::Absent,
            }];
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

// ---------- E + P. UTF-16 boundaries and ordering ----------

const ASTRAL: &str = "\u{1F600}";

fn astral_presentation_id(chars: usize, suffix: &str) -> String {
    format!("{}{}", ASTRAL.repeat(chars), suffix)
}

#[test]
fn e1_presentation_id_256_units_applies() {
    let mut runtime = AgentRuntime::new();
    let pid = astral_presentation_id(128, "");
    assert_eq!(pid.encode_utf16().count(), 256);
    let transition = runtime.dispatch(&observe_over("s-e1", |over| {
        over.presentation_attachments = vec![attach(&pid, "g1")];
    }));
    assert_kind(&transition, "applied");
    assert_eq!(runtime.snapshot().revision, 1);
}

#[test]
fn e2_presentation_id_258_units_invalid() {
    let mut runtime = AgentRuntime::new();
    let pid = astral_presentation_id(129, "");
    assert_eq!(pid.encode_utf16().count(), 258);
    assert_rejected(
        &runtime.dispatch(&observe_over("s-e2", |over| {
            over.presentation_attachments = vec![attach(&pid, "g1")];
        })),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn e3_presentation_attach_258_units_panics() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-e3"));
    let pid = astral_presentation_id(129, "");
    let event = sess_ev("s-e3", 1.0).attach(attach(&pid, "g1"));
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.dispatch(&event)));
    assert!(
        outcome.is_err(),
        "attach path mirrors the TypeScript applied() ZodError as a panic"
    );
}

#[test]
fn e4_attachment_utf16_ordering() {
    let mut runtime = AgentRuntime::new();
    let transition = runtime.dispatch(&observe_over("s-e4", |over| {
        over.presentation_attachments = vec![attach("\u{E000}", "g1"), attach(ASTRAL, "g1")];
    }));
    assert_kind(&transition, "applied");
    let attachments = &runtime.snapshot().sessions[0].presentation_attachments;
    assert_eq!(attachments[0].presentation_id.as_str(), ASTRAL);
    assert_eq!(attachments[1].presentation_id.as_str(), "\u{E000}");
}

#[test]
fn p1_observe_257_units_invalid() {
    let mut runtime = AgentRuntime::new();
    let pid = astral_presentation_id(128, "a");
    assert_eq!(pid.encode_utf16().count(), 257);
    assert_rejected(
        &runtime.dispatch(&observe_over("s-257", |over| {
            over.presentation_attachments = vec![attach(&pid, "g1")];
        })),
        AgentTransitionRejection::InvalidEvent,
    );
    assert_eq!(runtime.snapshot().revision, 0);
}

#[test]
fn p2_attach_257_units_panics() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-257a"));
    let pid = astral_presentation_id(128, "a");
    assert_eq!(pid.encode_utf16().count(), 257);
    let event = sess_ev("s-257a", 1.0).attach(attach(&pid, "g1"));
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.dispatch(&event)));
    assert!(
        outcome.is_err(),
        "the 257-unit attach must panic like the recorded ZodError, not reject"
    );
}

#[test]
fn p3_observe_256_units_control() {
    let mut runtime = AgentRuntime::new();
    let pid = astral_presentation_id(128, "");
    assert_eq!(pid.encode_utf16().count(), 256);
    let transition = runtime.dispatch(&observe_over("s-256", |over| {
        over.presentation_attachments = vec![attach(&pid, "g1")];
    }));
    assert_kind(&transition, "applied");
    assert_eq!(runtime.snapshot().revision, 1);
}

// ---------- F. projection group + plural locations ----------

#[test]
fn f1_presentation_group_selection() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-f1", |over| {
        over.presentation_attachments = vec![attach("p1", "g1"), attach("p2", "g1")];
    }));
    runtime.dispatch(&observe_over("s-f2", |over| {
        over.presentation_attachments = vec![attach("p3", "g1")];
    }));
    let snapshot = runtime.snapshot();
    let group_p1p2 =
        project_agent_presentation_group(snapshot, &["p1".to_string(), "p2".to_string()])
            .expect("p1+p2 resolves to the shared session");
    assert_eq!(group_p1p2.session_count, 1);
    let group_p1p3 =
        project_agent_presentation_group(snapshot, &["p1".to_string(), "p3".to_string()])
            .expect("p1+p3 resolves both sessions");
    assert_eq!(group_p1p3.session_count, 2);
    assert!(
        project_agent_presentation_group(snapshot, &["pX".to_string()]).is_none(),
        "unknown presentation yields no group"
    );
}

#[test]
fn f2_location_plural_sort_and_generation_tie() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-f3", |over| {
        over.presentation_attachments = vec![attach("pres", "g1")];
    }));
    runtime.dispatch(&sess_ev("s-f3", 1.0).detach("pres", "g1"));
    runtime.dispatch(&sess_ev("s-f3", 2.0).attach(attach("pres", "g2")));
    runtime.dispatch(&sess_ev("s-f3", 3.0).detach("pres", "g2"));
    let snapshot = runtime.snapshot();
    let plural = project_agent_locations(snapshot);
    let gens: Vec<&str> = plural
        .iter()
        .map(|location| location.location.generation.as_str())
        .collect();
    assert_eq!(
        gens,
        ["g1", "g2"],
        "plural sort is (pid, generation) ascending"
    );
    let singular = project_agent_location(snapshot, "pres").expect("one location wins");
    assert_eq!(
        singular.location.generation.as_str(),
        "g2",
        "singular preference breaks ties by descending generation"
    );
}

// ---------- G. journal invalid-input category ----------

#[test]
fn g_journal_invalid_bounds_return_error() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-g"));
    for input in [
        AgentChangeReadInput {
            after_revision: 0.0,
            limit: 0.0,
        },
        AgentChangeReadInput {
            after_revision: 0.0,
            limit: 101.0,
        },
        AgentChangeReadInput {
            after_revision: -1.0,
            limit: 100.0,
        },
        AgentChangeReadInput {
            after_revision: 1.5,
            limit: 100.0,
        },
        AgentChangeReadInput {
            after_revision: 9_007_199_254_740_992.0,
            limit: 100.0,
        },
    ] {
        assert!(
            runtime.read_changes(input).is_err(),
            "invalid private read input must fail with a typed error, not a result kind"
        );
    }
    let ok = runtime
        .read_changes(AgentChangeReadInput {
            after_revision: 0.0,
            limit: 1.0,
        })
        .expect("limit 1 is a valid page");
    match ok {
        super::super::changes::AgentRuntimeChangeReadResult::Changes { changes, .. } => {
            assert_eq!(changes.len(), 1);
        }
        _ => panic!("limit 1 must page the single retained change"),
    }
}

// ---------- H. activity tail ----------

#[test]
fn h_activity_tail_variants_apply() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-h"));
    runtime.dispatch(&sess_ev("s-h", 1.0).turn_started("t1", Some("tool"), "working"));
    for (index, activity) in ["subagent", "compacting", "retrying", "other"]
        .iter()
        .enumerate()
    {
        let transition =
            runtime.dispatch(&sess_ev("s-h", 2.0 + index as f64).turn_activity_changed(
                "t1",
                Some(activity),
                "working",
            ));
        assert_kind(&transition, "applied");
    }
    let session = &runtime.snapshot().sessions[0];
    match &session.turn {
        super::super::model::AgentTurnState::Active { activity, .. } => {
            assert_eq!(activity.map(|a| a.as_str()), Some("other"));
        }
        _ => panic!("turn must remain active"),
    }
}

// ---------- I. capacities ----------

#[test]
fn i1_sessions_512_capacity() {
    let sessions: Vec<RawSessionSnapshot> = (0..512)
        .map(|index| make_session(&format!("sess-{index}")))
        .collect();
    let mut runtime = seed_runtime(0, sessions);
    assert_rejected(
        &runtime.dispatch(&observe("sess-512")),
        AgentTransitionRejection::CapacityExceeded,
    );
}

#[test]
fn i2_observe_33_attachments() {
    let mut runtime = AgentRuntime::new();
    assert_rejected(
        &runtime.dispatch(&observe_over("s-i2", |over| {
            over.presentation_attachments = (0..33)
                .map(|index| attach(&format!("p-{index}"), "g1"))
                .collect();
        })),
        AgentTransitionRejection::CapacityExceeded,
    );
}

#[test]
fn i3_attach_33rd_attachment() {
    let mut session = make_session("s-i3");
    session.presentation_attachments = (0..32)
        .map(|index| attach(&format!("p-{index}"), "g1"))
        .collect();
    let mut runtime = seed_runtime(0, vec![session]);
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-i3", 1.0).attach(attach("p-32", "g1"))),
        AgentTransitionRejection::CapacityExceeded,
    );
}

#[test]
fn i4_requests_128_capacity() {
    let mut session = make_session("s-i4");
    session.pending_requests = (0..128)
        .map(|index| request(&format!("req-{index}"), "question", None))
        .collect();
    let mut runtime = seed_runtime(0, vec![session]);
    assert_rejected(
        &runtime.dispatch(
            &sess_ev("s-i4", 1.0).attention_requested(request("req-128", "question", None)),
        ),
        AgentTransitionRejection::CapacityExceeded,
    );
}

#[test]
fn i5_revision_max_safe_capacity() {
    let mut runtime = seed_runtime(MAX_SAFE_INTEGER, vec![make_session("s-i5")]);
    let transition = runtime.dispatch(&observe("s-i5b"));
    assert_rejected(&transition, AgentTransitionRejection::CapacityExceeded);
    assert_eq!(runtime.snapshot().revision, MAX_SAFE_INTEGER);
}

#[test]
fn i6_revision_max_safe_minus_one_applies() {
    let mut runtime = seed_runtime(MAX_SAFE_INTEGER - 1, vec![make_session("s-i6")]);
    let transition = runtime.dispatch(&observe("s-i6b"));
    assert_kind(&transition, "applied");
    assert_eq!(runtime.snapshot().revision, MAX_SAFE_INTEGER);
}

// ---------- J. session-observed emits no semantic events ----------

#[test]
fn j1_observe_empty_semantic_events() {
    let mut runtime = AgentRuntime::new();
    let transition = runtime.dispatch(&observe("s-j1"));
    assert_kind(&transition, "applied");
    assert_semantic_event_types(&transition, &[]);
}

// ---------- additional named B2 edges ----------

#[test]
fn k_overlength_session_identity_at_observe() {
    let mut runtime = AgentRuntime::new();
    let overlong = "a".repeat(129);
    assert_rejected(
        &runtime.dispatch(&observe(&overlong)),
        AgentTransitionRejection::InvalidEvent,
    );
}

#[test]
fn k2_post_end_reobserve_identity_vs_unchanged() {
    // Post-end mutation means an identical re-observe is an identity-conflict,
    // not unchanged — the identity comparison follows the stored state.
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe_over("s-k", |over| {
        over.presentation_attachments = vec![attach("p1", "g1")];
    }));
    runtime.dispatch(&sess_ev("s-k", 1.0).ended());
    runtime.dispatch(&sess_ev("s-k", 2.0).detach("p1", "g1"));
    assert_rejected(
        &runtime.dispatch(&observe_over("s-k", |over| {
            over.presentation_attachments = vec![attach("p1", "g1")];
        })),
        AgentTransitionRejection::IdentityConflict,
    );
}

#[test]
fn k3_reconnect_same_generation_fence() {
    let mut runtime = AgentRuntime::new();
    runtime.dispatch(&observe("s-k3"));
    assert_rejected(
        &runtime.dispatch(&sess_ev("s-k3", 1.0).reconnect(|_| {})),
        AgentTransitionRejection::GenerationMismatch,
    );
}
