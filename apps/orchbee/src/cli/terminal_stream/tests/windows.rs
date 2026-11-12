use super::*;
use crate::invocation::Target;
use orchbee_client_server_protocol::{ViewerSnapshotReason, ViewerUnavailableReason};
use orchbee_local_ipc::ChildOutputReader;
use std::io::Read;

pub(in crate::cli::terminal_stream) fn line(output: &mut ChildOutputReader) -> io::Result<Value> {
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
                let schema: Value = serde_json::from_str(include_str!(
                    "../../schemas/orchbee.terminal.stream.json"
                ))?;
                let validator = jsonschema::draft202012::new(&schema).map_err(io::Error::other)?;
                validator
                    .validate(&emitted)
                    .map_err(|error| io::Error::other(error.to_string()))?;
                assert_eq!(emitted["schema"], SCHEMA);
                assert_eq!(emitted["version"], 1);
                if invalid {
                    assert!(matches!(
                        feedback.action(),
                        Action::NeedSnapshot {
                            reason: ViewerSnapshotReason::DecoderRejected,
                            ..
                        }
                    ));
                    assert_eq!(emitted["kind"], "reset");
                    assert_eq!(emitted["result"]["reason"], "decoder-rejected");
                } else {
                    assert!(matches!(feedback.action(), Action::FrameApplied { .. }));
                    assert_eq!(emitted["kind"], "frame");
                    assert_eq!(emitted["result"]["sequence"], sequence.to_string());
                    assert_eq!(
                        emitted["result"]["sourceWidth"],
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
    let client = LocalClient::connect(server.endpoint())?;
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
        let client = LocalClient::connect(server.endpoint())?;
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
