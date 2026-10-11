use super::super::tests::membership::{place_refresh, snapshot};
use super::super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::mailbox::EventDeadline;
use crate::terminal_session::state::{ControllerMode, FrameIdentity};
use orchbee_workbench_protocol::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn timely_create_candidate_reserve_cleans_without_install_and_total_expiry_never_cleans() {
    for millis in [44_999, 45_000, 49_999, 50_000] {
        let elapsed = Arc::new(AtomicU64::new(0));
        let (mut ui, mailbox, clock) = ready_create_ui_at(Arc::clone(&elapsed));
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        let mut committed = snapshot(2, 2);
        rename_created_fixture(&mut committed, 1, "2");
        let (_lease, operation) = retain_ui_create(&mut ui, &mailbox, &clock, &committed, 1);
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::CreateSnapshotResult {
                    operation,
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: committed,
                    },
                },
                EventDeadline::PlacementOnly(clock.now() + std::time::Duration::from_secs(45)),
                &clock,
            )
            .unwrap();
        elapsed.store(millis, Ordering::SeqCst);
        ui.tick();
        if millis == 50_000 {
            assert!(ui.retirement_requested());
            assert_eq!(mailbox.is_closed(), Some(CarrierFailure::Deadline));
        } else {
            let Some(UiReply::CreateProjection(CreateProjectionDecision::Cleanup(cleanup))) =
                mailbox.take_deposited_reply(&wait).unwrap()
            else {
                panic!("cleanup only");
            };
            assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
            let wait = mailbox
                .publish_event_before(
                    WorkerEvent::MembershipDetached {
                        cleanup: cleanup.clone(),
                        detached: old.anchor().clone(),
                    },
                    EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                    &clock,
                )
                .unwrap();
            ui.tick();
            if millis < 45_000 {
                assert!(matches!(
                    mailbox.take_deposited_reply(&wait).unwrap(),
                    Some(UiReply::Membership(_))
                ));
                assert!(
                    ui.state
                        .as_ref()
                        .unwrap()
                        .settled_create_for_test()
                        .is_some()
                );
                continue;
            }
            assert!(ui.retirement_requested());
        }
        assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
        assert!(
            ui.state
                .as_ref()
                .unwrap()
                .settled_create_for_test()
                .is_none()
        );
        assert!(ui.state.as_ref().unwrap().committed_target().is_some());
    }
}

#[test]
fn actual_create_unavailable_consumed_after_total_keeps_old_identity_without_effects() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, mailbox, clock) = ready_create_ui_at(Arc::clone(&elapsed));
    let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let mut committed = snapshot(2, 2);
    rename_created_fixture(&mut committed, 1, "2");
    let (lease, operation) = retain_ui_create(&mut ui, &mailbox, &clock, &committed, 1);
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::CreateSnapshotResult {
                operation,
                outcome: WorkbenchSnapshotOutcome::Unavailable {
                    reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                },
            },
            EventDeadline::PlacementOnly(clock.now() + std::time::Duration::from_secs(45)),
            &clock,
        )
        .unwrap();
    elapsed.store(51_000, Ordering::SeqCst);
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::CreateProjection(
            CreateProjectionDecision::ProjectionUnavailable
        ))
    ));
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(ui.view().phase(), NativeDisplayPhase::ProjectionUnavailable);
    assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&old));
    assert_eq!(ui.state.as_ref().unwrap().attached_key(), Some(&old));
    assert_eq!(ui.state.as_ref().unwrap().projection_key(), Some(&old));
    assert!(!ui.forward(b"forbidden"));
    ui.observe_geometry(Geometry::new(100, 30, 8, 16).unwrap());
    assert_eq!(ui.create_tab(), SessionActionDisposition::KeepNavigator);
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(!ui.retirement_requested());
}

#[test]
fn actual_create_candidate_installs_reserved_survivor_fresh_fallback_or_empty() {
    for remaining in [2, 1, 0] {
        let (mut ui, mailbox, clock) = ready_create_ui();
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        let mut committed = snapshot(2, 2);
        rename_created_fixture(&mut committed, 1, "2");
        let (lease, operation) = retain_ui_create(&mut ui, &mailbox, &clock, &committed, 1);
        let mut candidate = committed.clone();
        if remaining != 2 {
            candidate.revision = WorkbenchRevision::new(3);
            candidate.terminal_contexts[0].tabs.truncate(remaining);
        }
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::CreateSnapshotResult {
                    operation: operation.clone(),
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate,
                    },
                },
                EventDeadline::PlacementOnly(clock.now() + std::time::Duration::from_secs(45)),
                &clock,
            )
            .unwrap();
        ui.tick();
        let Some(UiReply::CreateProjection(CreateProjectionDecision::Cleanup(cleanup))) =
            mailbox.take_deposited_reply(&wait).unwrap()
        else {
            panic!("validated Create cleanup");
        };
        assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&old));
        assert!(
            ui.state
                .as_ref()
                .unwrap()
                .settled_create_for_test()
                .is_none()
        );
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached {
                    detached: old.anchor().clone(),
                    cleanup: cleanup.clone(),
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(matches!(
            mailbox.take_deposited_reply(&wait).unwrap(),
            Some(UiReply::Membership(_))
        ));
        let state = ui.state.as_ref().unwrap();
        assert_eq!(
            state.settled_create_for_test().unwrap().operation().key(),
            &operation
        );
        assert_eq!(state.cache().tabs().len(), remaining);
        if remaining == 0 {
            assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
            assert_eq!(state.desired(), None);
            assert_eq!(mailbox.cursors().0, operation.reserved_generation());
        } else {
            let selected = state.desired().unwrap();
            assert_eq!(
                selected.generation(),
                operation.reserved_generation() + u64::from(remaining == 1)
            );
            assert_eq!(
                selected.tab_id(),
                &committed.terminal_contexts[0].tabs[remaining - 1].tab_id
            );
            assert_eq!(state.projection_key(), None);
        }
        assert!(state.invariant_holds());
        mailbox.finish_lease(lease).unwrap();
    }
}

