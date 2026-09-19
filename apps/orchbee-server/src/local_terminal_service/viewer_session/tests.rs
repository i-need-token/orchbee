use std::{
    num::NonZeroU64,
    os::unix::net::UnixStream,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    thread,
};

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, OperationId, TerminalViewerOutcome as WorkerOutcome,
    TerminalViewerResult as WorkerResult, WorkerGeneration, WorkerId as CapabilityWorkerId,
    read_message, write_message,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HostEpoch, MessageRole,
    PayloadEncoding, encode_transport_envelope,
};

use super::*;
use crate::{worker_obcn_carrier::create_worker_carrier, worker_registry::WorkerRegistry};

const WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);

fn viewer(service: &LocalTerminalService, handle: WorkerRequestHandle) -> ViewerAttachment<'_> {
    let attachment_id = OperationId::from_bytes([2; 16]).unwrap();
    let worker_id = CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes()).unwrap();
    ViewerAttachment {
        attachment_id: ViewerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(attachment_id.as_bytes())
        ))
        .unwrap(),
        service,
        handle,
        target: BindingViewerTarget {
            terminal_id: orchbee_capability_node_protocol::TerminalId::from_bytes([1; 16]).unwrap(),
            worker: crate::terminal_coordinator::WorkerPlacement {
                worker_id,
                generation: WorkerGeneration::new(NonZeroU64::MIN),
            },
            identity: orchbee_capability_node_protocol::HostTerminalIdentity::new(
                orchbee_capability_node_protocol::HostCreationId::from_bytes([1; 16]).unwrap(),
                orchbee_capability_node_protocol::HostEpoch::from_bytes([3; 16]).unwrap(),
                orchbee_capability_node_protocol::HostTerminalId::from_bytes([4; 16]).unwrap(),
            ),
        },
        controller: None,
        cleanup: Some(WorkerRequest {
            request_id: OperationId::from_bytes([5; 16]).unwrap(),
            worker_id,
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            attachment_id,
            action: WorkerAction::Detach,
        }),
        projection: ViewerProjection::new(
            AttachmentId::from_bytes(attachment_id.as_bytes()).unwrap(),
            HostEpoch::from_bytes([3; 16]).unwrap(),
        ),
    }
}

fn attached_result() -> Result<WorkerOperationResult, WorkerRequestError> {
    Ok(WorkerOperationResult::Viewer(WorkerResult {
        request_id: OperationId::from_bytes([6; 16]).unwrap(),
        attachment_id: OperationId::from_bytes([2; 16]).unwrap(),
        outcome: WorkerOutcome::Attached,
    }))
}

fn service() -> LocalTerminalService {
    LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Err(std::io::ErrorKind::PermissionDenied.into()),
    )
}

#[test]
fn worker_loss_before_post_ipc_current_check_prevents_attachment_publication() {
    let service = service();
    let mut session = service.session();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    let candidate = viewer(&service, handle.clone());
    drop(carrier);

    assert!(!handle.is_active());
    assert_eq!(
        session.publish_attach(candidate, attached_result()),
        unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    assert!(session.viewers.iter().all(Option::is_none));
}

#[test]
fn worker_loss_after_attachment_installation_is_observed_on_next_poll() {
    let service = service();
    let mut session = service.session();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    let candidate = viewer(&service, handle.clone());
    assert!(matches!(
        service.workers.lock().unwrap().admit(WORKER_ID),
        crate::worker_registry::WorkerAdmission::Accepted { .. }
    ));
    let owner =
        WorkerRegistry::register_carrier(&service.workers, NonZeroU64::MIN, carrier).unwrap();
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    assert!(session.viewers.iter().any(Option::is_some));
    drop(owner);
    assert!(!handle.is_active());

    let poll_id = RequestId::parse("77777777777777777777777777777777").unwrap();
    let ServerMessage::TerminalViewerResult(result) =
        session.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            poll_id.clone(),
            Action::Poll {
                attachment_id: ViewerId::parse(&format!("{:032x}", u128::from_be_bytes([2; 16])))
                    .unwrap(),
            },
        )))
    else {
        panic!("Poll must produce a viewer result")
    };
    assert_eq!(result.request_id(), &poll_id);
    assert_eq!(
        result.outcome(),
        &unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    assert!(session.viewers.iter().all(Option::is_none));
}

