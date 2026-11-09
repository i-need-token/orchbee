//! Viewer-selection relay witness: raw CNP action tags are converted into the
//! exact correlated Terminal Protocol controls, and data-bearing Host answers
//! return unchanged as viewer records.
use super::super::*;
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostTerminalId, OperationId, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerRequest, WorkerGeneration, WorkerId, decode_message,
    encode_message,
};
use orchbee_terminal_protocol::{
    AttachmentId, ConnectionControl, CorrelationId, Envelope, HostEpoch, MessageRole,
    PayloadEncoding, SelectionChanged, SelectionChangedState, SelectionCommand,
    SelectionCommandKind, SelectionControl, SelectionExport, SelectionExportCancel,
    SelectionExportCancelReason, SelectionExportChunkApplied, SelectionExportControl,
    SelectionExportFinished, SelectionExportFormat, SelectionExportStarted, SelectionExportStatus,
    SelectionGesture, SelectionGestureAction, SelectionGestureGeometry, SelectionGesturePosition,
    SelectionPoint, SelectionViewportPoint, TRANSPORT_ENVELOPE_HEADER_BYTES, TypedFailure,
    decode_connection_control, decode_selection_control, decode_selection_export_control,
    decode_transport_envelope, encode_connection_control, encode_selection_control,
    encode_selection_export_control, encode_transport_envelope,
};
use std::{
    cell::RefCell,
    error::Error,
    io::{self, Read, Write},
    num::NonZeroU64,
    rc::Rc,
    thread::{self, JoinHandle},
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type PeerResult = Result<(), Box<dyn Error + Send + Sync>>;

fn host() -> TestResult<(HostBrokerConnection, UnixStream)> {
    let (stream, peer) = UnixStream::pair()?;
    Ok((
        HostBrokerConnection {
            endpoint: std::path::PathBuf::new(),
            host_epoch: HostEpoch::from_bytes([4; 16])?,
            prompt_gated_input: true,
            negotiated_limits: NegotiatedHostLimits {
                frame_bytes: 1_048_576,
                input_bytes: 4096,
                row_window_bytes: 4096,
            },
            stream,
            viewers: RefCell::new(viewers::HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: RefCell::new(titles::Titles::default()),
            host_colors: RefCell::new(crate::host_connection::HostColors::default()),
            drain: Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())),
        },
        peer,
    ))
}

fn attach() -> TestResult<TerminalViewerRequest> {
    Ok(TerminalViewerRequest {
        request_id: OperationId::from_bytes([20; 16])?,
        attachment_id: OperationId::from_bytes([1; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Attach {
            host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16])?,
            terminal_id: HostTerminalId::from_bytes([5; 16])?,
        },
    })
}

fn raw_request(request_byte: u8, action: &[u8]) -> TestResult<TerminalViewerRequest> {
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([request_byte; 16])?,
        attachment_id: OperationId::from_bytes([1; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Detach,
    };
    let mut frame = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(request))?;
    frame.truncate(65);
    frame.extend_from_slice(action);
    let payload_length = u16::try_from(frame.len() - 8)?;
    frame[6..8].copy_from_slice(&payload_length.to_be_bytes());
    let CapabilityNodeMessage::TerminalViewerRequest(request) = decode_message(&frame)? else {
        panic!("raw viewer action decoded outside the viewer plane")
    };
    Ok(request)
}

