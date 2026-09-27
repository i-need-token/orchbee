use super::*;
use crate::{
    cli::terminal_stream::tests::{COHORT, SNAPSHOT, handshake, line, record, reply},
    local_client::test_server::FakeServer,
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlRejectedReason, ServerMessage, TerminalControlResult,
    read_client_message, write_server_message,
};
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader};
use orchbee_terminal_protocol::{decode_terminal_text_frame, inspect_terminal_frame_route};
use std::{
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const CONTROLLER: &str = "cccccccccccccccccccccccccccccccc";

fn schema_check(value: &Value) -> io::Result<()> {
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.terminal.stream.json"))?;
    jsonschema::draft202012::new(&schema)
        .map_err(io::Error::other)?
        .validate(value)
        .map_err(|e| io::Error::other(e.to_string()))
}

#[test]
fn control_orders_chunks_resize_receipts_and_exact_cleanup_without_replay() -> TestResult {
    for (takeover, release, refuse_second) in [
        (false, false, false),
        (true, true, false),
        (false, true, true),
    ] {
        let route = inspect_terminal_frame_route(SNAPSHOT)?;
        let viewer = ViewerId::parse(&route.attachment_id.to_text())?;
        let expected_viewer = viewer.clone();
        let (mut output, child_output) = ChildOutputReader::pipe()?;
        let server = FakeServer::start(move |mut stream| {
            handshake(&mut stream)?;
            let mut sent_frame = false;
            let mut frame_ack = false;
            let mut effects = Vec::new();
            let mut sequence = 0;
            loop {
                match read_client_message(&mut stream).map_err(io::Error::other)? {
                    ClientMessage::TerminalControl(request) => {
                        let outcome = match request.action() {
                            ControlAction::Acquire { attachment_id }
                            | ControlAction::Takeover { attachment_id } => {
                                assert_eq!(attachment_id, &expected_viewer);
                                assert_eq!(
                                    matches!(request.action(), ControlAction::Takeover { .. }),
                                    takeover
                                );
                                assert!(effects.is_empty());
                                ControlOutcome::Acquired {
                                    controller_id: ControllerId::parse(CONTROLLER).unwrap(),
                                }
                            }
                            ControlAction::Resize {
                                attachment_id,
                                controller_id,
                                sequence: seq,
                                geometry,
                            } => {
                                assert_eq!(attachment_id, &expected_viewer);
                                assert_eq!(controller_id.as_str(), CONTROLLER);
                                sequence += 1;
                                assert_eq!(seq.get(), sequence);
                                effects.push(format!(
                                    "resize:{}x{}",
                                    geometry.columns(),
                                    geometry.rows()
                                ));
                                ControlOutcome::Resized {
                                    geometry: *geometry,
                                }
                            }
                            ControlAction::Input {
                                attachment_id,
                                controller_id,
                                sequence: seq,
                                bytes,
                            } => {
                                assert_eq!(attachment_id, &expected_viewer);
                                assert_eq!(controller_id.as_str(), CONTROLLER);
                                sequence += 1;
                                assert_eq!(seq.get(), sequence);
                                assert!(frame_ack);
                                assert!(bytes.as_bytes().iter().all(|b| *b == b'x'));
                                effects.push(format!("input:{}", bytes.as_bytes().len()));
                                if refuse_second && sequence == 3 {
                                    ControlOutcome::Rejected {
                                        reason: ControlRejectedReason::HostInputBackpressure,
                                    }
                                } else {
                                    ControlOutcome::InputAccepted {}
                                }
                            }
                            _ => return Err(io::Error::other("unexpected controller effect")),
                        };
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                outcome,
                            )),
                        )
                        .map_err(io::Error::other)?;
                    }
                    ClientMessage::TerminalViewer(request) => {
                        assert_eq!(request.action().attachment_id(), Some(&expected_viewer));
                        let outcome = match request.action() {
                            Action::Poll { .. } if !sent_frame => {
                                sent_frame = true;
                                Outcome::Record(record(1, 1, false)?)
                            }
                            Action::Poll { .. } => Outcome::Empty {},
                            Action::FrameApplied { .. } => {
                                let emitted = line(&mut output)?;
                                assert_eq!(emitted["kind"], "frame");
                                schema_check(&emitted)?;
                                frame_ack = true;
                                Outcome::FeedbackAccepted {}
                            }
                            Action::Detach { .. } => {
                                reply(&mut stream, request, Outcome::Detached {})?;
                                let parsed = line(&mut output)?;
                                assert_eq!(parsed["operation"], "parse");
                                assert_eq!(parsed["ordinal"], "1");
                                assert_eq!(parsed["outcome"]["kind"], "rejected");
                                schema_check(&parsed)?;
                                let input = line(&mut output)?;
                                assert_eq!(input["ordinal"], "2");
                                assert_eq!(input["operation"], "input");
                                assert_eq!(
                                    input["outcome"]["acceptedBytes"],
                                    if refuse_second { 4096 } else { 5000 }
                                );
                                assert_eq!(
                                    input["outcome"]["kind"],
                                    if refuse_second {
                                        "failed"
                                    } else {
                                        "input-accepted"
                                    }
                                );
                                schema_check(&input)?;
                                if refuse_second {
                                    assert_eq!(effects, ["resize:8x2", "input:4096", "input:904"]);
                                    assert_eq!(input["outcome"]["deliveryMayBeUnknown"], true);
                                } else {
                                    let resized = line(&mut output)?;
                                    assert_eq!(resized["ordinal"], "3");
                                    assert_eq!(resized["outcome"]["kind"], "resized");
                                    schema_check(&resized)?;
                                    assert_eq!(
                                        effects,
                                        ["resize:8x2", "input:4096", "input:904", "resize:12x3"]
                                    );
                                }
                                let mut byte = [0];
                                assert_eq!(
                                    stream.read(&mut byte)?,
                                    0,
                                    "no effect replay or duplicate cleanup"
                                );
                                return Ok(());
                            }
                            _ => return Err(io::Error::other("unexpected Viewer effect")),
                        };
                        reply(&mut stream, request, outcome)?;
                    }
                    _ => return Err(io::Error::other("unexpected request")),
                }
            }
        })?;
        let client = LocalClient::connect(server.endpoint(), COHORT)?;
        let mut connection = Connection {
            client: Some(client),
            viewer: Some(viewer.clone()),
        };
        let gate = Gate {
            cancelled: &|| false,
            wake: None,
            total: Some(Instant::now() + STEP),
        };
        let mut relay = relay::Relay::with_output(child_output)?;
        let (mut writer, child_input) = ChildInputWriter::pipe()?;
        let mut reader = input::Reader::with_input(child_input)?;
        let mut ndjson =
            b"{\"type\":\"terminal.input\",\"text\":\"must-not-send\",\"extra\":1}\n".to_vec();
        ndjson.extend(serde_json::to_vec(
            &json!({"type":"terminal.input", "text":"x".repeat(5000)}),
        )?);
        ndjson.extend(b"\n{\"type\":\"terminal.resize\",\"cols\":12,\"rows\":3}\n");
        if release {
            ndjson.extend(b"{\"type\":\"terminal.release\"}\n{\"type\":\"terminal.input\",\"text\":\"too-late\"}\n");
        }
        let result = std::thread::scope(|scope| {
            let writing = scope.spawn(move || -> io::Result<()> {
                let deadline = Instant::now() + STEP;
                let mut remaining = ndjson.as_slice();
                while !remaining.is_empty() {
                    match writer.write(remaining) {
                        Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                        Ok(n) => remaining = &remaining[n..],
                        Err(e)
                            if e.kind() == io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(1))
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            });
            let client = connection.client.as_mut().unwrap();
            let action = if takeover {
                ControlAction::Takeover {
                    attachment_id: viewer.clone(),
                }
            } else {
                ControlAction::Acquire {
                    attachment_id: viewer.clone(),
                }
            };
            let ControlOutcome::Acquired { controller_id } = submit(client, action, &gate).unwrap()
            else {
                panic!("grant")
            };
            let geometry = Geometry::new(8, 2, 0, 0).unwrap();
            let mut session = Session {
                client,
                viewer: viewer.clone(),
                controller: controller_id,
                sequence: 0,
                ordinal: 0,
                geometry,
                projection: TerminalProjection::new(route.attachment_id, route.host_epoch),
                history: history::History::default(),
                gate: &gate,
                output: &mut relay,
            };
            session.resize(geometry).unwrap();
            let result = session.drive(&mut reader);
            writing.join().unwrap().unwrap();
            result
        });
        let cleanup = connection.cleanup();
        drop(reader);
        drop(relay);
        server.finish()?;
        assert_eq!(cleanup, "detached");
        if refuse_second {
            assert_eq!(result.unwrap_err().kind, FailureKind::Rejected);
        } else {
            assert_eq!(
                result.unwrap(),
                if release { "released" } else { "stdin-eof" }
            );
        }
    }
    Ok(())
}

