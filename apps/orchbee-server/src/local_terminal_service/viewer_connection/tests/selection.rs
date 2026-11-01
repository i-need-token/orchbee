use super::*;
use orchbee_capability_node_protocol::{
    TerminalViewerAction as WorkerAction, TerminalViewerOutcome as WorkerOutcome,
    TerminalViewerResult as WorkerResult, ViewerSelectionCommand as WorkerCommand,
    ViewerSelectionExportCancelReason as WorkerCancelReason,
    ViewerSelectionExportFormat as WorkerExportFormat,
    ViewerSelectionGestureAction as WorkerGesture,
    ViewerSelectionGestureGeometry as WorkerGeometry,
    ViewerSelectionGesturePosition as WorkerPosition, ViewerSelectionPoint as WorkerAbsolutePoint,
    ViewerSelectionViewportPoint as WorkerPoint,
};
use orchbee_client_server_protocol::{
    TerminalViewerAction as Action, TerminalViewerOutcome as Outcome,
    ViewerSelectionCommand as Command, ViewerSelectionExportCancelReason as CancelReason,
    ViewerSelectionExportFormat as ExportFormat, ViewerSelectionGestureAction as Gesture,
    ViewerSelectionGestureGeometry as Geometry, ViewerSelectionGesturePosition as Position,
    ViewerSelectionPoint as AbsolutePoint, ViewerSelectionViewportPoint as Point,
};
use orchbee_terminal_protocol::{
    ConnectionControl, SelectionChanged, SelectionChangedState, SelectionControl,
    SelectionExportChunk, SelectionExportControl, SelectionExportFinished, SelectionExportFormat,
    SelectionExportStarted, SelectionExportStatus, TypedFailure, decode_transport_envelope,
    encode_connection_control, encode_selection_control, encode_selection_export_chunk,
    encode_selection_export_control, encode_transport_envelope,
};
use std::{
    collections::VecDeque,
    net::Shutdown,
    sync::mpsc::{Sender, channel},
    thread,
    time::Duration,
};

fn viewer_id(byte: u8) -> ViewerId {
    ViewerId::parse(&format!("{:032x}", u128::from_be_bytes([byte; 16]))).unwrap()
}

fn selection_actions() -> [Action; 11] {
    [
        Action::SelectionGesture {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            gesture: Gesture::Start {
                click_count: 2,
                point: Point { column: 3, row: 4 },
                position: Position { x: -5, y: 6 },
                select_output: true,
            },
        },
        Action::SelectionGesture {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            gesture: Gesture::Update {
                geometry: Geometry {
                    cell_width: 8,
                    columns: 80,
                    padding_left: 2,
                    screen_height: 480,
                },
                point: Point { column: 7, row: 8 },
                position: Position { x: 9, y: -10 },
                rectangle: false,
            },
        },
        Action::SelectionGesture {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            gesture: Gesture::End {
                point: Point {
                    column: 11,
                    row: 12,
                },
            },
        },
        Action::SelectionGesture {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            gesture: Gesture::Cancel,
        },
        Action::SelectionExport {
            attachment_id: viewer_id(2),
            export_sequence: NonZeroU64::new(10).unwrap(),
            format: ExportFormat::Plain,
        },
        Action::SelectionExportChunkApplied {
            attachment_id: viewer_id(2),
            export_sequence: NonZeroU64::new(10).unwrap(),
            chunk_sequence: 42,
        },
        Action::SelectionExportCancel {
            attachment_id: viewer_id(2),
            export_sequence: NonZeroU64::new(10).unwrap(),
            reason: CancelReason::ClipboardFailed,
        },
        Action::SelectionCommand {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: Command::Clear,
        },
        Action::SelectionCommand {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: Command::SelectAll,
        },
        Action::SelectionCommand {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: Command::SetRange {
                end: AbsolutePoint { column: 3, row: 4 },
                rectangle: true,
                start: AbsolutePoint { column: 1, row: 2 },
            },
        },
        Action::SelectionCommand {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: Command::SelectOutput {
                point: AbsolutePoint { column: 5, row: 6 },
            },
        },
    ]
}

