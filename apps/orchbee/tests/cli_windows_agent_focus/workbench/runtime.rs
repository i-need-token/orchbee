use super::*;
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerResult, ViewerId, ViewerUnavailableReason, read_client_message,
    write_server_message,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, HostEpoch, MessageRole,
    PayloadEncoding, ProcessInfoControl, TerminalId, TerminalProcessInfo, TypedFailure,
    decode_terminal_text_frame, encode_connection_control, encode_process_info_control,
    encode_transport_envelope,
};

fn reply(stream: &mut LocalStream, expected: &str, outcome: TerminalViewerOutcome) {
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
}
pub(super) fn unavailable(stream: &mut LocalStream) {
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
        "../../../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
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

fn sample(stream: &mut LocalStream, mode: &str) {
    let bytes = frame_bytes();
    let frame = decode_terminal_text_frame(&bytes).unwrap();
    let route = frame.route;
    reply(
        stream,
        "attach",
        TerminalViewerOutcome::Attached {
            attachment_id: ViewerId::parse(&route.attachment_id.to_text()).unwrap(),
            host_epoch: orchbee_client_server_protocol::HostEpoch::parse(
                &route.host_epoch.to_text(),
            )
            .unwrap(),
        },
    );
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
        let mut process = peer_support::process(10, None, "cmd.exe");
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

#[test]
fn panel_runtime_uses_one_attachment_and_preserves_unavailable_metadata_without_partial_errors() {
    for current in [false, true] {
        for mode in [
            "observed",
            "process-unavailable",
            "terminal-unavailable",
            "wrong-epoch",
            "changed-workbench",
            "deadline",
        ] {
            let fixture = Fixture::new();
            let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
            let server = peer(&fixture, listener, move |mut stream| {
                snapshot_reply(&mut stream);
                page(&mut stream, 3);
                if mode == "terminal-unavailable" {
                    unavailable(&mut stream);
                } else {
                    sample(&mut stream, mode);
                }
                if !matches!(mode, "wrong-epoch" | "deadline") {
                    snapshot_at(
                        &mut stream,
                        if mode == "changed-workbench" { 13 } else { 12 },
                    );
                }
                assert_closed(&mut stream);
            });
            let started = Instant::now();
            let output = if current {
                fixture.run(&["panel", "current", "--panel", &id(6), "--format=json"])
            } else {
                fixture.run(&["panel", "get", &id(6), "--format=json"])
            };
            server.join().unwrap();
            let doc = document(&output);
            if mode == "deadline" {
                assert_eq!(output.status.code(), Some(7));
                assert_eq!(doc["failure"]["kind"], "timeout");
                assert!(doc.get("result").is_none());
                assert!(started.elapsed() >= Duration::from_secs(4));
                assert!(started.elapsed() < Duration::from_secs(8));
                continue;
            }
            if mode == "changed-workbench" {
                assert_eq!(output.status.code(), Some(3));
                assert_eq!(doc["failure"]["details"]["reason"], "workbench-changed");
                assert!(doc.get("result").is_none());
                continue;
            }
            if mode == "wrong-epoch" {
                assert_eq!(output.status.code(), Some(5));
                assert_eq!(doc["failure"]["kind"], "protocol-invalid");
                assert!(doc.get("result").is_none());
                continue;
            }
            assert!(output.status.success(), "{output:?}");
            validate(
                &doc,
                if current {
                    include_str!("../../../src/cli/schemas/orchbee.panel.current.json")
                } else {
                    include_str!("../../../src/cli/schemas/orchbee.panel.get.json")
                },
            );
            let panel = &doc["result"]["panel"];
            if mode == "terminal-unavailable" {
                assert_eq!(panel["terminalObservation"]["state"], "unavailable");
                assert!(panel["cwd"].is_null());
                continue;
            }
            assert_eq!(panel["terminalTitle"], "Orchbee 🐝");
            assert_eq!(panel["cwd"], "C:\\frame 项目");
            assert_eq!(panel["cwdSource"], "terminal-frame");
            assert_eq!(
                panel["foregroundCwd"],
                if mode == "observed" {
                    json!("C:\\root")
                } else {
                    Value::Null
                }
            );
            for field in ["terminalObservation", "cwd", "terminalTitle"] {
                let mut broken = doc.clone();
                broken["result"]["panel"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                let schema: Value = serde_json::from_str(if current {
                    include_str!("../../../src/cli/schemas/orchbee.panel.current.json")
                } else {
                    include_str!("../../../src/cli/schemas/orchbee.panel.get.json")
                })
                .unwrap();
                assert!(
                    !jsonschema::validator_for(&schema)
                        .unwrap()
                        .is_valid(&broken)
                );
            }
        }
    }
}
