use super::super::{
    AttachmentKey, ClientAction, ClientActionDisposition, Geometry, LocalOperationClock,
    OperationDeadlines, QueuedCommand, WorkerEvent,
};
use super::*;
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::mailbox::{CommandLease, EventDeadline, UiWaitKey};
use crate::workbench_state::{ControllerMode, FrameIdentity, PaneFault};
use std::sync::atomic::{AtomicU64, Ordering};

fn controlling(id: u32) -> ControllerMode {
    ControllerMode::Controlling(
        orchbee_client_server_protocol::ControllerId::parse(&format!("{id:032x}")).unwrap(),
    )
}

fn ready() -> (
    WorkbenchClientUi,
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
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_preferred_workspace(
                    &crate::workbench_client::ui::tests::membership::snapshot(2, 1),
                )
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
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
    ui: &mut WorkbenchClientUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
) -> (CommandLease, MembershipCleanup) {
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    ));
    let lease = mailbox.try_recv().unwrap().unwrap();
    let crate::workbench_client::mailbox::QueuedCommand::Switch(marker) = lease.record() else {
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
                    snapshot: crate::workbench_client::ui::tests::membership::snapshot(2, 1),
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
        super::super::tests::move_membership::ready_ui_with_snapshot(
            super::super::tests::publication_hint::split_snapshot(1),
        );
    let focus = super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    super::super::tests::publication_hint::attach_split_peer(&mut ui, &mailbox, &clock);
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
    let frame = super::super::tests::viewport::frame(&projection.value, 0, false, 24);
    projection.value.install_frame_for_test(frame);
    let state = ui.state.as_mut().unwrap();
    assert!(state.stage_drawn_frame(&sibling, FrameIdentity::new(7)));
    assert!(state.mark_feedback_accepted(&sibling, FrameIdentity::new(7)));
    let expected = state.pollable_panes();
    assert_eq!(expected.len(), 2);

    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
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

/// A focused pane that lost its viewer leaves its split sibling live, so a Switch away from it
/// owns that sibling: a refusal before cleanup restores the sibling's exact projection, and the
/// lost selection stays retryable instead of costing the Client.
#[test]
fn split_refusal_after_a_lost_focused_viewer_restores_the_live_sibling() {
    use crate::workbench_state::{OPERATION_TOTAL, PaneFault};
    let (mut ui, mailbox, clock, _elapsed) =
        super::super::tests::move_membership::ready_ui_with_snapshot(
            super::super::tests::publication_hint::split_snapshot(1),
        );
    let focus = super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    super::super::tests::publication_hint::attach_split_peer(&mut ui, &mailbox, &clock);
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
    let frame = super::super::tests::viewport::frame(&projection.value, 0, false, 24);
    projection.value.install_frame_for_test(frame);
    let state = ui.state.as_mut().unwrap();
    assert!(state.stage_drawn_frame(&sibling, FrameIdentity::new(7)));
    assert!(state.mark_feedback_accepted(&sibling, FrameIdentity::new(7)));
    mailbox
        .publish_notice_before(
            WorkerEvent::PaneFailed {
                key: focus.clone(),
                fault: PaneFault::ViewerLost,
            },
            clock.now() + OPERATION_TOTAL,
            &clock,
        )
        .unwrap();
    ui.tick();
    let state = ui.state.as_ref().unwrap();
    assert!(state.focused_viewer_lost());
    assert_eq!(state.pollable_panes(), std::slice::from_ref(&sibling));
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }

    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
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
    assert!(mailbox.is_closed().is_none());
    assert!(!ui.retirement_requested());
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    assert!(matches!(settlement.decision(), RefreshDecision::Continue));
    assert_eq!(settlement.desired(), Some(focus.pane()));
    mailbox.finish_lease(lease).unwrap();
    let state = ui.state.as_ref().unwrap();
    assert!(state.focused_viewer_lost());
    assert_eq!(state.focus_pane_ref(), Some(focus));
    assert_eq!(state.pollable_panes(), [sibling]);
    assert!(state.invariant_holds());
    assert!(ui.view().pane_projection(&peer).unwrap().frame().is_some());
}

