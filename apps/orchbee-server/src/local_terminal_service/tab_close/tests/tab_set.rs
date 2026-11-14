//! A Tab set closes its members one at a time through their own Tab Closes, stops at the first
//! member it cannot close, and closes unasked only on the Server's own evidence.
use super::*;
use crate::{
    local_terminal_service::{
        tab_close::evidence::{CloseFacts, ShellReport},
        tab_set_close::CloseFactsRead,
    },
    session_coordinator::ProcessObservationTarget,
};
use orchbee_capability_node_protocol::LifecycleUnknownReason;
use orchbee_workbench_protocol::{
    TabSetCloseAction, TabSetCloseCondition, TabSetCloseFailure, TabSetCloseOutcome,
    TabSetCloseStatus, TabSetCloseUnavailable, WorkbenchTabId, WorkbenchTabSetCloseRequest,
};
use std::sync::atomic::AtomicBool;

const RUNNING: LifecycleTerminalPhase = LifecycleTerminalPhase::Running {
    terminate_requested: false,
};
const EXITED: LifecycleTerminalPhase = LifecycleTerminalPhase::Exited {
    terminate_requested: false,
};

/// The Host terminal `add_terminal(_, index)` launched.
fn host(index: u8) -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([3 + index; 16]).unwrap(),
        HostEpoch::from_bytes([4; 16]).unwrap(),
        HostTerminalId::from_bytes([5 + index; 16]).unwrap(),
    )
}

fn tab(close: &WorkbenchCommand) -> WorkbenchTabId {
    let WorkbenchAction::TabClose { tab_id, .. } = &close.action else {
        panic!("Close")
    };
    tab_id.clone()
}

fn page(phase: LifecycleTerminalPhase) -> TerminalLifecycleOutcome {
    TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
        inventory_generation: NonZeroU64::MIN,
        next_cursor: None,
        returned_count: 1,
        target: Some(LifecycleTargetObservation { revision: 1, phase }),
    })
}

fn facts(
    report: ShellReport,
) -> impl Fn(Vec<ProcessObservationTarget>, Instant) -> Vec<CloseFacts> {
    move |targets, _| {
        targets
            .iter()
            .map(|_| CloseFacts {
                report: Some(report),
                sample: None,
            })
            .collect()
    }
}

fn unread(_: Vec<ProcessObservationTarget>, _: Instant) -> Vec<CloseFacts> {
    unreachable!("a confirmed set reads nothing")
}

fn request(
    service: &LocalTerminalService,
    request: u128,
    action: TabSetCloseAction,
) -> WorkbenchTabSetCloseRequest {
    WorkbenchTabSetCloseRequest {
        protocol_version: ProtocolVersion,
        request_id: id(request),
        session_id: read_snapshot(service).session_id,
        action,
    }
}

fn start(
    service: &LocalTerminalService,
    operation: u128,
    members: &[&WorkbenchCommand],
    condition: TabSetCloseCondition,
    read: &CloseFactsRead<'_>,
) -> TabSetCloseOutcome {
    let snapshot = read_snapshot(service);
    let action = TabSetCloseAction::Start {
        operation_id: id(operation),
        terminal_context_id: snapshot.navigation.workspaces[0]
            .terminal_context_id
            .clone(),
        tab_ids: members.iter().map(|close| tab(close)).collect(),
        expected_revision: snapshot.revision,
        condition,
    };
    service
        .start_tab_set_close(request(service, operation + 1, action), true, read)
        .outcome
}

fn set(service: &LocalTerminalService, operation: u128) -> TabSetCloseOutcome {
    let action = TabSetCloseAction::Read {
        operation_id: id(operation),
    };
    service
        .start_tab_set_close(request(service, operation + 2, action), true, &unread)
        .outcome
}

fn status(outcome: &TabSetCloseOutcome) -> &TabSetCloseStatus {
    let TabSetCloseOutcome::Available { status, .. } = outcome else {
        panic!("set {outcome:?}")
    };
    status
}

fn run(service: &LocalTerminalService, stop: bool, budget: Duration, read: &CloseFactsRead<'_>) {
    service.run_tab_sets(&AtomicBool::new(stop), budget, read);
}

/// Answers the next `count` lifecycle requests in order; each answer may first look at the
/// Server.
fn serve(
    mut peer: UnixStream,
    count: usize,
    mut answer: impl FnMut(&TerminalLifecycleRequest) -> TerminalLifecycleOutcome + Send + 'static,
) -> thread::JoinHandle<(
    UnixStream,
    Vec<(TerminalLifecycleAction, HostTerminalIdentity)>,
)> {
    thread::spawn(move || {
        let mut seen = Vec::with_capacity(count);
        for _ in 0..count {
            let request = lifecycle(&mut peer);
            seen.push((request.action, request.target));
            let outcome = answer(&request);
            reply(&mut peer, request, outcome);
        }
        (peer, seen)
    })
}