#[test]
fn control_inflight_cancel_and_timeout_retire_unknown_delivery_without_replay() -> TestResult {
    for cancel in [true, false] {
        let flag = Arc::new(AtomicU8::new(0));
        let signal = flag.clone();
        let server = FakeServer::start(move |mut stream| {
            handshake(&mut stream)?;
            assert!(matches!(
                read_client_message(&mut stream).map_err(io::Error::other)?,
                ClientMessage::TerminalControl(_)
            ));
            if cancel {
                signal.store(1, Ordering::Release);
            } else {
                std::thread::sleep(Duration::from_millis(80));
            }
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte)?, 0);
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), COHORT)?;
        let viewer = ViewerId::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")?;
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
        let failure = submit(
            connection.client.as_mut().unwrap(),
            ControlAction::Input {
                attachment_id: viewer,
                controller_id: ControllerId::parse(CONTROLLER)?,
                sequence: NonZeroU64::MIN,
                bytes: ControlInput::new(b"once".to_vec())?,
            },
            &gate,
        )
        .unwrap_err();
        assert_eq!(connection.cleanup(), "session-released");
        server.finish()?;
        assert_eq!(
            failure.kind,
            if cancel {
                FailureKind::Cancelled
            } else {
                FailureKind::Timeout
            }
        );
    }
    Ok(())
}

