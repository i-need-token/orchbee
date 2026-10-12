use super::*;
use crate::{invocation::Target, local_client::tests::server::FakeServer};
use base64::Engine;
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerResult, ViewerSnapshotReason,
    ViewerUnavailableReason, read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::{ChildOutputReader, LocalStream};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, TerminalTextCell,
    TerminalTextRow, decode_terminal_text_frame, encode_transport_envelope,
    inspect_terminal_frame_route,
};
use std::{
    io::{self, Read},
    num::NonZeroU64,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) const SNAPSHOT: &[u8] =
    include_bytes!("../../../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin");
pub(super) const COHORT: &str = "terminal-stream-test";
type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn handshake(stream: &mut LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}
pub(super) fn request(stream: &mut LocalStream) -> io::Result<TerminalViewerRequest> {
    let ClientMessage::TerminalViewer(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("observe must send Viewer requests only"));
    };
    assert_eq!(
        request
            .action()
            .attachment_id()
            .expect("retained Viewer action")
            .as_str(),
        inspect_terminal_frame_route(SNAPSHOT)
            .map_err(io::Error::other)?
            .attachment_id
            .to_text()
    );
    Ok(request)
}
pub(super) fn reply(
    stream: &mut LocalStream,
    request: TerminalViewerRequest,
    outcome: Outcome,
) -> io::Result<()> {
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .map_err(io::Error::other)
}
pub(super) fn record(sequence: u64, revision: u64, invalid_cells: bool) -> io::Result<Vec<u8>> {
    let mut bytes = SNAPSHOT.to_vec();
    bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
    bytes[64..72].copy_from_slice(&revision.to_le_bytes());
    // Preserve routing but invalidate the text payload: the projection must request a snapshot.
    if invalid_cells {
        bytes.truncate(160);
        bytes[8..12].copy_from_slice(&160_u32.to_le_bytes());
    }
    let route = inspect_terminal_frame_route(&bytes).map_err(io::Error::other)?;
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: &bytes,
    })
    .map_err(io::Error::other)
}
pub(super) fn line(output: &mut ChildOutputReader) -> io::Result<Value> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0];
        // After a feedback request the complete line must already be available.
        output.read_exact(&mut byte)?;
        line.push(byte[0]);
        if byte[0] == b'\n' {
            return serde_json::from_slice(&line).map_err(io::Error::other);
        }
    }
}
fn command() -> TerminalObserveCommand {
    TerminalObserveCommand {
        target: Target::Id("11111111111111111111111111111111".to_owned()),
        columns: 8,
        rows: 2,
        timeout_ms: None,
    }
}

