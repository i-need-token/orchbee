use super::*;
use orchbee_local_ipc::LocalStream as UnixStream;
mod interleavings;
use crate::{
    site_coordinator::PreparedTerminalTabCreate,
    terminal_coordinator::{Completion, WorkerPlacement},
    worker_obcn_carrier::create_worker_carrier,
    worker_registry::{WorkerAdmission, WorkerRegistry},
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostTerminalId, HostTerminalIdentity, LifecycleInventoryPage, LifecycleTargetObservation,
    LifecycleTerminalPhase, TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchResult,
    TerminalLifecycleAction, TerminalLifecycleOutcome, TerminalLifecycleRequest,
    TerminalLifecycleResult, WorkerGeneration, read_message, write_message,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchRequest,
    WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseStatus, WorkbenchTerminalGeometry,
};
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex, mpsc},
    thread,
};

mod admitted_control;
mod disposed_reuse;
mod idle_capacity;
mod target_fences;
mod workspace;
mod workspace_close;

struct Fixture {
    service: Arc<LocalTerminalService>,
    peer: UnixStream,
    worker_thread: thread::JoinHandle<crate::worker_obcn_carrier::WorkerWireExit>,
    close: WorkbenchCommand,
}

fn read_snapshot(service: &LocalTerminalService) -> orchbee_workbench_protocol::Snapshot {
    let request = WorkbenchRequest::SnapshotRead(orchbee_workbench_protocol::SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: id(900),
    });
    let WorkbenchResponse::Snapshot(result) = service
        .execute_workbench(request.try_into().unwrap(), false)
        .unwrap()
    else {
        panic!("snapshot")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available")
    };
    snapshot
}

fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}

fn fixture() -> Fixture {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let worker_id = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
    let WorkerAdmission::Accepted { generation } = registry.lock().unwrap().admit(worker_id) else {
        panic!("Worker admission")
    };
    let owner = WorkerRegistry::register_carrier(
        &registry,
        generation,
        create_worker_carrier(worker_id, generation),
    )
    .unwrap();
    let (server, peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let worker_thread = thread::spawn(move || owner.run(server));
    let service = Arc::new(LocalTerminalService::from_identity(
        registry,
        Ok(orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", 1)).unwrap()),
    ));
    {
        use crate::test_support::WorkbenchFixture;
        service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .seed_fixture_workspace(
                orchbee_workbench_protocol::WorkerId::parse("09090909090909090909090909090909")
                    .unwrap(),
            );
    }
    let close = add_terminal(&service, 0);
    Fixture {
        service,
        peer,
        worker_thread,
        close,
    }
}

fn add_terminal(service: &LocalTerminalService, index: u8) -> WorkbenchCommand {
    add_terminal_between(service, index, || {})
}

fn add_terminal_between(
    service: &LocalTerminalService,
    index: u8,
    before_completion: impl FnOnce(),
) -> WorkbenchCommand {
    let snapshot = read_snapshot(service);
    let create = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(10 + u128::from(index) * 100),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            label: WorkbenchLabel::parse("close witness").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let worker = WorkerPlacement {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    };
    let pending = {
        let mut site = service.coordinator.lock().unwrap();
        let PreparedTerminalTabCreate::Dispatch(pending) = site.prepare_terminal_tab_create(
            create,
            Some(
                orchbee_client_server_protocol::RequestId::parse(&format!(
                    "{:032x}",
                    11 + u128::from(index) * 100
                ))
                .unwrap(),
            ),
            lease_expiry(),
            Some(worker),
        ) else {
            panic!("Create pending")
        };
        pending
    };
    before_completion();
    {
        let mut site = service.coordinator.lock().unwrap();
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
            panic!("launch")
        };
        let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
            panic!("immediate")
        };
        let identity = HostTerminalIdentity::new(
            HostCreationId::from_bytes([3 + index; 16]).unwrap(),
            HostEpoch::from_bytes([4; 16]).unwrap(),
            HostTerminalId::from_bytes([5 + index; 16]).unwrap(),
        );
        let result = TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity,
                HostLaunchState::Started(geometry),
            )),
        )
        .unwrap();
        site.complete_terminal_tab_create(pending, Completion::Launch(result), Some(worker))
            .unwrap();
        drop(site);
    }
    let snapshot = read_snapshot(service);
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == snapshot.navigation.workspaces[0].terminal_context_id
        })
        .unwrap()
        .tabs
        .last()
        .unwrap()
        .tab_id
        .clone();
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(20 + u128::from(index) * 100),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            tab_id: tab,
        },
    }
}

