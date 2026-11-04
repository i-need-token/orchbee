//! A conditional Workspace Close reads every Tab's terminal as a conditional Tab Close reads its
//! own, decides by the same rule, and rechecks the whole scope at final admission.
use super::*;
use crate::local_terminal_service::tab_close::evidence::{CloseFacts, ShellReport};
use orchbee_capability_node_protocol::{LifecycleCloseActivity, LifecycleHostFailure};
use orchbee_workbench_protocol::WorkbenchRejectionReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    /// Both shells sit at their prompts.
    Quiet,
    /// The Host calls both idle, and nothing else is known: no sampler reads them.
    HostIdleOnly,
    /// The second shell says nothing and its sample never came.
    SecondAsks,
    /// The second terminal ended; only the first is read.
    SecondEnded,
    /// The second terminal is missing from its inventory.
    SecondMissing,
    /// The Workspace changes while it is inspected.
    Stale,
}

#[test]
fn a_workspace_closes_unasked_only_on_the_evidence_a_tab_close_reads() {
    for case in [
        Case::Quiet,
        Case::HostIdleOnly,
        Case::SecondAsks,
        Case::SecondEnded,
        Case::SecondMissing,
        Case::Stale,
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
        let accepted = matches!(case, Case::Quiet | Case::SecondEnded);
        let peer_thread = thread::spawn(move || {
            for index in 0..2 {
                let request = lifecycle(&mut peer);
                assert!(matches!(
                    request.action,
                    TerminalLifecycleAction::InspectInventoryPage { cursor: None }
                ));
                assert_eq!(
                    request.target.terminal_id().as_bytes(),
                    [u8::try_from(70 + index).unwrap(); 16],
                    "{case:?}: every Tab's inventory is read"
                );
                if index == 1 && case == Case::Stale {
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
                let phase = if index == 1 && case == Case::SecondEnded {
                    LifecycleTerminalPhase::Exited {
                        terminate_requested: false,
                    }
                } else {
                    LifecycleTerminalPhase::Running {
                        terminate_requested: false,
                    }
                };
                let target = (index == 0 || case != Case::SecondMissing).then_some(
                    LifecycleTargetObservation {
                        revision: 1,
                        phase,
                        // The Host's own guess never decides; the Server's read does.
                        close_activity: LifecycleCloseActivity::Idle,
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
            // An accepted close continues as an ordinary one; its first step is parked here.
            let step = accepted.then(|| {
                let request = lifecycle(&mut peer);
                let action = request.action;
                reply(
                    &mut peer,
                    request,
                    TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::HostRefused),
                );
                action
            });
            finished.recv_timeout(Duration::from_secs(5)).unwrap();
            step
        });
        let mut read = None;
        let result = if case == Case::HostIdleOnly {
            service.execute_workspace_close(close).unwrap()
        } else {
            service
                .execute_close_if_idle_with(close, |targets, _| {
                    read = Some(targets.len());
                    targets
                        .iter()
                        .enumerate()
                        .map(|(index, _)| CloseFacts {
                            report: Some(if index == 1 && case == Case::SecondAsks {
                                ShellReport::Silent
                            } else {
                                ShellReport::AtPrompt
                            }),
                            sample: None,
                        })
                        .collect()
                })
                .unwrap()
        };
        let expected = match case {
            Case::Quiet | Case::SecondEnded => None,
            Case::Stale => Some(WorkbenchRejectionReason::RevisionMismatch),
            Case::HostIdleOnly | Case::SecondAsks | Case::SecondMissing => {
                Some(WorkbenchRejectionReason::ConfirmationRequired)
            }
        };
        match expected {
            None => assert!(
                matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
                ),
                "{case:?}: {result:?}"
            ),
            Some(reason) => {
                assert!(
                    matches!(&result.outcome, WorkbenchCommandOutcome::Rejected { reason: actual, .. } if *actual == reason),
                    "{case:?}: {result:?}"
                );
                assert_eq!(
                    read_snapshot(&service).terminal_contexts,
                    before.terminal_contexts,
                    "{case:?}: a refusal removes no member"
                );
            }
        }
        // Live terminals are read together; an ended one needs no read, and a missing one asks
        // before anything is read.
        let expected_read = match case {
            Case::Quiet | Case::SecondAsks | Case::Stale => Some(2),
            Case::SecondEnded => Some(1),
            Case::HostIdleOnly | Case::SecondMissing => None,
        };
        assert_eq!(read, expected_read, "{case:?}");
        done.send(()).unwrap();
        let step = peer_thread.join().unwrap();
        // No signal waits for the Host's idle guard: the Server's evidence already decided.
        assert!(
            step.is_none_or(|action| matches!(
                action,
                TerminalLifecycleAction::Terminate | TerminalLifecycleAction::Dispose
            )),
            "{case:?}: {step:?}"
        );
        if case == Case::Quiet {
            assert_eq!(step, Some(TerminalLifecycleAction::Terminate));
        }
        service.stop_close_maintenance();
        worker_thread.join().unwrap();
    }
}
