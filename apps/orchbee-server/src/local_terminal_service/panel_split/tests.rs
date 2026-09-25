use super::*;
use crate::{
    site_coordinator::{PreparedOpen, PreparedTerminalTabCreate},
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
use orchbee_client_server_protocol::{Geometry, OpenOutcome, TerminalOpenRequest};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_site_protocol::WorkerId as LocalWorkerId;
use orchbee_server::CloseStepObservation;
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelSplitDirection,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome,
};
use std::{
    error::Error,
    num::NonZeroU64,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}

fn request_id(value: u128) -> RequestId {
    RequestId::parse(&format!("{value:032x}")).unwrap()
}

fn worker() -> WorkerPlacement {
    WorkerPlacement {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    }
}

fn open_request(request: u128) -> TerminalOpenRequest {
    TerminalOpenRequest::new(request_id(request), Geometry::new(80, 24, 8, 16).unwrap())
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

fn launch_result(command: &CapabilityNodeMessage, identity: u8) -> TerminalLaunchResult {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = command else {
        panic!("launch")
    };
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("immediate")
    };
    TerminalLaunchResult::new(
        launch.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            HostTerminalIdentity::new(
                HostCreationId::from_bytes([identity; 16]).unwrap(),
                HostEpoch::from_bytes([4; 16]).unwrap(),
                HostTerminalId::from_bytes([identity + 1; 16]).unwrap(),
            ),
            HostLaunchState::Started(geometry),
        )),
    )
    .unwrap()
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

fn exited_page() -> TerminalLifecycleOutcome {
    TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
        inventory_generation: NonZeroU64::MIN,
        next_cursor: None,
        returned_count: 1,
        target: Some(LifecycleTargetObservation {
            revision: 3,
            phase: LifecycleTerminalPhase::Exited {
                terminate_requested: false,
            },
        }),
    })
}

// Seeds standalone A through the real create owners; returns its context, Tab, and leaf Panel.
fn settle_first_terminal(
    service: &LocalTerminalService,
) -> (WorkbenchTerminalContextId, WorkbenchTabId, WorkbenchPanelId) {
    let snapshot = read_snapshot(service);
    let context = snapshot.navigation.standalone_terminal_context_id.clone();
    let pending = {
        let mut site = service.coordinator.lock().unwrap();
        let PreparedTerminalTabCreate::Dispatch(pending) = site.prepare_terminal_tab_create(
            WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: id(2400),
                site_id: snapshot.site_id,
                expected_revision: snapshot.revision,
                action: WorkbenchAction::TabCreate {
                    automatic_label: false,
                    terminal_context_id: context.clone(),
                    label: WorkbenchLabel::parse("first").unwrap(),
                    geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                    launch: None,
                    profile: None,
                    options: Default::default(),
                },
            },
            Some(request_id(2401)),
            lease_expiry(),
            Some(worker()),
        ) else {
            panic!("Create pending")
        };
        pending
    };
    let completed = {
        let result = launch_result(pending.command(), 3);
        let mut site = service.coordinator.lock().unwrap();
        site.complete_terminal_tab_create(pending, Completion::Launch(result), Some(worker()))
            .unwrap()
    };
    if completed.standalone_changed {
        service.standalone_changed.notify_all();
    }
    let snapshot = read_snapshot(service);
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs[0]
        .clone();
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("simple panel expected")
    };
    (context, tab.tab_id.clone(), panel_id.clone())
}