#[test]
fn scroll_uses_crossterm_modifiers_and_routes_shell_pages_mouse_and_alternate_screen() -> TestResult
{
    let frame = |bits: u32, alternate: bool| {
        let mut bytes = SNAPSHOT.to_vec();
        bytes[100..104].copy_from_slice(&bits.to_le_bytes());
        bytes[98] = if alternate { 2 } else { 1 };
        decode_terminal_text_frame(&bytes).unwrap()
    };
    let scroll = input::Scroll {
        direction: input::Direction::Up,
        lines: 80,
        source: input::ScrollSource::Wheel,
        column: None,
        row: None,
        modifiers: 7,
    };
    assert_eq!(
        scroll_effect(&frame(0, false), scroll, false),
        ScrollEffect::History(80)
    );
    assert_eq!(
        scroll_effect(&frame(0x220, false), scroll, false),
        ScrollEffect::Input(b"\x1b[<92;1;1M".to_vec())
    );
    assert_eq!(
        scroll_effect(
            &frame(0x220, true),
            input::Scroll {
                column: Some(1),
                row: Some(1),
                modifiers: 1,
                ..scroll
            },
            false
        ),
        ScrollEffect::Input(b"\x1b[<68;2;2M".to_vec())
    );
    assert_eq!(
        scroll_effect(&frame(1, true), scroll, false),
        ScrollEffect::Input(b"\x1bOA".to_vec())
    );
    let page = input::Scroll {
        source: input::ScrollSource::PageKey,
        ..scroll
    };
    assert_eq!(
        scroll_effect(&frame(1, false), page, false),
        ScrollEffect::Input(b"\x1b[5~".to_vec())
    );
    assert_eq!(
        scroll_effect(&frame(5, false), page, false),
        ScrollEffect::History(80)
    );
    assert_eq!(
        scroll_effect(&frame(5, true), page, false),
        ScrollEffect::Input(b"\x1b[5~".to_vec())
    );
    assert_eq!(
        scroll_effect(&frame(0x820, false), scroll, false),
        ScrollEffect::Rejected("pixel-mouse-unavailable")
    );
    Ok(())
}