#[test]
fn observe_repeated_frames_flush_before_ack_reset_and_detach_once() -> TestResult {
    const CLIENT_BUDGET: Duration = Duration::from_secs(4);
    let started = Instant::now();
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let viewer = ViewerId::parse(&route.attachment_id.to_text())?;
    let (mut output, child_output) = ChildOutputReader::pipe()?;
    let server = FakeServer::start(move |mut stream| {
        let mut stage = "handshake";
        let mut stage_started = Instant::now();
        let mut current_sequence = 0;
        let result = (|| {
            handshake(&mut stream)?;
            for (sequence, revision, invalid) in [(1, 1, false), (2, 2, true), (3, 3, false)] {
                current_sequence = sequence;
                stage = "read-poll";
                stage_started = Instant::now();
                let poll = request(&mut stream)?;
                assert!(matches!(poll.action(), Action::Poll { .. }));
                stage = "reply-frame";
                stage_started = Instant::now();
                reply(
                    &mut stream,
                    poll,
                    Outcome::Record(record(sequence, revision, invalid)?),
                )?;
                // Use the client budget for this peer's per-read guard.
                stream.set_read_timeout(Some(CLIENT_BUDGET))?;
                eprintln!(
                    "observe server sequence={sequence} frame-replied elapsed={:?}",
                    started.elapsed()
                );
                stage = "await-flush-feedback";
                stage_started = Instant::now();
                let feedback = request(&mut stream)?;
                eprintln!(
                    "observe server sequence={sequence} feedback-received elapsed={:?} wait={:?}",
                    started.elapsed(),
                    stage_started.elapsed()
                );
                stage = "read-flushed-output";
                stage_started = Instant::now();
                let emitted = line(&mut output)?;
                stage = "validate-output";
                stage_started = Instant::now();
                let schema: Value =
                    serde_json::from_str(include_str!("../schemas/orchbee.terminal.stream.json"))?;
                let validator = jsonschema::draft202012::new(&schema).map_err(io::Error::other)?;
                validator
                    .validate(&emitted)
                    .map_err(|error| io::Error::other(error.to_string()))?;
                assert_eq!(emitted["schema"], SCHEMA);
                assert_eq!(emitted["version"], 2);
                if invalid {
                    assert!(matches!(
                        feedback.action(),
                        Action::NeedSnapshot {
                            reason: ViewerSnapshotReason::DecoderRejected,
                            ..
                        }
                    ));
                    assert_eq!(emitted["kind"], "reset");
                    assert_eq!(emitted["reason"], "decoder-rejected");
                } else {
                    assert!(matches!(feedback.action(), Action::FrameApplied { .. }));
                    assert_eq!(emitted["kind"], "frame");
                    assert_eq!(emitted["sequence"], sequence.to_string());
                    assert_eq!(
                        emitted["sourceWidth"],
                        decode_terminal_text_frame(SNAPSHOT)
                            .map_err(io::Error::other)?
                            .geometry
                            .columns
                    );
                }
                stage = "reply-feedback";
                stage_started = Instant::now();
                reply(&mut stream, feedback, Outcome::FeedbackAccepted {})?;
            }
            stage = "read-final-poll";
            stage_started = Instant::now();
            let poll = request(&mut stream)?;
            stage = "reply-unavailable";
            stage_started = Instant::now();
            reply(
                &mut stream,
                poll,
                Outcome::Unavailable {
                    reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                },
            )?;
            stage = "read-detach";
            stage_started = Instant::now();
            let detach = request(&mut stream)?;
            assert!(matches!(detach.action(), Action::Detach { .. }));
            stage = "reply-detached";
            stage_started = Instant::now();
            reply(&mut stream, detach, Outcome::Detached {})?;
            stage = "read-cleanup-eof";
            stage_started = Instant::now();
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte)?,
                0,
                "no duplicate detach, control or resize"
            );
            Ok(())
        })();
        eprintln!(
            "observe server sequence={current_sequence} stage={stage} elapsed={:?} stage_elapsed={:?} result={result:?}",
            started.elapsed(),
            stage_started.elapsed()
        );
        result
    })?;
    let client = LocalClient::connect(server.endpoint(), COHORT)?;
    eprintln!("observe client connected elapsed={:?}", started.elapsed());
    let mut connection = Connection {
        client: Some(client),
        viewer: Some(viewer.clone()),
    };
    let gate = Gate {
        cancelled: &|| false,
        wake: None,
        total: Some(Instant::now() + CLIENT_BUDGET),
    };
    let relay_started = Instant::now();
    let mut relay = relay::Relay::with_output(child_output)?;
    eprintln!(
        "observe client relay-spawned elapsed={:?} spawn={:?}",
        started.elapsed(),
        relay_started.elapsed()
    );
    let client_result = observe_frames(
        connection.client.as_mut().unwrap(),
        &viewer,
        &mut TerminalProjection::new(route.attachment_id, route.host_epoch),
        &command(),
        &gate,
        &mut relay,
    );
    eprintln!(
        "observe client frames-returned elapsed={:?} relay_healthy={} result={client_result:?}",
        started.elapsed(),
        relay.healthy()
    );
    let cleanup = connection.cleanup();
    let second_cleanup = connection.cleanup();
    drop(relay);
    let server_result = server.finish();
    eprintln!(
        "observe finished elapsed={:?} client={client_result:?} server={server_result:?} cleanup={cleanup} second_cleanup={second_cleanup}",
        started.elapsed()
    );
    assert!(
        server_result.is_ok(),
        "server: {server_result:?}, client: {client_result:?}"
    );
    let failure = client_result.unwrap_err();
    assert_eq!(failure.kind, FailureKind::ServiceUnavailable);
    assert_eq!(cleanup, "detached");
    assert_eq!(second_cleanup, "not-attached");
    Ok(())
}

