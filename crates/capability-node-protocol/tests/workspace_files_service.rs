// The Server and its Worker are separate executables that must agree on these bytes, so the
// representation is pinned by hand-derived golden bytes and its refusals by malformed frames.
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage as Message, OperationId, WorkerGeneration, WorkerId,
    WorkerRequestDeclined, WorkspaceFilesService, WorkspaceFilesServiceOutcome,
    WorkspaceFilesServiceRequest, WorkspaceFilesServiceResult, WorkspaceFilesTicket,
    decode_message, encode_message, read_message,
};
use std::{io::Cursor, num::NonZeroU64};

fn request(service: WorkspaceFilesService) -> WorkspaceFilesServiceRequest {
    WorkspaceFilesServiceRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(0x0304).unwrap()),
        service,
        ticket: WorkspaceFilesTicket::from_bytes([5; 16]).unwrap(),
    }
}

/// The payload after the 8-byte frame header.
fn payload(message: &Message) -> Vec<u8> {
    let frame = encode_message(message).unwrap();
    assert_eq!(decode_message(&frame).unwrap(), *message);
    assert_eq!(
        read_message(&mut Cursor::new(frame.clone())).unwrap(),
        *message
    );
    frame[8..].to_vec()
}

fn golden(kind: u8, service: u8, outcome: Option<u8>) -> Vec<u8> {
    let mut bytes = vec![kind];
    bytes.extend([1; 16]);
    bytes.extend([2; 16]);
    bytes.extend([0, 0, 0, 0, 0, 0, 3, 4]);
    bytes.push(service);
    bytes.extend([5; 16]);
    bytes.extend(outcome);
    bytes
}

#[test]
fn every_service_and_outcome_has_its_golden_bytes() {
    for (service, code) in [
        (WorkspaceFilesService::Operations, 1),
        (WorkspaceFilesService::Watch, 2),
        (WorkspaceFilesService::Search, 3),
    ] {
        let request = request(service);
        assert_eq!(
            payload(&Message::WorkspaceFilesServiceRequest(request)),
            golden(45, code, None)
        );
        for (outcome, tag) in [
            (WorkspaceFilesServiceOutcome::Started, 1),
            (WorkspaceFilesServiceOutcome::NotStarted, 2),
            (WorkspaceFilesServiceOutcome::WorkerUnavailable, 3),
        ] {
            let result = Message::WorkspaceFilesServiceResult(WorkspaceFilesServiceResult {
                request,
                outcome,
            });
            assert_eq!(payload(&result), golden(46, code, Some(tag)));
            assert_eq!(WorkerRequestDeclined::of(&result), None);
        }
    }
}

#[test]
fn malformed_requests_and_results_are_refused() {
    let frame = encode_message(&Message::WorkspaceFilesServiceResult(
        WorkspaceFilesServiceResult {
            request: request(WorkspaceFilesService::Watch),
            outcome: WorkspaceFilesServiceOutcome::Started,
        },
    ))
    .unwrap();
    // After the 8-byte header: the kind, the operation, Worker and generation, then the service
    // byte at 49, the ticket at 50..66 and the outcome at 66.
    type Corruption = fn(&mut Vec<u8>);
    let cases: [(&str, Corruption); 6] = [
        ("zero generation", |bad| bad[41..49].fill(0)),
        ("unknown service", |bad| bad[49] = 4),
        ("zero ticket", |bad| bad[50..66].fill(0)),
        ("unknown outcome", |bad| bad[66] = 0),
        ("short", |bad| {
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
