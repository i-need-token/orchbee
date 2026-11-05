//! One retained duplication copies its members in order, each exactly once and right after its
//! source, and stops for good at the first member it cannot copy with a started terminal.
use super::*;
use crate::{
    test_support::WorkbenchFixture,
    worker_obcn_carrier::create_worker_carrier,
    worker_registry::{WorkerAdmission, WorkerRegistry},
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostTerminalId, HostTerminalIdentity, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchResult, read_message, write_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_worker_protocol::WorkerId as LocalWorkerId;
use orchbee_workbench_protocol::{
    PresentationBinding, Snapshot, SnapshotReadIntent, TabDuplicateFailure, TabDuplicateStatus,
    TerminalId, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchTabId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkerId,
};
use std::{
    sync::{Mutex, mpsc},
    thread,
};

fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}

struct Fixture {
    service: Arc<LocalTerminalService>,
    peer: LocalStream,
    sources: Vec<WorkbenchTabId>,
}

/// A Session with one live Worker and `count` Tabs to copy.
fn fixture(count: u128) -> Fixture {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let worker_id = LocalWorkerId::from_bytes([9; 16]);
    let WorkerAdmission::Accepted { generation } = registry.lock().unwrap().admit(worker_id, None)
    else {
        panic!("the test Worker was not admitted")
    };
    let owner = WorkerRegistry::register_carrier(
        &registry,
        generation,
        create_worker_carrier(worker_id, generation),
    )
    .unwrap();
    let (server, peer) = LocalStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    thread::spawn(move || owner.run(server));
    let service = Arc::new(LocalTerminalService::workspace_fixture(registry));
    let sources = {
        let mut coordinator = service.coordinator.lock().unwrap();
        let authority = coordinator.worktree_authority().unwrap();
        (0..count)
            .map(|index| {
                authority
                    .append_fixture_tab(PresentationBinding {
                        terminal_id: TerminalId::parse(&format!("{:032x}", 700 + index)).unwrap(),
                        worker_id: WorkerId::parse(&"09".repeat(16)).unwrap(),
                    })
                    .unwrap()
                    .tab_id()
                    .clone()
            })
            .collect()
    };
    Fixture {
        service,
        peer,
        sources,
    }
}

fn snapshot(service: &LocalTerminalService) -> Snapshot {
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: id(9_999),
    });
    let Some(WorkbenchResponse::Snapshot(result)) =
        service.execute_workbench(request.try_into().unwrap(), false)
    else {
        panic!("Snapshot result expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available Snapshot expected")
    };
    snapshot
}

fn tabs(service: &LocalTerminalService) -> Vec<WorkbenchTabId> {
    snapshot(service).terminal_contexts[0]
        .tabs
        .iter()
        .map(|tab| tab.tab_id.clone())
        .collect()
}

fn ask(service: &LocalTerminalService, action: Action) -> Outcome {
    let request = WorkbenchTabDuplicateRequest {
        protocol_version: ProtocolVersion,
        request_id: id(1),
        session_id: snapshot(service).session_id,
        action,
    };
    service.answer_tab_duplicate(request, true).outcome
}

fn start_action(
    service: &LocalTerminalService,
    operation: u128,
    sources: &[WorkbenchTabId],
) -> Action {
    let snapshot = snapshot(service);
    Action::Start {
        operation_id: id(operation),
        terminal_context_id: snapshot.terminal_contexts[0].terminal_context_id.clone(),
        tab_ids: sources.to_vec(),
        expected_revision: snapshot.revision,
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
    }
}

fn start(service: &LocalTerminalService, operation: u128, sources: &[WorkbenchTabId]) -> Outcome {
    ask(service, start_action(service, operation, sources))
}

fn read(service: &LocalTerminalService, operation: u128) -> Outcome {
    ask(
        service,
        Action::Read {
            operation_id: id(operation),
        },
    )
}

/// Answers each terminal launch with `answer(index)` and reports how many launches arrived.
fn serve(
    mut peer: LocalStream,
    answer: impl Fn(u8, HostEpoch, TerminalLaunchMode) -> TerminalLaunchOutcome + Send + 'static,
) -> mpsc::Receiver<u8> {
    let (launches, seen) = mpsc::channel();
    thread::spawn(move || {
        let mut index = 0;
        while let Ok(CapabilityNodeMessage::AuthorizedTerminalLaunch(launch)) =
            read_message(&mut peer)
        {
            let epoch = HostEpoch::from_bytes([2; 16]).unwrap();
            let outcome = answer(index, epoch, launch.mode());
            let result = TerminalLaunchResult::new(launch.authorization(), outcome).unwrap();
            write_message(
                &mut peer,
                &CapabilityNodeMessage::TerminalLaunchResult(result),
            )
            .unwrap();
            index += 1;
            launches.send(index).unwrap();
        }
    });
    seen
}

