use super::*;
use crate::{
    agent_service::{AgentPolicy, AgentService},
    client_connection_registry::{
        ClientConnectionAdmission, ClientConnectionRegistry, MAXIMUM_ACTIVE_CLIENT_CONNECTIONS,
    },
    local_client_service::{
        ConnectionSlots, handle_candidate_control, stop_control::StopControl,
        tests::handshake_client,
    },
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_connection_handshake::{Build, CohortIdentity, EnvironmentName, ProductVersion};
use std::{
    io::Read,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[test]
fn the_reply_lists_the_server_first_then_the_worker_it_started() {
    let cohort = "ab".repeat(32);
    let identity = ServerIdentity::local(Some(&cohort), Some(4242));
    let result = result(&identity, ProcessState::Ready, 3, Vec::new()).unwrap();
    assert_eq!(result.restart_keeps_terminals(), Some(false));
    let [server, worker] = result.processes() else {
        panic!("expected the Server and its Worker")
    };
    assert_eq!(server.role, ProcessRole::Server);
    assert_eq!(server.pid.get(), std::process::id());
    assert_eq!(server.product_version.as_str(), env!("CARGO_PKG_VERSION"));
    assert_eq!(server.build.cohort, CohortIdentity::parse(&cohort).ok());
    assert_eq!(
        server.environment,
        orchbee_local_runtime::environment_name()
    );
    assert_eq!((server.state, server.terminals), (ProcessState::Ready, 3));
    let client = server
        .connections
        .iter()
        .find(|entry| entry.kind.as_str() == ConnectionKind::ClientServer.name())
        .expect("the Client endpoint");
    assert_eq!(
        client.generations,
        ConnectionKind::ClientServer.supported_generations()
    );
    // Only the answering process states capabilities.
    assert!(
        server
            .connections
            .iter()
            .all(|entry| entry.capabilities.is_some())
    );
    assert_eq!(worker.role, ProcessRole::Worker);
    assert_eq!(worker.pid.get(), 4242);
    assert_eq!(worker.product_version, server.product_version);
    assert!(
        worker
            .connections
            .iter()
            .all(|entry| entry.capabilities.is_none())
    );
    assert!(
        worker
            .connections
            .iter()
            .any(|entry| entry.kind.as_str() == ConnectionKind::WorkerHost.name())
    );

    let alone = super::result(
        &ServerIdentity::local(None, None),
        ProcessState::Stopping,
        0,
        Vec::new(),
    )
    .unwrap();
    let [server] = alone.processes() else {
        panic!("an unmanaged Server lists only itself")
    };
    assert_eq!(server.build.cohort, None);
    assert_eq!(server.state, ProcessState::Stopping);
}

#[test]
fn hosts_follow_the_worker_in_its_order_without_capabilities_and_at_most_256() {
    let connection = |kind: &str| ConnectionEntry {
        kind: ContractName::new(kind).unwrap(),
        generations: ConnectionKind::WorkerHost.supported_generations(),
        capabilities: Some(CapabilitySet::default()),
    };
    // Each Host states its own facts; these arrive out of order and with capabilities.
    let host = |pid: u32| ProcessEntry {
        role: ProcessRole::TerminalHost,
        product_version: ProductVersion::new("0.2.0").unwrap(),
        build: Build::default(),
        environment: EnvironmentName::new("ob-test-host").unwrap(),
        pid: NonZeroU32::new(pid).unwrap(),
        state: ProcessState::Draining,
        terminals: pid,
        connections: vec![connection("worker-host"), connection("frame-a")],
    };
    let identity = ServerIdentity::local(None, Some(4242));
    let reply = result(
        &identity,
        ProcessState::Ready,
        2,
        (1..=300).map(host).collect(),
    )
    .unwrap();
    let [server, worker, hosts @ ..] = reply.processes() else {
        panic!("expected the Server and its Worker before the Hosts")
    };
    assert_eq!(
        (server.role, worker.role),
        (ProcessRole::Server, ProcessRole::Worker)
    );
    assert_eq!(hosts.len(), 256);
    for (entry, pid) in hosts.iter().zip(1..) {
        assert_eq!(entry.role, ProcessRole::TerminalHost);
        assert_eq!((entry.pid.get(), entry.terminals), (pid, pid));
        assert_eq!(entry.state, ProcessState::Draining);
        assert_eq!(entry.product_version.as_str(), "0.2.0");
        let kinds = entry
            .connections
            .iter()
            .map(|c| c.kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(kinds, ["frame-a", "worker-host"]);
        assert!(entry.connections.iter().all(|c| c.capabilities.is_none()));
    }

    let alone = result(
        &ServerIdentity::local(None, None),
        ProcessState::Ready,
        0,
        vec![host(1)],
    )
    .unwrap();
    assert_eq!(
        alone.processes().len(),
        1,
        "no Worker is listed, so no Host follows one"
    );
}

#[test]
fn query_bypasses_connection_admission_without_allocating_a_generation_or_stop_lease() {
    #[cfg(windows)]
    let modes = [None, Some(false), Some(true)];
    #[cfg(not(windows))]
    let modes = [None::<bool>];
    for mode in modes {
        let registry = Arc::new(Mutex::new(ClientConnectionRegistry::new()));
        for _ in 0..MAXIMUM_ACTIVE_CLIENT_CONNECTIONS {
            assert!(matches!(
                registry.lock().unwrap().admit(),
                ClientConnectionAdmission::Accepted { .. }
            ));
        }
        let before = registry.lock().unwrap().clone();
        let server_registry = Arc::clone(&registry);
        let (mut client, stream) = LocalStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let handler = thread::spawn(move || {
            let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
                WorkerRegistry::new(),
            ))));
            let agents = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let workbench = WorkbenchService::new(
                WorkbenchPolicy {
                    read: true,
                    manage_navigation: true,
                    associate_directory: true,
                    create_terminal: true,
                    manage_terminal_lifecycle: true,
                },
                Arc::clone(&terminals),
            );
            #[cfg(windows)]
            let shutdown = crate::session_shutdown::SessionShutdown::default();
            #[cfg(windows)]
            if mode == Some(true) {
                use orchbee_local_client_protocol::{SessionStopRequest, StopAction, StopId};
                shutdown.request(&SessionStopRequest {
                    id: StopId::new([1; 16]).unwrap(),
                    action: StopAction::Start,
                });
            }
            let slots = Arc::new(ConnectionSlots::new());
            handle_candidate_control(
                stream,
                &server_registry,
                &ServerIdentity::local(None, None),
                slots.acquire().unwrap(),
                &terminals,
                &workbench,
                StopControl {
                    agents: &agents,
                    #[cfg(windows)]
                    shutdown: mode.map(|_| &shutdown),
                },
            );
            #[cfg(windows)]
            {
                assert_eq!(shutdown.stopping(), mode == Some(true));
                assert!(shutdown.settle_records(std::time::Instant::now()));
            }
        });
        let reply = handshake_client::query(&mut client, orchbee_local_runtime::environment_name())
            .unwrap();
        let result = handshake_client::query_result(reply).unwrap();
        let expected = if mode == Some(true) {
            ProcessState::Stopping
        } else {
            ProcessState::Ready
        };
        assert_eq!(result.processes()[0].state, expected);
        assert_eq!(client.read(&mut [0]).unwrap(), 0);
        handler.join().unwrap();
        assert_eq!(*registry.lock().unwrap(), before);
    }
}