fn raw_requests() -> TestResult<Vec<TerminalViewerRequest>> {
    let mut actions = Vec::new();

    let mut start = vec![16];
    start.extend_from_slice(&9_u64.to_be_bytes());
    start.push(1);
    start.push(2);
    start.extend_from_slice(&3_u16.to_be_bytes());
    start.extend_from_slice(&4_u16.to_be_bytes());
    start.extend_from_slice(&(-5_i32).to_be_bytes());
    start.extend_from_slice(&6_i32.to_be_bytes());
    start.push(1);
    actions.push(start);

    let mut update = vec![16];
    update.extend_from_slice(&9_u64.to_be_bytes());
    update.push(2);
    update.extend_from_slice(&8_u16.to_be_bytes());
    update.extend_from_slice(&80_u16.to_be_bytes());
    update.extend_from_slice(&2_u16.to_be_bytes());
    update.extend_from_slice(&480_u16.to_be_bytes());
    update.extend_from_slice(&7_u16.to_be_bytes());
    update.extend_from_slice(&8_u16.to_be_bytes());
    update.extend_from_slice(&9_i32.to_be_bytes());
    update.extend_from_slice(&(-10_i32).to_be_bytes());
    update.push(0);
    actions.push(update);

    let mut end = vec![16];
    end.extend_from_slice(&9_u64.to_be_bytes());
    end.push(3);
    end.extend_from_slice(&11_u16.to_be_bytes());
    end.extend_from_slice(&12_u16.to_be_bytes());
    actions.push(end);

    let mut gesture_cancel = vec![16];
    gesture_cancel.extend_from_slice(&9_u64.to_be_bytes());
    gesture_cancel.push(4);
    actions.push(gesture_cancel);

    let mut export = vec![17];
    export.extend_from_slice(&10_u64.to_be_bytes());
    export.push(1);
    actions.push(export);

    let mut chunk_applied = vec![18];
    chunk_applied.extend_from_slice(&10_u64.to_be_bytes());
    chunk_applied.extend_from_slice(&42_u32.to_be_bytes());
    actions.push(chunk_applied);

    let mut export_cancel = vec![19];
    export_cancel.extend_from_slice(&10_u64.to_be_bytes());
    export_cancel.push(1);
    actions.push(export_cancel);

    let mut clear = vec![20];
    clear.extend_from_slice(&9_u64.to_be_bytes());
    clear.extend_from_slice(&7_u64.to_be_bytes());
    clear.push(1);
    actions.push(clear);

    let mut select_all = vec![20];
    select_all.extend_from_slice(&9_u64.to_be_bytes());
    select_all.extend_from_slice(&7_u64.to_be_bytes());
    select_all.push(2);
    actions.push(select_all);

    let mut set_range = vec![20];
    set_range.extend_from_slice(&9_u64.to_be_bytes());
    set_range.extend_from_slice(&7_u64.to_be_bytes());
    set_range.push(3);
    set_range.extend_from_slice(&1_u16.to_be_bytes());
    set_range.extend_from_slice(&2_u64.to_be_bytes());
    set_range.extend_from_slice(&3_u16.to_be_bytes());
    set_range.extend_from_slice(&4_u64.to_be_bytes());
    set_range.push(1);
    actions.push(set_range);

    let mut select_output = vec![20];
    select_output.extend_from_slice(&9_u64.to_be_bytes());
    select_output.extend_from_slice(&7_u64.to_be_bytes());
    select_output.push(4);
    select_output.extend_from_slice(&5_u16.to_be_bytes());
    select_output.extend_from_slice(&6_u64.to_be_bytes());
    actions.push(select_output);

    actions
        .iter()
        .enumerate()
        .map(|(index, action)| raw_request(21 + u8::try_from(index)?, action))
        .collect()
}

fn read_record(peer: &mut UnixStream) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
    peer.read_exact(&mut header)?;
    let length = usize::try_from(u32::from_le_bytes(header[8..12].try_into()?))?;
    let mut record = vec![0_u8; length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    peer.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}

fn reply(
    peer: &mut UnixStream,
    request: &[u8],
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> PeerResult {
    let request = decode_transport_envelope(request)?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: request.correlation_id,
        host_epoch: request.host_epoch,
        kind,
        optional_message: false,
        payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role,
    })?;
    peer.write_all(&record)?;
    Ok(())
}

