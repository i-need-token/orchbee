use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::{
    mailbox::{CommandLease, EventDeadline, UiWaitKey},
    state::{ControllerMode, FrameIdentity},
};
use std::sync::atomic::AtomicU64;

fn ready() -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        100,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                cache:
                    crate::terminal_session::workbench_projection::project_native_standalone_tabs(
                        &crate::terminal_session::ui::membership_tests::snapshot(2, 1),
                    )
                    .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&key, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    (ui, mailbox, clock, elapsed)
}

fn stage(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
) -> (CommandLease, MembershipCleanup) {
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(2)),
        SessionActionDisposition::DismissNavigator
    ));
    let lease = mailbox.try_recv().unwrap().unwrap();
    let crate::terminal_session::mailbox::QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease")
    };
    let target = marker.target();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, target, clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: crate::terminal_session::ui::membership_tests::snapshot(2, 1),
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("candidate staged")
    };
    (lease, cleanup.clone())
}

fn detached(
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    cleanup: &MembershipCleanup,
) -> UiWaitKey {
    mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            clock,
        )
        .unwrap()
}

#[test]
fn split_refusal_restores_both_visible_projections_and_local_focus_through_mailbox() {
    let (mut ui, mailbox, clock, _elapsed) =
        super::super::move_membership_tests::ready_ui_with_snapshot(
            super::super::publication_hint_tests::split_snapshot(1),
        );
    let focus = super::super::viewport_tests::install_projection(&mut ui, Some((0, false, 24)), 0);
    super::super::publication_hint_tests::attach_split_peer(&mut ui, &mailbox, &clock);
    let peer = ui.state.as_ref().unwrap().cache().tabs()[0]
        .panel(1)
        .unwrap()
        .panel_id()
        .clone();
    let sibling = ui
        .state
        .as_ref()
        .unwrap()
        .pane_by_panel(&peer)
        .unwrap()
        .pane_ref();
    let projection = ui
        .projections
        .iter_mut()
        .find(|item| item.key == sibling)
        .unwrap();
    let frame = super::super::viewport_tests::frame(&projection.value, 0, false, 24);
    projection.value.install_frame_for_test(frame);
    let state = ui.state.as_mut().unwrap();
    assert!(state.stage_drawn_frame(&sibling, FrameIdentity::new(7)));
    assert!(state.mark_feedback_accepted(&sibling, FrameIdentity::new(7)));
    let expected = state.pollable_panes();
    assert_eq!(expected.len(), 2);

    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        SessionActionDisposition::DismissNavigator
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease")
    };
    let target = marker.target();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, target, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let cursors = mailbox.cursors();
    let wait = mailbox.publish_event_before(
        WorkerEvent::SwitchSnapshotResult {
            marker: target.selected().key().clone(),
            outcome: WorkbenchSnapshotOutcome::Unavailable {
                reason: orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        },
        EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
        &clock,
    ).unwrap();
    ui.tick();
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    assert!(matches!(settlement.decision(), RefreshDecision::Continue));
    assert_eq!(settlement.desired(), Some(focus.pane()));
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(mailbox.cursors(), cursors);
    assert!(mailbox.is_closed().is_none());
    assert_eq!(ui.state.as_ref().unwrap().pollable_panes(), expected);
    assert!(ui.view().pane_projection(&peer).unwrap().frame().is_some());
    ui.apply_client_action(ClientAction::FocusPanel(peer));
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), Some(sibling));
    assert!(ui.state.as_ref().unwrap().ready().is_some());
    assert!(ui.state.as_ref().unwrap().invariant_holds());
    let poll = mailbox
        .try_recv()
        .unwrap()
        .expect("ordinary polling resumes after rollback");
    assert!(matches!(poll.record(), QueuedCommand::Poll(_)));
    mailbox.finish_lease(poll).unwrap();
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "rollback and local focus never reattach"
    );
}