fn lifecycle(peer: &mut UnixStream) -> TerminalLifecycleRequest {
    let CapabilityNodeMessage::TerminalLifecycleRequest(request) = read_message(peer).unwrap()
    else {
        panic!("lifecycle")
    };
    request
}

fn reply(
    peer: &mut UnixStream,
    request: TerminalLifecycleRequest,
    outcome: TerminalLifecycleOutcome,
) {
    write_message(
        peer,
        &CapabilityNodeMessage::TerminalLifecycleResult(
            TerminalLifecycleResult::new(request, outcome).unwrap(),
        ),
    )
    .unwrap();
}

fn settled_page() -> TerminalLifecycleOutcome {
    TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
        inventory_generation: NonZeroU64::MIN,
        next_cursor: None,
        returned_count: 1,
        target: Some(LifecycleTargetObservation {
            close_activity: orchbee_capability_node_protocol::LifecycleCloseActivity::Unknown,
            revision: 3,
            phase: LifecycleTerminalPhase::Exited {
                terminate_requested: false,
            },
        }),
    })
}

fn read(service: &LocalTerminalService, close: &WorkbenchCommand) -> WorkbenchTabCloseReadOutcome {
    let WorkbenchAction::TabClose {
        terminal_context_id,
        tab_id,
    } = &close.action
    else {
        panic!("Close")
    };
    service
        .read_terminal_tab_close(WorkbenchTabCloseRead {
            protocol_version: ProtocolVersion,
            request_id: id(40),
            site_id: close.site_id.clone(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
        })
        .unwrap()
        .outcome
}

#[test]
fn accepted_response_does_not_wait_for_worker_and_idle_close_publishes() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    service.start_close_maintenance().unwrap();
    let (send, result) = mpsc::sync_channel(1);
    let client = Arc::clone(&service);
    let command = close.clone();
    let client_thread = thread::spawn(move || {
        send.send(client.execute_terminal_tab_close(command))
            .unwrap()
    });
    let query = lifecycle(&mut peer);
    assert!(matches!(
        query.action,
        TerminalLifecycleAction::InventoryPage { .. }
    ));
    // The Worker reply is blocked while the independent command response completes.
    let accepted = result
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    client_thread.join().unwrap();
    reply(&mut peer, query, settled_page());
    let dispose = lifecycle(&mut peer);
    assert_eq!(dispose.action, TerminalLifecycleAction::Dispose);
    reply(&mut peer, dispose, TerminalLifecycleOutcome::Disposed);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. },
                ..
            }
        ) {
            break;
        }
        assert!(Instant::now() < deadline, "idle Close did not publish");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        read_snapshot(&service)
            .terminal_contexts
            .iter()
            .all(|context| context.tabs.is_empty())
    );
    service.stop_close_maintenance();
    assert_eq!(service.execute_terminal_tab_close(close), Some(accepted));
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn stopping_owner_waits_for_admitted_receipt_and_preserves_its_known_effect() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    service.start_close_maintenance().unwrap();
    let _accepted = service.execute_terminal_tab_close(close.clone()).unwrap();
    let query = lifecycle(&mut peer);
    reply(&mut peer, query, settled_page());
    let dispose = lifecycle(&mut peer);
    let stopping = Arc::clone(&service);
    let (send, stopped) = mpsc::sync_channel(1);
    let stopper = thread::spawn(move || {
        stopping.stop_close_maintenance();
        send.send(()).unwrap();
    });
    assert!(stopped.recv_timeout(Duration::from_millis(50)).is_err());
    reply(&mut peer, dispose, TerminalLifecycleOutcome::Disposed);
    stopped.recv_timeout(Duration::from_secs(2)).unwrap();
    stopper.join().unwrap();
    assert!(matches!(
        read(&service, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. },
            ..
        }
    ));
    assert_eq!(read_snapshot(&service).revision, WorkbenchRevision::new(1));
    let original = service.execute_terminal_tab_close(close.clone()).unwrap();
    let mut alias = close.clone();
    alias.request_id = id(21);
    let aliased = service.execute_terminal_tab_close(alias).unwrap();
    assert_eq!(
        original.outcome, aliased.outcome,
        "owner loss must not hide an effect-free alias"
    );
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn late_dispose_is_retained_and_local_resume_publishes_without_a_live_worker() {
    use orchbee_workbench_protocol::{
        WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseResumeOutcome,
    };
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    service.start_close_maintenance().unwrap();
    {
        let mut site = service.coordinator.lock().unwrap();
        let worker = WorkerPlacement {
            worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
            generation: WorkerGeneration::new(NonZeroU64::MIN),
        };
        let accepted = site
            .execute_terminal_tab_close(
                close.clone(),
                Some(worker),
                true,
                Instant::now() - Duration::from_millis(29_800),
            )
            .unwrap();
        assert!(matches!(
            accepted.outcome,
            WorkbenchCommandOutcome::TabCloseAccepted { .. }
        ));
    }
    service.close_maintenance.notify();
    let query = lifecycle(&mut peer);
    reply(&mut peer, query, settled_page());
    let dispose = lifecycle(&mut peer);
    assert_eq!(dispose.action, TerminalLifecycleAction::Dispose);
    thread::sleep(Duration::from_millis(250));
    let WorkbenchAction::TabClose {
        terminal_context_id,
        tab_id,
    } = &close.action
    else {
        panic!("Close")
    };
    let resume = orchbee_workbench_protocol::WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: id(60),
        site_id: close.site_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
    };
    assert!(
        matches!(
            service
                .resume_terminal_tab_close(resume.clone())
                .unwrap()
                .outcome,
            WorkbenchTabCloseResumeOutcome::Rejected { .. }
        ),
        "expiry cannot Resume across the admitted receipt"
    );
    reply(&mut peer, dispose, TerminalLifecycleOutcome::Disposed);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                    resumable: true,
                    ..
                },
                ..
            }
        ) {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    drop(peer);
    let _exit = worker_thread.join().unwrap();
    assert!(service.workers.lock().unwrap().request_handle().is_none());
    let resumed = service.resume_terminal_tab_close(resume.clone()).unwrap();
    assert!(matches!(
        resumed.outcome,
        WorkbenchTabCloseResumeOutcome::Resumed { .. }
    ));
    assert_eq!(service.resume_terminal_tab_close(resume), Some(resumed));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. },
                ..
            }
        ) {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    service.stop_close_maintenance();
}

