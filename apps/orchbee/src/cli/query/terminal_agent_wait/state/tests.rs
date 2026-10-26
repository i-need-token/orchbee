use super::*;
use crate::invocation::AgentTarget;
use orchbee_workbench_protocol::PanelAgentView;
use serde_json::json;

#[test]
fn idle_never_satisfies_completion_and_explicit_outcomes_do_not_require_a_client() {
    for (status, wanted) in [
        (Status::Idle, AgentWaitCondition::Idle),
        (Status::Done, AgentWaitCondition::Succeeded),
        (Status::Error, AgentWaitCondition::Failed),
    ] {
        let initial = read(1, 1, status, &[]);
        let state = State::new(
            &initial,
            &[
                AgentWaitCondition::Idle,
                AgentWaitCondition::Succeeded,
                AgentWaitCondition::Failed,
            ],
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            state.initial_match(&initial).unwrap().conditions,
            vec![wanted]
        );
        assert!(admit(&[wanted]).is_ok());
    }
    let initial = read(1, 1, Status::Idle, &[]);
    let mut state = State::new(&initial, &[AgentWaitCondition::Done], None, false).unwrap();
    assert!(state.initial_match(&initial).is_none());
    assert!(
        state
            .observe(&read(2, 2, Status::Working, &[(2, Status::Working)]))
            .unwrap()
            .is_none()
    );
    let result = state
        .observe(&read(3, 3, Status::Done, &[(3, Status::Done)]))
        .unwrap()
        .unwrap();
    assert_eq!(result.conditions, [AgentWaitCondition::Done]);
}