fn worker_actions() -> [WorkerAction; 11] {
    [
        WorkerAction::SelectionGesture {
            generation: NonZeroU64::new(9).unwrap(),
            gesture: WorkerGesture::Start {
                click_count: 2,
                point: WorkerPoint { column: 3, row: 4 },
                position: WorkerPosition { x: -5, y: 6 },
                select_output: true,
            },
        },
        WorkerAction::SelectionGesture {
            generation: NonZeroU64::new(9).unwrap(),
            gesture: WorkerGesture::Update {
                geometry: WorkerGeometry {
                    cell_width: 8,
                    columns: 80,
                    padding_left: 2,
                    screen_height: 480,
                },
                point: WorkerPoint { column: 7, row: 8 },
                position: WorkerPosition { x: 9, y: -10 },
                rectangle: false,
            },
        },
        WorkerAction::SelectionGesture {
            generation: NonZeroU64::new(9).unwrap(),
            gesture: WorkerGesture::End {
                point: WorkerPoint {
                    column: 11,
                    row: 12,
                },
            },
        },
        WorkerAction::SelectionGesture {
            generation: NonZeroU64::new(9).unwrap(),
            gesture: WorkerGesture::Cancel,
        },
        WorkerAction::SelectionExport {
            export_sequence: NonZeroU64::new(10).unwrap(),
            format: WorkerExportFormat::Plain,
        },
        WorkerAction::SelectionExportChunkApplied {
            export_sequence: NonZeroU64::new(10).unwrap(),
            chunk_sequence: 42,
        },
        WorkerAction::SelectionExportCancel {
            export_sequence: NonZeroU64::new(10).unwrap(),
            reason: WorkerCancelReason::ClipboardFailed,
        },
        WorkerAction::SelectionCommand {
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: WorkerCommand::Clear,
        },
        WorkerAction::SelectionCommand {
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: WorkerCommand::SelectAll,
        },
        WorkerAction::SelectionCommand {
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: WorkerCommand::SetRange {
                end: WorkerAbsolutePoint { column: 3, row: 4 },
                rectangle: true,
                start: WorkerAbsolutePoint { column: 1, row: 2 },
            },
        },
        WorkerAction::SelectionCommand {
            generation: NonZeroU64::new(9).unwrap(),
            revision: 7,
            command: WorkerCommand::SelectOutput {
                point: WorkerAbsolutePoint { column: 5, row: 6 },
            },
        },
    ]
}

fn response_record(
    request: &orchbee_capability_node_protocol::TerminalViewerRequest,
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Vec<u8> {
    encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes(request.request_id.as_bytes()).unwrap()),
        host_epoch: Some(HostEpoch::from_bytes([3; 16]).unwrap()),
        kind,
        optional_message: false,
        payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role,
    })
    .unwrap()
}