fn peer(mut peer: UnixStream) -> JoinHandle<PeerResult> {
    thread::spawn(move || {
        let attachment_id = AttachmentId::from_bytes([1; 16])?;
        let gestures = [
            SelectionGestureAction::Start {
                click_count: 2,
                point: SelectionViewportPoint { column: 3, row: 4 },
                position: SelectionGesturePosition { x: -5, y: 6 },
                select_output: true,
            },
            SelectionGestureAction::Update {
                geometry: SelectionGestureGeometry {
                    cell_width: 8,
                    columns: 80,
                    padding_left: 2,
                    screen_height: 480,
                },
                point: SelectionViewportPoint { column: 7, row: 8 },
                position: SelectionGesturePosition { x: 9, y: -10 },
                rectangle: false,
            },
            SelectionGestureAction::End {
                point: SelectionViewportPoint {
                    column: 11,
                    row: 12,
                },
            },
            SelectionGestureAction::Cancel,
        ];
        for (index, action) in gestures.into_iter().enumerate() {
            let request = read_record(&mut peer)?;
            let envelope = decode_transport_envelope(&request)?;
            assert_eq!(
                envelope.correlation_id,
                Some(CorrelationId::from_bytes([21 + u8::try_from(index)?; 16])?)
            );
            assert_eq!(envelope.host_epoch, Some(HostEpoch::from_bytes([4; 16])?));
            assert_eq!(envelope.role, MessageRole::Request);
            assert_eq!(envelope.payload_encoding, PayloadEncoding::ControlJson);
            assert_eq!(
                decode_selection_control(envelope.kind, envelope.role, envelope.payload)?,
                SelectionControl::Gesture(SelectionGesture {
                    action,
                    attachment_id,
                    generation: NonZeroU64::new(9).unwrap(),
                })
            );
            let response =
                encode_selection_control(&SelectionControl::ChangedResponse(SelectionChanged {
                    attachment_id,
                    state: SelectionChangedState::None,
                }))?;
            reply(
                &mut peer,
                &request,
                response.kind,
                response.role,
                &response.payload,
            )?;
        }

        let request = read_record(&mut peer)?;
        let envelope = decode_transport_envelope(&request)?;
        assert_eq!(
            envelope.correlation_id,
            Some(CorrelationId::from_bytes([25; 16])?)
        );
        assert_eq!(
            decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)?,
            SelectionExportControl::ExportRequest(SelectionExport {
                attachment_id,
                export_sequence: NonZeroU64::new(10).unwrap(),
                format: SelectionExportFormat::Plain,
            })
        );
        let response = encode_selection_export_control(&SelectionExportControl::StartedResponse(
            SelectionExportStarted {
                attachment_id,
                byte_length: 4,
                export_sequence: NonZeroU64::new(10).unwrap(),
                format: SelectionExportFormat::Plain,
            },
        ))?;
        reply(
            &mut peer,
            &request,
            response.kind,
            response.role,
            &response.payload,
        )?;

        let request = read_record(&mut peer)?;
        let envelope = decode_transport_envelope(&request)?;
        assert_eq!(
            envelope.correlation_id,
            Some(CorrelationId::from_bytes([26; 16])?)
        );
        let applied = SelectionExportChunkApplied {
            attachment_id,
            chunk_sequence: 42,
            export_sequence: NonZeroU64::new(10).unwrap(),
        };
        assert_eq!(
            decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)?,
            SelectionExportControl::ChunkAppliedRequest(applied)
        );
        let response = encode_selection_export_control(
            &SelectionExportControl::ChunkAppliedResponse(applied),
        )?;
        reply(
            &mut peer,
            &request,
            response.kind,
            response.role,
            &response.payload,
        )?;

        let request = read_record(&mut peer)?;
        let envelope = decode_transport_envelope(&request)?;
        assert_eq!(
            envelope.correlation_id,
            Some(CorrelationId::from_bytes([27; 16])?)
        );
        assert_eq!(
            decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)?,
            SelectionExportControl::CancelRequest(SelectionExportCancel {
                attachment_id,
                export_sequence: NonZeroU64::new(10).unwrap(),
                reason: SelectionExportCancelReason::ClipboardFailed,
            })
        );
        let response = encode_selection_export_control(&SelectionExportControl::FinishedResponse(
            SelectionExportFinished {
                attachment_id,
                byte_length: 4,
                export_sequence: NonZeroU64::new(10).unwrap(),
                status: SelectionExportStatus::Cancelled,
            },
        ))?;
        reply(
            &mut peer,
            &request,
            response.kind,
            response.role,
            &response.payload,
        )?;

        let commands = [
            SelectionCommandKind::Clear,
            SelectionCommandKind::SelectAll,
            SelectionCommandKind::SetRange {
                end: SelectionPoint { column: 3, row: 4 },
                rectangle: true,
                start: SelectionPoint { column: 1, row: 2 },
            },
            SelectionCommandKind::SelectOutput {
                point: SelectionPoint { column: 5, row: 6 },
            },
        ];
        for (index, command) in commands.into_iter().enumerate() {
            let request = read_record(&mut peer)?;
            let envelope = decode_transport_envelope(&request)?;
            assert_eq!(
                envelope.correlation_id,
                Some(CorrelationId::from_bytes([28 + u8::try_from(index)?; 16])?)
            );
            assert_eq!(
                decode_selection_control(envelope.kind, envelope.role, envelope.payload)?,
                SelectionControl::Command(SelectionCommand {
                    attachment_id,
                    command,
                    generation: NonZeroU64::new(9).unwrap(),
                    revision: 7,
                })
            );
            let response =
                encode_selection_control(&SelectionControl::ChangedResponse(SelectionChanged {
                    attachment_id,
                    state: SelectionChangedState::None,
                }))?;
            reply(
                &mut peer,
                &request,
                response.kind,
                response.role,
                &response.payload,
            )?;
        }
        Ok(())
    })
}

fn decode_record(outcome: TerminalViewerOutcome) -> TestResult<Vec<u8>> {
    let TerminalViewerOutcome::Record(record) = outcome else {
        return Err(io::Error::other("selection response was not an opaque record").into());
    };
    Ok(record)
}