// A self-consistent OBRW text-detail window of exactly `bytes` length that
// echoes every routing field the read request pinned, wrapped in a kind-46
// RowWindow record correlated to the Server-issued operation.
fn rows_record(request: &WorkerRequest, bytes: usize) -> Vec<u8> {
    let WorkerAction::ReadRows {
        start_row,
        row_count,
        revision,
        generation,
        request_sequence,
        ..
    } = request.action
    else {
        panic!("a row record requires a ReadRows request")
    };
    let mut payload = vec![0_u8; bytes];
    payload[..4].copy_from_slice(b"OBRW");
    payload[6] = 2;
    payload[8..12].copy_from_slice(&(bytes as u32).to_le_bytes());
    payload[12..14].copy_from_slice(&112_u16.to_le_bytes());
    payload[14..16].copy_from_slice(&80_u16.to_le_bytes());
    payload[16..32].copy_from_slice(&[3; 16]);
    payload[32..48].copy_from_slice(&request.attachment_id.as_bytes());
    payload[48..56].copy_from_slice(&request_sequence.get().to_le_bytes());
    payload[56..64].copy_from_slice(&revision.to_le_bytes());
    payload[64..72].copy_from_slice(&generation.get().to_le_bytes());
    payload[72..80].copy_from_slice(&(start_row + u64::from(row_count)).to_le_bytes());
    payload[80..88].copy_from_slice(&start_row.to_le_bytes());
    payload[88..90].copy_from_slice(&row_count.to_le_bytes());
    payload[96..100].copy_from_slice(&((bytes - 112) as u32).to_le_bytes());
    payload[100] = 3;
    encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes(request.request_id.as_bytes()).unwrap()),
        host_epoch: Some(HostEpoch::from_bytes([3; 16]).unwrap()),
        kind: 46,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::RowWindow,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    })
    .unwrap()
}

// The scripted Worker answers every ReadRows with a valid kind-46 record whose
// OBRW payload is exactly `payload_bytes`, reporting the record it sent.
fn rows_peer(mut peer: UnixStream, payload_bytes: usize, sent: Sender<Vec<u8>>) {
    loop {
        let message = match read_message(&mut peer) {
            Ok(message) => message,
            Err(_) => return,
        };
        let CapabilityNodeMessage::TerminalViewerRequest(request) = message else {
            return;
        };
        let outcome = match request.action {
            WorkerAction::ReadRows { .. } => {
                let record = rows_record(&request, payload_bytes);
                let _ = sent.send(record.clone());
                WorkerOutcome::Record(record)
            }
            WorkerAction::Detach => WorkerOutcome::Detached,
            _ => WorkerOutcome::Empty,
        };
        let reply = CapabilityNodeMessage::TerminalViewerResult(WorkerResult {
            request_id: request.request_id,
            attachment_id: request.attachment_id,
            outcome,
        });
        if write_message(&mut peer, &reply).is_err() {
            return;
        }
    }
}

type RowsSession = (
    LocalTerminalService,
    WorkerRequestHandle,
    Receiver<Vec<u8>>,
    thread::JoinHandle<crate::worker_obcn_carrier::WorkerWireExit>,
    thread::JoinHandle<()>,
    UnixStream,
);

fn rows_session(payload_bytes: usize) -> RowsSession {
    let service = service();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    assert!(matches!(
        service.workers.lock().unwrap().admit(WORKER_ID),
        crate::worker_registry::WorkerAdmission::Accepted { .. }
    ));
    let owner =
        WorkerRegistry::register_carrier(&service.workers, NonZeroU64::MIN, carrier).unwrap();
    let (server, peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let stop = server.try_clone().unwrap();
    let owner = thread::spawn(move || owner.run(server));
    let (sent, received) = channel();
    let peer = thread::spawn(move || rows_peer(peer, payload_bytes, sent));
    (service, handle, received, owner, peer, stop)
}

fn read_rows() -> Action {
    Action::ReadRows {
        attachment_id: ViewerId::parse(&format!("{:032x}", u128::from_be_bytes([2; 16]))).unwrap(),
        start_row: 2,
        row_count: 4,
        revision: 9,
        generation: NonZeroU64::MIN,
        request_sequence: NonZeroU64::MIN,
        maximum_bytes: 4096,
    }
}

#[test]
fn read_rows_record_at_the_requested_budget_is_delivered() {
    let (service, handle, received, owner, peer, stop) = rows_session(4096);
    let mut session = service.session();
    let candidate = viewer(&service, handle);
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    let ServerMessage::TerminalViewerResult(result) =
        session.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            RequestId::parse("88888888888888888888888888888888").unwrap(),
            read_rows(),
        )))
    else {
        panic!("ReadRows must produce a viewer result")
    };
    let Outcome::Record(record) = result.outcome() else {
        panic!("an exact-budget row record must reach the client")
    };
    assert_eq!(*record, received.recv().unwrap());
    drop(session);
    let _ = stop.shutdown(std::net::Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

#[test]
fn read_rows_record_one_byte_over_budget_is_rejected_before_delivery() {
    let (service, handle, _received, owner, peer, stop) = rows_session(4096 + 1);
    let mut session = service.session();
    let candidate = viewer(&service, handle);
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    let ServerMessage::TerminalViewerResult(result) =
        session.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            RequestId::parse("88888888888888888888888888888888").unwrap(),
            read_rows(),
        )))
    else {
        panic!("ReadRows must produce a viewer result")
    };
    assert_eq!(
        result.outcome(),
        &unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    // The oversized record was never accepted: the viewer is gone rather than
    // holding a delivered record or a fed projection.
    assert!(session.viewers.iter().all(Option::is_none));
    drop(session);
    let _ = stop.shutdown(std::net::Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

mod fixture;
mod stateful;