/// Finishes the queued sibling work the executor would run first and leases the next Switch.
fn leased_switch(mailbox: &CommandMailbox) -> CommandLease {
    loop {
        let lease = mailbox.try_recv().unwrap().expect("a queued Switch");
        if matches!(lease.record(), QueuedCommand::Switch(_)) {
            return lease;
        }
        mailbox.finish_lease(lease).unwrap();
    }
}

/// Answers the leased Switch with `snapshot`, runs the cleanup of the old anchor's live viewers
/// that the install asks for, and returns the pane the Switch attaches.
fn land_switch(
    ui: &mut WorkbenchClientUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    lease: CommandLease,
    snapshot: orchbee_workbench_protocol::Snapshot,
) -> PaneKey {
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease")
    };
    let target = marker.target().clone();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, &target, clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available { snapshot },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a Switch that lands never costs the Client"
    );
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("the old anchor's live viewers are cleaned up first")
    };
    let permit = mailbox
        .authorize_switch_cleanup(&lease, cleanup, cleanup.old(), clock)
        .unwrap()
        .expect("the Switch owns its cleanup");
    assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
    let wait = detached(mailbox, clock, cleanup);
    ui.tick();
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    let RefreshDecision::AttachRecovered(attach) = settlement.decision() else {
        panic!("the Switch attaches")
    };
    let landed = attach.selected().key().clone();
    assert_eq!(settlement.desired(), Some(&landed));
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(mailbox.is_closed(), None);
    assert!(!ui.retirement_requested());
    assert!(ui.state.as_ref().unwrap().invariant_holds());
    landed
}

