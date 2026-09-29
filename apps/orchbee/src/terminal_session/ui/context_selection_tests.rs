use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::mailbox::{CommandLease, EventDeadline, UiWaitKey};
use orchbee_workbench_protocol::*;
use std::sync::atomic::{AtomicU64, Ordering};

fn snapshot(empty: bool) -> Snapshot {
    let mut snapshot = super::membership_tests::snapshot(3, 1);
    let context_id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 72)).unwrap();
    let workspace_tab = snapshot.terminal_contexts[0].tabs.pop().unwrap();
    snapshot.terminal_contexts.push(TerminalContext {
        groups: Vec::new(),
        tabs: if empty {
            Vec::new()
        } else {
            vec![workspace_tab]
        },
        terminal_context_id: context_id.clone(),
    });
    snapshot.navigation.projects.push(Project {
        directories: Vec::new(),
        label: "Project".into(),
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 70)).unwrap(),
        workspaces: vec![Workspace {
            label: "Workspace".into(),
            primary_directory_id: None,
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 71)).unwrap(),
            terminal_context_id: context_id,
        }],
    });
    snapshot
}

fn ready(
    empty: bool,
) -> (
    NativeUi,
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
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                cache:
                    crate::terminal_session::workbench_projection::project_native_standalone_tabs(
                        &snapshot(empty),
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

fn place_selection(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    next: bool,
    candidate: Snapshot,
) -> (CommandLease, UiWaitKey) {
    ui.apply_client_action(if next {
        ClientAction::SelectNextContext
    } else {
        ClientAction::SelectPreviousContext
    });
    let lease = mailbox.try_recv().unwrap().expect("one Snapshot intent");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("context selection is Snapshot-only");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    let deadlines = marker.context_deadlines().unwrap();
    let wait = mailbox
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available {
                snapshot: candidate,
            },
            deadlines.forward_cutoff(),
            clock,
        )
        .unwrap();
    (lease, wait)
}

#[test]
fn context_ui_requires_exact_cleanup_and_installs_empty_without_burning_a_tab_generation() {
    for (empty, wrong_key) in [(true, false), (false, false), (false, true)] {
        let (mut ui, mailbox, clock, _) = ready(empty);
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        let cursors = mailbox.cursors();
        let (lease, wait) = place_selection(&mut ui, &mailbox, &clock, true, snapshot(empty));
        ui.tick();
        let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap()
        else {
            panic!("Snapshot settlement");
        };
        let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
            panic!("exact old Viewer cleanup required");
        };
        assert_eq!(cleanup.old(), &old);
        assert_eq!(ui.view().active_context_position(), Some(1));
        assert_eq!(ui.view().phase(), NativeDisplayPhase::Switching);
        assert_eq!(mailbox.cursors().0, cursors.0);
        let detached = if wrong_key {
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
        ui.tick();
        if wrong_key {
            assert!(ui.retirement_requested());
            assert_eq!(ui.view().active_context_position(), Some(1));
            continue;
        }
        assert_eq!(ui.view().active_context_position(), Some(2));
        #[cfg(windows)]
        assert_eq!(ui.view().window_title_context().workspace, "Workspace");
        assert_eq!(
            ui.view().context_at(1).unwrap().workspace_label(),
            Some("Workspace")
        );
        assert_eq!(mailbox.cursors().0, cursors.0 + u64::from(!empty));
        let Some(UiReply::Membership(settlement)) = mailbox.take_deposited_reply(&wait).unwrap()
        else {
            panic!("atomic installation");
        };
        if empty {
            assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
            assert_eq!(settlement.desired(), None);
        } else {
            let RefreshDecision::AttachRecovered(target) = settlement.decision() else {
                panic!("existing setup target");
            };
            assert_eq!(target.selected().index(), 0);
            assert_eq!(target.selected().key().generation(), cursors.0 + 1);
        }
        mailbox.finish_lease(lease).unwrap();
        assert!(mailbox.try_recv().unwrap().is_none());
        if empty {
            let (lease, wait) = place_selection(&mut ui, &mailbox, &clock, false, snapshot(true));
            ui.tick();
            let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap()
            else {
                panic!("Empty reentry installation");
            };
            assert!(matches!(
                settlement.decision(),
                RefreshDecision::AttachRecovered(_)
            ));
            assert_eq!(ui.view().active_context_position(), Some(1));
            assert_eq!(mailbox.cursors().0, cursors.0 + 1);
            mailbox.finish_lease(lease).unwrap();
        }
    }
}

#[test]
fn context_ui_uses_admission_budget_through_snapshot_and_detach() {
    for (snapshot_millis, detach_millis) in
        [(0, 44_999), (0, 45_000), (44_999, 45_000), (45_000, 45_000)]
    {
        let (mut ui, mailbox, clock, elapsed) = ready(true);
        let admitted_at = clock.now();
        let (lease, wait) = place_selection(&mut ui, &mailbox, &clock, true, snapshot(true));
        elapsed.store(snapshot_millis, Ordering::SeqCst);
        ui.tick();
        if snapshot_millis == 45_000 {
            assert!(ui.retirement_requested());
            assert_eq!(ui.view().active_context_position(), Some(1));
            continue;
        }
        let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap()
        else {
            panic!("Snapshot settlement");
        };
        let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
            panic!("cleanup");
        };
        assert_eq!(
            cleanup.deadlines().forward_cutoff(),
            admitted_at + std::time::Duration::from_secs(45)
        );
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
        elapsed.store(detach_millis, Ordering::SeqCst);
        ui.tick();
        if detach_millis < 45_000 {
            assert_eq!(ui.view().active_context_position(), Some(2));
            mailbox.take_deposited_reply(&wait).unwrap().unwrap();
            mailbox.finish_lease(lease).unwrap();
        } else {
            assert!(ui.retirement_requested());
            assert_eq!(ui.view().active_context_position(), Some(1));
        }
    }
}
