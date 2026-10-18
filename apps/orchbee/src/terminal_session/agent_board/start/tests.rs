use super::*;
use crate::agent_start::{DeliveryDisposition, Verdict};
use orchbee_agent_observation_protocol::{AgentSessionId, ObapRequestId};

fn panel(value: u8) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&format!("{value:032x}")).unwrap()
}

fn operation(generation: u64, panel: WorkbenchPanelId, phase: Phase) -> Operation {
    Operation {
        generation,
        episode: 1,
        panel,
        name: AgentName::parse("alpha").unwrap(),
        provider: AgentProviderId::parse("codex").unwrap(),
        phase,
    }
}

fn report(
    kind: FailureKind,
    reservation_state: ReservationState,
    delivery: DeliveryDisposition,
    submission_status: Option<u8>,
) -> Report {
    Report {
        name: AgentName::parse("alpha").unwrap(),
        provider: AgentProviderId::parse("codex").unwrap(),
        reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
        last_observed: None,
        reservation_state,
        submission: None,
        submission_status,
        delivery,
        elapsed_ms: 1,
        verdict: Verdict::Failed {
            failure: crate::agent_start::failure(kind, "fixture"),
            reason: "fixture".into(),
            cause: None,
        },
    }
}

#[test]
fn draft_is_bounded_and_every_visible_change_invalidates_the_draw_source() {
    let mut state = State::new();
    state.install_catalog(&["codex", "amp"]);
    let first = state.draw_source();
    assert!(state.edit(KeyEvent::press(
        KeyCode::Char('A'),
        Modifiers::NONE.with_shift()
    )));
    assert_eq!(state.name(), "a");
    assert_ne!(state.draw_source(), first);
    let named = state.draw_source();
    assert!(state.select_provider(1));
    assert_ne!(state.draw_source(), named);
    assert!(!state.select_provider(1));
    for _ in 1..MAXIMUM_NAME_BYTES {
        assert!(state.edit(KeyEvent::press(KeyCode::Char('z'), Modifiers::NONE)));
    }
    assert_eq!(state.name().len(), MAXIMUM_NAME_BYTES);
    assert!(!state.edit(KeyEvent::press(KeyCode::Char('z'), Modifiers::NONE)));
    assert!(!state.edit(KeyEvent::press(KeyCode::Char('.'), Modifiers::NONE)));
}

#[test]
fn completion_requires_the_exact_generation_and_panel() {
    let mut state = State::new();
    let target = panel(1);
    state.operation = Some(operation(7, target.clone(), Phase::Starting));
    assert!(!state.apply_completion(6, &target, Phase::Started));
    assert!(!state.apply_completion(7, &panel(2), Phase::Started));
    assert_eq!(state.operation.as_ref().unwrap().phase, Phase::Starting);
    assert!(state.apply_completion(7, &target, Phase::Started));
    assert_eq!(state.operation.as_ref().unwrap().phase, Phase::Started);
}

#[test]
fn only_proven_zero_delivery_with_a_settled_reservation_is_retryable() {
    assert_eq!(
        report_phase(&report(
            FailureKind::Rejected,
            ReservationState::None,
            DeliveryDisposition::NotApplied,
            None,
        )),
        Phase::Failed,
        "preflight and rejected reservations have no effect to recover"
    );
    assert_eq!(
        report_phase(&report(
            FailureKind::Cancelled,
            ReservationState::Released,
            DeliveryDisposition::NotApplied,
            None,
        )),
        Phase::Cancelled,
        "a confirmed release before Send is safe to retry"
    );
    assert_eq!(
        report_phase(&report(
            FailureKind::Cancelled,
            ReservationState::Unknown,
            DeliveryDisposition::NotApplied,
            None,
        )),
        Phase::Unknown
    );
    assert_eq!(
        report_phase(&report(
            FailureKind::Timeout,
            ReservationState::Pending,
            DeliveryDisposition::NotApplied,
            None,
        )),
        Phase::Unknown
    );
    for (delivery, status) in [
        (DeliveryDisposition::Applied, 0),
        (DeliveryDisposition::Applied, 5),
        (DeliveryDisposition::PartiallyApplied, 9),
        (DeliveryDisposition::Unknown, 8),
    ] {
        assert_eq!(
            report_phase(&report(
                FailureKind::Cancelled,
                ReservationState::Released,
                delivery,
                Some(status),
            )),
            Phase::Unknown,
            "status {status} may have applied input"
        );
    }
    assert_eq!(
        report_phase(&report(
            FailureKind::Cancelled,
            ReservationState::Released,
            DeliveryDisposition::NotApplied,
            Some(4),
        )),
        Phase::Cancelled
    );
    assert_eq!(
        report_phase(&report(
            FailureKind::Rejected,
            ReservationState::Released,
            DeliveryDisposition::NotApplied,
            Some(3),
        )),
        Phase::Failed
    );
    assert_eq!(
        report_phase(&report(
            FailureKind::TransportLoss,
            ReservationState::Released,
            DeliveryDisposition::NotApplied,
            Some(5),
        )),
        Phase::Failed,
        "status 5 with a typed zero-input verdict remains retryable"
    );
    let mut success = report(
        FailureKind::Rejected,
        ReservationState::Completed,
        DeliveryDisposition::Applied,
        Some(0),
    );
    success.verdict = Verdict::Started {
        session: AgentSessionId::parse("session-1").unwrap(),
    };
    assert_eq!(report_phase(&success), Phase::Started);
}

