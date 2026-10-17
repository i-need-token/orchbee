//! Framed inventory witnesses exercise whole-scope preflight and final CAS.
use super::*;
use orchbee_capability_node_protocol::LifecycleCloseActivity;
use orchbee_workbench_protocol::WorkbenchRejectionReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LastMember {
    Idle,
    Active,
    Unknown,
    Missing,
    Stale,
}

#[test]
fn workspace_idle_close_reads_every_tab_and_rechecks_before_any_guarded_signal() {
    for last in [
        LastMember::Idle,
        LastMember::Active,
        LastMember::Unknown,
        LastMember::Missing,
        LastMember::Stale,
    ] {
        let Fixture {
            service,
            mut peer,
            worker_thread,
            ..
        } = fixture();
        let context = workspace(&service);
        for (request, identity) in [(2100, 70), (2200, 71)] {
            let (_, pending) = pending_create(&service, &context, request);
            complete_create(&service, pending, identity);
        }
        let before = read_snapshot(&service);
        let mut close = close_command(&service, &context, 2300);
        let (_, workspace_id) = scope(&service, &context);
        close.action = WorkbenchAction::WorkspaceCloseIfIdle {
            workspace_id: workspace_id.clone(),
        };
        service.start_close_maintenance().unwrap();
        let client = Arc::clone(&service);
        let (done, finished) = mpsc::sync_channel(1);
        let peer_thread = thread::spawn(move || {
            for index in 0..2 {
                let request = lifecycle(&mut peer);
                assert!(matches!(
                    request.action,
                    TerminalLifecycleAction::InspectInventoryPage { cursor: None }
                ));
                assert_eq!(
                    request.target.terminal_id().as_bytes(),
                    [u8::try_from(70 + index).unwrap(); 16]
                );
                if index == 1 && last == LastMember::Stale {
                    let rename = command(
                        &client,
                        2400,
                        WorkbenchAction::WorkspaceRename {
                            workspace_id: workspace_id.clone(),
                            label: WorkbenchLabel::parse("peer rename").unwrap(),
                        },
                    );
                    navigation(&client, rename);
                }
                let activity = if index == 0 || last == LastMember::Idle {
                    LifecycleCloseActivity::Idle
                } else if last == LastMember::Active {
                    LifecycleCloseActivity::Active
                } else {
                    LifecycleCloseActivity::Unknown
                };
                let target = (index == 0 || last != LastMember::Missing).then_some(
                    LifecycleTargetObservation {
                        revision: 1,
                        phase: LifecycleTerminalPhase::Running {
                            terminate_requested: false,
                        },
                        close_activity: activity,
                    },
                );
                reply(
                    &mut peer,
                    request,
                    TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
                        inventory_generation: NonZeroU64::MIN,
                        next_cursor: None,
                        returned_count: 2,
                        target,
                    }),
                );
            }
            if last == LastMember::Idle {
                let request = lifecycle(&mut peer);
                assert_eq!(
                    request.action,
                    TerminalLifecycleAction::TerminateIfIdle,
                    "the pre-read never grants ordinary termination"
                );
                reply(&mut peer, request, TerminalLifecycleOutcome::HostFailure(orchbee_capability_node_protocol::LifecycleHostFailure::ConfirmationRequired));
            }
            finished.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        let result = service.execute_workspace_close(close).unwrap();
        match last {
            LastMember::Idle => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
            )),
            LastMember::Stale => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::RevisionMismatch,
                    ..
                }
            )),
            _ => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::ConfirmationRequired,
                    ..
                }
            )),
        }
        assert_eq!(
            read_snapshot(&service).terminal_contexts,
            before.terminal_contexts,
            "no member is removed during preflight/refusal"
        );
        done.send(()).unwrap();
        peer_thread.join().unwrap();
        service.stop_close_maintenance();
        worker_thread.join().unwrap();
    }
}
