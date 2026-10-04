use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::mailbox::{EventDeadline, SwitchMarker};
use crate::terminal_session::state::{
    CleanupContinuation, ControllerMode, FrameIdentity, PaneKey, SelectedTab, SessionNotice,
    SetupEventDisposition,
};
use crate::terminal_session::workbench_projection::NativeContextLocation;
use orchbee_workbench_protocol::*;
use std::sync::atomic::{AtomicU64, Ordering};

fn close_snapshot_waiting_ui(
    elapsed: Arc<AtomicU64>,
) -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    super::super::mailbox::CommandLease,
    super::super::mailbox::UiWaitKey,
) {
    use crate::local_client::workbench::WorkbenchTabCloseAccepted;
    use crate::terminal_session::state::CloseResult;
    let clock = adjustable_operation_clock(elapsed);
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let deadline = clock.now() + super::super::state::OPERATION_TOTAL;
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: super::super::workbench_projection::project_native_standalone_tabs(
                    &snapshot(1, 1),
                )
                .unwrap(),
            },
            EventDeadline::Startup(deadline),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let old = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&old));
    assert!(state.record_setup_configuration_t(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let close = state
        .plan_close_confirmation(
            WorkbenchRequestId::parse(&format!("{:032x}", 900)).unwrap(),
            deadline,
        )
        .unwrap();
    state.begin_close_exchange(&close, clock.now()).unwrap();
    state
        .settle_close_exchange(
            &close,
            &CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: close.key().request_id().clone(),
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                accepted_revision: close.selection().base_revision(),
            }),
        )
        .unwrap();
    let read = state
        .plan_close_read(
            WorkbenchRequestId::parse(&format!("{:032x}", 901)).unwrap(),
            deadline,
            clock.now(),
        )
        .unwrap();
    state.begin_close_exchange(&read, clock.now()).unwrap();
    state
        .settle_close_exchange(
            &read,
            &CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id: close.key().request_id().clone(),
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                status: WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(2),
                },
            }),
        )
        .unwrap();
    state.acknowledge_close_lease(
        crate::terminal_session::state::CloseLeaseAcknowledgement::Exchange(read.key().clone()),
    );
    ui.schedule_close_reconciliation(&mailbox);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::ReconcileClose(marker) = lease.record() else {
        panic!("Close Snapshot lease");
    };
    assert_eq!(
        mailbox.mark_close_reconciliation_in_flight(marker).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_close_reconciliation_before(
            marker.clone(),
            WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(0, 2),
            },
            deadline,
            &clock,
        )
        .unwrap();
    (ui, mailbox, clock, lease, wait)
}

