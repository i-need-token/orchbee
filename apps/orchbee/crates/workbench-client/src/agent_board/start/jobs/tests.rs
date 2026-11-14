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
        panel,
        provider: "codex",
        phase,
        fallback: false,
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
        name: None,
        provider: AgentProviderId::parse("codex").unwrap(),
        reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
        last_observed: None,
        reservation_state,
        submission: submission_status.map(|status| sent(status, delivery)),
        delivery,
        shell_refused: false,
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
    // Every refusal that typed nothing is a settled result once its reservation is settled.
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
            assert_ne!(
                report_phase(&report(
                    Failure::Start(failure),
                    reservation,
                    DeliveryDisposition::NotApplied,
                    status,
                )),
                Phase::Unknown,
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
    assert_eq!(jobs.drain(&mut state), Drain::default());
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
    assert_eq!(jobs.drain(&mut state), Drain { changed: true });
    assert_eq!(jobs.drain(&mut state), Drain::default());
}

#[test]
fn disconnected_start_worker_preserves_recovery() {
    for phase in [Phase::Starting, Phase::WaitingInput] {
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
            cancelled: false,
        });
        assert_eq!(jobs.drain(&mut state), Drain { changed: true });
        assert_eq!(state.phase(7), Some(Phase::Unknown));
        assert!(
            state.guards(&panel(1)),
            "an unknown result still holds its Panel"
        );
    }
}

/// A start guards only its own Panel: another Panel starts while it runs. The same Panel starts
/// again once observation showed its shell prompt, or once an unknown result reached the
/// workflow deadline.
#[test]
fn a_start_guards_only_its_own_panel_until_the_prompt_returns_or_the_deadline_passes() {
    let mut state = State::new(true);
    state.install_catalog(&["codex"]);
    let deadline = Instant::now() + START_DEADLINE;
    let first = state
        .begin_start("codex", panel(1), true, deadline)
        .unwrap();
    assert!(state.guards(&panel(1)));
    assert!(
        state
            .begin_start("codex", panel(1), true, deadline)
            .is_none()
    );
    assert!(
        !state.guards(&panel(2)),
        "another Panel's start never locks this one"
    );
    let second = state
        .begin_start("codex", panel(2), true, deadline)
        .unwrap();

    let exited = report_phase(&report(
        Failure::Start(StartFailure::AgentExited),
        ReservationState::Released,
        DeliveryDisposition::Applied,
        Some(0),
    ));
    assert!(state.apply_completion(first.generation, &panel(1), exited));
    assert!(!state.guards(&panel(1)), "the shell prompt returned");
    assert!(
        state
            .begin_start("codex", panel(1), true, deadline)
            .is_some()
    );

    assert!(state.apply_completion(second.generation, &panel(2), Phase::Unknown));
    assert_eq!(state.phase(second.generation), Some(Phase::Unknown));
    assert!(!state.expire(deadline - Duration::from_millis(1)));
    assert!(
        state
            .begin_start("codex", panel(2), true, deadline)
            .is_none()
    );
    assert!(state.expire(deadline));
    assert!(!state.guards(&panel(2)));
    assert!(
        state
            .begin_start("codex", panel(2), true, deadline)
            .is_some()
    );
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
    observed.verdict = Verdict::Sent { panel: panel(1) };
    assert_eq!(report_phase(&observed), Phase::Started);
    for failure in [
        StartFailure::AgentExited,
        StartFailure::LaunchFailed,
        StartFailure::CommandNotFound,
    ] {
        for reservation in [ReservationState::Released, ReservationState::Unknown] {
            assert_ne!(
                report_phase(&report(
                    Failure::Start(failure),
                    reservation,
                    DeliveryDisposition::Applied,
                    Some(0),
                )),
                Phase::Unknown,
                "{failure:?} showed the shell prompt again"
            );
        }
    }
}