#[test]
fn completion_waits_for_channel_authority_and_reports_started_once() {
    let mut state = State::new();
    let target = panel(1);
    state.operation = Some(operation(7, target.clone(), Phase::Starting));
    let (sender, outcomes) = mpsc::sync_channel(1);
    let (_reader, wake) = LocalStream::pair().unwrap();
    state.task = Some(StartTask {
        outcomes,
        wake,
        task: None,
        cancelled: false,
    });

    // Empty is not worker loss. The sender may publish immediately after this
    // observation, including after its worker handle has become finished.
    assert_eq!(state.drain(), Drain::default());
    let mut completed = report(
        FailureKind::Rejected,
        ReservationState::Completed,
        DeliveryDisposition::Applied,
        Some(0),
    );
    completed.verdict = Verdict::Started {
        session: AgentSessionId::parse("session-1").unwrap(),
    };
    sender
        .send(Completion {
            generation: 7,
            panel: target,
            report: completed,
        })
        .unwrap();
    drop(sender);
    assert_eq!(
        state.drain(),
        Drain {
            changed: true,
            started: true
        }
    );
    assert_eq!(state.drain(), Drain::default());
}

#[test]
fn disconnected_start_worker_preserves_recovery_for_start_and_cancel() {
    for phase in [Phase::Starting, Phase::Cancelling] {
        let mut state = State::new();
        state.operation = Some(operation(7, panel(1), phase));
        let (sender, outcomes) = mpsc::sync_channel(1);
        drop(sender);
        let (_reader, wake) = LocalStream::pair().unwrap();
        state.task = Some(StartTask {
            outcomes,
            wake,
            task: None,
            cancelled: phase == Phase::Cancelling,
        });
        assert_eq!(
            state.drain(),
            Drain {
                changed: true,
                started: false
            }
        );
        assert_eq!(state.phase(1), Some(Phase::Unknown));
        state.opened(2);
        assert_eq!(state.phase(2), Some(Phase::Unknown));
        assert!(!state.draft_editable());
    }
}

#[test]
fn catalog_result_survives_empty_then_sender_exit() {
    let mut state = State::new();
    state.catalog = Catalog::Loading;
    state.catalog_revision = 4;
    let (sender, outcomes) = mpsc::sync_channel(1);
    state.catalog_task = Some(CatalogTask {
        outcomes,
        cancelled: Arc::new(AtomicBool::new(false)),
        task: None,
    });
    assert!(!state.drain_catalog());
    sender.send((4, Some(vec!["codex", "amp"]))).unwrap();
    drop(sender);
    assert!(state.drain_catalog());
    assert_eq!(state.providers(), ["codex", "amp"]);
}

#[test]
fn disconnected_catalog_worker_settles_unavailable() {
    let mut state = State::new();
    state.catalog = Catalog::Loading;
    state.catalog_revision = 4;
    let (sender, outcomes) = mpsc::sync_channel(1);
    drop(sender);
    state.catalog_task = Some(CatalogTask {
        outcomes,
        cancelled: Arc::new(AtomicBool::new(false)),
        task: None,
    });
    assert!(state.drain_catalog());
    assert_eq!(state.catalog_status(), CatalogStatus::Unavailable);
}

#[test]
fn only_explicit_cancel_moves_an_accepted_operation_to_cancelling() {
    let mut state = State::new();
    let target = panel(1);
    state.operation = Some(operation(3, target, Phase::Starting));
    state.opened(2);
    assert_eq!(state.phase(2), Some(Phase::Starting));
    assert!(!state.draft_editable());
    assert!(state.cancel());
    assert_eq!(state.phase(2), Some(Phase::Cancelling));
    assert!(!state.cancel());
}