#[test]
fn admission_between_scan_and_wait_cannot_lose_its_wakeup() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    let (scanned, after_scan) = mpsc::sync_channel(1);
    let (proceed, resumed) = mpsc::sync_channel(1);
    service.after_close_scan_for_test(move || {
        scanned.send(()).unwrap();
        resumed.recv().unwrap();
    });
    service.start_close_maintenance().unwrap();
    after_scan.recv_timeout(Duration::from_secs(2)).unwrap();
    let accepted = service.execute_terminal_tab_close(close).unwrap();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    proceed.send(()).unwrap();
    let query = lifecycle(&mut peer);
    reply(
        &mut peer,
        query,
        TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
    );
    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn round_robin_serves_another_close_before_the_first_next_step() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        mut close,
    } = fixture();
    let second = add_terminal(&service, 1);
    close.expected_revision = second.expected_revision;
    service.start_close_maintenance().unwrap();
    let first_ack = service.execute_terminal_tab_close(close.clone()).unwrap();
    assert!(matches!(
        first_ack.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    let first = lifecycle(&mut peer);
    let second_ack = service.execute_terminal_tab_close(second.clone()).unwrap();
    assert!(matches!(
        second_ack.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    reply(
        &mut peer,
        first,
        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
            inventory_generation: NonZeroU64::MIN,
            next_cursor: None,
            returned_count: 1,
            target: Some(LifecycleTargetObservation {
                close_activity: orchbee_capability_node_protocol::LifecycleCloseActivity::Unknown,
                revision: 3,
                phase: LifecycleTerminalPhase::Running {
                    terminate_requested: false,
                },
            }),
        }),
    );
    let second_query = lifecycle(&mut peer);
    assert_ne!(second_query.close_operation_id, first.close_operation_id);
    assert!(matches!(
        second_query.action,
        TerminalLifecycleAction::InventoryPage { .. }
    ));
    reply(&mut peer, second_query, settled_page());
    let terminate = lifecycle(&mut peer);
    assert_eq!(terminate.close_operation_id, first.close_operation_id);
    assert_eq!(terminate.action, TerminalLifecycleAction::Terminate);
    reply(
        &mut peer,
        terminate,
        TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
    );
    let dispose = lifecycle(&mut peer);
    assert_eq!(dispose.close_operation_id, second_query.close_operation_id);
    let WorkbenchAction::TabClose {
        terminal_context_id,
        tab_id,
    } = &close.action
    else {
        panic!("Close")
    };
    let renamed = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(70),
        site_id: close.site_id.clone(),
        expected_revision: WorkbenchRevision::new(2),
        action: WorkbenchAction::TabRename {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            label: WorkbenchLabel::parse("survives other Close").unwrap(),
        },
    };
    assert!(
        service
            .execute_workbench(WorkbenchRequest::Command(renamed).try_into().unwrap(), true)
            .is_some()
    );
    reply(&mut peer, dispose, TerminalLifecycleOutcome::Disposed);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(
            read(&service, &second),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. },
                ..
            }
        ) {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(matches!(
        read(&service, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::OutcomeUnknown { .. },
            ..
        }
    ));
    let current = read_snapshot(&service);
    assert_eq!(current.revision, WorkbenchRevision::new(4));
    let survivor = current
        .terminal_contexts
        .iter()
        .find(|context| !context.tabs.is_empty())
        .unwrap();
    assert_eq!(survivor.tabs[0].label.as_str(), "survives other Close");
    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn current_reference_retargets_for_both_close_and_create_completion_orders() {
    for complete_first in [false, true] {
        let Fixture {
            service,
            mut peer,
            worker_thread,
            mut close,
        } = fixture();
        service.start_close_maintenance().unwrap();
        close.expected_revision = WorkbenchRevision::new(2);
        let mut remove_first = || {
            let accepted = service.execute_terminal_tab_close(close.clone()).unwrap();
            assert!(matches!(
                accepted.outcome,
                WorkbenchCommandOutcome::TabCloseAccepted { .. }
            ));
            let query = lifecycle(&mut peer);
            reply(&mut peer, query, settled_page());
            let dispose = lifecycle(&mut peer);
            reply(&mut peer, dispose, TerminalLifecycleOutcome::Disposed);
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if matches!(
                    read(&service, &close),
                    WorkbenchTabCloseReadOutcome::Available {
                        status: WorkbenchTabCloseStatus::Closed { .. },
                        ..
                    }
                ) {
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(5));
            }
        };
        if complete_first {
            let _second = add_terminal(&service, 1);
            remove_first();
        } else {
            let _second = add_terminal_between(&service, 1, remove_first);
        }
        assert_eq!(read_snapshot(&service).terminal_contexts[0].tabs.len(), 1);
        assert_eq!(read_snapshot(&service).revision, WorkbenchRevision::new(3));
        service.stop_close_maintenance();
        drop(peer);
        let _exit = worker_thread.join().unwrap();
    }
}

#[test]
fn spawn_failure_prevents_primary_acceptance_and_duplicate_start_is_refused() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        close,
    } = fixture();
    let failed = service.start_close_maintenance_with(|_| {
        Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
    });
    assert_eq!(failed.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    assert!(!service.close_maintenance.available());
    assert!(matches!(
        service
            .execute_terminal_tab_close(close.clone())
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::ServerBusy
        }
    ));
    assert_eq!(
        read(&service, &close),
        WorkbenchTabCloseReadOutcome::NotClosing {}
    );
    service.start_close_maintenance().unwrap();
    assert_eq!(
        service.start_close_maintenance().unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert!(matches!(
        service.execute_terminal_tab_close(close).unwrap().outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    let query = lifecycle(&mut peer);
    reply(
        &mut peer,
        query,
        TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
    );
    service.stop_close_maintenance();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}
