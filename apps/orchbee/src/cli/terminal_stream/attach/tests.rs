use super::*;
use crate::{
    cli::terminal_stream::tests::{COHORT, SNAPSHOT, handshake, record, reply, request},
    local_client::test_server::FakeServer,
};
use orchbee_client_server_protocol::{
    ClientMessage, ControllerId, PasteAcceptedBytes, ServerMessage, TerminalControlResult,
    read_client_message, write_server_message,
};
use orchbee_terminal_protocol::inspect_terminal_frame_route;
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct ConsoleWriter {
    flushed: Arc<AtomicBool>,
    fail_flush: bool,
    bytes: Vec<u8>,
}
impl Write for ConsoleWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            self.flushed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
}

#[test]
fn console_flush_gates_ack_and_paste_keeps_semantic_authority()
-> Result<(), Box<dyn std::error::Error>> {
    for fail_flush in [false, true] {
        let route = inspect_terminal_frame_route(SNAPSHOT)?;
        let viewer = ViewerId::parse(&route.attachment_id.to_text())?;
        let controller = ControllerId::parse("cccccccccccccccccccccccccccccccc")?;
        let flushed = Arc::new(AtomicBool::new(false));
        let server_flushed = flushed.clone();
        let server_viewer = viewer.clone();
        let server_controller = controller.clone();
        let server = FakeServer::start(move |mut stream| {
            handshake(&mut stream)?;
            let poll = request(&mut stream)?;
            assert!(matches!(poll.action(), Action::Poll { .. }));
            reply(&mut stream, poll, Outcome::Record(record(1, 1, false)?))?;
            if !fail_flush {
                let ack = request(&mut stream)?;
                assert!(matches!(ack.action(), Action::FrameApplied { .. }));
                assert!(
                    server_flushed.load(Ordering::SeqCst),
                    "never acknowledge before console flush"
                );
                reply(&mut stream, ack, Outcome::FeedbackAccepted {})?;
                for expected_sequence in 1..=3 {
                    let ClientMessage::TerminalControl(control) =
                        read_client_message(&mut stream).map_err(io::Error::other)?
                    else {
                        panic!("controller effect")
                    };
                    let outcome = match control.action() {
                        TerminalControlAction::Input {
                            attachment_id,
                            controller_id,
                            sequence,
                            bytes,
                        } => {
                            assert_eq!(attachment_id, &server_viewer);
                            assert_eq!(controller_id, &server_controller);
                            assert_eq!(sequence.get(), expected_sequence);
                            assert_eq!(
                                bytes.as_bytes(),
                                if expected_sequence == 1 {
                                    &b"\x1b[I"[..]
                                } else {
                                    &b"done"[..]
                                }
                            );
                            TerminalControlOutcome::InputAccepted {}
                        }
                        TerminalControlAction::Paste {
                            attachment_id,
                            controller_id,
                            sequence,
                            content,
                        } => {
                            assert_eq!(attachment_id, &server_viewer);
                            assert_eq!(controller_id, &server_controller);
                            assert_eq!(sequence.get(), 2);
                            assert_eq!(expected_sequence, 2);
                            assert_eq!(content.as_bytes(), "中\x02q".as_bytes());
                            TerminalControlOutcome::Pasted {
                                accepted_bytes: PasteAcceptedBytes::new(17).unwrap(),
                            }
                        }
                        _ => panic!("unexpected control action"),
                    };
                    write_server_message(
                        &mut stream,
                        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                            control.request_id().clone(),
                            outcome,
                        )),
                    )
                    .map_err(io::Error::other)?;
                }
            }
            let detach = request(&mut stream)?;
            assert!(
                matches!(detach.action(), Action::Detach { .. }),
                "failed flush must detach without FrameApplied or input"
            );
            reply(&mut stream, detach, Outcome::Detached {})?;
            assert_eq!(stream.read(&mut [0])?, 0, "no duplicate detach");
            Ok(())
        })?;
        let mut connection = Connection {
            client: Some(LocalClient::connect(server.endpoint(), COHORT)?),
            viewer: Some(viewer.clone()),
        };
        let gate = Gate {
            cancelled: &|| false,
            wake: None,
            total: Some(Instant::now() + Duration::from_secs(4)),
        };
        let mut display = ConsoleDisplay {
            writer: ConsoleWriter {
                flushed,
                fail_flush,
                bytes: Vec::new(),
            },
            locale: Locale::EnUs,
            columns: 8,
        };
        let result = {
            let mut session = Session::new(
                connection.client.as_mut().unwrap(),
                viewer,
                controller,
                Geometry::new(8, 6, 0, 0)?,
                TerminalProjection::new(route.attachment_id, route.host_epoch),
                &gate,
                &mut display,
            );
            session.focus(true).unwrap(); // No validated frame means no focus report.
            session.poll().and_then(|()| {
                session.focus(true)?;
                session.paste("中\x02q".to_owned())?;
                session.input(b"done").map_err(|(failure, _)| failure)?;
                Ok(())
            })
        };
        assert_eq!(connection.cleanup(), "detached");
        server.finish()?;
        assert_eq!(result.is_err(), fail_flush);
        assert!(display.writer.bytes.starts_with(b"\x1b["));
        assert!(!display.writer.bytes.windows(8).any(|s| s == b"\"schema\""));
    }
    Ok(())
}
