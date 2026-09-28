//! Shared one-attachment terminal metadata peer for Panel and Agent queries.
use crate::peer_support;
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerResult, ViewerId, ViewerUnavailableReason, read_client_message,
    write_server_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, HostEpoch, MessageRole,
    PayloadEncoding, ProcessInfoControl, TerminalId, TerminalProcessInfo, TypedFailure,
    decode_terminal_text_frame, encode_connection_control, encode_process_info_control,
    encode_transport_envelope,
};

fn reply(
    stream: &mut LocalStream,
    expected: &str,
    outcome: TerminalViewerOutcome,
) -> orchbee_client_server_protocol::TerminalViewerRequest {
    let ClientMessage::TerminalViewer(request) = read_client_message(stream).unwrap() else {
        panic!("only viewer {expected}")
    };
    match (expected, request.action()) {
        ("attach", TerminalViewerAction::AttachBinding { .. })
        | ("poll", TerminalViewerAction::Poll { .. })
        | ("process", TerminalViewerAction::ReadProcessInfo { .. })
        | ("detach", TerminalViewerAction::Detach { .. }) => {}
        _ => panic!(
            "unexpected viewer action for {expected}: {:?}",
            request.action()
        ),
    }
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .unwrap();
    request
}
pub(crate) fn unavailable(stream: &mut LocalStream) {
    reply(
        stream,
        "attach",
        TerminalViewerOutcome::Unavailable {
            reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
        },
    );
}
fn frame_bytes() -> Vec<u8> {
    let mut bytes = include_bytes!(
        "../../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
    )
    .to_vec();
    let count = u32::from_le_bytes(bytes[124..128].try_into().unwrap());
    let mut end = 160;
    for _ in 0..count {
        let size = u32::from_le_bytes(bytes[end..end + 4].try_into().unwrap()) as usize;
        end += 4 + size;
    }
    let mut metadata = Vec::new();
    for text in ["Orchbee 🐝", "file:///C:/frame%20%E9%A1%B9%E7%9B%AE"] {
        metadata.extend_from_slice(&(text.len() as u32).to_le_bytes());
        metadata.extend_from_slice(text.as_bytes());
    }
    bytes.splice(end..end, metadata);
    let size = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&size.to_le_bytes());
    bytes[116..120].copy_from_slice(&count.to_le_bytes());
    bytes[120..124].copy_from_slice(&(count + 1).to_le_bytes());
    bytes[124..128].copy_from_slice(&(count + 2).to_le_bytes());
    bytes
}

pub(crate) fn sample(stream: &mut LocalStream, mode: &str, image: &str) {
    sample_for(stream, mode, image, None);
}
pub(crate) fn sample_for(
    stream: &mut LocalStream,
    mode: &str,
    image: &str,
    expected_terminal: Option<&str>,
) {
    let bytes = frame_bytes();
    let frame = decode_terminal_text_frame(&bytes).unwrap();
    let route = frame.route;
    let attached = reply(
        stream,
        "attach",
        TerminalViewerOutcome::Attached {
            attachment_id: ViewerId::parse(&route.attachment_id.to_text()).unwrap(),
            host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(
                &route.host_epoch.to_text(),
            )
            .unwrap(),
            host_epoch: orchbee_client_server_protocol::HostEpoch::parse(
                &route.host_epoch.to_text(),
            )
            .unwrap(),
        },
    );
    if let Some(expected) = expected_terminal {
        assert!(
            matches!(attached.action(), TerminalViewerAction::AttachBinding { terminal_id, .. } if terminal_id.as_str() == expected)
        );
    }
    if mode == "deadline" {
        let ClientMessage::TerminalViewer(request) = read_client_message(stream).unwrap() else {
            panic!("frame poll")
        };
        assert!(matches!(
            request.action(),
            TerminalViewerAction::Poll { .. }
        ));
        // The client must abandon the pending read when its original budget ends.
        return;
    }
    let wrong = HostEpoch::from_bytes([77; 16]).unwrap();
    let record = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(if mode == "wrong-epoch" {
            wrong
        } else {
            route.host_epoch
        }),
        correlation_id: None,
        optional_message: false,
        payload: &bytes,
    })
    .unwrap();
    reply(stream, "poll", TerminalViewerOutcome::Record(record));
    if mode != "wrong-epoch" {
        let mut process = peer_support::process(10, None, image);
        process.current_directory = Some("C:\\root".into());
        let (kind, payload) = if mode == "process-unavailable" {
            let encoded =
                encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                    code: "process-sample-unavailable".into(),
                    message: "fixture".into(),
                    retryable: false,
                }))
                .unwrap();
            (encoded.kind, encoded.payload)
        } else {
            let info = TerminalProcessInfo {
                attachment_id: route.attachment_id,
                terminal_id: TerminalId::from_bytes([7; 16]).unwrap(),
                root_pid: 10,
                processes: vec![process],
            };
            let encoded =
                encode_process_info_control(&ProcessInfoControl::ProcessInfo(info)).unwrap();
            (encoded.kind, encoded.payload)
        };
        let record = encode_transport_envelope(&Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Response,
            kind,
            payload_encoding: PayloadEncoding::ControlJson,
            host_epoch: Some(route.host_epoch),
            correlation_id: Some(CorrelationId::from_bytes([9; 16]).unwrap()),
            optional_message: false,
            payload: &payload,
        })
        .unwrap();
        reply(stream, "process", TerminalViewerOutcome::Record(record));
    }
    reply(stream, "detach", TerminalViewerOutcome::Detached {});
}