/// A lost focus re-attaches through a same-Tab Switch. When the Server removed that Panel before
/// the Switch read its snapshot, the Switch lands on the surviving leaf.
#[test]
fn a_reattach_whose_panel_was_removed_lands_on_the_surviving_leaf() {
    use crate::workbench_state::{OPERATION_TOTAL, PaneFault};
    use std::sync::atomic::Ordering;
    let (mut ui, mailbox, clock, elapsed) = super::super::tests::publication_hint::ready_split_ui();
    super::super::tests::publication_hint::attach_split_peer(&mut ui, &mailbox, &clock);
    let tab = &ui.state.as_ref().unwrap().cache().tabs()[0];
    let survivor = tab.panel(0).unwrap().panel_id().clone();
    let removed = tab.panel(1).unwrap().panel_id().clone();
    ui.apply_client_action(ClientAction::FocusPanel(removed.clone()));
    let focus = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    assert_eq!(focus.pane().panel(), &removed);
    mailbox
        .publish_notice_before(
            WorkerEvent::PaneFailed {
                key: focus.clone(),
                fault: PaneFault::ViewerLost,
            },
            clock.now() + OPERATION_TOTAL,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(ui.state.as_ref().unwrap().focused_viewer_lost());
    elapsed.fetch_add(300, Ordering::SeqCst);
    ui.tick();
    let lease = leased_switch(&mailbox);
    let landed = land_switch(
        &mut ui,
        &mailbox,
        &clock,
        lease,
        super::super::tests::publication_hint::split_survivor_snapshot(2, 0),
    );
    assert_eq!(landed.tab_id(), focus.pane().tab_id());
    assert_eq!(
        landed.panel(),
        &survivor,
        "the surviving leaf takes the focus"
    );
}

/// A peer removed the Panel a Tab selection targets before this Client installed that removal:
/// the Switch lands on the Tab's surviving leaf.
#[test]
fn a_tab_switch_whose_panel_was_removed_lands_on_the_surviving_leaf() {
    use super::super::tests::publication_hint::{split_snapshot, split_survivor_snapshot};
    // The split Tab comes second, so the Client starts on the single-leaf Tab.
    fn split_second(
        mut snapshot: orchbee_workbench_protocol::Snapshot,
    ) -> orchbee_workbench_protocol::Snapshot {
        snapshot.terminal_contexts[0].tabs.swap(0, 1);
        snapshot
    }
    let (mut ui, mailbox, clock, _elapsed) =
        super::super::tests::move_membership::ready_ui_with_snapshot(split_second(split_snapshot(
            1,
        )));
    let tab = &ui.state.as_ref().unwrap().cache().tabs()[1];
    let tab_id = tab.tab_id().clone();
    let removed = tab.panel(0).unwrap().panel_id().clone();
    let survivor = tab.panel(1).unwrap().panel_id().clone();
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = leased_switch(&mailbox);
    let QueuedCommand::Switch(marker) = lease.record() else {
        unreachable!("leased_switch answers a Switch")
    };
    assert_eq!(marker.target().selected().key().panel(), &removed);
    let landed = land_switch(
        &mut ui,
        &mailbox,
        &clock,
        lease,
        split_second(split_survivor_snapshot(2, 1)),
    );
    assert_eq!(landed.tab_id(), &tab_id);
    assert_eq!(
        landed.panel(),
        &survivor,
        "the surviving leaf takes the focus"
    );
}

/// A peer closed this Client's Workspace before a Tab Switch read its snapshot. A Switch cannot
/// land outside its context, so it restores the selection and owes the publication read whose
/// refresh classifies the removal and lands on the surviving Workspace.
#[test]
fn a_switch_whose_workspace_was_closed_restores_and_the_refresh_lands() {
    use super::super::tests::membership::{removal_context, removal_snapshot};
    let (mut ui, mailbox, clock, _elapsed) = ready();
    let focus = super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    ));
    let lease = leased_switch(&mailbox);
    let QueuedCommand::Switch(marker) = lease.record() else {
        unreachable!("leased_switch answers a Switch")
    };
    let target = marker.target();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, target, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: removal_snapshot(2, 2),
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a peer Workspace close never costs a Switch the Client"
    );
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    assert!(matches!(settlement.decision(), RefreshDecision::Continue));
    assert_eq!(settlement.desired(), Some(focus.pane()));
    mailbox.finish_lease(lease).unwrap();

    ui.tick();
    let lease = loop {
        let lease = mailbox
            .try_recv()
            .unwrap()
            .expect("a publication read follows");
        if matches!(lease.record(), QueuedCommand::Refresh(_)) {
            break lease;
        }
        mailbox.finish_lease(lease).unwrap();
    };
    let QueuedCommand::Refresh(refresh) = lease.record() else {
        unreachable!("the loop answers a Refresh")
    };
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(refresh.generation())
            .unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_refresh_result_before(
            refresh.generation(),
            WorkbenchSnapshotOutcome::Available {
                snapshot: removal_snapshot(2, 2),
            },
            refresh.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Refresh(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Refresh reply")
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("the removed Workspace's viewer is cleaned up first")
    };
    let wait = detached(&mailbox, &clock, cleanup);
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap().unwrap(),
        UiReply::Membership(_)
    ));
    mailbox.finish_lease(lease).unwrap();
    let state = ui.state.as_ref().unwrap();
    assert_eq!(
        state.cache().location().terminal_context_id(),
        Some(&removal_context())
    );
    assert_eq!(mailbox.is_closed(), None);
    assert!(state.invariant_holds());
}

