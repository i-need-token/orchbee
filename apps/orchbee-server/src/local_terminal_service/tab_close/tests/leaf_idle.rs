//! A conditional Tab or Panel Close: the Server's own evidence decides, and final admission counts
//! that evidence only for the exact runtime and revision it was read from.
use super::*;
use crate::local_terminal_service::tab_close::evidence::{CloseFacts, ShellReport};
use orchbee_capability_node_protocol::{LifecycleCloseActivity, LifecycleHostFailure};
use orchbee_server::{WorkbenchTabCloseMemberRuntime, WorkbenchTabCloseRuntimeTarget};
use orchbee_workbench_protocol::{PanelNode, WorkbenchRejectionReason};

const RUNNING: LifecycleTerminalPhase = LifecycleTerminalPhase::Running {
    terminate_requested: false,
};

fn conditional(
    service: &LocalTerminalService,
    close: &WorkbenchCommand,
    request: u128,
    panel: bool,
) -> WorkbenchCommand {
    let WorkbenchAction::TabClose {
        terminal_context_id,
        tab_id,
    } = close.action.clone()
    else {
        panic!("Close")
    };
    let action = if panel {
        let panel_id = read_snapshot(service)
            .terminal_contexts
            .iter()
            .flat_map(|context| &context.tabs)
            .find_map(|tab| match &tab.root {
                PanelNode::Panel { panel_id, .. } if tab.tab_id == tab_id => Some(panel_id.clone()),
                _ => None,
            })
            .unwrap();
        WorkbenchAction::PanelCloseIfIdle {
            terminal_context_id,
            tab_id,
            panel_id,
        }
    } else {
        WorkbenchAction::TabCloseIfIdle {
            terminal_context_id,
            tab_id,
        }
    };
    WorkbenchCommand {
        request_id: id(request),
        action,
        ..close.clone()
    }
}

/// Answers the close's read-only inventory preflight of the fixture's terminal with `phase`.
fn inventory(peer: &mut UnixStream, phase: LifecycleTerminalPhase) {
    let request = lifecycle(peer);
    assert!(matches!(
        request.action,
        TerminalLifecycleAction::InspectInventoryPage { cursor: None }
    ));
    reply(
        peer,
        request,
        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
            inventory_generation: NonZeroU64::MIN,
            next_cursor: None,
            returned_count: 1,
            target: Some(LifecycleTargetObservation {
                revision: 1,
                phase,
                // The Host's own guess never decides a Tab or Panel; the Server's read does.
                close_activity: LifecycleCloseActivity::Unknown,
            }),
        }),
    );
}

fn rejected(result: &WorkbenchCommandResult, expected: WorkbenchRejectionReason) -> bool {
    matches!(&result.outcome, WorkbenchCommandOutcome::Rejected { reason, .. } if *reason == expected)
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Quiet,
    QuietPanel,
    Asks,
    Ended,
}

#[test]
fn a_tab_or_panel_closes_unasked_only_on_the_servers_own_evidence() {
    for case in [Case::Quiet, Case::QuietPanel, Case::Asks, Case::Ended] {
        let Fixture {
            service,
            mut peer,
            worker_thread,
            close,
        } = fixture();
        let before = read_snapshot(&service);
        let command = conditional(&service, &close, 3000, matches!(case, Case::QuietPanel));
        service.start_close_maintenance().unwrap();
        let phase = if matches!(case, Case::Ended) {
            LifecycleTerminalPhase::Exited {
                terminate_requested: false,
            }
        } else {
            RUNNING
        };
        let peer_thread = thread::spawn(move || {
            inventory(&mut peer, phase);
            // An accepted close continues as an ordinary one; its first step is parked here.
            let step = (!matches!(case, Case::Asks)).then(|| {
                let request = lifecycle(&mut peer);
                let action = request.action;
                reply(
                    &mut peer,
                    request,
                    TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::HostRefused),
                );
                action
            });
            (peer, step)
        });
        let mut read = false;
        let result = service
            .execute_close_if_idle_with(command.clone(), |targets, deadline| {
                read = true;
                assert_eq!(targets.len(), 1);
                assert!(
                    deadline <= Instant::now() + Duration::from_secs(1),
                    "one budget bounds the whole decision"
                );
                vec![match case {
                    // A shell at its prompt whose sample came too late.
                    Case::Quiet | Case::QuietPanel => CloseFacts {
                        report: Some(ShellReport::AtPrompt),
                        sample: None,
                    },
                    // A silent shell whose sample never came.
                    Case::Asks | Case::Ended => CloseFacts {
                        report: Some(ShellReport::Silent),
                        sample: None,
                    },
                }]
            })
            .unwrap();
        let (peer, step) = peer_thread.join().unwrap();
        match case {
            Case::Quiet => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::TabCloseAccepted { .. }
            )),
            Case::QuietPanel => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::PanelCloseAccepted { .. }
            )),
            Case::Asks => {
                assert!(rejected(
                    &result,
                    WorkbenchRejectionReason::ConfirmationRequired
                ));
                assert_eq!(read_snapshot(&service), before, "a refusal removes nothing");
                let mut ordinary = close.clone();
                ordinary.request_id = command.request_id.clone();
                assert!(
                    rejected(
                        &service.execute_terminal_tab_close(ordinary).unwrap(),
                        WorkbenchRejectionReason::RequestIdentityConflict
                    ),
                    "a refusal never authorizes an unconditional close under its identity"
                );
            }
            Case::Ended => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::TabCloseAccepted { .. }
            )),
        }
        // The inventory alone settles an ended terminal; a live one is read.
        assert_eq!(read, !matches!(case, Case::Ended), "{case:?}");
        // Without the Host's idle guard: the Server's evidence already decided.
        assert_eq!(
            step,
            match case {
                Case::Quiet | Case::QuietPanel => Some(TerminalLifecycleAction::Terminate),
                Case::Asks => None,
                Case::Ended => Some(TerminalLifecycleAction::Dispose),
            },
            "{case:?}"
        );
        assert_eq!(
            service.execute_close_if_idle_with(command, |_, _| unreachable!()),
            Some(result),
            "a retried request replays its answer and reads nothing"
        );
        service.stop_close_maintenance();
        drop(peer);
        let _exit = worker_thread.join().unwrap();
    }
}