#[test]
fn close_acknowledgement_after_empty_install_requests_the_status_removal_redraw() {
    let (mut ui, mailbox, clock, lease, wait) =
        close_snapshot_waiting_ui(Arc::new(AtomicU64::new(0)));
    assert!(ui.tick());
    let Some(UiReply::CloseReconciliation(reply)) = mailbox.take_deposited_reply(&wait).unwrap()
    else {
        panic!("cleanup reply");
    };
    let RefreshDecision::Cleanup(cleanup) = reply.settlement().decision() else {
        panic!("exact Gone cleanup");
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipGone {
                cleanup: cleanup.clone(),
                proof: Arc::clone(cleanup.closed_proof().unwrap()),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    assert!(ui.tick());
    drop(reply);
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    assert!(
        ui.view().close_status().is_some(),
        "the worker still retains its exact marker"
    );
    assert!(!ui.tick(), "no worker completion or event has arrived");
    mailbox.finish_lease(lease).unwrap();
    assert!(
        ui.tick(),
        "ACK-only pruning must redraw the removed Closed status"
    );
    assert!(ui.view().close_status().is_none());
    assert!(!ui.tick(), "the acknowledgement is consumed once");
}

#[test]
fn close_snapshot_late_ui_starts_one_budget_and_gone_never_installs_after_forward_expiry() {
    for offset in [0, 44_999, 45_000, 49_999, 50_000] {
        let elapsed = Arc::new(AtomicU64::new(0));
        let (mut ui, mailbox, clock, lease, wait) = close_snapshot_waiting_ui(Arc::clone(&elapsed));
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let cursors = mailbox.cursors();
        elapsed.store(75_000, Ordering::SeqCst);
        assert!(
            ui.tick(),
            "timely Snapshot placement survives late UI consumption"
        );
        let Some(UiReply::CloseReconciliation(reply)) =
            mailbox.take_deposited_reply(&wait).unwrap()
        else {
            panic!("cleanup reply");
        };
        let RefreshDecision::Cleanup(cleanup) = reply.settlement().decision() else {
            panic!("exact Gone cleanup");
        };
        assert_eq!(
            cleanup.deadlines(),
            OperationDeadlines::from_sample(clock.now()).unwrap()
        );
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipGone {
                    cleanup: cleanup.clone(),
                    proof: Arc::clone(cleanup.closed_proof().unwrap()),
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &clock,
            )
            .unwrap();
        elapsed.store(75_000 + offset, Ordering::SeqCst);
        assert!(ui.tick());
        if offset < 45_000 {
            assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
            mailbox.take_deposited_reply(&wait).unwrap().unwrap();
            mailbox.finish_lease(lease).unwrap();
        } else {
            assert!(ui.retirement_requested());
            let state = ui.state.as_ref().unwrap();
            assert_eq!(state.cache(), &baseline);
            assert!(state.has_retained_close_records());
        }
        assert_eq!(
            mailbox.cursors().0,
            cursors.0,
            "cleanup never allocates a replacement key after expiry"
        );
    }
}

#[test]
fn actual_attached_refresh_freezes_until_exact_cleanup_and_never_installs_after_forward_expiry() {
    for (millis, wrong_generation) in [
        (0, false),
        (44_999, false),
        (45_000, false),
        (49_999, false),
        (50_000, false),
        (0, true),
    ] {
        let elapsed = Arc::new(AtomicU64::new(0));
        let clock = adjustable_operation_clock(Arc::clone(&elapsed));
        let geometry = Geometry::new(80, 24, 8, 16).unwrap();
        let mailbox = Arc::new(CommandMailbox::new(10, geometry));
        let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::Bootstrap {
                    selection: None,
                    cache: super::super::workbench_projection::project_native_standalone_tabs(
                        &snapshot(1, 1),
                    )
                    .unwrap(),
                },
                EventDeadline::Startup(clock.now() + super::super::state::OPERATION_TOTAL),
                &clock,
            )
            .unwrap();
        ui.tick();
        mailbox.take_deposited_reply(&wait).unwrap().unwrap();
        let state = ui.state.as_mut().unwrap();
        let old = state.desired().unwrap().clone();
        assert!(state.install_projection_t(&old));
        assert!(state.record_setup_configuration_t(&old, ControllerMode::ReadOnly, None));
        assert!(state.stage_drawn_frame_t(&old, FrameIdentity::new(1)));
        assert!(state.mark_feedback_accepted_t(&old, FrameIdentity::new(1)));
        assert!(state.publish_ready_t(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
        let baseline = state.cache().clone();
        let cursors = mailbox.cursors();
        let (lease, wait) = place_refresh(&mut ui, &mailbox, &clock, snapshot(0, 2));
        ui.tick();
        let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap()
        else {
            panic!("staged cleanup reply");
        };
        let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
            panic!("actual cleanup required");
        };
        let cleanup = cleanup.clone();
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.desired(), Some(&old));
        assert_eq!(state.attached_key(), Some(&old));
        assert!(state.ready().is_none());
        assert!(state.invariant_holds());
        assert_eq!(mailbox.cursors().0, cursors.0);
        let detached = if wrong_generation {
            AttachmentKey::new(old.anchor().generation() + 1, old.tab_id().clone())
        } else {
            old.anchor().clone()
        };
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached,
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &clock,
            )
            .unwrap();
        elapsed.store(millis, Ordering::SeqCst);
        ui.tick();
        if millis < 45_000 && !wrong_generation {
            assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
            assert!(ui.state.as_ref().unwrap().cache().tabs().is_empty());
            assert!(
                matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Membership(settlement)) if settlement.desired().is_none())
            );
            mailbox.finish_lease(lease).unwrap();
        } else {
            assert!(ui.retirement_requested());
            assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
        }
        assert_eq!(mailbox.cursors().0, cursors.0);
    }
}