// The report-state projection of fixed Herdr e0507237's wait.rs: ordinary waits retain
// the last eligible event; prompt settlement requires activity and current resting state.
// Client done/release events and wall-clock scheduling are outside this pure oracle.
#[test]
fn generated_status_pages_refine_retained_wait_and_prompt_activity_policy() {
    fn disagrees(initial: Status, events: &[Status], wanted: Status, prompt: bool) -> bool {
        let mut states = vec![initial];
        for event in events {
            if states.last() != Some(event) {
                states.push(*event);
            }
        }
        let changes: Vec<_> = states
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (i as u64 + 1, *s))
            .collect();
        let current = *states.last().unwrap();
        let condition = match wanted {
            Status::Idle => AgentWaitCondition::Idle,
            Status::Working => AgentWaitCondition::Working,
            Status::Blocked => AgentWaitCondition::Blocked,
            Status::Unknown => AgentWaitCondition::Unknown,
            Status::Done => AgentWaitCondition::Succeeded,
            Status::Error => AgentWaitCondition::Failed,
        };
        let mut state = State::new(&read(1, 1, initial, &[]), &[condition], None, prompt).unwrap();
        let actual = state
            .observe(&read(states.len() as u64, 2, current, &changes))
            .unwrap();
        let historical = if !prompt {
            changes.iter().rev().find(|(_, s)| *s == wanted).copied()
        } else if initial != Status::Working {
            changes
                .iter()
                .find(|(_, s)| matches!(s, Status::Working | Status::Blocked))
                .copied()
                .filter(|(_, s)| *s == wanted)
        } else {
            None
        };
        let active = !prompt
            || initial == Status::Working
            || states
                .iter()
                .skip(1)
                .any(|s| matches!(s, Status::Working | Status::Blocked));
        let expected = historical
            .map(|(seq, _)| Some(seq))
            .or_else(|| (active && current == wanted).then_some(None));
        actual.map(|m| m.change.map(|c| c.sequence.value())) != expected
    }
    let states = [
        Status::Idle,
        Status::Working,
        Status::Blocked,
        Status::Unknown,
    ];
    for seed in 1_u64..=32 {
        let mut rng = seed;
        for _ in 0..100 {
            let mut next = || {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                states[(rng >> 32) as usize % 4]
            };
            let initial = next();
            let wanted = next();
            let events: Vec<_> = (0..12).map(|_| next()).collect();
            for prompt in [false, true] {
                if disagrees(initial, &events, wanted, prompt) {
                    let mut reduced = events.clone();
                    let mut i = 0;
                    while i < reduced.len() {
                        let mut candidate = reduced.clone();
                        candidate.remove(i);
                        if disagrees(initial, &candidate, wanted, prompt) {
                            reduced = candidate;
                        } else {
                            i += 1;
                        }
                    }
                    panic!(
                        "replay seed={seed} initial={initial:?} wanted={wanted:?} prompt={prompt} events={reduced:?}"
                    );
                }
            }
        }
    }
}
fn read(
    sequence: u64,
    revision: u64,
    status: Status,
    changes: &[(u64, Status)],
) -> TerminalAgentRead {
    let id = "1".repeat(32);
    TerminalAgentRead {
        source: json!({}),
        report_revision: WorkbenchRevision::new(revision),
        observation: Box::new(TerminalAgentObservation {
            binding: serde_json::from_value(
                json!({"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id}),
            )
            .unwrap(),
            sequence: WorkbenchRevision::new(sequence),
            changes: changes
                .iter()
                .map(|(n, s)| TerminalAgentChange {
                    sequence: WorkbenchRevision::new(*n),
                    agent: Some("pi".into()),
                    state: *s,
                })
                .collect(),
            view: PanelAgentView {
                agent: Some("pi".into()),
                state: status,
                ..Default::default()
            },
        }),
    }
}
#[test]
fn prompt_does_not_match_existing_idle_or_message_updates_and_retains_transient_activity() {
    let initial = read(1, 5, Status::Idle, &[]);
    let mut s = State::new(&initial, &[AgentWaitCondition::Idle], None, true).unwrap();
    assert!(s.initial_match(&initial).is_none());
    assert!(s.observe(&read(1, 6, Status::Idle, &[])).unwrap().is_none());
    assert!(!s.active);
    let matched = s
        .observe(&read(
            3,
            8,
            Status::Idle,
            &[(2, Status::Working), (3, Status::Idle)],
        ))
        .unwrap()
        .unwrap();
    assert!(s.active);
    assert_eq!(matched.conditions, [AgentWaitCondition::Idle]);
    assert!(matched.change.is_none());
}
#[test]
fn wait_matches_transients_but_prompt_settlement_requires_current_state() {
    let initial = read(1, 1, Status::Working, &[]);
    let latest = read(
        3,
        3,
        Status::Working,
        &[(2, Status::Idle), (3, Status::Working)],
    );
    let mut ordinary = State::new(&initial, &[AgentWaitCondition::Idle], None, false).unwrap();
    assert_eq!(
        ordinary
            .observe(&latest)
            .unwrap()
            .unwrap()
            .change
            .unwrap()
            .sequence
            .value(),
        2
    );
    let mut prompt = State::new(&initial, &[AgentWaitCondition::Idle], None, true).unwrap();
    assert!(prompt.observe(&latest).unwrap().is_none());
}
#[test]
fn prompt_activity_can_match_blocked_and_preserves_condition_order() {
    let initial = read(1, 1, Status::Unknown, &[]);
    let mut s = State::new(
        &initial,
        &[
            AgentWaitCondition::NeedsAttention,
            AgentWaitCondition::Blocked,
        ],
        None,
        true,
    )
    .unwrap();
    let matched = s
        .observe(&read(
            3,
            3,
            Status::Idle,
            &[(2, Status::Blocked), (3, Status::Idle)],
        ))
        .unwrap()
        .unwrap();
    assert_eq!(
        matched.conditions,
        [
            AgentWaitCondition::NeedsAttention,
            AgentWaitCondition::Blocked
        ]
    );
    assert_eq!(matched.change.unwrap().state, Status::Blocked);
}
#[test]
fn physical_wait_pins_identity_and_rejects_gaps_future_fences_and_provider_aba() {
    let initial = read(1, 1, Status::Idle, &[]);
    assert!(
        State::new(
            &initial,
            &[AgentWaitCondition::Idle],
            Some(AgentSafeInteger::new(2)),
            false
        )
        .is_err()
    );
    let mut s = State::new(
        &initial,
        &[AgentWaitCondition::Idle],
        Some(AgentSafeInteger::new(1)),
        false,
    )
    .unwrap();
    assert!(s.initial_match(&initial).is_none());
    assert!(s.observe(&read(1, 2, Status::Idle, &[])).unwrap().is_some());
    assert!(
        s.observe(&read(3, 3, Status::Idle, &[(3, Status::Idle)]))
            .is_err()
    );
    let mut aba = read(
        3,
        4,
        Status::Idle,
        &[(2, Status::Working), (3, Status::Idle)],
    );
    aba.observation.changes[0].agent = Some("claude".into());
    assert!(s.observe(&aba).is_err());
    let mut other = read(1, 2, Status::Idle, &[]);
    other.observation.binding.host_epoch =
        orchbee_agent_observation_protocol::AgentHostEpoch::parse(&"2".repeat(32)).unwrap();
    assert!(s.observe(&other).is_err());
    assert!(
        admit(&[
            AgentWaitCondition::Idle,
            AgentWaitCondition::Done,
            AgentWaitCondition::Blocked
        ])
        .is_ok()
    );
    assert!(admit(&[AgentWaitCondition::Done]).is_ok());
    assert!(admit(&[AgentWaitCondition::Fresh]).is_err());
    assert!(admit(&[AgentWaitCondition::Idle]).is_ok());
}

#[test]
fn physical_wait_documents_reject_fabricated_fields_and_incomplete_client_views() {
    let mut initial = read(1, 1, Status::Idle, &[]);
    let id = "1".repeat(32);
    initial.source =
        json!({"siteId":id,"workbenchRevision":"1","reportRevision":"1","requestId":id});
    let target = AgentTarget::Name {
        name: orchbee_agent_observation_protocol::AgentName::parse("test").unwrap(),
    };
    let start = std::time::Instant::now();
    let options = super::super::Options {
        selected: None,
        target: &target,
        binding: &initial.observation.binding.clone(),
        until: &[AgentWaitCondition::Idle],
        after_revision: None,
        timeout_ms: Some(1000),
        start,
        total: super::super::WaitDeadline::new(start, Some(1000)).unwrap(),
    };
    let matched = State::new(&initial, options.until, None, false)
        .unwrap()
        .initial_match(&initial)
        .unwrap();
    let matched = super::super::matched_result(&options, initial, matched, 1);
    let mut doc = json!({"schema":"orchbee.agent.wait","version":1,"source":matched.read.source,"result":matched.document()});
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/orchbee.agent.wait.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator.iter_errors(&doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    doc["result"]["clientView"] = json!({"clientId":id,"operationId":id,
        "view":{"workbenchRevision":"1","phase":"starting","selection":null},
        "reportRevision":"1","sequence":"1","seen":true,"status":"idle","outerFocused":null});
    validator.validate(&doc).unwrap();
    let receipt = crate::cli::send::test_receipt(
        orchbee_client_server_protocol::TerminalSendOutcome::Accepted {
            progress: orchbee_client_server_protocol::SendProgress::complete(2, 5),
            cleanup: orchbee_client_server_protocol::SendCleanupStatus::Confirmed,
        },
    )
    .document();
    let mut prompt = json!({"schema":"orchbee.agent.prompt","version":1,"source":receipt["source"],
        "result":{"target":doc["result"]["target"],"submission":receipt,"wait":doc["result"]}});
    let prompt_schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/orchbee.agent.prompt.json")).unwrap();
    let prompt_validator = jsonschema::validator_for(&prompt_schema).unwrap();
    prompt_validator.validate(&prompt).unwrap();
    prompt["result"]["wait"] = serde_json::Value::Null;
    assert!(prompt_validator.is_valid(&prompt));
    doc["result"]["clientView"]
        .as_object_mut()
        .unwrap()
        .remove("sequence");
    assert!(!validator.is_valid(&doc));
    doc["result"].as_object_mut().unwrap().remove("clientView");
    doc["result"]["session"] = json!({});
    assert!(!validator.is_valid(&doc));
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/orchbee.cli.failure.agent.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for (kind, reason) in [
        (FailureKind::Rejected, "history-lost"),
        (FailureKind::NotFound, "binding-changed"),
        (FailureKind::Timeout, "read-deadline"),
        (FailureKind::OutputTooLarge, "output-too-large"),
    ] {
        let mut doc = error(kind, reason).document();
        assert!(validator.is_valid(&doc), "{doc}");
        doc["version"] = json!(3);
        assert!(!validator.is_valid(&doc));
    }
}
