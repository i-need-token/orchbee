use super::*;
use crate::{
    worker_obcn_carrier::create_worker_carrier,
    worker_registry::{WorkerAdmission, WorkerRegistry},
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostQueryResult, read_message, write_message,
};
use orchbee_connection_handshake::{
    Build, EnvironmentName, ProcessRole, ProcessState, ProductVersion,
};
use orchbee_local_ipc::LocalStream;
use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

fn answer(peer: &mut LocalStream, hosts: Vec<ProcessEntry>) {
    let CapabilityNodeMessage::HostQueryRequest(request) = read_message(peer).unwrap() else {
        panic!("expected a Host query");
    };
    let result = HostQueryResult { request, hosts };
    write_message(peer, &CapabilityNodeMessage::HostQueryResult(result)).unwrap();
}

/// The query reply is due within the candidate deadline, so a Worker that answers late must read
/// as no Host by the reader's deadline, and its late reply must not end the carrier.
#[test]
fn a_late_worker_lists_no_host_by_the_deadline_and_keeps_its_carrier() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let worker_id = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
    let WorkerAdmission::Accepted { generation } = registry.lock().unwrap().admit(worker_id, None)
    else {
        panic!("Worker admission");
    };
    let owner = WorkerRegistry::register_carrier(
        &registry,
        generation,
        create_worker_carrier(worker_id, generation),
    )
    .unwrap();
    let (server, mut peer) = LocalStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let stop = server.try_clone().unwrap();
    let owner = thread::spawn(move || owner.run(server));
    let host = ProcessEntry {
        role: ProcessRole::TerminalHost,
        product_version: ProductVersion::new("0.2.0").unwrap(),
        build: Build::default(),
        environment: EnvironmentName::new("ob-test-host").unwrap(),
        pid: NonZeroU32::new(77).unwrap(),
        state: ProcessState::Ready,
        terminals: 2,
        connections: Vec::new(),
    };
    let (late, release) = mpsc::channel::<()>();
    let worker = {
        let host = host.clone();
        thread::spawn(move || {
            answer(&mut peer, vec![host.clone()]);
            release.recv().unwrap();
            answer(&mut peer, Vec::new());
            answer(&mut peer, vec![host]);
        })
    };
    let service = LocalTerminalService::new(registry);
    let patient = Instant::now() + Duration::from_secs(5);
    assert_eq!(service.connected_hosts(patient), vec![host.clone()]);
    let started = Instant::now();
    assert!(
        service
            .connected_hosts(started + Duration::from_millis(200))
            .is_empty()
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    late.send(()).unwrap();
    let patient = Instant::now() + Duration::from_secs(5);
    assert_eq!(service.connected_hosts(patient), vec![host]);
    worker.join().unwrap();
    // macOS reports NotConnected once the carrier has already closed; Linux shuts down quietly.
    match stop.shutdown(std::net::Shutdown::Both) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotConnected => {}
        Err(error) => panic!("stop shutdown: {error}"),
    }
    owner.join().unwrap();
}