#[test]
fn response_before_ui_new_marker_preserves_exact_cleanup_and_never_installs_stale_candidate() {
    let (mut ui, mailbox, clock, _elapsed) = ready();
    let (lease, cleanup) = stage(&mut ui, &mailbox, &clock);
    let permit = mailbox
        .authorize_switch_cleanup(&lease, &cleanup, cleanup.old(), &clock)
        .unwrap()
        .unwrap();
    assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(1)),
        SessionActionDisposition::DismissNavigator
    ));
    let current = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let wait = detached(&mailbox, &clock, &cleanup);
    ui.tick();
    let state = ui.state.as_ref().unwrap();
    assert_eq!(state.desired(), Some(&current));
    assert_eq!(state.attached_key(), None);
    assert!(state.invariant_holds());
    assert!(
        matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Switch(settlement)) if matches!(settlement.decision(), RefreshDecision::Continue))
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn ui_commit_before_worker_read_allows_new_marker_and_exact_old_reply_consumption() {
    let (mut ui, mailbox, clock, _elapsed) = ready();
    let (lease, cleanup) = stage(&mut ui, &mailbox, &clock);
    let intent = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let permit = mailbox
        .authorize_switch_cleanup(&lease, &cleanup, cleanup.old(), &clock)
        .unwrap()
        .unwrap();
    assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
    let wait = detached(&mailbox, &clock, &cleanup);
    ui.tick();
    let installed = ui.state.as_ref().unwrap().desired().unwrap().clone();
    assert_eq!(installed.generation(), intent.generation() + 1);
    assert_eq!(installed.tab_id(), intent.tab_id());
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(1)),
        SessionActionDisposition::DismissNavigator
    ));
    let current = ui.state.as_ref().unwrap().desired().unwrap().clone();
    assert_eq!(current.generation(), installed.generation() + 1);
    assert!(
        matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Switch(settlement)) if settlement.desired() == Some(&installed))
    );
    assert_eq!(
        mailbox.execution_fence(&lease).desired().key(),
        Some(&current)
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn superseded_busy_waits_for_typed_ui_settlement_and_preserves_viewer() {
    let (mut ui, mailbox, clock, _elapsed) = ready();
    let (lease, cleanup) = stage(&mut ui, &mailbox, &clock);
    let permit = mailbox
        .authorize_switch_cleanup(&lease, &cleanup, cleanup.old(), &clock)
        .unwrap()
        .unwrap();
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(1)),
        SessionActionDisposition::DismissNavigator
    ));
    assert!(mailbox.answer_switch_cleanup(permit, false).unwrap());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipCleanupSkipped {
                cleanup: cleanup.clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        ui.state
            .as_ref()
            .unwrap()
            .attached_key()
            .map(|pane| pane.anchor()),
        Some(cleanup.old())
    );
    assert!(ui.state.as_ref().unwrap().invariant_holds());
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Switch(_))
    ));
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn new_marker_before_cleanup_authorization_has_no_request_cursor_effect() {
    let (mut ui, mailbox, clock, _elapsed) = ready();
    let (lease, cleanup) = stage(&mut ui, &mailbox, &clock);
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(1)),
        SessionActionDisposition::DismissNavigator
    ));
    let cursors = mailbox.cursors();
    assert!(
        mailbox
            .authorize_switch_cleanup(&lease, &cleanup, cleanup.old(), &clock)
            .unwrap()
            .is_none()
    );
    assert_eq!(mailbox.cursors(), cursors);
    assert_eq!(
        ui.state
            .as_ref()
            .unwrap()
            .attached_key()
            .map(|pane| pane.anchor()),
        Some(cleanup.old())
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn forward_expired_before_snapshot_cleans_exact_old_viewer_without_fabricating_empty_candidate() {
    let (mut ui, mailbox, clock, elapsed) = ready();
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(2)),
        SessionActionDisposition::DismissNavigator
    ));
    let lease = mailbox.try_recv().unwrap().unwrap();
    let crate::terminal_session::mailbox::QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease")
    };
    let target = marker.target().clone();
    let baseline = ui.state.as_ref().unwrap().cache().clone();
    let cursors = mailbox.cursors();
    elapsed.store(45_000, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        mailbox.authorize_switch_snapshot(&lease, &target, &clock),
        Err(CarrierFailure::Deadline)
    ));
    assert_eq!(mailbox.cursors(), cursors);
    let permit = mailbox
        .authorize_switch_expiry(&lease, &target, &clock)
        .unwrap()
        .unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchForwardExpired { permit },
            EventDeadline::TotalCleanup(target.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::SwitchExpired(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap()
    else {
        panic!("expired result")
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("expiry cleanup")
    };
    assert!(!cleanup.installs_candidate());
    let permit = mailbox
        .authorize_switch_cleanup(&lease, cleanup, cleanup.old(), &clock)
        .unwrap()
        .unwrap();
    assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
    let _wait = detached(&mailbox, &clock, cleanup);
    ui.tick();
    assert!(ui.retirement_requested());
    assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
    assert_eq!(mailbox.cursors().0, cursors.0);
}

#[test]
fn expiry_permit_rejects_new_target_on_old_lease_and_settles_superseded_original_without_effects() {
    for authorize_before_new in [false, true] {
        let (mut ui, mailbox, clock, elapsed) = ready();
        assert!(matches!(
            ui.select_tab(ClientAction::SelectTab(2)),
            SessionActionDisposition::DismissNavigator
        ));
        let lease = mailbox.try_recv().unwrap().unwrap();
        let crate::terminal_session::mailbox::QueuedCommand::Switch(marker) = lease.record() else {
            panic!("Switch lease")
        };
        let original = marker.target().clone();
        elapsed.store(45_000, std::sync::atomic::Ordering::SeqCst);
        let permit = authorize_before_new.then(|| {
            mailbox
                .authorize_switch_expiry(&lease, &original, &clock)
                .unwrap()
                .unwrap()
        });
        let state = ui.state.as_mut().unwrap();
        let current_target = state
            .target_for_switch(
                0,
                AttachmentKey::new(
                    mailbox.cursors().0 + 1,
                    state.tab(0).unwrap().tab_id().clone(),
                ),
                OperationDeadlines::from_sample(clock.now()).unwrap(),
            )
            .unwrap();
        assert!(matches!(
            ui.select_tab(ClientAction::SelectTab(1)),
            SessionActionDisposition::DismissNavigator
        ));
        let state = ui.state.as_ref().unwrap();
        let current = state.desired().unwrap().clone();
        assert_eq!(current_target.selected().key(), &current);
        let old = state.attached_key().cloned();
        let cursors = mailbox.cursors();
        assert!(matches!(
            mailbox.authorize_switch_expiry(&lease, &current_target, &clock),
            Err(CarrierFailure::Protocol)
        ));
        assert!(
            mailbox
                .authorize_switch_expiry(&lease, &original, &clock)
                .unwrap()
                .is_none()
        );
        if let Some(permit) = permit {
            let wait = mailbox
                .publish_event_before(
                    WorkerEvent::SwitchForwardExpired { permit },
                    EventDeadline::TotalCleanup(original.deadlines().total()),
                    &clock,
                )
                .unwrap();
            ui.tick();
            assert!(
                matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::SwitchExpired(settlement)) if matches!(settlement.decision(), RefreshDecision::Continue))
            );
        }
        assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&current));
        assert_eq!(ui.state.as_ref().unwrap().attached_key(), old.as_ref());
        assert_eq!(mailbox.cursors(), cursors);
        assert!(!ui.retirement_requested());
        mailbox.finish_lease(lease).unwrap();
    }
}