// A pending split B, a real leaf Close of A publishing first, and then B's completion must
// settle Creating(B) and wake a bare-Open waiter parked on the shared standalone slot.
#[test]
fn retargeted_split_completion_notifies_a_parked_bare_open_waiter() -> TestResult {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let worker_id = LocalWorkerId::from_bytes([9; 16]);
    let generation = {
        let WorkerAdmission::Accepted { generation } = registry.lock().unwrap().admit(worker_id)
        else {
            panic!("Worker admission")
        };
        generation
    };
    let owner = WorkerRegistry::register_carrier(
        &registry,
        generation,
        create_worker_carrier(worker_id, generation),
    )
    .ok_or("carrier registration")?;
    let (server, mut peer) = UnixStream::pair()?;
    peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    let worker_thread = thread::spawn(move || owner.run(server));
    let service = Arc::new(LocalTerminalService::new(Arc::clone(&registry)));

    let (context, tab, panel_a) = settle_first_terminal(&service);
    let committed = read_snapshot(&service);
    let split = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(2402),
        site_id: committed.site_id.clone(),
        expected_revision: committed.revision,
        action: WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: context.clone(),
            tab_id: tab.clone(),
            panel_id: panel_a.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("split").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };

    let splitter_service = Arc::clone(&service);
    let (split_send, split_result) = mpsc::sync_channel(1);
    let splitter = thread::spawn(move || {
        let _ignored = split_send.send(
            splitter_service.execute_terminal_panel_split_with_candidates(
                split,
                Some(request_id(2403)),
                lease_expiry(),
            ),
        );
    });
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_message(&mut peer)? else {
        return Err("panel.split did not dispatch a terminal Launch".into());
    };

    // Hold the registry gate so the completed Launch cannot reach Site settlement yet.
    let handle = service.workers.lock().unwrap().request_handle().unwrap();
    let workers = service.workers.lock().unwrap();
    write_message(
        &mut peer,
        &CapabilityNodeMessage::TerminalLaunchResult(launch_result(
            &CapabilityNodeMessage::AuthorizedTerminalLaunch(launch),
            7,
        )),
    )?;

    // The real leaf Close of A publishes while B's Launch is complete on the wire but
    // still unsettled in the coordinator, so the retarget lands on Creating(B).
    let committed = read_snapshot(&service);
    let close = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(2404),
        site_id: committed.site_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::PanelClose {
            terminal_context_id: context,
            tab_id: tab,
            panel_id: panel_a,
        },
    };
    let accepted = service
        .coordinator
        .lock()
        .unwrap()
        .execute_terminal_tab_close(close.clone(), Some(worker()), true, Instant::now())
        .unwrap();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));
    for _step in 0..8 {
        let Some(token) = service
            .coordinator
            .lock()
            .unwrap()
            .reserve_close_lifecycle_step(
                &close.request_id,
                Instant::now(),
                lease_expiry().unwrap(),
            )
            .unwrap()
        else {
            break;
        };
        let request = token.request();
        let receipt = handle.submit_lifecycle(request).unwrap();
        assert_eq!(lifecycle(&mut peer), request);
        service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .confirm_terminal_tab_close_step_admitted(
                &close.request_id,
                &token,
                receipt.admitted_observed_at(),
            )
            .unwrap();
        let outcome = match request.action {
            TerminalLifecycleAction::InventoryPage { .. } => exited_page(),
            _ => TerminalLifecycleOutcome::Disposed,
        };
        reply(&mut peer, request, outcome);
        let response = receipt.wait().unwrap();
        service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .settle_terminal_tab_close_step(
                &close.request_id,
                token,
                Instant::now(),
                CloseStepObservation::Result(&response),
            )
            .unwrap();
    }
    let published = {
        let mut site = service.coordinator.lock().unwrap();
        let runtime = site
            .close_authority()
            .unwrap()
            .terminal_tab_close_target(&close.request_id)
            .unwrap()
            .runtime()
            .expect("created Host runtime");
        service
            .complete_disposed_target_cleanup(&site, &close.request_id, runtime)
            .unwrap();
        site.close_authority()
            .unwrap()
            .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
            .unwrap();
        site.publish_terminal_tab_close(&close.request_id, Instant::now())
            .unwrap()
    };
    assert!(
        published,
        "removing A must retarget the standalone slot to B"
    );

    // A bare Open now waits on Creating(B); park one waiter on the shared condvar before
    // the splitter can finish settlement.
    assert!(matches!(
        service.coordinator.lock().unwrap().prepare_open(
            &open_request(2405),
            None,
            Some(worker()),
            None,
        ),
        PreparedOpen::Wait
    ));
    let parked_service = Arc::clone(&service);
    let (parked_send, parked_result) = mpsc::sync_channel(1);
    let (ready_send, ready_recv) = mpsc::sync_channel::<()>(1);
    let parked = thread::spawn(move || {
        let mut site = parked_service.coordinator.lock().unwrap();
        let waited = matches!(
            site.prepare_open(&open_request(2406), None, Some(worker()), None),
            PreparedOpen::Wait
        );
        let _ignored = ready_send.send(());
        let (mut site, _) = parked_service
            .standalone_changed
            .wait_timeout(site, Duration::from_secs(10))
            .unwrap();
        let outcome = match site.prepare_open(&open_request(2406), None, Some(worker()), None) {
            PreparedOpen::Ready(outcome) => outcome,
            PreparedOpen::Wait | PreparedOpen::Dispatch(_) => OpenOutcome::OutcomeUnknown {},
        };
        let _ignored = parked_send.send((waited, outcome));
    });
    // The parked waiter holds Site through the condvar handoff, so once it signals ready the
    // notify cannot be lost; only then release the registry gate so the splitter can settle.
    ready_recv
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_timeout| "parked Open waiter never observed Creating(B)")?;
    drop(workers);

    let completed = split_result
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_timeout| "split did not settle")??;
    assert!(matches!(
        completed.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
    let (waited, outcome) = parked_result
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_timeout| "parked Open waiter was never notified")?;
    assert!(waited, "the waiter must have observed Creating(B)");
    assert_eq!(
        outcome,
        OpenOutcome::Opened {
            terminal_create_request_id: request_id(2403)
        }
    );
    // A fresh bare Open resolves B immediately without waiting.
    assert_eq!(
        service.open_standalone(&open_request(2407)),
        OpenOutcome::Opened {
            terminal_create_request_id: request_id(2403)
        }
    );
    splitter.join().unwrap();
    parked.join().unwrap();
    drop(peer);
    let _exit = worker_thread.join().unwrap();
    Ok(())
}