fn tabs(service: &LocalTerminalService) -> Vec<WorkbenchTabId> {
    read_snapshot(service)
        .terminal_contexts
        .iter()
        .flat_map(|context| &context.tabs)
        .map(|tab| tab.tab_id.clone())
        .collect()
}

#[test]
fn an_if_idle_set_closes_unasked_only_while_every_member_proves_quiet() {
    let Fixture {
        service,
        peer,
        worker_thread,
        close,
    } = fixture();
    let second = add_terminal(&service, 1);
    service.start_close_maintenance().unwrap();
    let before = read_snapshot(&service);

    let inspections = serve(peer, 2, |_| page(RUNNING));
    let asked = start(
        &service,
        3000,
        &[&close, &second],
        TabSetCloseCondition::IfIdle,
        &|targets, _| {
            assert_eq!(targets.len(), 2, "one read covers every member");
            vec![
                CloseFacts {
                    report: Some(ShellReport::AtPrompt),
                    sample: None,
                },
                CloseFacts {
                    report: Some(ShellReport::Running),
                    sample: None,
                },
            ]
        },
    );
    let (peer, _) = inspections.join().unwrap();
    assert_eq!(asked, TabSetCloseOutcome::ConfirmationRequired {});
    assert_eq!(
        set(&service, 3000),
        TabSetCloseOutcome::NotFound {},
        "a set that asks retains nothing"
    );
    assert_eq!(
        read_snapshot(&service),
        before,
        "a set that asks closes nothing"
    );

    let inspections = serve(peer, 2, |_| page(RUNNING));
    let admitted = start(
        &service,
        3000,
        &[&close, &second],
        TabSetCloseCondition::IfIdle,
        &facts(ShellReport::AtPrompt),
    );
    let (peer, _) = inspections.join().unwrap();
    assert_eq!(status(&admitted), &TabSetCloseStatus::Pending {});

    // The first member's terminal ended meanwhile; the second now runs a command.
    let steps = serve(peer, 3, |request| match request.action {
        TerminalLifecycleAction::InspectInventoryPage { .. } if request.target == host(0) => {
            page(EXITED)
        }
        TerminalLifecycleAction::InspectInventoryPage { .. } => page(RUNNING),
        TerminalLifecycleAction::Dispose => TerminalLifecycleOutcome::Disposed,
        action => panic!("unexpected {action:?}"),
    });
    run(
        &service,
        false,
        Duration::from_secs(10),
        &facts(ShellReport::Running),
    );
    let (peer, seen) = steps.join().unwrap();
    assert_eq!(seen[1], (TerminalLifecycleAction::Dispose, host(0)));
    let ended = set(&service, 3000);
    assert_eq!(
        status(&ended),
        &TabSetCloseStatus::Failed {
            reason: TabSetCloseFailure::ChildBusy
        }
    );
    let TabSetCloseOutcome::Available { members, .. } = &ended else {
        unreachable!()
    };
    assert!(members[0].is_done());
    assert_eq!(members[1].close, None, "a member that asks is never closed");
    assert_eq!(tabs(&service), vec![tab(&second)]);

    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn a_set_closes_one_member_at_a_time_and_counts_a_gone_tab_done() {
    let Fixture {
        service,
        peer,
        worker_thread,
        close,
    } = fixture();
    let second = add_terminal(&service, 1);
    let third = add_terminal(&service, 2);
    service.start_close_maintenance().unwrap();
    assert_eq!(
        status(&start(
            &service,
            4000,
            &[&close, &second, &third],
            TabSetCloseCondition::Confirmed,
            &unread,
        )),
        &TabSetCloseStatus::Pending {}
    );

    // The middle member closes on its own before the set reaches it.
    let independent = serve(peer, 2, |request| match request.action {
        TerminalLifecycleAction::InventoryPage { .. } => settled_page(),
        TerminalLifecycleAction::Dispose => TerminalLifecycleOutcome::Disposed,
        action => panic!("unexpected {action:?}"),
    });
    let own = WorkbenchCommand {
        request_id: id(4100),
        expected_revision: read_snapshot(&service).revision,
        ..second.clone()
    };
    assert!(matches!(
        service
            .execute_terminal_tab_close(own.clone())
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    let (peer, _) = independent.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !matches!(
        read(&service, &own),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ) {
        assert!(
            Instant::now() < deadline,
            "the independent Close did not publish"
        );
        thread::sleep(Duration::from_millis(5));
    }

    let observer = Arc::clone(&service);
    let steps = serve(peer, 4, move |request| {
        if request.target == host(2) && request.action != TerminalLifecycleAction::Dispose {
            let TabSetCloseOutcome::Available { members, .. } = set(&observer, 4000) else {
                panic!("set")
            };
            assert!(
                members[0].is_done() && members[1].absent,
                "a later member's Close starts only after every earlier member is done"
            );
        }
        match request.action {
            TerminalLifecycleAction::InventoryPage { .. } => settled_page(),
            TerminalLifecycleAction::Dispose => TerminalLifecycleOutcome::Disposed,
            action => panic!("unexpected {action:?}"),
        }
    });
    run(&service, false, Duration::from_secs(10), &unread);
    let (peer, seen) = steps.join().unwrap();
    assert_eq!(
        seen.iter().map(|(_, target)| *target).collect::<Vec<_>>(),
        vec![host(0), host(0), host(2), host(2)]
    );
    let closed = set(&service, 4000);
    assert_eq!(status(&closed), &TabSetCloseStatus::Closed {});
    let TabSetCloseOutcome::Available { members, .. } = &closed else {
        unreachable!()
    };
    assert!(members[1].absent && members[1].close.is_none());
    assert!(tabs(&service).is_empty());

    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn a_set_stops_at_an_unknown_member_and_never_submits_a_later_one() {
    let Fixture {
        service,
        peer,
        worker_thread,
        close,
    } = fixture();
    let second = add_terminal(&service, 1);
    service.start_close_maintenance().unwrap();
    assert!(matches!(
        start(
            &service,
            5000,
            &[&close, &second],
            TabSetCloseCondition::Confirmed,
            &unread,
        ),
        TabSetCloseOutcome::Available { .. }
    ));
    let steps = serve(peer, 2, move |request| match request.action {
        TerminalLifecycleAction::InventoryPage { .. } => settled_page(),
        TerminalLifecycleAction::Dispose => {
            TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost)
        }
        action => panic!("unexpected {action:?}"),
    });
    run(&service, false, Duration::from_secs(10), &unread);
    let (peer, seen) = steps.join().unwrap();
    assert!(seen.iter().all(|(_, target)| *target == host(0)));
    let ended = set(&service, 5000);
    assert_eq!(
        status(&ended),
        &TabSetCloseStatus::Failed {
            reason: TabSetCloseFailure::OutcomeUnknown
        }
    );
    let TabSetCloseOutcome::Available { members, .. } = &ended else {
        unreachable!()
    };
    assert_eq!(members[1].close, None);
    assert!(tabs(&service).contains(&tab(&second)));
    // The member's own Close stays retained for its own Resume.
    assert!(matches!(
        read(&service, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::CleanupBlocked {
                resumable: true,
                ..
            } | WorkbenchTabCloseStatus::OutcomeUnknown { .. },
            ..
        }
    ));
    assert_eq!(
        start(
            &service,
            5100,
            &[&close],
            TabSetCloseCondition::Confirmed,
            &unread
        ),
        TabSetCloseOutcome::Unavailable {
            reason: TabSetCloseUnavailable::Busy
        },
        "an open Close owns its Tab"
    );
    assert!(
        matches!(
            start(
                &service,
                5200,
                &[&second],
                TabSetCloseCondition::Confirmed,
                &unread
            ),
            TabSetCloseOutcome::Available { .. }
        ),
        "an ended set releases the members it never closed"
    );

    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[derive(Clone, Copy, Debug)]
enum Stop {
    Deadline,
    Session,
    Worker,
}

#[test]
fn a_set_stops_before_its_next_child_when_time_runs_out_the_session_stops_or_the_worker_changes() {
    for case in [Stop::Deadline, Stop::Session, Stop::Worker] {
        let Fixture {
            service,
            peer,
            worker_thread,
            close,
        } = fixture();
        assert!(matches!(
            start(
                &service,
                6000,
                &[&close],
                TabSetCloseCondition::Confirmed,
                &unread
            ),
            TabSetCloseOutcome::Available { .. }
        ));
        let (peer, worker_thread) = if matches!(case, Stop::Worker) {
            drop(peer);
            let _exit = worker_thread.join().unwrap();
            let worker_id = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
            let WorkerAdmission::Accepted { generation } =
                service.workers.lock().unwrap().admit(worker_id, None)
            else {
                panic!("replacement admission")
            };
            let owner = WorkerRegistry::register_carrier(
                &service.workers,
                generation,
                create_worker_carrier(worker_id, generation),
            )
            .unwrap();
            let (server, peer) = UnixStream::pair().unwrap();
            (peer, thread::spawn(move || owner.run(server)))
        } else {
            (peer, worker_thread)
        };
        let (budget, stop) = match case {
            Stop::Deadline => (Duration::ZERO, false),
            Stop::Session => (Duration::from_secs(10), true),
            Stop::Worker => (Duration::from_secs(10), false),
        };
        run(&service, stop, budget, &unread);
        let ended = set(&service, 6000);
        assert_eq!(
            status(&ended),
            &TabSetCloseStatus::Failed {
                reason: match case {
                    Stop::Deadline => TabSetCloseFailure::Deadline,
                    Stop::Session => TabSetCloseFailure::SessionStopping,
                    Stop::Worker => TabSetCloseFailure::SourceChanged,
                }
            },
            "{case:?}"
        );
        let TabSetCloseOutcome::Available { members, .. } = &ended else {
            unreachable!()
        };
        assert_eq!(members[0].close, None, "{case:?}");
        assert_eq!(tabs(&service), vec![tab(&close)], "{case:?}");

        drop(peer);
        let _exit = worker_thread.join().unwrap();
    }
}