#[test]
fn actual_refresh_reply_installs_empty_and_later_peer_membership_with_one_shared_generation() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let mailbox = Arc::new(CommandMailbox::new(10, geometry));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let bootstrap = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: super::super::workbench_projection::project_native_standalone_tabs(
                    &snapshot(2, 1),
                )
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + super::super::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&bootstrap).unwrap(),
        Some(UiReply::BootstrapApplied(_))
    ));
    let state = ui.state.as_mut().unwrap();
    let old = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&old));
    assert!(state.record_setup_configuration_t(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    let deadlines = OperationDeadlines::from_sample(clock.now()).unwrap();
    mailbox
        .try_enqueue_switch_for(
            state,
            |state, generation| {
                let key = AttachmentKey::new(generation, state.tab(1).unwrap().tab_id().clone());
                let target = state.target_for_switch(1, key, deadlines).unwrap();
                assert!(state.begin_switch(target.clone()));
                Ok::<_, ()>(SwitchMarker::new(target))
            },
            |state, result, facts| {
                if result.is_ok() {
                    state.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                }
            },
        )
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease")
    };
    let permit = mailbox
        .authorize_switch_snapshot(&lease, marker.target(), &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: marker.target().selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(2, 1),
                },
            },
            EventDeadline::PlacementOnly(deadlines.forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Switch(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Switch candidate reply")
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("old viewer cleanup")
    };
    let permit = mailbox
        .authorize_switch_cleanup(&lease, cleanup, old.anchor(), &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_cleanup(permit, true).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: old.anchor().clone(),
            },
            EventDeadline::TotalCleanup(deadlines.total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Switch(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Switch successor reply")
    };
    let RefreshDecision::AttachRecovered(target) = settlement.decision() else {
        panic!("fresh successor")
    };
    let selected = target.selected().key().clone();
    let state = ui.state.as_mut().unwrap();
    assert_eq!(
        state.settle_unattached_setup_t(&selected, target.selected().index(), target.provenance()),
        Some(CleanupContinuation::Stable)
    );
    mailbox.finish_lease(lease).unwrap();
    assert!(state.invariant_holds());

    let (lease, wait) = place_refresh(&mut ui, &mailbox, &clock, snapshot(0, 2));
    // Timely placement owns transport completion; this later UI tick starts the one fresh reconciliation budget.
    elapsed.store(75_000, Ordering::SeqCst);
    ui.tick();
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    assert_eq!(ui.view().active_position(), None);
    assert!(ui.view().projection().is_none());
    assert!(!ui.retirement_requested());
    assert!(
        matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Refresh(settlement))
        if settlement.desired().is_none() && *settlement.decision() == RefreshDecision::SettledWithoutAttachment)
    );
    assert_eq!(mailbox.cursors().0, selected.generation());
    mailbox.finish_lease(lease).unwrap();

    let (lease, wait) = place_refresh(&mut ui, &mailbox, &clock, snapshot(0, 2));
    ui.tick();
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    assert!(
        matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Refresh(settlement))
        if settlement.desired().is_none() && *settlement.decision() == RefreshDecision::Continue)
    );
    assert_eq!(mailbox.cursors().0, selected.generation());
    mailbox.finish_lease(lease).unwrap();

    let (lease, wait) = place_refresh_outcome(
        &mut ui,
        &mailbox,
        &clock,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    );
    elapsed.store(150_000, Ordering::SeqCst);
    ui.tick();
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    assert_eq!(
        ui.view().refresh_status(),
        Some(NativeRefreshStatus::Failure(2))
    );
    assert!(
        matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Refresh(settlement)) if settlement.desired().is_none())
    );
    assert_eq!(mailbox.cursors().0, selected.generation());
    mailbox.finish_lease(lease).unwrap();

    assert_eq!(ui.create_tab(), SessionActionDisposition::DismissNavigator);
    let create = mailbox.try_recv().unwrap().expect("manual Empty Create");
    let QueuedCommand::Create(marker) = create.record() else {
        panic!("Create only");
    };
    assert_eq!(
        marker.operation().origin(),
        super::super::state::CreateOriginKind::Empty
    );
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::CreatePrecommit {
                operation: marker.operation().key().clone(),
                outcome: CreatePrecommitOutcome::NotSubmitted,
                applied_geometry: None,
            },
            EventDeadline::Forward(marker.operation().deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::PrecommitRestored)
    ));
    assert_eq!(mailbox.cursors().0, selected.generation() + 1);
    mailbox.finish_lease(create).unwrap();

    let mut peer = snapshot(3, 3);
    peer.terminal_contexts[0].tabs.drain(..2);
    let (lease, wait) = place_refresh(&mut ui, &mailbox, &clock, peer);
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Refresh reply");
    };
    let RefreshDecision::AttachRecovered(target) = settlement.decision() else {
        panic!("peer member attaches from canonical binding");
    };
    assert_eq!(
        target.selected().key().generation(),
        selected.generation() + 2
    );
    assert_eq!(settlement.desired(), Some(target.selected().key()));
    assert_eq!(ui.state.as_ref().unwrap().desired(), settlement.desired());
    assert_eq!(mailbox.cursors().0, target.selected().key().generation());
    assert_eq!(
        target.binding().expect("bound fixture").terminal_id(),
        &snapshot_terminal(2)
    );
    assert!(ui.state.as_ref().unwrap().invariant_holds());
    mailbox.finish_lease(lease).unwrap();
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "no Open/Create/Start or replacement-shell command is queued"
    );
}

