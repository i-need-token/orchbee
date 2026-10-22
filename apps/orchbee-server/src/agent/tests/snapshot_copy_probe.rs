//! Timing probe for canonical snapshot copies in `AgentRuntime::dispatch`.
//!
//! It fills a runtime with 1 to the 512-session ceiling and times applied, unchanged and rejected
//! dispatches against one whole-snapshot clone. It asserts only the transition kinds it relies on;
//! the printed medians are evidence for `docs/agent-canonical-model.md#snapshot-copy-cost`,
//! not a budget.
//! Run in release so the numbers reflect shipped code generation:
//! `cargo test -p orchbee-server --release --bin orchbee-server snapshot_copy -- --ignored --nocapture`.

use super::super::model::{MAX_AGENT_SESSIONS, RawAttachment};
use super::super::{
    AgentObservationEvent, AgentRuntime, RawExecutionLocation, RawObservation, RawPresence,
};
use std::time::{Duration, Instant};

const CAPABILITIES: [&str; 8] = [
    "sessionLifecycle",
    "turnLifecycle",
    "questions",
    "approvals",
    "authentication",
    "activity",
    "subagents",
    "progress",
];
const ROUNDS: usize = 9;
const ITERATIONS: usize = 2_000;

fn observed(index: usize) -> AgentObservationEvent {
    AgentObservationEvent::SessionObserved {
        capabilities: CAPABILITIES.iter().map(|cap| cap.to_string()).collect(),
        event_id: format!("probe:observed:{index}"),
        execution_location: RawExecutionLocation {
            execution_location_id: format!("probe:location:{index}"),
        },
        observation: RawObservation {
            fidelity: "rich".to_string(),
            freshness: "fresh".to_string(),
            generation: 0.0,
            sequence: 1.0,
            source: "hook".to_string(),
        },
        parent_session_id: RawPresence::Null,
        presentation_attachments: (0..2)
            .map(|slot| RawAttachment {
                generation: format!("probe:generation:{index}:{slot}"),
                navigable: slot == 0,
                presentation_id: format!("probe:presentation:{index}:{slot}"),
                visibility: "visible".to_string(),
            })
            .collect(),
        provider_id: "claude-code".to_string(),
        session_id: format!("probe:session:{index:04}"),
    }
}

fn activity(sequence: f64, generation: f64, activity: &str) -> AgentObservationEvent {
    AgentObservationEvent::TurnActivityChanged {
        activity: RawPresence::Value(activity.to_string()),
        event_id: format!("probe:activity:{sequence}"),
        generation,
        phase: "working".to_string(),
        sequence,
        session_id: "probe:session:0000".to_string(),
        turn_id: "probe:turn".to_string(),
    }
}

fn filled(sessions: usize) -> AgentRuntime {
    let mut runtime = AgentRuntime::new();
    for index in 0..sessions {
        assert_eq!(runtime.dispatch(&observed(index)).kind(), "applied");
    }
    let started = AgentObservationEvent::TurnStarted {
        activity: RawPresence::Value("tool".to_string()),
        event_id: "probe:turn-started".to_string(),
        generation: 0.0,
        phase: "working".to_string(),
        sequence: 2.0,
        session_id: "probe:session:0000".to_string(),
        turn_id: "probe:turn".to_string(),
    };
    assert_eq!(runtime.dispatch(&started).kind(), "applied");
    runtime
}

/// Median per-operation time across rounds, so one scheduler hiccup cannot dominate.
fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn per_operation(mut body: impl FnMut()) -> Duration {
    median(
        (0..ROUNDS)
            .map(|_| {
                let started = Instant::now();
                for _ in 0..ITERATIONS {
                    body();
                }
                started.elapsed() / u32::try_from(ITERATIONS).unwrap()
            })
            .collect(),
    )
}

#[test]
#[ignore = "timing probe for snapshot copies — run explicitly in release"]
fn snapshot_copy_costs_scale_with_the_session_count() {
    println!("sessions\tclone_ns\tapplied_ns\tunchanged_ns\trejected_ns");
    for sessions in [1, 32, 128, MAX_AGENT_SESSIONS] {
        let mut runtime = filled(sessions);
        let clone = per_operation(|| {
            std::hint::black_box(runtime.snapshot().clone());
        });
        let mut sequence = 2.0;
        let applied = per_operation(|| {
            sequence += 1.0;
            let next = if sequence % 2.0 == 0.0 {
                "tool"
            } else {
                "thinking"
            };
            let transition = runtime.dispatch(&activity(sequence, 0.0, next));
            assert_eq!(transition.kind(), "applied");
            std::hint::black_box(transition);
        });
        let stale = activity(2.0, 0.0, "tool");
        let unchanged = per_operation(|| {
            let transition = runtime.dispatch(&stale);
            assert_eq!(transition.kind(), "unchanged");
            std::hint::black_box(transition);
        });
        let foreign = activity(sequence + 1.0, 7.0, "tool");
        let rejected = per_operation(|| {
            let transition = runtime.dispatch(&foreign);
            assert_eq!(transition.kind(), "rejected");
            std::hint::black_box(transition);
        });
        println!(
            "{sessions}\t{}\t{}\t{}\t{}",
            clone.as_nanos(),
            applied.as_nanos(),
            unchanged.as_nanos(),
            rejected.as_nanos()
        );
    }
}
