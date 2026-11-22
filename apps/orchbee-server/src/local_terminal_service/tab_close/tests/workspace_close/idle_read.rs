//! The idle check answers busy exactly where a conditional Workspace Close asks, names every
//! terminal that keeps the Workspace from closing unasked, and changes nothing.
use super::*;
use crate::local_terminal_service::tab_close::evidence::{CloseFacts, ShellReport};
use orchbee_workbench_protocol::{WorkspaceIdleOutcome, WorkspaceIdleReason, WorkspaceIdleRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    /// Both shells sit at their prompts.
    Quiet,
    /// The first shell runs a command; the second sits at its prompt, and is still read.
    FirstRuns,
    /// The second shell says nothing and its sample never came.
    SecondAsks,
    /// The second terminal ended.
    SecondEnded,
    /// The second terminal is missing from its inventory.
    SecondMissing,
}

#[test]
fn the_idle_check_is_busy_where_a_workspace_close_asks_and_changes_nothing() {
    for case in [
        Case::Quiet,
        Case::FirstRuns,
        Case::SecondAsks,
        Case::SecondEnded,
        Case::SecondMissing,
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
        let (_, workspace_id) = scope(&service, &context);
        let peer_thread = thread::spawn(move || {
            for index in 0..2 {
                let request = lifecycle(&mut peer);
                assert!(matches!(
                    request.action,
                    TerminalLifecycleAction::InspectInventoryPage { cursor: None }
                ));
                let phase = if index == 1 && case == Case::SecondEnded {
                    LifecycleTerminalPhase::Exited {
                        terminate_requested: false,
                    }
                } else {
                    LifecycleTerminalPhase::Running {
                        terminate_requested: false,
                    }
                };
                let target = (index == 0 || case != Case::SecondMissing)
                    .then_some(LifecycleTargetObservation { revision: 1, phase });
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
            peer
        });
        let mut read = None;
        let result = service.read_workspace_idle_with(
            WorkspaceIdleRequest {
                protocol_version: ProtocolVersion,
                request_id: id(2300),
                session_id: before.session_id.clone(),
                workspace_id,
            },
            |targets, _| {
                read = Some(targets.len());
                (0..targets.len())
                    .map(|index| CloseFacts {
                        report: Some(match (case, index) {
                            (Case::FirstRuns, 0) => ShellReport::Running,
                            (Case::SecondAsks, 1) => ShellReport::Silent,
                            _ => ShellReport::AtPrompt,
                        }),
                        sample: None,
                    })
                    .collect()
            },
        );
        let busy = match &result.outcome {
            WorkspaceIdleOutcome::Idle {} => Vec::new(),
            WorkspaceIdleOutcome::Busy { members } => members
                .iter()
                .map(|member| member.reason)
                .collect::<Vec<_>>(),
            WorkspaceIdleOutcome::Failed { reason } => panic!("{case:?}: {reason:?}"),
        };
        let expected = match case {
            Case::Quiet | Case::SecondEnded => Vec::new(),
            Case::FirstRuns => vec![WorkspaceIdleReason::Running],
            Case::SecondAsks | Case::SecondMissing => vec![WorkspaceIdleReason::Unknown],
        };
        assert_eq!(busy, expected, "{case:?}");
        // Live terminals are read together, every one of them; an ended one needs no read.
        let expected_read = match case {
            Case::Quiet | Case::FirstRuns | Case::SecondAsks => Some(2),
            Case::SecondEnded | Case::SecondMissing => Some(1),
        };
        assert_eq!(read, expected_read, "{case:?}");
        assert_eq!(
            read_snapshot(&service),
            before,
            "{case:?}: the check changes nothing"
        );
        let peer = peer_thread.join().unwrap();
        drop(peer);
        let _exit = worker_thread.join().unwrap();
    }
}
