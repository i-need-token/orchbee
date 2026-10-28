use super::*;
use crate::local_client::test_support::server::adjustable_operation_clock;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
fn scope() -> Scope {
    Scope {
        session: SessionId::parse("00000000000000000000000000000001").unwrap(),
        revision: WorkbenchRevision::new(1),
        panels: vec![],
    }
}
fn fixture(epoch: u64, outcome_epoch: u64) -> (PanelReports, Arc<AtomicU64>) {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mut state = PanelReports::new(clock.clone());
    state.scope = Some(scope());
    state.inflight = Some((epoch, clock.now() + DEADLINE));
    state.lane = Some(AgentReadLane::for_test(vec![LaneOutcome::PanelReports {
        epoch: outcome_epoch,
        result: Ok(Round {
            scope: scope(),
            rows: BTreeMap::new(),
        }),
    }]));
    (state, elapsed)
}
#[test]
fn stale_epoch_changed_scope_and_expired_completion_never_publish() {
    let (mut state, _) = fixture(2, 1);
    state.tick(Some(scope()));
    assert!(state.round.is_none());
    assert!(state.unavailable);
    let (mut state, _) = fixture(1, 1);
    let mut changed = scope();
    changed.revision = WorkbenchRevision::new(2);
    state.tick(Some(changed.clone()));
    assert!(state.round.is_none());
    assert_eq!(state.scope, Some(changed));
    let (mut state, elapsed) = fixture(1, 1);
    elapsed.store(6000, Ordering::SeqCst);
    state.tick(Some(scope()));
    assert!(state.round.is_none());
    assert!(state.unavailable);
    let (mut state, _) = fixture(1, 1);
    state.tick(Some(scope()));
    assert!(state.round.is_some());
    assert!(!state.unavailable);
    state.close();
    assert!(state.round.is_none());
    assert!(state.lane.is_none());
    assert!(state.scope.is_none());
}

#[test]
fn changed_presentation_rejects_inflight_round_without_canonical_revision_change() {
    let (mut reports, _) = fixture(1, 1);
    let mut primary = scope();
    primary.panels.push(SelectedPanel {
        panel_id: WorkbenchPanelId::parse(&"1".repeat(32)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&"2".repeat(32)).unwrap(),
        binding: serde_json::from_value(
            serde_json::json!({"workerId":"3".repeat(32),"terminalId":"4".repeat(32)}),
        )
        .unwrap(),
    });
    reports.scope = Some(primary.clone());
    reports.lane = Some(AgentReadLane::for_test(vec![LaneOutcome::PanelReports {
        epoch: 1,
        result: Ok(Round {
            scope: primary.clone(),
            rows: BTreeMap::new(),
        }),
    }]));
    let mut child = primary;
    child.panels[0].presentation_id = TerminalPresentationId::parse(&"5".repeat(32)).unwrap();
    child.panels[0].binding.terminal_id =
        orchbee_workbench_protocol::TerminalId::parse(&"6".repeat(32)).unwrap();
    reports.tick(Some(child.clone()));
    assert_eq!(reports.scope, Some(child));
    assert!(reports.round.is_none());
    reports.close();
}