#[test]
fn reopening_keeps_accepted_and_unknown_work_but_retires_retryable_results() {
    let target = panel(1);
    for phase in [Phase::Starting, Phase::Cancelling, Phase::Unknown] {
        let mut state = State::new();
        state.operation = Some(operation(3, target.clone(), phase));
        state.opened(2);
        assert_eq!(state.phase(2), Some(phase));
        assert!(!state.draft_editable());
    }
    for phase in [Phase::Started, Phase::Failed, Phase::Cancelled] {
        let mut state = State::new();
        state.operation = Some(operation(3, target.clone(), phase));
        state.opened(2);
        assert_eq!(state.phase(2), None);
        assert!(state.draft_editable());
    }
}

#[test]
fn remote_origin_never_publishes_the_local_process_path_as_target_availability() {
    let mut state = State::new();
    state.set_origin(crate::local_client::tests::server::disconnected_remote_origin());
    state.ensure_catalog(std::env::var_os("PATH"));
    assert_eq!(state.catalog_status(), CatalogStatus::Unavailable);
    assert!(state.providers().is_empty());
    assert!(state.catalog_task.is_none());
}

#[test]
fn catalog_mapping_deadline_and_cancel_are_deterministic() {
    use std::cell::Cell;

    let mut observed = Vec::new();
    let found = catalog::resolve_candidates(
        || false,
        |command| {
            observed.push(command.to_owned());
            match command {
                "codex" | "agent" | "amp" | "maki" | "muse" => catalog::Query::Found,
                _ => catalog::Query::Missing,
            }
        },
    )
    .unwrap();
    let expected_commands = crate::agent_provider::CANONICAL_IDS
        .iter()
        .filter_map(|provider| crate::agent_provider::executable(provider))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(
        observed, expected_commands,
        "every canonical mapping is queried"
    );
    assert_eq!(found, ["codex", "cursor", "amp", "maki", "muse"]);

    let probes = Cell::new(0);
    assert!(
        catalog::resolve_candidates(
            || probes.get() == 2,
            |_| {
                probes.set(probes.get() + 1);
                catalog::Query::Missing
            },
        )
        .is_none(),
        "a stop verdict interrupts the mapping before another lookup"
    );
    assert_eq!(probes.get(), 2);
    assert!(
        catalog::resolve_candidates(|| false, |_| catalog::Query::Unavailable).is_none(),
        "one unavailable OS lookup makes the whole catalog unavailable"
    );

    let cancelled = AtomicBool::new(true);
    assert!(
        catalog::client_path_candidates(None, &cancelled, Instant::now() + CATALOG_DEADLINE,)
            .is_none(),
        "pre-cancel returns without starting an OS lookup"
    );
    let active = AtomicBool::new(false);
    assert!(
        catalog::client_path_candidates(None, &active, Instant::now()).is_none(),
        "an already-expired catalog budget returns without starting an OS lookup"
    );
}

#[test]
fn catalog_where_query_honors_supported_extensions_and_rejects_directories() {
    use std::{fs, path::PathBuf, time::SystemTime};

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            for command in ["codex.cmd", "amp.ps1"] {
                let _ = fs::remove_file(self.0.join(command));
            }
            let _ = fs::remove_dir(self.0.join("claude.exe"));
            let _ = fs::remove_dir(&self.0);
        }
    }

    let root = std::env::temp_dir().join(format!(
        "orchbee-agent-start-catalog-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    for command in ["codex.cmd", "amp.ps1"] {
        fs::write(fixture.0.join(command), []).unwrap();
    }
    fs::create_dir(fixture.0.join("claude.exe")).unwrap();
    let system = PathBuf::from(winsafe::GetSystemDirectory().unwrap());
    let executable = system.join("where.exe");
    let cancelled = AtomicBool::new(false);
    let lookup = |command| {
        // This guard exists only to reclaim a stuck owned Job. The semantic
        // witness does not make the product's whole-catalog deadline a claim
        // about completion time under parallel test-process contention.
        let deadline = Instant::now() + Duration::from_secs(15);
        catalog::query(
            &executable,
            &system,
            Some(fixture.0.as_os_str()),
            command,
            &cancelled,
            deadline,
            deadline,
        )
    };
    assert_eq!(lookup("codex"), catalog::Query::Found);
    assert_eq!(lookup("amp"), catalog::Query::Found);
    assert_eq!(lookup("claude"), catalog::Query::Missing);
}