/// A window resize while a Switch is in flight is only a newer observation: a refused Switch
/// still restores its selection, and the restored focus follows the window's current size.
#[test]
fn a_switch_refused_after_a_resize_restores_and_resizes_the_focus() {
    let (mut ui, mailbox, clock, _elapsed) = super::super::test_support::controlling_ui();
    let focus = super::super::test_support::install_projection(&mut ui, Some((0, false, 24)), 0);
    assert!(matches!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    ));
    let resized = Geometry::new(100, 30, 8, 16).unwrap();
    ui.observe_geometry(resized);
    let lease = leased_switch(&mailbox);
    let QueuedCommand::Switch(marker) = lease.record() else {
        unreachable!("leased_switch answers a Switch")
    };
    let target = marker.target();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, target, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Unavailable {
                    reason: orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a resize during a Switch never costs the Client"
    );
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    assert!(matches!(settlement.decision(), RefreshDecision::Continue));
    assert_eq!(settlement.desired(), Some(focus.pane()));
    mailbox.finish_lease(lease).unwrap();

    // The shell observes the restored layout at the window's current size.
    ui.observe_geometry(resized);
    let resize = loop {
        let lease = mailbox
            .try_recv()
            .unwrap()
            .expect("the restored focus follows the window");
        if let QueuedCommand::Resize(resize) = lease.record() {
            let resize = (resize.key().clone(), *resize.value());
            mailbox.finish_lease(lease).unwrap();
            break resize;
        }
        mailbox.finish_lease(lease).unwrap();
    };
    assert_eq!(resize, (focus, resized));
    assert_eq!(mailbox.is_closed(), None);
    assert!(ui.state.as_ref().unwrap().invariant_holds());
}

/// Controlling Resizes the Host has not acknowledged when a Switch starts settle while the Switch
/// is in flight. Neither those acknowledgements nor a refused Switch cost the Client: the restore
/// returns both panes with their geometry unknown, and the next observation re-sends both Resizes.
#[test]
fn a_switch_refused_while_resizes_await_the_host_restores_and_resends_them() {
    use super::super::tests::publication_hint::{
        attach_split_peer_with_mode, ready_split_ui_with_mode,
    };
    let (mut ui, mailbox, clock, _elapsed) = ready_split_ui_with_mode(controlling(8));
    attach_split_peer_with_mode(&mut ui, &mailbox, &clock, controlling(9));
    let state = ui.state.as_ref().unwrap();
    let focus_panel = state.focus_panel_id().unwrap().clone();
    let peer_panel = state.cache().tabs()[0].panel(1).unwrap().panel_id().clone();
    let focus = state.focus_pane_ref().unwrap();
    let peer = state.pane_by_panel(&peer_panel).unwrap().pane_ref();
    let projection = ui
        .projections
        .iter_mut()
        .find(|item| item.key == peer)
        .unwrap();
    let frame = super::super::tests::viewport::frame(&projection.value, 0, false, 24);
    projection.value.install_frame_for_test(frame);
    let state = ui.state.as_mut().unwrap();
    assert!(state.stage_drawn_frame(&peer, FrameIdentity::new(7)));
    assert!(state.mark_feedback_accepted(&peer, FrameIdentity::new(7)));

    // The window grows, so both controlling panes owe the Host a Resize.
    let grown = [
        (focus_panel, 0, Geometry::new(50, 30, 8, 16).unwrap()),
        (peer_panel, 1, Geometry::new(49, 30, 8, 16).unwrap()),
    ];
    ui.observe_panes(&grown);
    // The Switch starts before the Host acknowledged either Resize.
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    // The executor runs the earlier Resizes first, so both acknowledgements settle while the
    // Switch is in flight.
    let modes = [
        (focus.clone(), controlling(8)),
        (peer.clone(), controlling(9)),
    ];
    let mut acknowledged = Vec::new();
    let lease = loop {
        let lease = mailbox.try_recv().unwrap().expect("a queued Switch");
        if matches!(lease.record(), QueuedCommand::Switch(_)) {
            break lease;
        }
        if let QueuedCommand::Resize(resize) = lease.record() {
            let (key, mode) = modes
                .iter()
                .find(|(key, _)| key == resize.key())
                .unwrap()
                .clone();
            let wait = mailbox
                .publish_event_before(
                    WorkerEvent::SetupConfigured {
                        key: key.clone(),
                        mode,
                        applied_geometry: Some(*resize.value()),
                    },
                    EventDeadline::Forward(clock.now() + crate::workbench_state::OPERATION_TOTAL),
                    &clock,
                )
                .unwrap();
            ui.tick();
            assert_eq!(
                mailbox.is_closed(),
                None,
                "an acknowledged Resize never costs the Client"
            );
            assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
            acknowledged.push(key);
        }
        mailbox.finish_lease(lease).unwrap();
    };
    assert_eq!(acknowledged, [focus.clone(), peer.clone()]);
    let QueuedCommand::Switch(marker) = lease.record() else {
        unreachable!("the loop breaks on a Switch")
    };
    let target = marker.target();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, target, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Unavailable {
                    reason: orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a refused Switch never costs the Client"
    );
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    assert!(matches!(settlement.decision(), RefreshDecision::Continue));
    assert_eq!(settlement.desired(), Some(focus.pane()));
    mailbox.finish_lease(lease).unwrap();

    // The shell observes the restored layout at the window's current size.
    ui.observe_panes(&grown);
    let mut resizes = Vec::new();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        if let QueuedCommand::Resize(resize) = lease.record() {
            resizes.push((resize.key().clone(), *resize.value()));
        }
        mailbox.finish_lease(lease).unwrap();
    }
    assert_eq!(resizes, [(focus, grown[0].2), (peer, grown[1].2)]);
    assert_eq!(mailbox.is_closed(), None);
    assert!(ui.state.as_ref().unwrap().invariant_holds());
}