/// The session's own context can be deleted remotely — a peer Client's
/// workspace close removes it from the canonical snapshot while this
/// session still owns live viewers. The real refresh entry must stage the
/// exact cleanup (old cache and desired stay in place while fresh input is
/// refused as busy), and only the anchor's `MembershipDetached` proof may
/// install the surviving standalone context: every pane record of the
/// removed epoch drops, the deleted location is never parked back as a
/// retained context, and a bound landing member recovers as an attach.
#[test]
fn removed_context_refresh_cleans_every_pending_viewer_then_lands_on_standalone() {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: super::super::workbench_projection::project_native_standalone_tabs(
                    &split_snapshot(),
                )
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + super::super::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let old = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&old));
    assert!(state.record_setup_configuration_t(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    // The split tab's second leaf carries its own live viewer, so the
    // removed epoch owes the cleanup two pane records, not only the focus.
    let peer_panel = state
        .tab(0)
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .nth(1)
        .unwrap();
    let peer = PaneKey::new(old.anchor().clone(), peer_panel);
    state.install_unattached_pane(&SelectedTab::new_panel(peer.clone(), 0, 1));
    let peer_ref = state.pane_ref_for(&peer).unwrap();
    assert!(matches!(
        state.settle_attached_event_t(&peer_ref),
        Some(SetupEventDisposition::Applied)
    ));
    assert!(state.record_setup_configuration_t(&peer_ref, ControllerMode::ReadOnly, None));
    assert!(state.has_attachments());
    let baseline = state.cache().clone();

    let (lease, wait) = place_refresh(&mut ui, &mailbox, &clock, removal_snapshot(1, 2));
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("cleanup settlement");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("live viewers make the exact cleanup mandatory before landing");
    };
    let cleanup = cleanup.clone();
    assert_eq!(cleanup.old(), old.anchor());
    let state = ui.state.as_ref().unwrap();
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.desired(), Some(&old));
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Switching);

    // ReconcilingMembership freezes Ready-based admission: fresh input is
    // refused as busy, never queued, and never retiring.
    assert_eq!(ui.create_tab(), SessionActionDisposition::KeepNavigator);
    assert_eq!(
        ui.state.as_ref().unwrap().notice(),
        Some(SessionNotice::LocalBusy)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(!ui.retirement_requested());

    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Membership(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("membership install reply");
    };
    let RefreshDecision::AttachRecovered(target) = settlement.decision() else {
        panic!("the surviving standalone context carries an attachable member");
    };
    let state = ui.state.as_ref().unwrap();
    assert_eq!(
        state.cache().location(),
        &NativeContextLocation::Standalone(removal_context())
    );
    assert_eq!(state.desired(), Some(target.selected().key()));
    // Both pending pane records of the removed epoch are cleaned — the
    // anchor proof covered every live viewer, not only the focus pane.
    assert!(state.pane_record(&old).is_none());
    assert!(state.pane_record(&peer).is_none());
    assert!(!state.has_attachments());
    // The deleted context is never parked back: navigation surfaces only
    // the surviving standalone location and the full invariant — which
    // includes the retained-context contract — holds.
    assert!(state.invariant_holds());
    assert_eq!(ui.view().context_count(), 1);
    assert_eq!(
        ui.view().context_at(0).unwrap().location(),
        &NativeContextLocation::Standalone(removal_context())
    );
    assert_eq!(ui.view().active_context_position(), Some(1));
    assert!(state.pending_refresh_generation().is_none());
    assert!(state.invariant_holds());
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// The workbench's standalone context moved to `ctx9` while the session's
/// own `ctx2` is absent from navigation and terminal_contexts — the shape
/// the removal classifier mints a landing permit for.
fn removal_snapshot(tab_count: usize, revision: u64) -> Snapshot {
    let mut snapshot = snapshot(tab_count, revision);
    let landing = removal_context();
    snapshot.navigation.standalone_terminal_context_id = landing.clone();
    snapshot.terminal_contexts[0].terminal_context_id = landing;
    snapshot
}

fn removal_context() -> WorkbenchTerminalContextId {
    WorkbenchTerminalContextId::parse(&format!("{:032x}", 9)).unwrap()
}

/// One ctx2 tab whose root splits two leaves so both can carry live
/// viewers under the same epoch anchor.
fn split_snapshot() -> Snapshot {
    let mut snapshot = snapshot(1, 1);
    let original_root = snapshot.terminal_contexts[0].tabs[0].root.clone();
    snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: original_root,
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 7_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 7_010)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: TerminalId::parse(&format!("{:032x}", 7_012)).unwrap(),
                            worker_id: WorkerId::parse(&format!("{:032x}", 7_011)).unwrap(),
                        }),
                        label: "peer".into(),
                        presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 7_013))
                            .unwrap(),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 7_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&format!("{:032x}", 7_020)).unwrap(),
    };
    snapshot
}

