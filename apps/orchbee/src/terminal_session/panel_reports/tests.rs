use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
fn scope() -> Scope {
    Scope {
        site: SiteId::parse("00000000000000000000000000000001").unwrap(),
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
