use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    collections::VecDeque,
    num::NonZeroU64,
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
    AttachmentId, CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HistoryAnchorStatus,
    HistoryChanged, HistoryControl, HostEpoch, MessageRole, PayloadEncoding,
    encode_history_control, encode_transport_envelope,
};

use super::*;
use crate::{worker_obcn_carrier::create_worker_carrier, worker_registry::WorkerRegistry};

const WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);

pub(super) fn viewer(
    service: &LocalTerminalService,
    handle: WorkerRequestHandle,
) -> ViewerAttachment<'_> {
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

pub(super) fn service() -> LocalTerminalService {
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
fn rows_record(
    request: &WorkerRequest,
    bytes: usize,
    reply_detail: Option<orchbee_capability_node_protocol::ViewerRowDetail>,
) -> Vec<u8> {
    let WorkerAction::ReadRows {
        detail,
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
    if reply_detail.is_some() {
        assert_eq!(
            detail,
            orchbee_capability_node_protocol::ViewerRowDetail::Render
        );
    }
    let mut payload = vec![0_u8; bytes];
    payload[..4].copy_from_slice(b"OBRW");
    payload[5] = 1;
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
    payload[100] = 0;
    if reply_detail.unwrap_or(detail) == orchbee_capability_node_protocol::ViewerRowDetail::Render {
        payload[6] = 1;
        payload[92..96].copy_from_slice(&(u32::from(row_count) * 80).to_le_bytes());
        payload[101..105].copy_from_slice(&((bytes - 152) as u32).to_le_bytes());
        payload[105..109].copy_from_slice(&40_u32.to_le_bytes());
    }
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
fn rows_peer(
    mut peer: UnixStream,
    payload_bytes: usize,
    sent: Sender<Vec<u8>>,
    reply_detail: Option<orchbee_capability_node_protocol::ViewerRowDetail>,
) {
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
                let record = rows_record(&request, payload_bytes, reply_detail);
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

fn rows_session(
    payload_bytes: usize,
    reply_detail: Option<orchbee_capability_node_protocol::ViewerRowDetail>,
) -> RowsSession {
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
    let peer = thread::spawn(move || rows_peer(peer, payload_bytes, sent, reply_detail));
    (service, handle, received, owner, peer, stop)
}

fn read_rows() -> Action {
    Action::ReadRows {
        detail: orchbee_client_server_protocol::ViewerRowDetail::Text,
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
    let (service, handle, received, owner, peer, stop) = rows_session(4096, None);
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

// A kind-41 snapshot record for the fixture viewer (attachment [2;16], Host
// epoch [3;16]) whose route sequence and revision both equal `sequence`.
fn frame_record(sequence: u64) -> Vec<u8> {
    let mut payload = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/terminal-protocol/fixtures/terminal-frame.bin"
    ))
    .to_vec();
    payload[6] = 1;
    payload[16..32].copy_from_slice(&[3; 16]);
    payload[32..48].copy_from_slice(&[2; 16]);
    payload[48..56].copy_from_slice(&sequence.to_le_bytes());
    payload[56..64].fill(255);
    payload[64..72].copy_from_slice(&sequence.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::from_bytes([3; 16]).unwrap()),
        kind: 41,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::TerminalFrame,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap()
}

fn history_record(attachment: [u8; 16], sequence: u64) -> Vec<u8> {
    let control = encode_history_control(&HistoryControl::HistoryChanged(HistoryChanged {
        anchor_status: HistoryAnchorStatus::Valid,
        attachment_id: AttachmentId::from_bytes(attachment).unwrap(),
        generation: NonZeroU64::MIN,
        revision: 5,
        sequence: NonZeroU64::new(sequence).unwrap(),
        total_rows: 100,
    }))
    .unwrap();
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::from_bytes([3; 16]).unwrap()),
        kind: control.kind,
        optional_message: false,
        payload: &control.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap()
}

// The scripted Worker answers every non-Detach request with the next scripted
// outcome, so a test can place a record on any action — including the wrong
// one — and observe the route the Server enforces.
fn scripted_peer(mut peer: UnixStream, mut script: VecDeque<WorkerOutcome>) {
    loop {
        let message = match read_message(&mut peer) {
            Ok(message) => message,
            Err(_) => return,
        };
        let CapabilityNodeMessage::TerminalViewerRequest(request) = message else {
            return;
        };
        let outcome = match request.action {
            WorkerAction::Detach => WorkerOutcome::Detached,
            _ => script.pop_front().unwrap_or(WorkerOutcome::Empty),
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

type ScriptedSession = (
    LocalTerminalService,
    WorkerRequestHandle,
    thread::JoinHandle<crate::worker_obcn_carrier::WorkerWireExit>,
    thread::JoinHandle<()>,
    UnixStream,
);

fn scripted_session(script: VecDeque<WorkerOutcome>) -> ScriptedSession {
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
    let peer = thread::spawn(move || scripted_peer(peer, script));
    (service, handle, owner, peer, stop)
}

fn poll() -> Action {
    Action::Poll {
        attachment_id: ViewerId::parse(&format!("{:032x}", u128::from_be_bytes([2; 16]))).unwrap(),
    }
}

fn execute_viewer(
    session: &mut LocalTerminalSession<'_>,
    id: &str,
    action: Action,
) -> TerminalViewerResult {
    let ServerMessage::TerminalViewerResult(result) =
        session.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            RequestId::parse(id).unwrap(),
            action,
        )))
    else {
        panic!("a viewer action must produce a viewer result")
    };
    result
}

#[test]
fn poll_delivers_a_history_event_while_a_frame_flight_is_pending() {
    let frame = frame_record(1);
    let history = history_record([2; 16], 1);
    let (service, handle, owner, peer, stop) = scripted_session(VecDeque::from([
        WorkerOutcome::Record(frame),
        WorkerOutcome::Record(history.clone()),
        WorkerOutcome::FeedbackAccepted,
    ]));
    let mut session = service.session();
    let candidate = viewer(&service, handle);
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    // The frame feeds the projection flight; the pending flight does not
    // short-circuit the next Poll into an Empty.
    assert!(matches!(
        execute_viewer(&mut session, "88888888888888888888888888888881", poll()).outcome(),
        Outcome::Record(_)
    ));
    let result = execute_viewer(&mut session, "88888888888888888888888888888882", poll());
    let Outcome::Record(record) = result.outcome() else {
        panic!("a queued history event must deliver during the frame flight")
    };
    assert_eq!(*record, history);
    // The flight was pending the whole time: its exact feedback still settles.
    let feedback = execute_viewer(
        &mut session,
        "88888888888888888888888888888883",
        Action::FrameApplied {
            attachment_id: poll().attachment_id().unwrap().clone(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
        },
    );
    assert_eq!(feedback.outcome(), &Outcome::FeedbackAccepted {});
    drop(session);
    let _ = stop.shutdown(std::net::Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

#[test]
fn a_history_record_on_any_action_but_poll_is_a_strict_route_violation() {
    let (service, handle, owner, peer, stop) = scripted_session(VecDeque::from([
        WorkerOutcome::Record(frame_record(1)),
        WorkerOutcome::Record(history_record([2; 16], 1)),
    ]));
    let mut session = service.session();
    let candidate = viewer(&service, handle);
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    execute_viewer(&mut session, "88888888888888888888888888888884", poll());
    // FrameApplied never admits a record: the kind-53 reply goes through the
    // frame-only projection, fails, and retires the viewer.
    let result = execute_viewer(
        &mut session,
        "88888888888888888888888888888885",
        Action::FrameApplied {
            attachment_id: poll().attachment_id().unwrap().clone(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
        },
    );
    assert_eq!(
        result.outcome(),
        &unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    assert!(session.viewers.iter().all(Option::is_none));
    drop(session);
    let _ = stop.shutdown(std::net::Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

#[test]
fn a_history_event_for_another_attachment_is_a_strict_route_violation() {
    let (service, handle, owner, peer, stop) =
        scripted_session(VecDeque::from([WorkerOutcome::Record(history_record(
            [9; 16], 1,
        ))]));
    let mut session = service.session();
    let candidate = viewer(&service, handle);
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    let result = execute_viewer(&mut session, "88888888888888888888888888888886", poll());
    assert_eq!(
        result.outcome(),
        &unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    assert!(session.viewers.iter().all(Option::is_none));
    drop(session);
    let _ = stop.shutdown(std::net::Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

#[test]
fn history_observed_acknowledgement_leaves_the_frame_flight_untouched() {
    let (service, handle, owner, peer, stop) = scripted_session(VecDeque::from([
        WorkerOutcome::Record(frame_record(1)),
        WorkerOutcome::FeedbackAccepted,
        WorkerOutcome::FeedbackAccepted,
    ]));
    let mut session = service.session();
    let candidate = viewer(&service, handle);
    assert!(matches!(
        session.publish_attach(candidate, attached_result()),
        Outcome::Attached { .. }
    ));
    execute_viewer(&mut session, "88888888888888888888888888888887", poll());
    let observed = execute_viewer(
        &mut session,
        "88888888888888888888888888888888",
        Action::HistoryObserved {
            attachment_id: poll().attachment_id().unwrap().clone(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 5,
        },
    );
    assert_eq!(observed.outcome(), &Outcome::FeedbackAccepted {});
    // HistoryObserved is its own flight: had it completed the frame
    // acknowledgement, this exact FrameApplied would now be FeedbackMismatch.
    let applied = execute_viewer(
        &mut session,
        "88888888888888888888888888888889",
        Action::FrameApplied {
            attachment_id: poll().attachment_id().unwrap().clone(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
        },
    );
    assert_eq!(applied.outcome(), &Outcome::FeedbackAccepted {});
    drop(session);
    let _ = stop.shutdown(std::net::Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

#[test]
fn read_rows_record_one_byte_over_budget_is_rejected_before_delivery() {
    let (service, handle, _received, owner, peer, stop) = rows_session(4096 + 1, None);
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

mod clear;
mod effects;
mod fixture;
mod process_info;
mod selection;
mod stateful;

#[test]
fn render_row_read_forwards_detail_and_rejects_a_text_reply() {
    use orchbee_capability_node_protocol::ViewerRowDetail as WorkerDetail;
    use orchbee_client_server_protocol::ViewerRowDetail;
    for reply_detail in [WorkerDetail::Render, WorkerDetail::Text] {
        let (service, handle, received, owner, peer, stop) = rows_session(4096, Some(reply_detail));
        let mut session = service.session();
        assert!(matches!(
            session.publish_attach(viewer(&service, handle), attached_result()),
            Outcome::Attached { .. }
        ));
        let mut action = read_rows();
        let Action::ReadRows { detail, .. } = &mut action else {
            unreachable!()
        };
        *detail = ViewerRowDetail::Render;
        let ServerMessage::TerminalViewerResult(result) =
            session.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
                RequestId::parse("88888888888888888888888888888888").unwrap(),
                action,
            )))
        else {
            panic!("viewer result")
        };
        assert_eq!(
            matches!(result.outcome(), Outcome::Record(_)),
            reply_detail == WorkerDetail::Render
        );
        received.recv().unwrap();
        drop(session);
        let _ = stop.shutdown(std::net::Shutdown::Both);
        let _ = owner.join();
        peer.join().unwrap();
    }
}

#[test]
fn explicit_takeover_transfers_cleanup_and_old_session_cannot_dispatch_effects() {
    use orchbee_capability_node_protocol::{
        TerminalControlAction as WAction, TerminalControlOutcome as WOutcome, TerminalControlResult,
    };
    use orchbee_client_server_protocol::{
        ControlInput, ControlRejectedReason, TerminalControlAction as CAction,
        TerminalControlOutcome as COutcome, TerminalControlRequest,
    };
    for refuse in [false, true] {
        let service = service();
        let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
        let handle = carrier.request_handle();
        assert!(matches!(
            service.workers.lock().unwrap().admit(WORKER_ID),
            crate::worker_registry::WorkerAdmission::Accepted { .. }
        ));
        let owner =
            WorkerRegistry::register_carrier(&service.workers, NonZeroU64::MIN, carrier).unwrap();
        let (server, mut peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        thread::scope(|scope| {
            let server_thread = scope.spawn(move || owner.run(server));
            let worker=scope.spawn(move || {
                let mut incumbent=None;
                let mut replacement=None;
                for step in 0..if refuse {3} else {4} {
                    let CapabilityNodeMessage::TerminalControlRequest(request)=read_message(&mut peer).unwrap() else {panic!("expected control")};
                    let outcome=match step {
                        0=>{
                            assert!(matches!(request.action,WAction::Acquire));
                            incumbent=Some(request.controller_id);
                            WOutcome::Acquired
                        },
                        1=>{
                            assert_eq!(request.action,WAction::Takeover {previous_controller_id:incumbent.unwrap()});
                            assert_ne!(Some(request.controller_id),incumbent);
                            replacement=Some(request.controller_id);
                            if refuse {WOutcome::Rejected(orchbee_capability_node_protocol::ControlRejection::HostRejected)} else {WOutcome::Acquired}
                        },
                        2 if !refuse => {
                            assert_eq!(request.action,WAction::Takeover {previous_controller_id:replacement.unwrap()});
                            assert_ne!(Some(request.controller_id),replacement);
                            incumbent=Some(request.controller_id);
                            WOutcome::Acquired
                        },
                        _=>{
                            assert!(matches!(request.action,WAction::Release));
                            assert_eq!(Some(request.controller_id),incumbent);
                            WOutcome::Released
                        },
                    };
                    write_message(&mut peer,&CapabilityNodeMessage::TerminalControlResult(TerminalControlResult {request_id:request.request_id,viewer_id:request.viewer_id,controller_id:request.controller_id,outcome})).unwrap();
                }
            });
            let mut old = viewer(&service, handle.clone());
            let mut new = viewer(&service, handle.clone());
            let new_viewer = OperationId::from_bytes([20; 16]).unwrap();
            new.attachment_id = ViewerId::parse(&format!(
                "{:032x}",
                u128::from_be_bytes(new_viewer.as_bytes())
            ))
            .unwrap();
            new.cleanup.as_mut().unwrap().attachment_id = new_viewer;
            fn run<'a>(
                service: &'a LocalTerminalService,
                v: &mut ViewerAttachment<'a>,
                action: CAction,
            ) -> COutcome {
                super::super::controller_session::execute(
                    service,
                    Some(v),
                    &TerminalControlRequest::new(
                        RequestId::parse("99999999999999999999999999999999").unwrap(),
                        action,
                    ),
                )
            }
            let old_action = CAction::Acquire {
                attachment_id: old.attachment_id.clone(),
            };
            let COutcome::Acquired { controller_id } = run(&service, &mut old, old_action) else {
                panic!("old acquire")
            };
            let new_action = CAction::Takeover {
                attachment_id: new.attachment_id.clone(),
            };
            let result = run(&service, &mut new, new_action);
            if refuse {
                assert_eq!(
                    result,
                    COutcome::Rejected {
                        reason: ControlRejectedReason::HostRejected
                    }
                );
            } else {
                assert!(matches!(result, COutcome::Acquired { .. }));
            }
            let input = CAction::Input {
                attachment_id: old.attachment_id.clone(),
                controller_id,
                sequence: NonZeroU64::MIN,
                bytes: ControlInput::new(b"old".to_vec()).unwrap(),
            };
            assert_eq!(
                run(&service, &mut old, input),
                COutcome::Rejected {
                    reason: ControlRejectedReason::NotController
                }
            );
            if !refuse {
                let take_back = CAction::Takeover {
                    attachment_id: old.attachment_id.clone(),
                };
                let retaken = run(&service, &mut old, take_back.clone());
                assert!(
                    matches!(retaken, COutcome::Acquired { .. }),
                    "same old Viewer must retake: {retaken:?}"
                );
                // No Worker request: exact active self ownership is already proven.
                assert_eq!(run(&service, &mut old, take_back), retaken);
            }
            // These manually seeded viewers have no Host viewing attachment; only controller cleanup is exercised.
            old.cleanup = None;
            new.cleanup = None;
            drop(old);
            drop(new);
            assert!(service.controllers.lock().unwrap().is_empty());
            worker.join().unwrap();
            let _exit = server_thread.join().unwrap();
        });
    }
}

#[cfg(windows)]
mod pane_graphics;
#[cfg(windows)]
mod plugin_overlays;
#[cfg(windows)]
mod process_sampling;

#[cfg(windows)]
mod editor;
#[cfg(windows)]
mod plugin_popup;
