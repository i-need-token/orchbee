use super::*;
use crate::local_client::test_support::server::adjustable_operation_clock;
use orchbee_workbench_protocol::{
    SessionId, TerminalPresentationId, WorkbenchPanelId, WorkbenchRevision,
};
use std::collections::BTreeMap;
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
fn a_round_outlives_commits_and_failed_refreshes_until_it_ages_out() {
    fn panel(id: char) -> SelectedPanel {
        SelectedPanel {
            panel_id: WorkbenchPanelId::parse(&id.to_string().repeat(32)).unwrap(),
            presentation_id: TerminalPresentationId::parse(&"2".repeat(32)).unwrap(),
            binding: serde_json::from_value(
                serde_json::json!({"workerId":"3".repeat(32),"terminalId":"4".repeat(32)}),
            )
            .unwrap(),
        }
    }
    let ready = |reports: &PanelReports| {
        matches!(
            reports.selected_view(&panel('1')),
            PanelReportView::Ready(_)
        )
    };
    let current = {
        let mut scope = scope();
        scope.panels.push(panel('1'));
        scope
    };
    let row = PanelReport {
        report: None,
        metadata: serde_json::from_value(
            serde_json::json!({"title":null,"displayAgent":null,"stateLabels":{}}),
        )
        .unwrap(),
        tokens: serde_json::from_value(serde_json::json!({})).unwrap(),
    };
    let (mut reports, elapsed) = fixture(1, 2);
    reports.lane = None;
    reports.inflight = None;
    reports.scope = Some(current.clone());
    reports.publish_for_test(Round {
        scope: current.clone(),
        rows: BTreeMap::from([(panel('1').panel_id, row)]),
    });
    assert!(ready(&reports));
    // A commit keeps the Presentation selected: its glyph has no reason to blink.
    let mut committed = current.clone();
    committed.revision = WorkbenchRevision::new(2);
    reports.next_read = Some(reports.clock.now() + INTERVAL);
    reports.origin = None;
    reports.tick(Some(committed.clone()));
    assert!(ready(&reports));
    // A commit landing inside the next read fails it without saying anything about the facts.
    assert!(!reports.fail(reports.clock.now()));
    assert!(ready(&reports));
    // Another Presentation in that Panel never inherits the row.
    let mut replaced = panel('1');
    replaced.presentation_id = TerminalPresentationId::parse(&"5".repeat(32)).unwrap();
    assert!(matches!(
        reports.selected_view(&replaced),
        PanelReportView::Pending
    ));
    // Sustained loss withdraws it, and only then does the selected Panel read as unavailable.
    elapsed.store(ROUND_FRESH_FOR.as_millis() as u64 + 1, Ordering::SeqCst);
    assert!(reports.fail(reports.clock.now()));
    assert!(matches!(
        reports.selected_view(&panel('1')),
        PanelReportView::Unavailable
    ));
    // Switching to a Tab that shares nothing has nothing to keep.
    reports.publish_for_test(Round {
        scope: committed.clone(),
        rows: BTreeMap::new(),
    });
    let mut other = scope();
    other.panels.push(panel('6'));
    reports.tick(Some(other));
    assert!(reports.round.is_none());
    // Another Session never inherits a row, even under the same identifiers.
    let publish = |reports: &mut PanelReports| {
        reports.publish_for_test(Round {
            scope: committed.clone(),
            rows: BTreeMap::new(),
        });
    };
    publish(&mut reports);
    let mut replacement = committed.clone();
    replacement.session = SessionId::parse(&"7".repeat(32)).unwrap();
    reports.tick(Some(replacement));
    assert!(reports.round.is_none());
    // Commits that outrun every read never call it a failure, and the round still ages out.
    publish(&mut reports);
    let mut later = committed.clone();
    later.revision = WorkbenchRevision::new(3);
    elapsed.store(2 * ROUND_FRESH_FOR.as_millis() as u64 + 2, Ordering::SeqCst);
    reports.tick(Some(later));
    assert!(reports.round.is_none());
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