#[test]
fn actual_empty_create_installs_without_old_cleanup_and_preserves_prior_receipt_until_install() {
    let (mut ui, mailbox, clock) = ready_create_ui();
    let (lease, wait) = place_refresh(&mut ui, &mailbox, &clock, snapshot(0, 2));
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("cleanup");
    };
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
    mailbox.take_deposited_reply(&wait).unwrap();
    mailbox.finish_lease(lease).unwrap();
    for revision in [3, 5] {
        let previous = ui
            .state
            .as_ref()
            .unwrap()
            .settled_create_for_test()
            .map(|commit| commit.operation().key().clone());
        assert_eq!(ui.create_tab(), SessionActionDisposition::DismissNavigator);
        let refusal = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Create(marker) = refusal.record() else {
            panic!("Create");
        };
        assert_eq!(
            ui.state
                .as_ref()
                .unwrap()
                .settled_create_for_test()
                .map(|commit| commit.operation().key().clone()),
            previous
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
        mailbox.take_deposited_reply(&wait).unwrap();
        mailbox.finish_lease(refusal).unwrap();
        assert_eq!(
            ui.state
                .as_ref()
                .unwrap()
                .settled_create_for_test()
                .map(|commit| commit.operation().key().clone()),
            previous
        );
        let mut committed = snapshot(4, revision);
        let created = committed.terminal_contexts[0]
            .tabs
            .remove(if revision == 3 { 2 } else { 3 });
        committed.terminal_contexts[0].tabs = vec![created];
        rename_created_fixture(&mut committed, 0, "1");
        let (lease, operation) = retain_ui_create(&mut ui, &mailbox, &clock, &committed, 0);
        assert_eq!(
            ui.state
                .as_ref()
                .unwrap()
                .settled_create_for_test()
                .map(|commit| commit.operation().key().clone()),
            previous
        );
        let mut candidate = committed;
        if revision == 3 {
            candidate.revision = WorkbenchRevision::new(revision + 1);
            candidate.terminal_contexts[0].tabs.clear();
        }
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::CreateSnapshotResult {
                    operation: operation.clone(),
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate,
                    },
                },
                EventDeadline::PlacementOnly(clock.now() + std::time::Duration::from_secs(45)),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(matches!(
            mailbox.take_deposited_reply(&wait).unwrap(),
            Some(UiReply::CreateProjection(
                CreateProjectionDecision::SettledWithoutAttachment
            ))
        ));
        assert_eq!(
            ui.view().phase(),
            if revision == 3 {
                NativeDisplayPhase::Empty
            } else {
                NativeDisplayPhase::SelectedWithoutAttachment
            }
        );
        assert_eq!(
            ui.state
                .as_ref()
                .unwrap()
                .settled_create_for_test()
                .unwrap()
                .operation()
                .key(),
            &operation
        );
        mailbox.finish_lease(lease).unwrap();
    }
}

fn ready_create_ui() -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock) {
    ready_create_ui_at(Arc::new(AtomicU64::new(0)))
}

fn ready_create_ui_at(
    elapsed: Arc<AtomicU64>,
) -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock) {
    let clock = adjustable_operation_clock(elapsed);
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let mailbox = Arc::new(CommandMailbox::new(10, geometry));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: super::super::super::workbench_projection::project_native_standalone_tabs(
                    &snapshot(1, 1),
                )
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + super::super::super::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap();
    let state = ui.state.as_mut().unwrap();
    let old = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&old));
    assert!(state.record_setup_configuration_t(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    (ui, mailbox, clock)
}

fn rename_created_fixture(candidate: &mut Snapshot, index: usize, label: &str) {
    let created = &mut candidate.terminal_contexts[0].tabs[index];
    created.label = label.to_owned();
    created.automatic_label = true;
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut created.root
    else {
        panic!("Panel");
    };
    primary_presentation.label = label.to_owned();
}

fn retain_ui_create(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    committed: &Snapshot,
    index: usize,
) -> (
    super::super::super::mailbox::CommandLease,
    CreateOperationKey,
) {
    assert_eq!(ui.create_tab(), SessionActionDisposition::DismissNavigator);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Create(marker) = lease.record() else {
        panic!("Create");
    };
    let operation = marker.operation().key().clone();
    let created = &committed.terminal_contexts[0].tabs[index];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        panic!("Panel");
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::CreateCommittedKnown {
                operation: operation.clone(),
                commit: crate::local_client::workbench::WorkbenchTabCreateCommit::Pending {
                    revision: committed.revision,
                    terminal_context_id: committed.terminal_contexts[0].terminal_context_id.clone(),
                    tab_id: created.tab_id.clone(),
                    panel_id: panel_id.clone(),
                    presentation_id: primary_presentation.presentation_id.clone(),
                },
            },
            EventDeadline::TotalCleanup(marker.operation().deadlines().total()),
            clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::CommitRetained)
    ));
    (lease, operation)
}
