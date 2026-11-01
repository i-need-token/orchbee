use super::*;
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::agent_notifications::AgentNotificationKind;
use orchbee_workbench_protocol::{TerminalAgentStateSnapshot, WorkbenchRevision};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

fn round(sequence: u64, state: &str) -> PhysicalRound {
    let row: TerminalAgentStateSnapshot = serde_json::from_value(serde_json::json!({
        "binding":{"presentationId":format!("{:032x}", 1),"generation":format!("{:032x}", 2),
        "hostEpoch":"1".repeat(32),"hostTerminalId":format!("{:032x}", 3)},
        "sequence":sequence.to_string(),"agent":"pi","state":state
    }))
    .unwrap();
    PhysicalRound {
        report_revision: WorkbenchRevision::new(sequence),
        states: vec![row],
    }
}

#[test]
fn a_failed_round_keeps_the_notification_baseline_until_it_ages_out() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mut observer = PhysicalObserver::new(clock.clone());
    observer.scheduler.configure(
        1,
        AgentNotificationPolicy {
            popup_enabled: true,
            delay_millis: 0,
            ..AgentNotificationPolicy::default()
        },
    );
    observer.settle(clock.now(), Ok(round(1, "working")));
    // The Agent finishes: its commit lands inside this read and fails it.
    assert!(!observer.settle(clock.now(), Err(LaneFailure::Unavailable)));
    assert!(observer.dirty);
    assert!(observer.state.view_version().is_some());
    // The retry sees the finished state and still knows what it finished from.
    observer.settle(clock.now(), Ok(round(2, "done")));
    observer.after_draw();
    assert_eq!(
        observer
            .take_notification(true)
            .map(|(_, command)| command.kind),
        Some(AgentNotificationKind::Done)
    );
    // Sustained loss is a real gap: the next complete round is a fresh baseline.
    elapsed.store(
        (ROUND_FRESH_FOR + Duration::from_millis(1)).as_millis() as u64,
        Ordering::SeqCst,
    );
    assert!(observer.settle(clock.now(), Err(LaneFailure::Unavailable)));
    assert!(observer.state.view_version().is_none());
    observer.settle(clock.now(), Ok(round(3, "working")));
    observer.settle(clock.now(), Ok(round(4, "working")));
    observer.after_draw();
    assert!(observer.take_notification(true).is_none());
}