#[test]
fn final_admission_counts_evidence_only_for_the_runtime_and_revision_it_was_read_from() {
    // The Workbench changes while the terminal is read.
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    service.start_close_maintenance().unwrap();
    let command = conditional(&service, &close, 3100, false);
    let peer_thread = thread::spawn(move || {
        inventory(&mut peer, RUNNING);
        peer
    });
    let result = service
        .execute_close_if_idle_with(command, |_, _| {
            let WorkbenchAction::TabClose {
                terminal_context_id,
                tab_id,
            } = close.action.clone()
            else {
                panic!("Close")
            };
            let rename = WorkbenchCommand {
                request_id: id(3101),
                action: WorkbenchAction::TabRename {
                    terminal_context_id,
                    tab_id,
                    label: WorkbenchLabel::parse("renamed").unwrap(),
                },
                ..close.clone()
            };
            let WorkbenchResponse::Command(renamed) = service
                .execute_workbench(WorkbenchRequest::Command(rename).try_into().unwrap(), true)
                .unwrap()
            else {
                panic!("rename")
            };
            assert!(!matches!(
                renamed.outcome,
                WorkbenchCommandOutcome::Rejected { .. }
            ));
            vec![CloseFacts {
                report: Some(ShellReport::AtPrompt),
                sample: None,
            }]
        })
        .unwrap();
    assert!(rejected(
        &result,
        WorkbenchRejectionReason::RevisionMismatch
    ));
    let peer = peer_thread.join().unwrap();

    // Evidence read from another incarnation of the terminal proves nothing about this one.
    let worker = WorkerPlacement {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    };
    let command = conditional(&service, &close, 3200, false);
    let command = WorkbenchCommand {
        expected_revision: read_snapshot(&service).revision,
        ..command
    };
    let mut session = service.coordinator.lock().unwrap();
    let Some(Ok(members)) = session.prepare_terminal_tab_idle_close(command.clone(), Some(worker))
    else {
        panic!("prepared")
    };
    let [(WorkbenchTabCloseMemberRuntime::Runtime { target, .. }, Some(_))] = members.as_slice()
    else {
        panic!("one live member")
    };
    let restarted = WorkbenchTabCloseRuntimeTarget::new(
        target.worker_id(),
        target.worker_generation(),
        target.terminal_id(),
        HostTerminalIdentity::new(
            HostCreationId::from_bytes([200; 16]).unwrap(),
            HostEpoch::from_bytes([4; 16]).unwrap(),
            HostTerminalId::from_bytes([201; 16]).unwrap(),
        ),
        target.fence(),
    );
    let quiet = |target| WorkbenchTabCloseMemberRuntime::QuietRuntime {
        target,
        observed_phase: RUNNING,
    };
    let result = session
        .execute_terminal_tab_close_with_idle_proof(
            command,
            Some(worker),
            true,
            Instant::now(),
            &[quiet(restarted)],
        )
        .unwrap();
    assert!(rejected(
        &result,
        WorkbenchRejectionReason::ConfirmationRequired
    ));
    drop(session);

    // Without a sampler to read the terminal, the close asks.
    let command = conditional(&service, &close, 3300, false);
    let command = WorkbenchCommand {
        expected_revision: read_snapshot(&service).revision,
        ..command
    };
    let mut peer = peer;
    let peer_thread = thread::spawn(move || {
        inventory(&mut peer, RUNNING);
        peer
    });
    let result = service.execute_terminal_tab_close(command).unwrap();
    assert!(rejected(
        &result,
        WorkbenchRejectionReason::ConfirmationRequired
    ));
    let peer = peer_thread.join().unwrap();
    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}