#[test]
fn observe_eof_is_failure_and_deadline_or_cancel_prevents_polling() -> TestResult {
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let viewer = ViewerId::parse(&route.attachment_id.to_text())?;
    let server = FakeServer::start(|mut stream| {
        handshake(&mut stream)?;
        assert!(matches!(
            request(&mut stream)?.action(),
            Action::Poll { .. }
        ));
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let cancelled = AtomicBool::new(false);
    let check = || cancelled.load(Ordering::Acquire);
    let expired = Gate {
        cancelled: &check,
        wake: None,
        total: Some(Instant::now()),
    };
    assert_eq!(
        exchange(
            &mut client,
            Action::Poll {
                attachment_id: viewer.clone()
            },
            &expired
        )
        .unwrap_err()
        .kind,
        FailureKind::Timeout
    );
    cancelled.store(true, Ordering::Release);
    assert_eq!(
        exchange(
            &mut client,
            Action::Poll {
                attachment_id: viewer.clone()
            },
            &expired
        )
        .unwrap_err()
        .kind,
        FailureKind::Cancelled
    );
    cancelled.store(false, Ordering::Release);
    let gate = Gate {
        cancelled: &check,
        wake: None,
        total: None,
    };
    assert_eq!(
        exchange(
            &mut client,
            Action::Poll {
                attachment_id: viewer
            },
            &gate
        )
        .unwrap_err()
        .kind,
        FailureKind::TransportLoss
    );
    server.finish()?;
    Ok(())
}

#[test]
fn observe_inflight_cancel_or_timeout_retires_without_claiming_detach() -> TestResult {
    use std::sync::{Arc, atomic::AtomicU8};
    for cancel in [true, false] {
        let flag = Arc::new(AtomicU8::new(0));
        let server_flag = Arc::clone(&flag);
        let server = FakeServer::start(move |mut stream| {
            handshake(&mut stream)?;
            assert!(matches!(
                request(&mut stream)?.action(),
                Action::Poll { .. }
            ));
            if cancel {
                server_flag.store(1, Ordering::Release);
            } else {
                std::thread::sleep(Duration::from_millis(80));
            }
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte)?,
                0,
                "no retry or fabricated detach acknowledgement"
            );
            Ok(())
        })?;
        let viewer = ViewerId::parse(
            &inspect_terminal_frame_route(SNAPSHOT)?
                .attachment_id
                .to_text(),
        )?;
        let client = LocalClient::connect(server.endpoint(), COHORT)?;
        let mut connection = Connection {
            client: Some(client),
            viewer: Some(viewer.clone()),
        };
        let requested = || flag.load(Ordering::Acquire) == 1;
        let gate = Gate {
            cancelled: &requested,
            wake: Some(orchbee_local_ipc::LocalIo::Wake(&flag, 1)),
            total: Some(Instant::now() + Duration::from_millis(if cancel { 2000 } else { 30 })),
        };
        let failure = exchange(
            connection.client.as_mut().unwrap(),
            Action::Poll {
                attachment_id: viewer,
            },
            &gate,
        )
        .unwrap_err();
        let cleanup = connection.cleanup();
        server.finish()?;
        assert_eq!(
            failure.kind,
            if cancel {
                FailureKind::Cancelled
            } else {
                FailureKind::Timeout
            }
        );
        assert_eq!(cleanup, "session-released");
    }
    Ok(())
}

#[test]
fn ansi_viewport_clips_wide_cells_sanitizes_controls_and_preserves_styles() -> TestResult {
    let mut source = decode_terminal_text_frame(SNAPSHOT)?;
    source.cursor.visible = true;
    source.cursor.column = 7;
    source.cursor.row = 0;
    let style = source.rows[0].cells[0].style;
    source.rows = vec![TerminalTextRow {
        prompt_class: 0,
        index: 0,
        flags: 0,
        cells: vec![
            TerminalTextCell {
                text: "\x1b]52;c;payload\x07".into(),
                style,
                width: 0,
            },
            TerminalTextCell {
                text: "中".into(),
                style,
                width: 1,
            },
            TerminalTextCell {
                text: "".into(),
                style,
                width: 2,
            },
        ],
    }];
    let narrow = frame::record(&source, 2, 1).map_err(|_| "render")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(narrow["bytes"].as_str().unwrap())
        .expect("base64 frame");
    let text = String::from_utf8(bytes)?;
    assert!(!text.contains("中"));
    assert!(!text.contains("payload"));
    assert!(!text.contains("\x1b]"));
    assert!(!text.contains("\x1b[?25h"));
    let wide = frame::record(&source, 8, 3).map_err(|_| "render")?;
    let text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(wide["bytes"].as_str().unwrap())
            .expect("base64 frame"),
    )?;
    assert!(text.contains("中"));
    assert!(text.ends_with("\x1b[1;8H\x1b[?25h"));
    assert!(text.contains("\x1b[2J"));
    assert_eq!(wide["width"], 8);
    assert_eq!(wide["height"], 3);
    Ok(())
}
