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
use orchbee_connection_handshake::CohortIdentity;
use std::{
    io::Read,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[test]
fn the_reply_lists_the_server_first_then_the_worker_it_started() {
    let cohort = "ab".repeat(32);
    let identity = ServerIdentity::local(Some(&cohort), Some(4242));
    let result = result(&identity, false, 3).unwrap();
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

    let alone = super::result(&ServerIdentity::local(None, None), true, 0).unwrap();
    let [server] = alone.processes() else {
        panic!("an unmanaged Server lists only itself")
    };
    assert_eq!(server.build.cohort, None);
    assert_eq!(server.state, ProcessState::Stopping);
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