fn selection_action_peer(
    mut peer: UnixStream,
    records: Sender<Vec<u8>>,
    detach_replied: Sender<()>,
) {
    let attachment_id = AttachmentId::from_bytes([2; 16]).unwrap();
    for (index, expected) in worker_actions().into_iter().enumerate() {
        let CapabilityNodeMessage::TerminalViewerRequest(request) =
            read_message(&mut peer).unwrap()
        else {
            panic!("expected viewer request")
        };
        assert_eq!(request.attachment_id.as_bytes(), [2; 16]);
        assert_eq!(request.action, expected);
        let outcome = match index {
            0..=3 | 7..=10 => {
                let encoded = encode_selection_control(&SelectionControl::ChangedResponse(
                    SelectionChanged {
                        attachment_id,
                        state: SelectionChangedState::None,
                    },
                ))
                .unwrap();
                let record =
                    response_record(&request, encoded.kind, encoded.role, &encoded.payload);
                records.send(record.clone()).unwrap();
                WorkerOutcome::Record(record)
            }
            4 => {
                let encoded = encode_selection_export_control(
                    &SelectionExportControl::StartedResponse(SelectionExportStarted {
                        attachment_id,
                        byte_length: 4,
                        export_sequence: NonZeroU64::new(10).unwrap(),
                        format: SelectionExportFormat::Plain,
                    }),
                )
                .unwrap();
                let record =
                    response_record(&request, encoded.kind, encoded.role, &encoded.payload);
                records.send(record.clone()).unwrap();
                WorkerOutcome::Record(record)
            }
            5 => WorkerOutcome::FeedbackAccepted,
            6 => {
                let encoded = encode_selection_export_control(
                    &SelectionExportControl::FinishedResponse(SelectionExportFinished {
                        attachment_id,
                        byte_length: 4,
                        export_sequence: NonZeroU64::new(10).unwrap(),
                        status: SelectionExportStatus::Cancelled,
                    }),
                )
                .unwrap();
                let record =
                    response_record(&request, encoded.kind, encoded.role, &encoded.payload);
                records.send(record.clone()).unwrap();
                WorkerOutcome::Record(record)
            }
            _ => unreachable!(),
        };
        write_message(
            &mut peer,
            &CapabilityNodeMessage::TerminalViewerResult(WorkerResult {
                request_id: request.request_id,
                attachment_id: request.attachment_id,
                outcome,
            }),
        )
        .unwrap();
    }

    // The Client names the admitted viewer, but a data-bearing Host record
    // that names another attachment must still retire this viewer.
    let CapabilityNodeMessage::TerminalViewerRequest(request) = read_message(&mut peer).unwrap()
    else {
        panic!("expected wrong-route viewer request")
    };
    assert_eq!(
        request.action,
        WorkerAction::SelectionGesture {
            generation: NonZeroU64::new(9).unwrap(),
            gesture: WorkerGesture::Cancel,
        }
    );
    let encoded = encode_selection_control(&SelectionControl::ChangedResponse(SelectionChanged {
        attachment_id: AttachmentId::from_bytes([9; 16]).unwrap(),
        state: SelectionChangedState::None,
    }))
    .unwrap();
    let record = response_record(&request, encoded.kind, encoded.role, &encoded.payload);
    write_message(
        &mut peer,
        &CapabilityNodeMessage::TerminalViewerResult(WorkerResult {
            request_id: request.request_id,
            attachment_id: request.attachment_id,
            outcome: WorkerOutcome::Record(record),
        }),
    )
    .unwrap();

    let CapabilityNodeMessage::TerminalViewerRequest(request) = read_message(&mut peer).unwrap()
    else {
        panic!("expected exact cleanup detach")
    };
    assert_eq!(request.action, WorkerAction::Detach);
    write_message(
        &mut peer,
        &CapabilityNodeMessage::TerminalViewerResult(WorkerResult {
            request_id: request.request_id,
            attachment_id: request.attachment_id,
            outcome: WorkerOutcome::Detached,
        }),
    )
    .unwrap();
    detach_replied.send(()).unwrap();
    assert!(
        read_message(&mut peer).is_err(),
        "selection violation must enqueue exactly one cleanup detach"
    );
}

