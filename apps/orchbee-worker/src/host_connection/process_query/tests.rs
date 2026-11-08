use super::*;
use orchbee_connection_handshake::{
    AcceptorMessage, Build, CapabilitySet, ConnectionEntry, ConnectionKind, ContractName,
    InitiatorMessage, ProcessState, ProductVersion, QueryResult, encode_acceptor_frame,
    read_initiator_message,
};
use orchbee_local_ipc::LocalListener;
use std::{io::Write, num::NonZeroU32, thread, time::Duration};

fn host_entry() -> ProcessEntry {
    ProcessEntry {
        role: ProcessRole::TerminalHost,
        product_version: ProductVersion::new("0.2.0").unwrap(),
        build: Build::default(),
        environment: EnvironmentName::new(orchbee_local_runtime::environment_stem()).unwrap(),
        pid: NonZeroU32::new(77).unwrap(),
        state: ProcessState::Draining,
        terminals: 2,
        connections: vec![ConnectionEntry {
            kind: ContractName::new(ConnectionKind::WorkerHost.name()).unwrap(),
            generations: ConnectionKind::WorkerHost.supported_generations(),
            capabilities: Some(CapabilitySet::default()),
        }],
    }
}

/// A Host that answers is relayed as itself without capabilities; one that stays silent is left
/// out by the deadline instead of holding the Worker.
#[test]
fn an_answering_host_is_relayed_and_a_silent_one_is_left_out_by_the_deadline() {
    let endpoint = crate::tests::endpoint();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let host = thread::spawn(move || {
        let (mut answering, _) = listener.accept().unwrap();
        let InitiatorMessage::Request(request) = read_initiator_message(&mut answering).unwrap()
        else {
            panic!("expected a control request");
        };
        assert_eq!(request.kind, RequestKind::Query);
        let result = QueryResult::new(vec![host_entry()], None, Extensions::new()).unwrap();
        answering
            .write_all(&encode_acceptor_frame(&AcceptorMessage::QueryResult(result)).unwrap())
            .unwrap();
        let (silent, _) = listener.accept().unwrap();
        thread::sleep(Duration::from_secs(1));
        drop(silent);
    });
    let mut expected = host_entry();
    expected.connections[0].capabilities = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(query(&endpoint, deadline).unwrap(), expected);
    let started = Instant::now();
    assert!(query(&endpoint, started + Duration::from_millis(200)).is_err());
    assert!(started.elapsed() < Duration::from_millis(900));
    host.join().unwrap();
}
