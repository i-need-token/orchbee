use super::*;
use crate::agent_start::Verdict;
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

fn report(kind: FailureKind, reservation_state: ReservationState) -> Report {
    Report {
        name: AgentName::parse("alpha").unwrap(),
        provider: AgentProviderId::parse("codex").unwrap(),
        reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
        last_observed: None,
        reservation_state,
        submission: None,
        submission_status: None,
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
    assert!(state.edit(KeyEvent::press(KeyCode::Char('A'), Modifiers::SHIFT)));
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
fn reservation_uncertainty_dominates_a_cancelled_surface_result() {
    assert_eq!(
        report_phase(&report(FailureKind::Cancelled, ReservationState::Unknown)),
        Phase::Unknown
    );
    assert_eq!(
        report_phase(&report(FailureKind::Cancelled, ReservationState::Released)),
        Phase::Cancelled
    );
    let mut success = report(FailureKind::Rejected, ReservationState::Completed);
    success.verdict = Verdict::Started {
        session: AgentSessionId::parse("session-1").unwrap(),
    };
    assert_eq!(report_phase(&success), Phase::Started);
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
