use super::*;

fn identity(presentation: &str, generation: &str) -> AgentNotificationIdentity {
    AgentNotificationIdentity {
        presentation_id: presentation.into(),
        generation: generation.into(),
        host_epoch: "host-epoch".into(),
        host_terminal_id: "terminal".into(),
    }
}

fn observation(
    identity: &AgentNotificationIdentity,
    sequence: u64,
    state: AgentNotificationState,
) -> AgentNotificationObservation {
    AgentNotificationObservation {
        identity: identity.clone(),
        sequence,
        agent: Some("codex".into()),
        state,
        seen: false,
    }
}

fn observe(
    scheduler: &mut AgentNotificationScheduler,
    now: u64,
    observation: AgentNotificationObservation,
) {
    scheduler.observe(now, vec![observation]).unwrap();
}

#[test]
fn baseline_and_exact_transition_matrix_schedule_only_done_and_request() {
    let id = identity("pane", "generation-1");
    let mut scheduler = AgentNotificationScheduler::default();
    observe(
        &mut scheduler,
        0,
        observation(&id, 1, AgentNotificationState::Working),
    );
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(5_000, true), None);

    observe(
        &mut scheduler,
        5_000,
        observation(&id, 2, AgentNotificationState::Idle),
    );
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(5_999, true), None);
    assert_eq!(
        scheduler.take_ready(6_000, true),
        Some(AgentNotificationCommand {
            identity: id.clone(),
            sequence: 2,
            agent: "codex".into(),
            kind: AgentNotificationKind::Done,
            popup: true,
            sound: false,
        })
    );
    assert_eq!(scheduler.take_ready(6_000, true), None);

    observe(
        &mut scheduler,
        7_000,
        observation(&id, 3, AgentNotificationState::Working),
    );
    observe(
        &mut scheduler,
        7_100,
        observation(&id, 4, AgentNotificationState::Blocked),
    );
    scheduler.after_draw();
    assert_eq!(
        scheduler
            .take_ready(8_100, true)
            .map(|command| command.kind),
        Some(AgentNotificationKind::Request)
    );
}

#[test]
fn unknown_baselines_non_agents_and_rebindings_never_infer_completion() {
    let first = identity("pane", "generation-1");
    let replacement = identity("pane", "generation-2");
    let mut scheduler = AgentNotificationScheduler::default();
    observe(
        &mut scheduler,
        0,
        observation(&first, 1, AgentNotificationState::Unknown),
    );
    observe(
        &mut scheduler,
        1,
        observation(&first, 2, AgentNotificationState::Idle),
    );
    observe(
        &mut scheduler,
        2,
        observation(&first, 3, AgentNotificationState::Unknown),
    );
    observe(
        &mut scheduler,
        3,
        observation(&first, 4, AgentNotificationState::Blocked),
    );
    observe(
        &mut scheduler,
        4,
        observation(&replacement, 0, AgentNotificationState::Idle),
    );
    let mut no_agent = observation(&replacement, 1, AgentNotificationState::Working);
    no_agent.agent = None;
    observe(&mut scheduler, 5, no_agent);
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(10_000, true), None);
}

#[test]
fn seen_state_identity_and_configuration_changes_cancel_old_candidates() {
    let first = identity("pane", "generation-1");
    let replacement = identity("pane", "generation-2");
    let mut scheduler = AgentNotificationScheduler::default();
    observe(
        &mut scheduler,
        0,
        observation(&first, 1, AgentNotificationState::Working),
    );
    observe(
        &mut scheduler,
        1,
        observation(&first, 2, AgentNotificationState::Idle),
    );
    assert!(scheduler.acknowledge(&first, 2));
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(2_000, true), None);

    observe(
        &mut scheduler,
        2_000,
        observation(&first, 3, AgentNotificationState::Working),
    );
    observe(
        &mut scheduler,
        2_001,
        observation(&first, 4, AgentNotificationState::Idle),
    );
    observe(
        &mut scheduler,
        2_002,
        observation(&first, 5, AgentNotificationState::Working),
    );
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(4_000, true), None);

    observe(
        &mut scheduler,
        4_000,
        observation(&first, 6, AgentNotificationState::Idle),
    );
    observe(
        &mut scheduler,
        4_001,
        observation(&replacement, 0, AgentNotificationState::Idle),
    );
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(6_000, true), None);

    observe(
        &mut scheduler,
        6_000,
        observation(&replacement, 1, AgentNotificationState::Working),
    );
    observe(
        &mut scheduler,
        6_001,
        observation(&replacement, 2, AgentNotificationState::Idle),
    );
    assert!(scheduler.configure(1, AgentNotificationPolicy::default()));
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(8_000, true), None);
}