fn next_resize(mailbox: &CommandMailbox) -> CommandLease {
    loop {
        let lease = mailbox.try_recv().unwrap().expect("a queued Resize");
        if matches!(lease.record(), QueuedCommand::Resize(_)) {
            return lease;
        }
        mailbox.finish_lease(lease).unwrap();
    }
}

/// Refuses the leased Switch whose restore point is unusable. A refused Switch never costs the
/// Client: the viewers left under the origin's anchor are detached, and the origin is selected
/// again without a viewer.
fn refuse_unrestorable_switch(
    ui: &mut WorkbenchClientUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    lease: CommandLease,
    origin: &PaneKey,
    viewers_left: bool,
) {
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease")
    };
    let target = marker.target().clone();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, &target, clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Unavailable {
                    reason: orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a refused Switch never costs the Client"
    );
    let UiReply::Switch(mut settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap()
    else {
        panic!("Switch reply")
    };
    if viewers_left {
        let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
            panic!("the viewers the restore cannot vouch for are detached")
        };
        assert_eq!(cleanup.old(), origin.anchor());
        let permit = mailbox
            .authorize_switch_cleanup(&lease, cleanup, cleanup.old(), clock)
            .unwrap()
            .expect("the Switch owns its cleanup");
        assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
        let wait = detached(mailbox, clock, cleanup);
        ui.tick();
        assert_eq!(
            mailbox.is_closed(),
            None,
            "a refused Switch never costs the Client"
        );
        let UiReply::Switch(parked) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
            panic!("Switch reply")
        };
        settlement = parked;
    }
    assert!(matches!(settlement.decision(), RefreshDecision::Continue));
    assert_eq!(settlement.desired(), Some(origin));
    mailbox.finish_lease(lease).unwrap();
    let state = ui.state.as_ref().unwrap();
    assert!(
        state.focus_parked(),
        "the origin is selected without a viewer"
    );
    assert!(state.invariant_holds());
}

/// A parked focus that had a viewer waits as a lost one: after its pause, the Client re-attaches
/// that same panel on its own.
fn parked_focus_reattaches(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    elapsed: &AtomicU64,
    origin: &PaneKey,
) {
    assert!(ui.state.as_ref().unwrap().focused_viewer_lost());
    elapsed.fetch_add(1_000, Ordering::SeqCst);
    ui.tick();
    let reattach = loop {
        let lease = mailbox
            .try_recv()
            .unwrap()
            .expect("the parked focus re-attaches on its own");
        if let QueuedCommand::Switch(marker) = lease.record() {
            break marker.target().selected().key().clone();
        }
        mailbox.finish_lease(lease).unwrap();
    };
    assert_eq!(reattach.anchor().tab_id(), origin.anchor().tab_id());
    assert_eq!(reattach.panel(), origin.panel());
    assert_eq!(mailbox.is_closed(), None);
    assert!(!ui.retirement_requested());
}

