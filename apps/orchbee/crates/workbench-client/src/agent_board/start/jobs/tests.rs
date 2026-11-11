use super::super::*;
use super::*;
use crate::agent_start::{
    DeliveryDisposition, ExchangeFailure, Failure, StartFailure, Submission, Verdict,
};
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
        held_until: Some(Instant::now() + START_DEADLINE),
    }
}

fn report(
    failure: Failure,
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
        submission: submission_status.map(|status| sent(status, delivery)),
        delivery,
        elapsed_ms: 1,
        verdict: Verdict::Failed {
            failure,
            read_cause: None,
        },
    }
}

// Retain the old status cases as their actual protocol receipt, independent of the later startup failure.
fn sent(status: u8, delivery: DeliveryDisposition) -> Submission {
    use crate::agent_runtime::startup::{
        binding::Binding,
        send::{Context, Receipt},
    };
    use orchbee_client_server_protocol::{
        HostEpoch, HostTerminalId, RequestId, SendCleanupStatus, SendProgress, SendRejectedReason,
        SendTarget, SendUnavailableReason, TerminalSendOutcome, TerminalSendResult,
    };
    let outcome = match status {
        0 => TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(2, 10),
            cleanup: SendCleanupStatus::Confirmed,
        },
        5 if delivery == DeliveryDisposition::Applied => TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(2, 10),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        5 => TerminalSendOutcome::Unavailable {
            reason: SendUnavailableReason::HostProtocolFault,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
        3 => TerminalSendOutcome::Rejected {
            reason: SendRejectedReason::HostInputBackpressure,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Confirmed,
        },
        4 => TerminalSendOutcome::Unavailable {
            reason: SendUnavailableReason::WorkerUnavailable,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
        8 => TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        9 => TerminalSendOutcome::Rejected {
            reason: SendRejectedReason::HostInputBackpressure,
            progress: SendProgress::stopped(1, 4),
            cleanup: SendCleanupStatus::Confirmed,
        },
        _ => panic!("fixture status is not declared"),
    };
    let request = RequestId::parse(&"1".repeat(32)).unwrap();
    let target = SendTarget::new(
        std::num::NonZeroU64::MIN,
        HostEpoch::parse(&"2".repeat(32)).unwrap(),
        HostTerminalId::parse(&"3".repeat(32)).unwrap(),
    );
    Submission::Sent(Box::new(Receipt {
        context: Context {
            binding: Binding::parse(&"4".repeat(32), &"5".repeat(32), &"6".repeat(32)).unwrap(),
            revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
            request: request.clone(),
        },
        result: TerminalSendResult::new(request, Some(target), outcome).unwrap(),
        records: 2,
        bytes: 10,
    }))
}

#[test]
fn draft_is_bounded_and_every_visible_change_invalidates_the_draw_source() {
    let mut state = State::new(true);
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
    let mut state = State::new(true);
    let target = panel(1);
    state.operations = vec![operation(7, target.clone(), Phase::Starting)];
    assert!(!state.apply_completion(6, &target, Phase::Started));
    assert!(!state.apply_completion(7, &panel(2), Phase::Started));
    assert_eq!(state.operations[0].phase, Phase::Starting);
    assert!(state.apply_completion(7, &target, Phase::Started));
    assert_eq!(state.operations[0].phase, Phase::Started);
}

#[test]
fn only_proven_zero_delivery_with_a_settled_reservation_is_retryable() {
    // Every refusal that typed nothing is retryable once its reservation is settled.
    for failure in [
        StartFailure::TerminalNotIdle,
        StartFailure::PromptUnconfirmed(SendPromptState::Busy),
        StartFailure::PromptUnconfirmed(SendPromptState::Unmarked),
        StartFailure::PromptUnconfirmed(SendPromptState::Settling),
        StartFailure::SessionTooOld,
        StartFailure::TerminalTooOld,
    ] {
        for (reservation, status) in [
            (ReservationState::None, None),
            (ReservationState::Released, Some(3)),
        ] {
            assert!(
                report_phase(&report(
                    Failure::Start(failure),
                    reservation,
                    DeliveryDisposition::NotApplied,
                    status,
                ))
                .retryable(),
                "{failure:?} typed nothing"
            );
        }
        assert_eq!(
            report_phase(&report(
                Failure::Start(failure),
                ReservationState::Unknown,
                DeliveryDisposition::NotApplied,
                None,
            )),
            Phase::Unknown
        );
    }
    assert_eq!(
        report_phase(&report(
            Failure::Start(StartFailure::Cancelled),
            ReservationState::Released,
            DeliveryDisposition::NotApplied,
            None,
        )),
        Phase::Cancelled,
        "a confirmed release before Send is safe to retry"
    );
    assert_eq!(
        report_phase(&report(
            Failure::Start(StartFailure::Cancelled),
            ReservationState::Unknown,
            DeliveryDisposition::NotApplied,
            None,
        )),
        Phase::Unknown
    );
    assert_eq!(
        report_phase(&report(
            Failure::Start(StartFailure::Deadline),
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
                Failure::Start(StartFailure::Cancelled),
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
            Failure::Start(StartFailure::Cancelled),
            ReservationState::Released,
            DeliveryDisposition::NotApplied,
            Some(4),
        )),
        Phase::Cancelled
    );
    assert_eq!(
        report_phase(&report(
            Failure::Start(StartFailure::Exchange(ExchangeFailure::Transport)),
            ReservationState::Released,
            DeliveryDisposition::NotApplied,
            Some(5),
        )),
        Phase::Failed,
        "status 5 with a typed zero-input verdict remains retryable"
    );
    let mut success = report(
        Failure::Start(StartFailure::TerminalNotIdle),
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
    let mut state = State::new(true);
    let mut jobs = Jobs::default();
    let target = panel(1);
    state.operations = vec![operation(7, target.clone(), Phase::Starting)];
    let (sender, outcomes) = mpsc::sync_channel(1);
    let (_reader, wake) = LocalStream::pair().unwrap();
    jobs.start.push(StartTask {
        generation: 7,
        panel: target.clone(),
        progress: Arc::new(Mutex::new(None)),
        outcomes,
        wake,
        task: crate::background_thread::BackgroundThread::ended(),
        cancelled: false,
    });

    // Empty is not worker loss. The sender may publish immediately after this
    // observation, including after its worker handle has become finished.
    assert_eq!(
        jobs.drain(&mut state, CatalogAdmission::Closed),
        Drain::default()
    );
    let mut completed = report(
        Failure::Start(StartFailure::TerminalNotIdle),
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
        jobs.drain(&mut state, CatalogAdmission::Closed),
        Drain {
            changed: true,
            started: true
        }
    );
    assert_eq!(
        jobs.drain(&mut state, CatalogAdmission::Closed),
        Drain::default()
    );
}

#[test]
fn disconnected_start_worker_preserves_recovery_for_start_and_cancel() {
    for phase in [Phase::Starting, Phase::Cancelling] {
        let mut state = State::new(true);
        let mut jobs = Jobs::default();
        state.operations = vec![operation(7, panel(1), phase)];
        let (sender, outcomes) = mpsc::sync_channel(1);
        drop(sender);
        let (_reader, wake) = LocalStream::pair().unwrap();
        jobs.start.push(StartTask {
            generation: 7,
            panel: panel(1),
            progress: Arc::new(Mutex::new(None)),
            outcomes,
            wake,
            task: crate::background_thread::BackgroundThread::ended(),
            cancelled: phase == Phase::Cancelling,
        });
        assert_eq!(
            jobs.drain(&mut state, CatalogAdmission::Closed),
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
    let mut state = State::new(true);
    let mut jobs = Jobs::default();
    state.catalog = Catalog::Loading;
    state.catalog_revision = 4;
    let (sender, outcomes) = mpsc::sync_channel(1);
    jobs.catalog = Some(CatalogTask {
        revision: 4,
        outcomes,
        cancelled: Arc::new(AtomicBool::new(false)),
        task: crate::background_thread::BackgroundThread::ended(),
    });
    assert!(!jobs.drain_catalog(&mut state));
    sender.send((4, CatalogUpdate::Found("codex"))).unwrap();
    assert!(jobs.drain_catalog(&mut state));
    assert_eq!(state.detected(), ["codex"]);
    assert_eq!(state.catalog_status(), CatalogStatus::Loading);
    assert_eq!(state.selected_provider(), Some("codex"));
    assert_eq!(state.name(), "codex-1");
    sender.send((4, CatalogUpdate::Finished(false))).unwrap();
    drop(sender);
    assert!(jobs.drain_catalog(&mut state));
    assert_eq!(state.detected(), ["codex"]);
    assert_eq!(state.catalog_status(), CatalogStatus::Unavailable);
    assert!(state.providers().contains(&"cursor"));
}

#[test]
fn disconnected_catalog_worker_settles_unavailable() {
    let mut state = State::new(true);
    let mut jobs = Jobs::default();
    state.catalog = Catalog::Loading;
    state.catalog_revision = 4;
    let (sender, outcomes) = mpsc::sync_channel(1);
    drop(sender);
    jobs.catalog = Some(CatalogTask {
        revision: 4,
        outcomes,
        cancelled: Arc::new(AtomicBool::new(false)),
        task: crate::background_thread::BackgroundThread::ended(),
    });
    assert!(jobs.drain_catalog(&mut state));
    assert_eq!(state.catalog_status(), CatalogStatus::Unavailable);
    assert_eq!(
        state.selected_provider(),
        Some(crate::agent_provider::CANONICAL_IDS[0]),
        "a settled empty catalog keeps the form startable"
    );
}

#[test]
fn detection_selects_no_undetected_default_and_lists_detected_providers_first() {
    let mut state = State::new(true);
    let mut jobs = Jobs::default();
    state.catalog = Catalog::Loading;
    state.catalog_revision = 4;
    let (sender, outcomes) = mpsc::sync_channel(3);
    jobs.catalog = Some(CatalogTask {
        revision: 4,
        outcomes,
        cancelled: Arc::new(AtomicBool::new(false)),
        task: crate::background_thread::BackgroundThread::ended(),
    });
    assert_eq!(state.selected_provider(), None);
    assert_eq!(state.name(), "");
    assert!(!state.can_start());

    sender.send((4, CatalogUpdate::Found("codex"))).unwrap();
    assert!(jobs.drain_catalog(&mut state));
    assert_eq!(state.providers()[0], "codex");
    assert_eq!(state.selected_provider(), Some("codex"));

    // A later detection reorders the list but never moves the selection to another provider.
    sender
        .send((4, CatalogUpdate::Found("claude-code")))
        .unwrap();
    sender.send((4, CatalogUpdate::Finished(true))).unwrap();
    assert!(jobs.drain_catalog(&mut state));
    assert_eq!(state.providers()[..2], ["claude-code", "codex"]);
    assert_eq!(state.selected_provider(), Some("codex"));
    assert_eq!(state.name(), "codex-1");
    assert_eq!(
        state.providers().len(),
        crate::agent_provider::CANONICAL_IDS.len(),
        "undetected providers stay manually selectable"
    );

    assert!(state.move_provider(-1));
    assert_eq!(state.selected_provider(), Some("claude-code"));
    state.retire_catalog();
    assert_eq!(
        state.providers(),
        crate::agent_provider::CANONICAL_IDS,
        "retiring detection restores canonical order"
    );
    assert_eq!(state.selected_provider(), Some("claude-code"));
}

#[test]
fn only_explicit_cancel_moves_an_accepted_operation_to_cancelling() {
    let mut state = State::new(true);
    let target = panel(1);
    state.operations = vec![operation(3, target, Phase::Starting)];
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
    for phase in [
        Phase::Starting,
        Phase::WaitingInput,
        Phase::Cancelling,
        Phase::Unknown,
    ] {
        let mut state = State::new(true);
        state.operations = vec![operation(3, target.clone(), phase)];
        state.opened(2);
        assert_eq!(state.phase(2), Some(phase));
        assert!(!state.draft_editable());
    }
    for phase in [Phase::Started, Phase::Failed, Phase::Cancelled] {
        let mut state = State::new(true);
        state.operations = vec![operation(3, target.clone(), phase)];
        state.opened(2);
        assert_eq!(state.phase(2), None);
        assert!(state.draft_editable());
    }
}

#[test]
fn remote_origin_never_publishes_the_local_process_path_as_target_availability() {
    let mut state = State::new(true);
    let mut jobs = Jobs::default();
    let origin = crate::local_client::test_support::server::disconnected_remote_origin();
    state.set_origin_locality(origin.is_local());
    jobs.set_origin(origin);
    jobs.ensure_catalog(&mut state, std::env::var_os("PATH"));
    assert_eq!(state.catalog_status(), CatalogStatus::Unavailable);
    assert!(state.detected().is_empty());
    assert!(!state.providers().is_empty());
    assert!(jobs.catalog.is_none());
}

#[test]
fn catalog_keeps_partial_results_and_never_guesses_cursor_from_agent() {
    use std::cell::Cell;
    let mut observed = Vec::new();
    let mut found = Vec::new();
    let complete = client_path::resolve_candidates(
        crate::agent_provider::CANONICAL_IDS,
        || false,
        |command| {
            observed.push(command.to_owned());
            match command {
                "codex" | "agent" | "amp" | "mcode" | "snow" => client_path::Query::Found,
                "pi" => client_path::Query::Unavailable,
                _ => client_path::Query::Missing,
            }
        },
        |provider| found.push(provider),
    );
    assert!(!complete);
    assert_eq!(found, ["codex", "amp", "minimax-code"]);
    // A command the registry classes as generic never advertises its provider.
    for generic in ["agent", "snow"] {
        assert!(
            !observed.iter().any(|command| command == generic),
            "{generic}"
        );
    }
    let probes = Cell::new(0);
    assert!(!client_path::resolve_candidates(
        crate::agent_provider::CANONICAL_IDS,
        || probes.get() == 2,
        |_| {
            probes.set(probes.get() + 1);
            client_path::Query::Missing
        },
        |_| {}
    ));
    assert_eq!(probes.get(), 2);
    let cancelled = AtomicBool::new(true);
    assert!(!client_path::client_path_candidates(
        None,
        &cancelled,
        Instant::now() + CATALOG_DEADLINE,
        |_| panic!("cancelled lookup")
    ));
    assert!(!client_path::client_path_candidates(
        None,
        &AtomicBool::new(false),
        Instant::now(),
        |_| panic!("expired lookup")
    ));
}

#[test]
fn automatic_names_follow_provider_until_edited_and_advance_after_an_attempt() {
    let mut state = State::new(true);
    state.install_catalog(&["claude-code", "codex"]);
    assert_eq!(state.name(), "claude-1");
    state.select_provider(1);
    assert_eq!(state.name(), "codex-1");
    state.edit(KeyEvent::press(KeyCode::Char('m'), Modifiers::NONE));
    state.select_provider(0);
    assert_eq!(state.name(), "m");
    state.automatic_name = true;
    state.generation = 1;
    state.operations = vec![operation(1, panel(1), Phase::Cancelled)];
    state.opened(2);
    assert_eq!(state.name(), "claude-2");
}

#[cfg(windows)]
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
        client_path::query(
            &executable,
            &system,
            Some(fixture.0.as_os_str()),
            command,
            &cancelled,
            deadline,
            deadline,
        )
    };
    assert_eq!(lookup("codex"), client_path::Query::Found);
    assert_eq!(lookup("amp"), client_path::Query::Found);
    assert_eq!(lookup("claude"), client_path::Query::Missing);
}

#[test]
fn canonical_attention_progress_is_cancellable_without_replacing_the_operation() {
    let mut state = State::new(true);
    let mut jobs = Jobs::default();
    state.operations = vec![operation(1, panel(1), Phase::Starting)];
    let (_sender, outcomes) = mpsc::sync_channel(1);
    let (_reader, wake) = LocalStream::pair().unwrap();
    let progress = Arc::new(Mutex::new(Some(Progress::WaitingInput)));
    jobs.start.push(StartTask {
        generation: 1,
        panel: panel(1),
        progress: Arc::clone(&progress),
        outcomes,
        wake,
        task: crate::background_thread::BackgroundThread::ended(),
        cancelled: false,
    });
    assert!(jobs.drain(&mut state, CatalogAdmission::Closed).changed);
    assert_eq!(state.phase(1), Some(Phase::WaitingInput));
    assert!(state.cancel());
    *progress.lock().unwrap() = Some(Progress::Observing);
    jobs.drain(&mut state, CatalogAdmission::Closed);
    assert_eq!(state.phase(1), Some(Phase::Cancelling));
}

/// A start guards only its own Panel: another Panel starts while it runs. The same Panel starts
/// again once observation showed its shell prompt, or once an unknown result reached the
/// workflow deadline; reopening the Board never ends a hold early.
#[test]
fn a_start_guards_only_its_own_panel_until_the_prompt_returns_or_the_deadline_passes() {
    let mut state = State::new(true);
    state.set_origin_locality(true);
    state.install_catalog(&["codex"]);
    let deadline = Instant::now() + START_DEADLINE;
    let first = state.begin_start(1, panel(1), deadline).unwrap();
    assert!(!state.draft_editable());
    assert!(state.begin_start(1, panel(1), deadline).is_none());
    state.set_target(Some(panel(2)));
    assert!(
        state.draft_editable(),
        "another Panel's start never locks this one"
    );
    let second = state.begin_start(1, panel(2), deadline).unwrap();

    let exited = report_phase(&report(
        Failure::Start(StartFailure::AgentExited),
        ReservationState::Released,
        DeliveryDisposition::Applied,
        Some(0),
    ));
    assert!(state.apply_completion(first.generation, &panel(1), exited));
    state.set_target(Some(panel(1)));
    assert!(state.draft_editable(), "the shell prompt returned");
    assert!(state.begin_start(1, panel(1), deadline).is_some());

    assert!(state.apply_completion(second.generation, &panel(2), Phase::Unknown));
    state.set_target(Some(panel(2)));
    state.opened(2);
    assert_eq!(state.phase(2), Some(Phase::Unknown));
    assert!(!state.expire(deadline - Duration::from_millis(1)));
    assert!(state.begin_start(2, panel(2), deadline).is_none());
    assert!(state.expire(deadline));
    assert!(state.draft_editable());
    assert!(state.begin_start(2, panel(2), deadline).is_some());
}

#[test]
fn an_observed_start_is_started_and_a_returned_prompt_is_retryable() {
    let mut observed = report(
        Failure::Start(StartFailure::Deadline),
        ReservationState::Released,
        DeliveryDisposition::Applied,
        Some(0),
    );
    observed.verdict = Verdict::Observed { panel: panel(1) };
    assert_eq!(report_phase(&observed), Phase::Started);
    for failure in [StartFailure::AgentExited, StartFailure::LaunchFailed] {
        for reservation in [ReservationState::Released, ReservationState::Unknown] {
            assert!(
                report_phase(&report(
                    Failure::Start(failure),
                    reservation,
                    DeliveryDisposition::Applied,
                    Some(0),
                ))
                .retryable(),
                "{failure:?} showed the shell prompt again"
            );
        }
    }
}