fn started(index: u8, epoch: HostEpoch, mode: TerminalLaunchMode) -> TerminalLaunchOutcome {
    let TerminalLaunchMode::Immediate(geometry) = mode else {
        panic!("a copy starts immediately")
    };
    TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
        HostTerminalIdentity::new(
            HostCreationId::from_bytes([10 + index; 16]).unwrap(),
            epoch,
            HostTerminalId::from_bytes([20 + index; 16]).unwrap(),
        ),
        HostLaunchState::Started(geometry),
    ))
}

fn status(outcome: &Outcome) -> &TabDuplicateStatus {
    let Outcome::Available { status, .. } = outcome else {
        panic!("operation {outcome:?}")
    };
    status
}

fn copies(outcome: &Outcome) -> Vec<Option<WorkbenchTabId>> {
    let Outcome::Available { members, .. } = outcome else {
        panic!("operation {outcome:?}")
    };
    members
        .iter()
        .map(|member| member.copy.as_ref().map(|copy| copy.tab_id.clone()))
        .collect()
}

#[test]
fn copies_each_member_once_right_after_its_source() {
    let Fixture {
        service,
        peer,
        sources,
    } = fixture(3);
    let launches = serve(peer, started);
    let chosen = start_action(&service, 40, &[sources[0].clone(), sources[2].clone()]);
    assert_eq!(
        status(&ask(&service, chosen.clone())),
        &TabDuplicateStatus::Pending {}
    );

    service.run_tab_duplicates(&AtomicBool::new(false), Duration::from_secs(10));
    let ended = read(&service, 40);
    assert_eq!(status(&ended), &TabDuplicateStatus::Duplicated {});
    let Outcome::Available { members, .. } = &ended else {
        unreachable!()
    };
    assert!(members.iter().all(|member| matches!(
        member.copy.as_ref().and_then(|copy| copy.terminal.as_ref()),
        Some(WorkbenchTerminalOutcome::Started { .. })
    )));
    let made = copies(&ended).into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(
        tabs(&service),
        vec![
            sources[0].clone(),
            made[0].clone(),
            sources[1].clone(),
            sources[2].clone(),
            made[1].clone(),
        ]
    );
    assert_eq!(launches.try_iter().last(), Some(2));
    // A lost reply is answered by the same Start or a Read; neither copies again.
    assert_eq!(ask(&service, chosen), ended);
    service.run_tab_duplicates(&AtomicBool::new(false), Duration::from_secs(10));
    assert_eq!(tabs(&service).len(), 5);
}

#[test]
fn stops_at_the_first_copy_without_a_started_terminal_and_never_copies_a_later_member() {
    let Fixture {
        service,
        peer,
        sources,
    } = fixture(2);
    let launches = serve(peer, |_, host_epoch, _| {
        TerminalLaunchOutcome::OutcomeUnknown { host_epoch }
    });
    start(&service, 50, &sources);
    service.run_tab_duplicates(&AtomicBool::new(false), Duration::from_secs(10));
    let ended = read(&service, 50);
    assert_eq!(
        status(&ended),
        &TabDuplicateStatus::Failed {
            reason: TabDuplicateFailure::OutcomeUnknown
        }
    );
    let made = copies(&ended);
    assert!(made[0].is_some() && made[1].is_none());
    assert_eq!(tabs(&service).len(), 3);
    assert_eq!(launches.try_iter().last(), Some(1));
}

#[test]
fn a_stopping_session_or_spent_budget_creates_nothing_more() {
    for (stop, budget, reason) in [
        (
            true,
            Duration::from_secs(10),
            TabDuplicateFailure::SessionStopping,
        ),
        (false, Duration::ZERO, TabDuplicateFailure::Deadline),
    ] {
        let Fixture {
            service,
            peer,
            sources,
        } = fixture(2);
        let launches = serve(peer, started);
        start(&service, 60, &sources);
        service.run_tab_duplicates(&AtomicBool::new(stop), budget);
        let ended = read(&service, 60);
        assert_eq!(status(&ended), &TabDuplicateStatus::Failed { reason });
        assert_eq!(copies(&ended), vec![None, None]);
        assert_eq!(tabs(&service), sources);
        assert_eq!(launches.try_iter().count(), 0);
    }
}