#[test]
fn selection_actions_convert_exactly_and_preserve_data_bearing_responses() -> TestResult {
    // Decode every raw CNP request before starting the Host peer, so a missing
    // public action tag fails without leaving a blocked test thread behind.
    let requests = raw_requests()?;
    let (host, host_peer) = host()?;
    host.viewers
        .borrow_mut()
        .admit(&attach()?)
        .map_err(|reason| format!("{reason:?}"))?;
    let (server, _quiet) = UnixStream::pair()?;
    let peer = peer(host_peer);

    let mut outcomes = Vec::new();
    for request in requests {
        outcomes.push(
            crate::terminal_viewer_operation::execute(
                request,
                orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
                NonZeroU64::MIN,
                &host,
                &server,
            )?
            .outcome,
        );
    }

    let attachment_id = AttachmentId::from_bytes([1; 16])?;
    for outcome in outcomes.drain(..4) {
        let record = decode_record(outcome)?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(
            decode_selection_control(envelope.kind, envelope.role, envelope.payload)?,
            SelectionControl::ChangedResponse(SelectionChanged {
                attachment_id,
                state: SelectionChangedState::None,
            })
        );
    }

    let record = decode_record(outcomes.remove(0))?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(
        decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)?,
        SelectionExportControl::StartedResponse(SelectionExportStarted {
            attachment_id,
            byte_length: 4,
            export_sequence: NonZeroU64::new(10).unwrap(),
            format: SelectionExportFormat::Plain,
        })
    );
    assert_eq!(outcomes.remove(0), TerminalViewerOutcome::FeedbackAccepted);
    let record = decode_record(outcomes.remove(0))?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(
        decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)?,
        SelectionExportControl::FinishedResponse(SelectionExportFinished {
            attachment_id,
            byte_length: 4,
            export_sequence: NonZeroU64::new(10).unwrap(),
            status: SelectionExportStatus::Cancelled,
        })
    );
    for outcome in outcomes.drain(..4) {
        let record = decode_record(outcome)?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(
            decode_selection_control(envelope.kind, envelope.role, envelope.payload)?,
            SelectionControl::ChangedResponse(SelectionChanged {
                attachment_id,
                state: SelectionChangedState::None,
            })
        );
    }
    assert!(outcomes.is_empty());

    peer.join()
        .unwrap()
        .map_err(|error| -> Box<dyn Error> { error })?;
    Ok(())
}

#[test]
fn selection_failures_preserve_the_exact_host_record_for_client_cancellation() -> TestResult {
    let requests = raw_requests()?;
    let count = requests.len();
    let (host, mut host_peer) = host()?;
    host.viewers
        .borrow_mut()
        .admit(&attach()?)
        .map_err(|reason| format!("{reason:?}"))?;
    let (server, _quiet) = UnixStream::pair()?;
    let peer = thread::spawn(
        move || -> Result<Vec<Vec<u8>>, Box<dyn Error + Send + Sync>> {
            let mut originals = Vec::new();
            for _ in 0..count {
                let request = read_record(&mut host_peer)?;
                let envelope = decode_transport_envelope(&request)?;
                let failure =
                    encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                        code: "stale-generation".into(),
                        message: "The selection generation is no longer current".into(),
                        retryable: false,
                        reason: None,
                    }))?;
                let original = encode_transport_envelope(&Envelope {
                    correlation_id: envelope.correlation_id,
                    host_epoch: envelope.host_epoch,
                    kind: failure.kind,
                    optional_message: false,
                    payload: &failure.payload,
                    payload_encoding: PayloadEncoding::ControlJson,
                    protocol: CURRENT_PROTOCOL_VERSION,
                    role: failure.role,
                })?;
                host_peer.write_all(&original)?;
                originals.push(original);
            }
            Ok(originals)
        },
    );
    let mut outcomes = Vec::new();
    for request in requests {
        outcomes.push(
            crate::terminal_viewer_operation::execute(
                request,
                orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
                NonZeroU64::MIN,
                &host,
                &server,
            )?
            .outcome,
        );
    }
    let originals = peer
        .join()
        .unwrap()
        .map_err(|error| -> Box<dyn Error> { error })?;
    for (outcome, original) in outcomes.into_iter().zip(originals) {
        let record = decode_record(outcome)?;
        assert_eq!(record, original);
        let envelope = decode_transport_envelope(&record)?;
        assert!(
            matches!(decode_connection_control(envelope.kind, envelope.role, envelope.payload)?,
            ConnectionControl::TypedFailure(TypedFailure { code, .. }) if code == "stale-generation")
        );
    }
    Ok(())
}
