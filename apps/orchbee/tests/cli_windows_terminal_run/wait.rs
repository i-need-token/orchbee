use super::*;
use orchbee_local_ipc::LocalStream;
use orchbee_terminal_protocol as obtp;

const FRAME: &[u8] = include_bytes!("../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin");
const TERMINAL: &str = "33333333333333333333333333333333";

fn request(stream: &mut LocalStream) -> TerminalViewerRequest {
    let ClientMessage::TerminalViewer(request) = read_client_message(stream).unwrap() else {
        panic!("viewer operation, never another submission")
    };
    request
}

fn reply(
    stream: &mut LocalStream,
    request: &TerminalViewerRequest,
    outcome: TerminalViewerOutcome,
) {
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .unwrap();
}

fn attach(stream: &mut LocalStream) -> ViewerId {
    let route = obtp::inspect_terminal_frame_route(FRAME).unwrap();
    let viewer = ViewerId::parse(&route.attachment_id.to_text()).unwrap();
    let req = request(stream);
    assert!(matches!(
        req.action(),
        TerminalViewerAction::AttachBinding { .. }
    ));
    reply(
        stream,
        &req,
        TerminalViewerOutcome::Attached {
            creation_id: orchbee_client_server_protocol::HostCreationId::parse(&"c".repeat(32))
                .unwrap(),
            attachment_id: viewer.clone(),
            host_epoch: HostEpoch::parse(&route.host_epoch.to_text()).unwrap(),
            host_terminal_id: HostTerminalId::parse(&route.host_epoch.to_text()).unwrap(),
        },
    );
    viewer
}

fn frame(stream: &mut LocalStream, sequence: u64, ready: bool, command: Option<(&str, i32)>) {
    let req = request(stream);
    assert!(matches!(req.action(), TerminalViewerAction::Poll { .. }));
    let mut bytes = FRAME.to_vec();
    bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
    let count = u16::from_le_bytes(bytes[132..134].try_into().unwrap());
    bytes[132..134].copy_from_slice(&(count + 1).to_le_bytes());
    let mut observation = Vec::new();
    observation.extend_from_slice(&1_u64.to_le_bytes());
    observation.push(u8::from(ready));
    observation.push(if command.is_some() { 2 } else { 0 });
    if let Some((id, status)) = command {
        for pair in id.as_bytes().chunks_exact(2) {
            observation.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
        }
        observation.extend_from_slice(&0_u64.to_le_bytes());
        observation.extend_from_slice(&0_u16.to_le_bytes());
        observation.extend_from_slice(&1_u64.to_le_bytes());
        observation.extend_from_slice(&0_u16.to_le_bytes());
        observation.extend_from_slice(&1_u64.to_le_bytes());
        observation.extend_from_slice(&1_u64.to_le_bytes());
        observation.extend_from_slice(&status.to_le_bytes());
    }
    bytes.extend_from_slice(&5_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(observation.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&observation);
    let length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
    let route = obtp::inspect_terminal_frame_route(&bytes).unwrap();
    let record = obtp::encode_transport_envelope(&obtp::Envelope {
        protocol: obtp::CURRENT_PROTOCOL_VERSION,
        role: obtp::MessageRole::Event,
        kind: 41,
        optional_message: false,
        payload_encoding: obtp::PayloadEncoding::TerminalFrame,
        correlation_id: None,
        host_epoch: Some(route.host_epoch),
        payload: &bytes,
    })
    .unwrap();
    reply(stream, &req, TerminalViewerOutcome::Record(record));
    let req = request(stream);
    assert!(matches!(
        req.action(),
        TerminalViewerAction::FrameApplied { .. }
    ));
    reply(stream, &req, TerminalViewerOutcome::FeedbackAccepted {});
}

fn detach(stream: &mut LocalStream, viewer: &ViewerId) {
    let req = request(stream);
    assert!(
        matches!(req.action(), TerminalViewerAction::Detach { attachment_id } if attachment_id == viewer)
    );
    reply(stream, &req, TerminalViewerOutcome::Detached {});
    assert_closed(stream);
}

#[test]
fn correlated_wait_ignores_old_completion_and_keeps_the_single_submission() {
    for exit_code in [0, 1] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = attach(&mut stream);
            let sequence = obtp::inspect_terminal_frame_route(FRAME)
                .unwrap()
                .sequence
                .get();
            frame(
                &mut stream,
                sequence,
                true,
                Some(("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 0)),
            );
            let ClientMessage::TerminalSend(req) = read_client_message(&mut stream).unwrap() else {
                panic!("exactly one paste/Enter request")
            };
            let SendRecord::Paste { content } = &req.records().as_slice()[0] else {
                panic!("paste")
            };
            let text = std::str::from_utf8(content.as_bytes()).unwrap();
            let id = text
                .strip_prefix("<#orchbee-run:")
                .unwrap()
                .split_once("#> ")
                .unwrap();
            assert_eq!(id.1, "Write-Output 'ok'");
            assert_eq!(id.0.len(), 32);
            assert_eq!(req.records().as_slice().len(), 2);
            let run_id = id.0.to_owned();
            let target = SendTarget::new(
                std::num::NonZeroU64::MIN,
                HostEpoch::parse("66666666666666666666666666666666").unwrap(),
                HostTerminalId::parse("77777777777777777777777777777777").unwrap(),
            );
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalSendResult(
                    TerminalSendResult::new(
                        req.request_id().clone(),
                        Some(target),
                        TerminalSendOutcome::Accepted {
                            progress: SendProgress::complete(2, (text.len() + 1) as u64),
                            cleanup: SendCleanupStatus::Confirmed,
                        },
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
            frame(
                &mut stream,
                sequence + 1,
                true,
                Some(("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 0)),
            );
            frame(&mut stream, sequence + 2, true, Some((&run_id, exit_code)));
            detach(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal",
            "run",
            TERMINAL,
            "Write-Output 'ok'",
            "--wait",
            "--format=json",
        ]);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit_code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../../src/cli/schemas/orchbee.terminal.run.json"),
        );
        assert_eq!(
            document["result"]["submission"]["result"]["recordsAccepted"],
            2
        );
        assert_eq!(document["result"]["completion"]["exitCode"], exit_code);
        assert_eq!(document["result"]["cleanup"], "confirmed");
        assert!(document["result"]["failure"].is_null());
    }
}

#[test]
fn unready_shell_refuses_before_any_input() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        let viewer = attach(&mut stream);
        frame(
            &mut stream,
            obtp::inspect_terminal_frame_route(FRAME)
                .unwrap()
                .sequence
                .get(),
            false,
            None,
        );
        detach(&mut stream, &viewer);
    });
    let output = fixture.run(&[
        "terminal",
        "run",
        TERMINAL,
        "echo ok",
        "--wait",
        "--format=json",
    ]);
    handler.join().unwrap();
    assert_ne!(output.status.code(), Some(0));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../../src/cli/schemas/orchbee.terminal.run.json"),
    );
    assert!(document["result"]["submission"].is_null());
    assert_eq!(
        document["result"]["failure"]["details"]["reason"],
        "shell-not-ready-or-unsupported"
    );
}