#[test]
fn selection_actions_map_to_the_exact_viewer_and_preserve_correlated_outcomes() {
    let service = service();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    assert!(matches!(
        service.workers.lock().unwrap().admit(WORKER_ID, None),
        crate::worker_registry::WorkerAdmission::Accepted { .. }
    ));
    let owner =
        WorkerRegistry::register_carrier(&service.workers, NonZeroU64::MIN, carrier).unwrap();
    let (server, worker) = UnixStream::pair().unwrap();
    worker
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let stop = server.try_clone().unwrap();
    let owner = thread::spawn(move || owner.run(server));
    let (sent, records) = channel();
    let (detach_replied, detach_reply) = channel();
    let peer = thread::spawn(move || selection_action_peer(worker, sent, detach_replied));
    let mut connection = service.client_connection();
    assert!(matches!(
        connection.publish_attach(viewer(&service, handle), attached_result()),
        Outcome::Attached { .. }
    ));

    let wrong = Action::SelectionGesture {
        attachment_id: viewer_id(9),
        generation: NonZeroU64::new(9).unwrap(),
        gesture: Gesture::Cancel,
    };
    assert_eq!(
        execute_viewer(&mut connection, "81818181818181818181818181818181", wrong).outcome(),
        &rejected(Rejected::TerminalUnavailable)
    );

    for (index, action) in selection_actions().into_iter().enumerate() {
        let result = execute_viewer(
            &mut connection,
            &format!("{:032x}", 0x8282_u128 + index as u128),
            action,
        );
        if index == 5 {
            assert_eq!(result.outcome(), &Outcome::FeedbackAccepted {});
        } else {
            let Outcome::Record(actual) = result.outcome() else {
                panic!("selection action {index} did not preserve its record")
            };
            assert_eq!(actual, &records.recv().unwrap());
            let envelope = decode_transport_envelope(actual).unwrap();
            assert_eq!(envelope.role, MessageRole::Response);
            assert!(envelope.correlation_id.is_some());
        }
    }
    assert!(connection.viewers.iter().any(Option::is_some));
    let wrong_record = execute_viewer(
        &mut connection,
        "83838383838383838383838383838383",
        Action::SelectionGesture {
            attachment_id: viewer_id(2),
            generation: NonZeroU64::new(9).unwrap(),
            gesture: Gesture::Cancel,
        },
    );
    assert_eq!(
        wrong_record.outcome(),
        &unavailable(Unavailable::TerminalRuntimeUnavailable)
    );
    assert!(connection.viewers.iter().all(Option::is_none));
    detach_reply.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(connection);
    let _ = stop.shutdown(Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

/// Answers each selection control with the Host's correlated refusal, as when the control raced
/// a resize or an export that already finished, and any Poll with an empty reply.
fn refusing_selection_peer(mut peer: UnixStream, records: Sender<Vec<u8>>) {
    while let Ok(CapabilityNodeMessage::TerminalViewerRequest(request)) = read_message(&mut peer) {
        let outcome = match request.action {
            WorkerAction::Detach => WorkerOutcome::Detached,
            WorkerAction::Poll => WorkerOutcome::Empty,
            WorkerAction::SelectionGesture { .. }
            | WorkerAction::SelectionCommand { .. }
            | WorkerAction::SelectionExport { .. }
            | WorkerAction::SelectionExportChunkApplied { .. }
            | WorkerAction::SelectionExportCancel { .. } => {
                let code = if matches!(
                    request.action,
                    WorkerAction::SelectionGesture { .. } | WorkerAction::SelectionCommand { .. }
                ) {
                    "stale-generation"
                } else {
                    "invalid-acknowledgement"
                };
                let encoded =
                    encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                        code: code.to_owned(),
                        message: "refused".to_owned(),
                        retryable: false,
                    }))
                    .unwrap();
                let record =
                    response_record(&request, encoded.kind, encoded.role, &encoded.payload);
                records.send(record.clone()).unwrap();
                WorkerOutcome::Record(record)
            }
            ref action => panic!("unexpected viewer action {action:?}"),
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

#[test]
fn correlated_host_refusals_answer_selection_controls_and_keep_the_viewer() {
    let service = service();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    assert!(matches!(
        service.workers.lock().unwrap().admit(WORKER_ID, None),
        crate::worker_registry::WorkerAdmission::Accepted { .. }
    ));
    let owner =
        WorkerRegistry::register_carrier(&service.workers, NonZeroU64::MIN, carrier).unwrap();
    let (server, worker) = UnixStream::pair().unwrap();
    worker
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let stop = server.try_clone().unwrap();
    let owner = thread::spawn(move || owner.run(server));
    let (sent, records) = channel();
    let peer = thread::spawn(move || refusing_selection_peer(worker, sent));
    let mut connection = service.client_connection();
    assert!(matches!(
        connection.publish_attach(viewer(&service, handle), attached_result()),
        Outcome::Attached { .. }
    ));

    for (index, action) in selection_actions().into_iter().enumerate() {
        let result = execute_viewer(
            &mut connection,
            &format!("{:032x}", 0x8484_u128 + index as u128),
            action,
        );
        assert_eq!(
            result.outcome(),
            &Outcome::Record(records.recv().unwrap()),
            "selection action {index} lost the Host refusal"
        );
        assert!(connection.viewers.iter().any(Option::is_some));
    }
    assert_eq!(
        execute_viewer(&mut connection, "85858585858585858585858585858585", poll()).outcome(),
        &Outcome::Empty {}
    );
    drop(connection);
    let _ = stop.shutdown(Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

fn selection_events(attachment: [u8; 16]) -> Vec<Vec<u8>> {
    let attachment_id = AttachmentId::from_bytes(attachment).unwrap();
    let changed = encode_selection_control(&SelectionControl::ChangedEvent(SelectionChanged {
        attachment_id,
        state: SelectionChangedState::None,
    }))
    .unwrap();
    let chunk = encode_selection_export_chunk(&SelectionExportChunk {
        attachment_id,
        bytes: b"text".to_vec(),
        chunk_sequence: 0,
        export_sequence: NonZeroU64::new(10).unwrap(),
        final_chunk: true,
    })
    .unwrap();
    let finished = encode_selection_export_control(&SelectionExportControl::FinishedEvent(
        SelectionExportFinished {
            attachment_id,
            byte_length: 4,
            export_sequence: NonZeroU64::new(10).unwrap(),
            status: SelectionExportStatus::Completed,
        },
    ))
    .unwrap();
    vec![
        event_record(changed.kind, &changed.payload, PayloadEncoding::ControlJson),
        event_record(chunk.kind, &chunk.payload, PayloadEncoding::RawBytes),
        event_record(
            finished.kind,
            &finished.payload,
            PayloadEncoding::ControlJson,
        ),
    ]
}

fn event_record(kind: u16, payload: &[u8], payload_encoding: PayloadEncoding) -> Vec<u8> {
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::from_bytes([3; 16]).unwrap()),
        kind,
        optional_message: false,
        payload,
        payload_encoding,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap()
}

#[test]
fn poll_routes_only_selection_events_for_the_exact_viewer() {
    let expected = selection_events([2; 16]);
    let (service, handle, owner, peer, stop) = scripted_connection(VecDeque::from(
        expected
            .iter()
            .cloned()
            .map(WorkerOutcome::Record)
            .collect::<Vec<_>>(),
    ));
    let mut connection = service.client_connection();
    assert!(matches!(
        connection.publish_attach(viewer(&service, handle), attached_result()),
        Outcome::Attached { .. }
    ));
    for (index, expected) in expected.into_iter().enumerate() {
        let result = execute_viewer(
            &mut connection,
            &format!("{:032x}", 0x9191_u128 + index as u128),
            poll(),
        );
        assert_eq!(result.outcome(), &Outcome::Record(expected));
    }
    drop(connection);
    let _ = stop.shutdown(Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();

    for wrong in selection_events([9; 16]) {
        let (service, handle, owner, peer, stop) =
            scripted_connection(VecDeque::from([WorkerOutcome::Record(wrong)]));
        let mut connection = service.client_connection();
        assert!(matches!(
            connection.publish_attach(viewer(&service, handle), attached_result()),
            Outcome::Attached { .. }
        ));
        assert_eq!(
            execute_viewer(&mut connection, "92929292929292929292929292929292", poll(),).outcome(),
            &unavailable(Unavailable::TerminalRuntimeUnavailable)
        );
        assert!(connection.viewers.iter().all(Option::is_none));
        drop(connection);
        let _ = stop.shutdown(Shutdown::Both);
        let _ = owner.join();
        peer.join().unwrap();
    }
}
