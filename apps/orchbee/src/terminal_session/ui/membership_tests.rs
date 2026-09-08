use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::mailbox::{EventDeadline, SwitchMarker};
use crate::terminal_session::state::{CleanupContinuation, ControllerMode, FrameIdentity};
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
    assert!(state.install_projection(&old));
    assert!(state.record_setup_configuration(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
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
        assert!(state.install_projection(&old));
        assert!(state.record_setup_configuration(&old, ControllerMode::ReadOnly, None));
        assert!(state.stage_drawn_frame(&old, FrameIdentity::new(1)));
        assert!(state.mark_feedback_accepted(&old, FrameIdentity::new(1)));
        assert!(state.publish_ready(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
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
            AttachmentKey::new(old.generation() + 1, old.tab_id().clone())
        } else {
            old.clone()
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
    assert!(state.install_projection(&old));
    assert!(state.record_setup_configuration(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
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
        .authorize_switch_cleanup(&lease, cleanup, &old, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_cleanup(permit, true).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: old,
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
        state.settle_unattached_setup(&selected, target.selected().index(), target.provenance()),
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
    assert_eq!(target.binding().terminal_id(), &snapshot_terminal(2));
    assert!(ui.state.as_ref().unwrap().invariant_holds());
    mailbox.finish_lease(lease).unwrap();
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "no Open/Create/Start or replacement-shell command is queued"
    );
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

pub(super) fn snapshot(count: usize, revision: u64) -> Snapshot {
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
    Snapshot {
        navigation: Navigation {
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
                        group_id: None,
                        pinned: false,
                        label: format!("Tab {index}"),
                        tab_id: WorkbenchTabId::parse(&format!("{base:032x}")).unwrap(),
                        root: PanelNode::Panel {
                            child_presentations: Vec::new(),
                            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", base + 1))
                                .unwrap(),
                            primary_presentation: Presentation {
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