pub(super) fn place_refresh(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: Snapshot,
) -> (
    super::super::mailbox::CommandLease,
    super::super::mailbox::UiWaitKey,
) {
    place_refresh_outcome(
        ui,
        mailbox,
        clock,
        WorkbenchSnapshotOutcome::Available { snapshot },
    )
}

fn place_refresh_outcome(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    outcome: WorkbenchSnapshotOutcome,
) -> (
    super::super::mailbox::CommandLease,
    super::super::mailbox::UiWaitKey,
) {
    ui.open_navigator();
    let lease = mailbox.try_recv().unwrap().expect("Refresh queued");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("Refresh only");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_refresh_result_before(marker.generation(), outcome, marker.deadline(), clock)
        .unwrap();
    (lease, wait)
}

fn snapshot_terminal(index: usize) -> TerminalId {
    TerminalId::parse(&format!("{:032x}", 1004 + index * 10)).unwrap()
}

pub(in crate::terminal_session) fn snapshot(count: usize, revision: u64) -> Snapshot {
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
    Snapshot {
        navigation: Navigation {
            workspace_order: Vec::new(),
            projects: Vec::new(),
            standalone_terminal_context_id: context.clone(),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            terminal_context_id: context,
            tabs: (0..count)
                .map(|index| {
                    let base = 1000 + index * 10;
                    Tab {
                        automatic_label: false,
                        group_id: None,
                        pinned: false,
                        label: format!("Tab {index}"),
                        tab_id: WorkbenchTabId::parse(&format!("{base:032x}")).unwrap(),
                        root: PanelNode::Panel {
                            right_click: Default::default(),
                            child_presentations: Vec::new(),
                            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", base + 1))
                                .unwrap(),
                            primary_presentation: Presentation {
                                custom_label: None,
                                label: format!("Terminal {index}"),
                                presentation_id: TerminalPresentationId::parse(&format!(
                                    "{:032x}",
                                    base + 2
                                ))
                                .unwrap(),
                                binding: Some(PresentationBinding {
                                    terminal_id: snapshot_terminal(index),
                                    worker_id: WorkerId::parse(&format!("{:032x}", base + 3))
                                        .unwrap(),
                                }),
                            },
                        },
                    }
                })
                .collect(),
        }],
    }
}
