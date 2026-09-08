use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::mailbox::{EventDeadline, SwitchMarker};
use crate::terminal_session::state::{ControllerMode, FrameIdentity, SwitchDetachContinuation};
use orchbee_workbench_protocol::*;
use std::sync::atomic::{AtomicU64, Ordering};

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
                        facts.abandoned_causal(),
                        facts.authorized_cleanup(),
                    );
                }
            },
        )
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let selected = state.desired().unwrap().clone();
    assert!(matches!(
        state.settle_switch_old_detached(&selected, &old, deadlines.forward_cutoff()),
        Some(SwitchDetachContinuation::SettledWithoutAttachment)
    ));
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

fn place_refresh(
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
