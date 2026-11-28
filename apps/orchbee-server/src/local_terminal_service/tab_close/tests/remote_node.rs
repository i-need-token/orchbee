//! A Workspace whose Directory is on a remote node's Worker runs its terminals there: creation,
//! each Close step and the Worker's retirement reach that node's carrier, never the local one.
use super::*;

#[test]
fn a_workspace_on_a_remote_node_creates_and_closes_its_terminals_there() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let local_id = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
    let node_id = orchbee_local_worker_protocol::WorkerId::from_bytes([7; 16]);
    let machine = orchbee_machine_protocol::MachineId::new("7".repeat(32)).unwrap();
    let (local_generation, node_generation) = {
        let mut registry = registry.lock().unwrap();
        let WorkerAdmission::Accepted { generation: local } = registry.admit(local_id, None) else {
            panic!("local Worker admission")
        };
        let WorkerAdmission::Accepted { generation: node } =
            registry.admit_node(machine, node_id, None)
        else {
            panic!("node Worker admission")
        };
        (local, node)
    };
    let local_owner = WorkerRegistry::register_carrier(
        &registry,
        local_generation,
        create_worker_carrier(local_id, local_generation),
    )
    .unwrap();
    let node_owner = WorkerRegistry::register_node_carrier(
        &registry,
        node_generation,
        create_worker_carrier(node_id, node_generation),
        true,
    )
    .unwrap();
    let (local_server, local_peer) = UnixStream::pair().unwrap();
    let (node_server, mut node_peer) = UnixStream::pair().unwrap();
    node_peer
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let local_thread = thread::spawn(move || local_owner.run(local_server));
    let node_thread = thread::spawn(move || node_owner.run(node_server));
    let service = Arc::new(LocalTerminalService::from_identity(
        registry,
        Ok(orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", 1)).unwrap()),
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
                orchbee_workbench_protocol::WorkerId::parse(&"07".repeat(16)).unwrap(),
            );
    }

    let snapshot = read_snapshot(&service);
    let terminal_context_id = snapshot.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(10),
        session_id: snapshot.session_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: terminal_context_id.clone(),
            label: WorkbenchLabel::parse("remote witness").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let creator = Arc::clone(&service);
    let creation = thread::spawn(move || creator.execute_terminal_tab_create(create));
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) =
        read_message(&mut node_peer).unwrap()
    else {
        panic!("the node's Worker launches the Workspace's terminal")
    };
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("immediate")
    };
    write_message(
        &mut node_peer,
        &CapabilityNodeMessage::TerminalLaunchResult(
            TerminalLaunchResult::new(
                launch.authorization(),
                TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                    HostTerminalIdentity::new(
                        HostCreationId::from_bytes([3; 16]).unwrap(),
                        HostEpoch::from_bytes([4; 16]).unwrap(),
                        HostTerminalId::from_bytes([5; 16]).unwrap(),
                    ),
                    HostLaunchState::Started(geometry),
                )),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let WorkbenchCommandOutcome::TabCreated { tab_id, .. } =
        creation.join().unwrap().unwrap().outcome
    else {
        panic!("the Tab starts on the node")
    };

    service.start_close_maintenance().unwrap();
    let snapshot = read_snapshot(&service);
    let close = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(20),
        session_id: snapshot.session_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id,
            tab_id,
        },
    };
    assert!(matches!(
        service
            .execute_terminal_tab_close(close.clone())
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    let query = lifecycle(&mut node_peer);
    assert!(matches!(
        query.action,
        TerminalLifecycleAction::InventoryPage { .. }
    ));
    reply(&mut node_peer, query, settled_page());
    let dispose = lifecycle(&mut node_peer);
    assert_eq!(dispose.action, TerminalLifecycleAction::Dispose);
    reply(&mut node_peer, dispose, TerminalLifecycleOutcome::Disposed);
    // Once the Close ends, the node's Worker is told to forget its Disposed record.
    let retire = lifecycle(&mut node_peer);
    assert_eq!(retire.action, TerminalLifecycleAction::Retire);
    assert_eq!(
        retire.authorization.worker_generation.get(),
        node_generation.get()
    );
    reply(&mut node_peer, retire, TerminalLifecycleOutcome::Retired);
    assert!(matches!(
        read(&service, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));

    service.stop_close_maintenance();
    drop(node_peer);
    drop(local_peer);
    let _node_exit = node_thread.join().unwrap();
    let _local_exit = local_thread.join().unwrap();
}
