use orchbee_capability_node_protocol::{
    CapabilityNodeMessage as Message, HostQueryRequest, HostQueryResult, MAXIMUM_QUERY_HOSTS,
    OperationId, WorkerGeneration, WorkerId, decode_message, encode_message, read_message,
};
use orchbee_connection_handshake::{
    Build, CapabilitySet, CohortIdentity, ConnectionEntry, ContractName, EnvironmentName,
    Generation, GenerationRange, MAXIMUM_PROCESS_CONNECTIONS, ProcessEntry, ProcessRole,
    ProcessState, ProductVersion, SourceCommit,
};
use std::{
    io::Cursor,
    num::{NonZeroU32, NonZeroU64},
};

fn request() -> HostQueryRequest {
    HostQueryRequest {
        request_id: OperationId::from_bytes([5; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([6; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MAX),
    }
}

fn connection(kind: &str, lowest: u16, highest: u16) -> ConnectionEntry {
    ConnectionEntry {
        kind: ContractName::new(kind).unwrap(),
        generations: GenerationRange::new(
            Generation::new(lowest).unwrap(),
            Generation::new(highest).unwrap(),
        )
        .unwrap(),
        capabilities: None,
    }
}

/// A Host entry with every member at its largest.
fn largest_host(pid: u32) -> ProcessEntry {
    ProcessEntry {
        role: ProcessRole::TerminalHost,
        product_version: ProductVersion::new("~".repeat(64)).unwrap(),
        build: Build {
            commit: Some(SourceCommit::from_bytes([0xab; 20])),
            cohort: Some(CohortIdentity::from_bytes([0xcd; 32])),
        },
        environment: EnvironmentName::new("z".repeat(64)).unwrap(),
        pid: NonZeroU32::new(pid).unwrap(),
        state: ProcessState::Draining,
        terminals: u32::MAX,
        connections: (0..MAXIMUM_PROCESS_CONNECTIONS)
            .map(|index| connection(&format!("{index:0>64}"), 1, u16::MAX))
            .collect(),
    }
}

fn small_host(state: ProcessState) -> ProcessEntry {
    ProcessEntry {
        role: ProcessRole::TerminalHost,
        product_version: ProductVersion::new("0.1.0").unwrap(),
        build: Build::default(),
        environment: EnvironmentName::new("ob-test-a1").unwrap(),
        pid: NonZeroU32::MIN,
        state,
        terminals: 0,
        connections: vec![connection("worker-host", 1, 2)],
    }
}

fn roundtrip(message: &Message) {
    let frame = encode_message(message).unwrap();
    assert_eq!(&decode_message(&frame).unwrap(), message);
    assert_eq!(&read_message(&mut Cursor::new(frame)).unwrap(), message);
}

#[test]
fn host_query_messages_roundtrip_up_to_256_largest_entries_on_both_readers() {
    roundtrip(&Message::HostQueryRequest(request()));
    roundtrip(&Message::HostQueryResult(HostQueryResult {
        request: request(),
        hosts: Vec::new(),
    }));
    for state in [
        ProcessState::Ready,
        ProcessState::Draining,
        ProcessState::Stopping,
        ProcessState::Unknown,
    ] {
        roundtrip(&Message::HostQueryResult(HostQueryResult {
            request: request(),
            hosts: vec![small_host(state)],
        }));
    }
    let largest = Message::HostQueryResult(HostQueryResult {
        request: request(),
        hosts: (1..=256).map(largest_host).collect(),
    });
    roundtrip(&largest);
    assert_eq!(MAXIMUM_QUERY_HOSTS, 256);
}

#[test]
fn host_query_results_refuse_entries_the_server_cannot_list() {
    let refused = |hosts: Vec<ProcessEntry>| {
        let result = HostQueryResult {
            request: request(),
            hosts,
        };
        assert!(result.validate().is_err());
        assert!(encode_message(&Message::HostQueryResult(result)).is_err());
    };
    refused(
        (1..=257)
            .map(|pid| ProcessEntry {
                pid: NonZeroU32::new(pid).unwrap(),
                ..small_host(ProcessState::Ready)
            })
            .collect(),
    );
    let mut capabilities = small_host(ProcessState::Ready);
    capabilities.connections[0].capabilities = Some(CapabilitySet::default());
    refused(vec![capabilities]);
    let mut unordered = small_host(ProcessState::Ready);
    unordered
        .connections
        .insert(0, connection("worker-hosts", 1, 1));
    refused(vec![unordered]);
    let mut repeated = small_host(ProcessState::Ready);
    repeated.connections.push(connection("worker-host", 1, 1));
    refused(vec![repeated]);
    let mut crowded = largest_host(1);
    crowded
        .connections
        .push(connection(&"9".repeat(65)[1..], 1, 1));
    refused(vec![crowded]);
    for role in [
        ProcessRole::Server,
        ProcessRole::Worker,
        ProcessRole::Unknown,
    ] {
        let mut other = small_host(ProcessState::Ready);
        other.role = role;
        refused(vec![other]);
    }
}

#[test]
fn malformed_host_query_records_are_refused() {
    let mut request_frame = encode_message(&Message::HostQueryRequest(request())).unwrap();
    request_frame[7] += 1;
    request_frame.push(0);
    assert!(decode_message(&request_frame).is_err());
    let record = encode_message(&Message::HostQueryResult(HostQueryResult {
        request: request(),
        hosts: vec![small_host(ProcessState::Ready)],
    }))
    .unwrap();
    // The prefix is 12 bytes, the echoed request 40 and the count 2; the one entry follows.
    const ENTRY: usize = 54;
    type Corruption = fn(&mut Vec<u8>);
    let cases: [(&str, Corruption); 11] = [
        ("record version", |bad| bad[5] = 2),
        ("length beyond the bound", |bad| bad[8] = 0xff),
        ("zero Worker generation", |bad| bad[44..52].fill(0)),
        ("count past the entries", |bad| bad[53] = 2),
        ("zero PID", |bad| bad[ENTRY + 3] = 0),
        ("unknown state", |bad| bad[ENTRY + 4] = 4),
        ("empty version", |bad| bad[ENTRY + 9] = 0),
        ("uppercase environment", |bad| bad[ENTRY + 16] = b'A'),
        ("unknown commit tag", |bad| bad[ENTRY + 26] = 2),
        ("zero generation", |bad| bad[ENTRY + 42] = 0),
        ("inverted generations", |bad| bad[ENTRY + 42] = 3),
    ];
    for (case, corrupt) in cases {
        let mut bad = record.clone();
        corrupt(&mut bad);
        assert!(decode_message(&bad).is_err(), "{case}");
        assert!(read_message(&mut Cursor::new(bad)).is_err(), "{case}");
    }
    let mut trailing = record.clone();
    trailing.push(0);
    assert!(decode_message(&trailing).is_err());
    let mut short = record;
    short.pop();
    assert!(decode_message(&short).is_err());
    assert!(read_message(&mut Cursor::new(short)).is_err());
}