/// The Host acknowledged a Resize, so the focus has no frame proof until a frame at the new size
/// is drawn. A Switch that starts in that gap and is refused cannot restore it.
#[test]
fn a_switch_refused_before_a_resized_frame_parks_and_reattaches_the_focus() {
    let (mut ui, mailbox, clock, elapsed) = super::super::test_support::controlling_ui();
    let focus = super::super::test_support::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.observe_geometry(Geometry::new(100, 30, 8, 16).unwrap());
    let lease = next_resize(&mailbox);
    let QueuedCommand::Resize(resize) = lease.record() else {
        unreachable!("next_resize answers a Resize")
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SetupConfigured {
                key: resize.key().clone(),
                mode: controlling(8),
                applied_geometry: Some(*resize.value()),
            },
            EventDeadline::Forward(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = leased_switch(&mailbox);
    refuse_unrestorable_switch(&mut ui, &mailbox, &clock, lease, focus.pane(), true);
    parked_focus_reattaches(&mut ui, &mailbox, &elapsed, focus.pane());
}

/// A Resize admitted before a Switch runs ahead of it. When the Host refuses it because another
/// Client took control, the focus becomes view-only while the Switch is in flight, so a refused
/// Switch cannot restore the controlling pane it captured.
#[test]
fn a_switch_refused_after_the_focus_lost_control_parks_and_reattaches_the_focus() {
    let (mut ui, mailbox, clock, elapsed) = super::super::test_support::controlling_ui();
    let focus = super::super::test_support::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.observe_geometry(Geometry::new(100, 30, 8, 16).unwrap());
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = next_resize(&mailbox);
    mailbox
        .publish_notice_before(
            WorkerEvent::PaneFailed {
                key: focus.clone(),
                fault: PaneFault::ControlLost,
            },
            clock.now() + crate::workbench_state::OPERATION_TOTAL,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "losing control never costs the Client"
    );
    mailbox.finish_lease(lease).unwrap();
    let lease = leased_switch(&mailbox);
    refuse_unrestorable_switch(&mut ui, &mailbox, &clock, lease, focus.pane(), true);
    parked_focus_reattaches(&mut ui, &mailbox, &elapsed, focus.pane());
}

/// A Switch admitted while an earlier one cleans up its origin is left without an anchor once that
/// cleanup lands. Refused, it parks the origin with no viewer left to detach.
#[test]
fn a_second_switch_refused_after_the_first_cleaned_up_parks_and_reattaches_the_origin() {
    let (mut ui, mailbox, clock, elapsed) = ready();
    let origin = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let (lease, cleanup) = stage(&mut ui, &mailbox, &clock);
    let permit = mailbox
        .authorize_switch_cleanup(&lease, &cleanup, cleanup.old(), &clock)
        .unwrap()
        .unwrap();
    assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(1)),
        ClientActionDisposition::DismissNavigator
    );
    let wait = detached(&mailbox, &clock, &cleanup);
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    let lease = leased_switch(&mailbox);
    refuse_unrestorable_switch(&mut ui, &mailbox, &clock, lease, &origin, false);
    parked_focus_reattaches(&mut ui, &mailbox, &elapsed, &origin);
}

