use super::*;
use crate::session_coordinator::PreparedTerminalTabCreate;
use orchbee_workbench_protocol::{
    ProtocolVersion, SnapshotReadIntent, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTabMoveDirection,
    WorkbenchTerminalGeometry,
};

#[test]
fn committed_move_finishes_while_another_owner_holds_the_worker_registry() {
    let workers = Arc::new(Mutex::new(WorkerRegistry::new()));
    let service = Arc::new(LocalTerminalService::workspace_fixture(Arc::clone(
        &workers,
    )));
    let read = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
    });
    let WorkbenchResponse::Snapshot(result) = service
        .execute_workbench(read.try_into().unwrap(), false)
        .unwrap()
    else {
        panic!("snapshot expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available snapshot expected")
    };
    for index in 0..2 {
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 10 + index)).unwrap(),
            session_id: snapshot.session_id.clone(),
            expected_revision: WorkbenchRevision::new(index),
            action: WorkbenchAction::TabCreate {
                placement: None,
                automatic_label: false,
                terminal_context_id: snapshot.navigation.workspaces[0]
                    .terminal_context_id
                    .clone(),
                label: WorkbenchLabel::parse("pending").unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        };
        assert!(matches!(
            service
                .coordinator
                .lock()
                .unwrap()
                .prepare_terminal_tab_create(
                    command,
                    Some(
                        orchbee_client_server_protocol::RequestId::parse(&format!(
                            "{:032x}",
                            20 + index
                        ))
                        .unwrap()
                    ),
                    lease_expiry(),
                    Some(WorkerPlacement {
                        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16])
                            .unwrap(),
                        generation: WorkerGeneration::new(NonZeroU64::MIN),
                    }),
                ),
            PreparedTerminalTabCreate::Dispatch(_)
        ));
    }
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 30)).unwrap(),
        session_id: snapshot.session_id,
        expected_revision: WorkbenchRevision::new(2),
        action: WorkbenchAction::TabMove {
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&format!("{:032x}", 2))
                .unwrap(),
            direction: WorkbenchTabMoveDirection::Last,
        },
    };
    let held_registry = workers.lock().unwrap();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let actor = Arc::clone(&service);
    let thread = std::thread::spawn(move || {
        let _sent = sender.send(actor.execute_terminal_tab_move(command));
    });
    let result = receiver.recv_timeout(std::time::Duration::from_secs(2));
    drop(held_registry);
    thread.join().unwrap();
    assert!(
        matches!(result.unwrap().unwrap().outcome, WorkbenchCommandOutcome::TabMoved { revision, .. } if revision == WorkbenchRevision::new(3))
    );
    assert_eq!(
        service.coordinator.lock().unwrap().allocate_identity(),
        Some(3_u128.to_be_bytes())
    );
}
