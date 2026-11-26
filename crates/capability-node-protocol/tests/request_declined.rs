use orchbee_capability_node_protocol::{
    AgentDrainRequest, CapabilityNodeMessage as Message, DeclinedRequestKind as Kind,
    HostQueryRequest, HostQueryResult, OperationId, WorkerGeneration, WorkerId,
    WorkerRequestDeclined, decode_message, encode_message, read_message,
};
use std::{io::Cursor, num::NonZeroU64};

const KINDS: [Kind; 21] = [
    Kind::Launch,
    Kind::Start,
    Kind::Viewer,
    Kind::Control,
    Kind::Lifecycle,
    Kind::CwdValidation,
    Kind::Send,
    Kind::WorktreeJob,
    Kind::AgentDrain,
    Kind::Shutdown,
    Kind::WorktreeMutation,
    Kind::Titles,
    Kind::Handoff,
    Kind::AgentIntegrationJob,
    Kind::AgentProbeJob,
    Kind::HostQuery,
    Kind::TitleChanges,
    Kind::PluginJob,
    Kind::GraphicsDecode,
    Kind::AdoptHost,
    Kind::WorkspaceFilesService,
];

#[test]
fn every_declined_kind_roundtrips_on_both_readers_and_a_request_names_its_own_identity() {
    for (index, kind) in KINDS.into_iter().enumerate() {
        let declined = Message::WorkerRequestDeclined(WorkerRequestDeclined {
            kind,
            identity: [u8::try_from(index).unwrap(); 16],
        });
        let frame = encode_message(&declined).unwrap();
        assert_eq!(decode_message(&frame).unwrap(), declined, "{kind:?}");
        assert_eq!(read_message(&mut Cursor::new(frame)).unwrap(), declined);
    }
    let query = HostQueryRequest {
        request_id: OperationId::from_bytes([5; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([6; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
    };
    assert_eq!(
        WorkerRequestDeclined::of(&Message::HostQueryRequest(query)),
        Some(WorkerRequestDeclined {
            kind: Kind::HostQuery,
            identity: [5; 16],
        })
    );
    let drain = AgentDrainRequest {
        worker_id: query.worker_id,
        worker_generation: query.worker_generation,
        token: NonZeroU64::new(0x0102).unwrap(),
        acknowledged: 0x0304,
    };
    let mut identity = [0; 16];
    identity[6..8].copy_from_slice(&[1, 2]);
    identity[14..].copy_from_slice(&[3, 4]);
    assert_eq!(
        WorkerRequestDeclined::of(&Message::AgentDrainRequest(drain)),
        Some(WorkerRequestDeclined {
            kind: Kind::AgentDrain,
            identity,
        })
    );
    // A Worker answers requests; nothing answers its own results.
    let result = Message::HostQueryResult(HostQueryResult {
        request: query,
        hosts: Vec::new(),
    });
    assert_eq!(WorkerRequestDeclined::of(&result), None);
}

#[test]
fn malformed_declines_are_refused() {
    let frame = encode_message(&Message::WorkerRequestDeclined(WorkerRequestDeclined {
        kind: Kind::Handoff,
        identity: [7; 16],
    }))
    .unwrap();
    // The header is 8 bytes; the kind 39, the declined kind and the identity follow.
    type Corruption = fn(&mut Vec<u8>);
    let cases: [(&str, Corruption); 4] = [
        ("unknown declined kind", |bad| bad[9] = 23),
        ("no declined kind", |bad| bad[9] = 0),
        ("short identity", |bad| {
            bad.pop();
            bad[7] -= 1;
        }),
        ("trailing byte", |bad| {
            bad.push(0);
            bad[7] += 1;
        }),
    ];
    for (case, corrupt) in cases {
        let mut bad = frame.clone();
        corrupt(&mut bad);
        assert!(decode_message(&bad).is_err(), "{case}");
        assert!(read_message(&mut Cursor::new(bad)).is_err(), "{case}");
    }
}

/// The answer to a request the Worker could not decode is the kind alone: it echoes nothing, and
/// as a message only a Worker sends it has no decline of its own.
#[test]
fn an_unreadable_answer_is_the_kind_alone() {
    let frame = encode_message(&Message::WorkerRequestUnreadable).unwrap();
    assert_eq!(frame[8..], [42]);
    assert_eq!(
        decode_message(&frame).unwrap(),
        Message::WorkerRequestUnreadable
    );
    assert_eq!(
        read_message(&mut Cursor::new(frame.clone())).unwrap(),
        Message::WorkerRequestUnreadable
    );
    assert_eq!(
        WorkerRequestDeclined::of(&Message::WorkerRequestUnreadable),
        None
    );
    let mut trailing = frame;
    trailing.push(0);
    trailing[7] += 1;
    assert!(decode_message(&trailing).is_err());
    assert!(read_message(&mut Cursor::new(trailing)).is_err());
}