/// A split Tab whose first leaf failed to launch, beside a live leaf. The Client leaves for the
/// second Tab and comes straight back, so the Switch back lands on the failed leaf, which has no
/// viewer of its own, and the live leaf then attaches through reconcile.
fn launch_failed_focus_beside_a_live_sibling() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    orchbee_workbench_protocol::Snapshot,
    PaneKey,
) {
    use orchbee_workbench_protocol::PanelNode;
    let mut snapshot = super::super::tests::publication_hint::split_snapshot(1);
    let PanelNode::Split { children, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
        unreachable!("the split fixture")
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[0].node
    else {
        unreachable!("the split fixture")
    };
    primary_presentation.launch_failure =
        Some(orchbee_workbench_protocol::PresentationLaunchFailure {
            executable: None,
            reason:
                orchbee_workbench_protocol::WorkbenchTerminalFailureReason::TerminalCreateFailed,
        });
    let (mut ui, mailbox, clock, _elapsed) =
        super::super::tests::move_membership::ready_ui_with_snapshot(snapshot.clone());
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = leased_switch(&mailbox);
    land_switch(&mut ui, &mailbox, &clock, lease, snapshot.clone());
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(1)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = leased_switch(&mailbox);
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease")
    };
    let target = marker.target().clone();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, &target, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply")
    };
    assert!(matches!(
        settlement.decision(),
        RefreshDecision::SettledWithoutAttachment
    ));
    mailbox.finish_lease(lease).unwrap();
    super::super::tests::publication_hint::attach_split_peer(&mut ui, &mailbox, &clock);
    let state = ui.state.as_ref().unwrap();
    assert!(state.focus_parked() && !state.selected_is_unbound());
    let origin = state.desired().unwrap().clone();
    (ui, mailbox, clock, snapshot, origin)
}

/// A Switch from a focused leaf that failed to launch first detaches the live sibling under its
/// anchor, as a Switch from any selection without its own viewer does.
#[test]
fn a_switch_from_a_launch_failed_leaf_detaches_its_live_sibling_and_lands() {
    let (mut ui, mailbox, clock, snapshot, origin) = launch_failed_focus_beside_a_live_sibling();
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = leased_switch(&mailbox);
    let landed = land_switch(&mut ui, &mailbox, &clock, lease, snapshot);
    assert_ne!(landed.anchor().tab_id(), origin.anchor().tab_id());
}

/// The same Switch refused cannot restore the sibling beside a leaf that never attaches. It
/// detaches the sibling, parks the failed leaf, and the sibling attaches again once observed.
#[test]
fn a_switch_refused_from_a_launch_failed_leaf_parks_it_and_reattaches_its_sibling() {
    let (mut ui, mailbox, clock, _snapshot, origin) = launch_failed_focus_beside_a_live_sibling();
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = leased_switch(&mailbox);
    refuse_unrestorable_switch(&mut ui, &mailbox, &clock, lease, &origin, true);
    let state = ui.state.as_ref().unwrap();
    let peer = state.cache().tabs()[0].panel(1).unwrap().panel_id().clone();
    let sibling = PaneKey::new(origin.anchor().clone(), peer.clone());
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    ui.observe_panes(&[(origin.panel().clone(), 0, geometry), (peer, 1, geometry)]);
    let mut reconciled = false;
    while let Some(lease) = mailbox.try_recv().unwrap() {
        reconciled |= matches!(lease.record(), QueuedCommand::ReconcilePanes(marker)
            if marker.targets().iter().any(|target| target.pane_ref().pane() == &sibling));
        mailbox.finish_lease(lease).unwrap();
    }
    assert!(reconciled, "the sibling attaches again through reconcile");
    assert_eq!(mailbox.is_closed(), None);
    assert!(!ui.retirement_requested());
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
        ClientActionDisposition::DismissNavigator
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
        ClientActionDisposition::DismissNavigator
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
        ClientActionDisposition::DismissNavigator
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
        ClientActionDisposition::DismissNavigator
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
        ClientActionDisposition::DismissNavigator
    ));
    let lease = mailbox.try_recv().unwrap().unwrap();
    let crate::workbench_client::mailbox::QueuedCommand::Switch(marker) = lease.record() else {
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
            ClientActionDisposition::DismissNavigator
        ));
        let lease = mailbox.try_recv().unwrap().unwrap();
        let crate::workbench_client::mailbox::QueuedCommand::Switch(marker) = lease.record() else {
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
            ClientActionDisposition::DismissNavigator
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
