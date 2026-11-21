//! The Ready Client that UI, connection and menu tests start from: one bootstrapped projection
//! drawn and published under an adjustable clock.
use super::super::*;
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::mailbox::EventDeadline;
use orchbee_workbench_protocol::Snapshot;
use std::sync::atomic::AtomicU64;

pub(in crate::workbench_client) fn ready_ui() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    ready_ui_with_snapshot(super::super::tests::membership::snapshot(2, 1))
}

pub(in crate::workbench_client) fn ready_ui_with_snapshot(
    source: Snapshot,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    ready_ui_with_mode(source, ControllerMode::ReadOnly)
}

pub(in crate::workbench_client) fn ready_ui_with_mode(
    source: Snapshot,
    mode: ControllerMode,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_preferred_workspace(&source).unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    let geometry = matches!(mode, ControllerMode::Controlling(_))
        .then(|| Geometry::new(80, 24, 8, 16).unwrap());
    assert!(state.record_setup_configuration_t(&key, mode.clone(), geometry));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&key, FrameIdentity::new(1), mode, geometry));
    (ui, mailbox, clock, elapsed)
}