#[test]
fn client_acknowledgement_is_local_and_busy_sink_preserves_one_exact_candidate() {
    let id = identity("pane", "generation-1");
    let mut foreground = AgentNotificationScheduler::default();
    let mut background = AgentNotificationScheduler::default();
    for scheduler in [&mut foreground, &mut background] {
        observe(
            scheduler,
            0,
            observation(&id, 1, AgentNotificationState::Working),
        );
        observe(
            scheduler,
            1,
            observation(&id, 2, AgentNotificationState::Idle),
        );
    }
    assert!(foreground.acknowledge(&id, 2));
    foreground.after_draw();
    background.after_draw();
    assert_eq!(foreground.take_ready(1_001, true), None);
    assert_eq!(background.take_ready(1_001, false), None);
    assert_eq!(
        background
            .take_ready(1_001, true)
            .map(|command| command.kind),
        Some(AgentNotificationKind::Done)
    );
    assert_eq!(background.take_ready(10_000, true), None);
}

#[test]
fn an_older_ready_candidate_settles_while_lower_keys_keep_becoming_ready() {
    let lower_key = identity("a-new", "generation-1");
    let older = identity("z-old", "generation-1");
    let mut scheduler = AgentNotificationScheduler::default();
    scheduler
        .observe(
            0,
            vec![
                observation(&lower_key, 1, AgentNotificationState::Working),
                observation(&older, 1, AgentNotificationState::Working),
            ],
        )
        .unwrap();
    scheduler
        .observe(
            1,
            vec![
                observation(&lower_key, 1, AgentNotificationState::Working),
                observation(&older, 2, AgentNotificationState::Idle),
            ],
        )
        .unwrap();
    scheduler.after_draw();
    scheduler
        .observe(
            100,
            vec![
                observation(&lower_key, 2, AgentNotificationState::Idle),
                observation(&older, 2, AgentNotificationState::Idle),
            ],
        )
        .unwrap();
    scheduler.after_draw();

    let mut now = 1_100;
    let mut lower_sequence = 2;
    let mut delivered = Vec::new();
    for _ in 0..3 {
        let command = scheduler
            .take_ready(now, true)
            .expect("at least one candidate remains ready");
        delivered.push(command.identity.presentation_id);
        if delivered.last() == Some(&older.presentation_id) {
            break;
        }

        lower_sequence += 1;
        scheduler
            .observe(
                now,
                vec![
                    observation(&lower_key, lower_sequence, AgentNotificationState::Working),
                    observation(&older, 2, AgentNotificationState::Idle),
                ],
            )
            .unwrap();
        lower_sequence += 1;
        scheduler
            .observe(
                now,
                vec![
                    observation(&lower_key, lower_sequence, AgentNotificationState::Idle),
                    observation(&older, 2, AgentNotificationState::Idle),
                ],
            )
            .unwrap();
        scheduler.after_draw();
        now += 1_000;
    }

    assert!(
        delivered
            .iter()
            .any(|presentation| presentation == &older.presentation_id),
        "the older ready candidate was starved by lower presentation keys: {delivered:?}"
    );
}

#[test]
fn zero_delay_waits_for_a_successful_draw_opportunity() {
    let id = identity("pane", "generation-1");
    let mut scheduler = AgentNotificationScheduler::default();
    let policy = AgentNotificationPolicy {
        delay_millis: 0,
        ..AgentNotificationPolicy::default()
    };
    scheduler.configure(1, policy);
    observe(
        &mut scheduler,
        0,
        observation(&id, 1, AgentNotificationState::Working),
    );
    observe(
        &mut scheduler,
        0,
        observation(&id, 2, AgentNotificationState::Idle),
    );
    assert_eq!(scheduler.take_ready(0, true), None);
    scheduler.after_draw();
    assert_eq!(
        scheduler.take_ready(0, true).map(|command| command.kind),
        Some(AgentNotificationKind::Done)
    );
}

#[test]
fn sound_is_global_opt_in_and_per_agent_off_is_a_hard_mute() {
    let id = identity("pane", "generation-1");
    for (override_value, expected) in [
        (AgentSoundPolicy::Default, true),
        (AgentSoundPolicy::On, true),
        (AgentSoundPolicy::Off, false),
    ] {
        let mut scheduler = AgentNotificationScheduler::default();
        let policy = AgentNotificationPolicy {
            delay_millis: 0,
            popup_enabled: false,
            sound_enabled: true,
            agent_sounds: BTreeMap::from([("codex".into(), override_value)]),
        };
        scheduler.configure(1, policy);
        observe(
            &mut scheduler,
            0,
            observation(&id, 1, AgentNotificationState::Working),
        );
        observe(
            &mut scheduler,
            0,
            observation(&id, 2, AgentNotificationState::Idle),
        );
        scheduler.after_draw();
        assert_eq!(scheduler.take_ready(0, true).is_some(), expected);
    }

    let mut disabled = AgentNotificationScheduler::default();
    disabled.configure(
        1,
        AgentNotificationPolicy {
            delay_millis: 0,
            popup_enabled: false,
            sound_enabled: false,
            agent_sounds: BTreeMap::from([("codex".into(), AgentSoundPolicy::On)]),
        },
    );
    observe(
        &mut disabled,
        0,
        observation(&id, 1, AgentNotificationState::Working),
    );
    observe(
        &mut disabled,
        0,
        observation(&id, 2, AgentNotificationState::Idle),
    );
    disabled.after_draw();
    assert_eq!(disabled.take_ready(0, true), None);
}

#[test]
fn malformed_complete_rounds_are_atomic_and_retirement_drops_all_work() {
    let id = identity("pane", "generation-1");
    let mut scheduler = AgentNotificationScheduler::default();
    let baseline = observation(&id, 1, AgentNotificationState::Working);
    observe(&mut scheduler, 0, baseline.clone());
    assert_eq!(
        scheduler.observe(1, vec![baseline.clone(), baseline]),
        Err(())
    );
    observe(
        &mut scheduler,
        1,
        observation(&id, 2, AgentNotificationState::Idle),
    );
    scheduler.after_draw();
    scheduler.retire();
    assert_eq!(scheduler.take_ready(10_000, true), None);
    assert_eq!(
        scheduler.observe(
            10_000,
            vec![observation(&id, 3, AgentNotificationState::Working)]
        ),
        Ok(false)
    );
}

#[test]
fn observation_gap_discards_transition_history_without_retiring_the_scheduler() {
    let id = identity("pane", "generation-1");
    let mut scheduler = AgentNotificationScheduler::default();
    observe(
        &mut scheduler,
        0,
        observation(&id, 1, AgentNotificationState::Working),
    );
    assert!(scheduler.unavailable());
    observe(
        &mut scheduler,
        1,
        observation(&id, 2, AgentNotificationState::Idle),
    );
    scheduler.after_draw();
    assert_eq!(scheduler.take_ready(10_000, true), None);
    observe(
        &mut scheduler,
        10_000,
        observation(&id, 3, AgentNotificationState::Working),
    );
    observe(
        &mut scheduler,
        10_001,
        observation(&id, 4, AgentNotificationState::Idle),
    );
    scheduler.after_draw();
    assert_eq!(
        scheduler
            .take_ready(11_001, true)
            .map(|command| command.kind),
        Some(AgentNotificationKind::Done)
    );
}

#[path = "tests/generated.rs"]
mod generated;
