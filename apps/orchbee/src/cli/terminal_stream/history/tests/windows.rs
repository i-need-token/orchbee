use super::*;
use crate::cli::terminal_stream::{STEP, relay};
use crate::{
    cli::terminal_stream::tests::{COHORT, SNAPSHOT, handshake, line, reply, request},
    local_client::tests::server::FakeServer,
};
use orchbee_local_ipc::ChildOutputReader;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HistoryAnchorStatus, HistoryChanged,
    HistoryControl, MessageRole, PayloadEncoding, decode_terminal_render_row_window,
    decode_terminal_text_frame, encode_history_control, encode_transport_envelope,
};
use std::{io, num::NonZeroU64};
const ROWS: &[u8] = include_bytes!(
    "../../../../../../../schemas/terminal/v1/fixtures/ghostty-render-row-window.bin"
);
fn rows() -> Vec<u8> {
    let mut bytes = ROWS.to_vec();
    let frame = decode_terminal_text_frame(SNAPSHOT).unwrap();
    bytes[16..32].copy_from_slice(frame.route.host_epoch.as_bytes());
    bytes[32..48].copy_from_slice(frame.route.attachment_id.as_bytes());
    bytes[112 + 24..112 + 40].copy_from_slice(frame.route.host_epoch.as_bytes());
    bytes[112 + 40..112 + 56].copy_from_slice(frame.route.attachment_id.as_bytes());
    bytes
}
type TestResult = Result<(), Box<dyn std::error::Error>>;

use serde_json::Value;
impl History {
    pub(super) fn record(&self) -> Result<Value, Failure> {
        self.core.as_ref().map_or_else(
            || Ok(json!({"kind":"reset","reason":"history-recovering"})),
            surface,
        )
    }
}
fn surface(core: &TerminalViewport) -> Result<Value, Failure> {
    let ViewportSurface::Window(window) = core.surface() else {
        return Ok(json!({"kind":"reset","reason":"history-pending"}));
    };
    let (columns, rows) = core.geometry();
    crate::cli::terminal_stream::frame::history(window, columns, rows)
}

#[test]
fn history_demands_exact_windows_flush_before_ack_and_restore_before_live() -> TestResult {
    struct TimedRelay {
        relay: relay::Relay,
        started: Instant,
    }
    impl Display for TimedRelay {
        fn event(&mut self, record: Value, gate: &Gate<'_>) -> Result<(), Failure> {
            let flushing = Instant::now();
            eprintln!(
                "history client relay-flush kind={} sequence={} elapsed={:?}",
                record["kind"],
                record["sequence"],
                self.started.elapsed()
            );
            let result = self.relay.event(record, gate);
            eprintln!(
                "history client relay-flush-returned elapsed={:?} flush={:?} result={result:?}",
                self.started.elapsed(),
                flushing.elapsed()
            );
            result
        }
    }
    let started = Instant::now();
    let window = decode_terminal_render_row_window(&rows())?.route();
    let (mut output, child_output) = ChildOutputReader::pipe()?;
    let epoch = window.host_epoch;
    let server = FakeServer::start(move |mut stream| {
        let mut stage = "handshake";
        let mut stage_started = Instant::now();
        let mut current_sequence = 0;
        let result = (|| {
            handshake(&mut stream)?;
            for sequence in [8_u64, 9] {
                current_sequence = sequence;
                stage = "read-history-demand";
                stage_started = Instant::now();
                let enter = request(&mut stream)?;
                assert!(matches!(
                    enter.action(),
                    Action::SetHistoryDemand {
                        demand: HistoryDemand::ViewingHistory,
                        ..
                    }
                ));
                stage = "reply-history-demand";
                stage_started = Instant::now();
                reply(&mut stream, enter, Outcome::FeedbackAccepted {})?;
                stage = "read-projection-pause";
                stage_started = Instant::now();
                let pause = request(&mut stream)?;
                assert!(matches!(
                    pause.action(),
                    Action::SetProjectionDemand {
                        demand: ProjectionDemand::Paused,
                        ..
                    }
                ));
                stage = "reply-projection-pause";
                stage_started = Instant::now();
                reply(&mut stream, pause, Outcome::FeedbackAccepted {})?;
                stage = "read-viewport-request";
                stage_started = Instant::now();
                let read = request(&mut stream)?;
                assert!(
                    matches!(read.action(), Action::ReadViewportRows { request_sequence, generation, detail: orchbee_client_server_protocol::ViewerRowDetail::Render, revision: 11, row_count: 6, .. }
                if request_sequence.get() == sequence && generation.get() == 13)
                );
                let mut bytes = rows();
                bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
                bytes[112 + 56..112 + 64].copy_from_slice(&sequence.to_le_bytes());
                let bytes = encode_transport_envelope(&Envelope {
                    protocol: CURRENT_PROTOCOL_VERSION,
                    role: MessageRole::Response,
                    kind: 46,
                    payload_encoding: PayloadEncoding::RowWindow,
                    host_epoch: Some(epoch),
                    correlation_id: Some(CorrelationId::from_bytes([3; 16]).unwrap()),
                    optional_message: false,
                    payload: &bytes,
                })
                .map_err(io::Error::other)?;
                stage = "reply-rows";
                stage_started = Instant::now();
                reply(&mut stream, read, Outcome::Record(bytes))?;
                // Use the client budget for this peer's per-read guard.
                stream.set_read_timeout(Some(STEP))?;
                eprintln!(
                    "history server sequence={sequence} rows-replied elapsed={:?}",
                    started.elapsed()
                );
                stage = "await-flush-rows-applied";
                stage_started = Instant::now();
                let ack = request(&mut stream)?;
                eprintln!(
                    "history server sequence={sequence} rows-applied-received elapsed={:?} wait={:?}",
                    started.elapsed(),
                    stage_started.elapsed()
                );
                assert!(
                    matches!(ack.action(), Action::RowsApplied { request_sequence, generation, revision: 11, .. }
                if request_sequence.get() == sequence && generation.get() == 13)
                );
                stage = "read-flushed-output";
                stage_started = Instant::now();
                let emitted = line(&mut output)?;
                assert_eq!(emitted["kind"], "history");
                assert_eq!(emitted["sequence"], sequence.to_string());
                assert_eq!(emitted["startRow"], "0");
                assert_eq!(emitted["totalRows"], "6");
                stage = "validate-output";
                stage_started = Instant::now();
                let schema: Value = serde_json::from_str(include_str!(
                    "../../../schemas/orchbee.terminal.stream.json"
                ))?;
                jsonschema::draft202012::new(&schema)
                    .map_err(io::Error::other)?
                    .validate(&emitted)
                    .map_err(|e| io::Error::other(e.to_string()))?;
                stage = "reply-rows-applied";
                stage_started = Instant::now();
                reply(&mut stream, ack, Outcome::FeedbackAccepted {})?;
                stage = "read-following-bottom";
                stage_started = Instant::now();
                let bottom = request(&mut stream)?;
                assert!(matches!(
                    bottom.action(),
                    Action::SetHistoryDemand {
                        demand: HistoryDemand::FollowingBottom,
                        ..
                    }
                ));
                stage = "reply-following-bottom";
                stage_started = Instant::now();
                reply(&mut stream, bottom, Outcome::FeedbackAccepted {})?;
                stage = "read-projection-active";
                stage_started = Instant::now();
                let active = request(&mut stream)?;
                assert!(matches!(
                    active.action(),
                    Action::SetProjectionDemand {
                        demand: ProjectionDemand::Active,
                        ..
                    }
                ));
                stage = "reply-projection-active";
                stage_started = Instant::now();
                reply(&mut stream, active, Outcome::FeedbackAccepted {})?;
            }
            Ok(())
        })();
        eprintln!(
            "history server sequence={current_sequence} stage={stage} elapsed={:?} stage_elapsed={:?} result={result:?}",
            started.elapsed(),
            stage_started.elapsed()
        );
        result
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    eprintln!("history client connected elapsed={:?}", started.elapsed());
    let viewer = ViewerId::parse(&window.attachment_id.to_text())?;
    let gate = Gate {
        cancelled: &|| false,
        wake: None,
        total: Some(Instant::now() + STEP),
    };
    let relay_started = Instant::now();
    let mut relay = TimedRelay {
        relay: relay::Relay::with_output(child_output)?,
        started,
    };
    eprintln!(
        "history client relay-spawned elapsed={:?} spawn={:?}",
        started.elapsed(),
        relay_started.elapsed()
    );
    let mut frame = decode_terminal_text_frame(SNAPSHOT)?;
    frame.route.attachment_id = window.attachment_id;
    frame.route.host_epoch = window.host_epoch;
    frame.geometry.columns = window.columns;
    frame.geometry.rows = 6;
    frame.scrollback_rows = 1;
    let projection = TerminalProjection::new(window.attachment_id, window.host_epoch);
    let mut history = History {
        sequence: 7,
        ..History::default()
    };
    let mut client_stage = "scroll";
    let mut client_stage_started = Instant::now();
    let mut current_sequence = 0;
    // Join and report the peer before propagating the original error or panic.
    let client_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> TestResult {
        for sequence in [1, 2] {
            current_sequence = sequence;
            client_stage = "scroll";
            client_stage_started = Instant::now();
            history
                .scroll(23, &frame, &mut client, &viewer, &gate)
                .unwrap();
            let changed =
                encode_history_control(&HistoryControl::HistoryChanged(HistoryChanged {
                    attachment_id: window.attachment_id,
                    sequence: NonZeroU64::new(sequence).unwrap(),
                    revision: window.revision,
                    generation: window.generation,
                    total_rows: window.total_rows,
                    anchor_status: HistoryAnchorStatus::None,
                }))?;
            let record = encode_transport_envelope(&Envelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                role: changed.role,
                kind: changed.kind,
                payload_encoding: PayloadEncoding::ControlJson,
                host_epoch: Some(window.host_epoch),
                correlation_id: None,
                optional_message: false,
                payload: &changed.payload,
            })?;
            client_stage = "history-event";
            client_stage_started = Instant::now();
            history.event(&record, &projection).unwrap();
            assert_eq!(
                history.event(&record, &projection).unwrap_err().kind,
                FailureKind::ProtocolInvalid
            );
            client_stage = "read-rows";
            client_stage_started = Instant::now();
            history
                .advance(&mut client, &viewer, &gate, Some(&mut relay))
                .unwrap();
            assert_eq!(history.record().unwrap()["kind"], "history");
            client_stage = "flush-and-ack-rows";
            client_stage_started = Instant::now();
            history
                .advance(&mut client, &viewer, &gate, Some(&mut relay))
                .unwrap();
            client_stage = "resume-live";
            client_stage_started = Instant::now();
            history.resume(&mut client, &viewer, &gate).unwrap();
            assert!(
                history.active(),
                "recovery must await a subsequent live frame"
            );
            history.frame().unwrap();
            assert!(!history.active());
        }
        Ok(())
    }));
    match &client_result {
        Ok(result) => eprintln!(
            "history client sequence={current_sequence} stage={client_stage} elapsed={:?} stage_elapsed={:?} result={result:?}",
            started.elapsed(),
            client_stage_started.elapsed()
        ),
        Err(_) => eprintln!(
            "history client sequence={current_sequence} stage={client_stage} elapsed={:?} stage_elapsed={:?} panicked (original diagnostic above)",
            started.elapsed(),
            client_stage_started.elapsed()
        ),
    }
    drop(relay);
    drop(client);
    let server_result = server.finish();
    eprintln!(
        "history finished elapsed={:?} server={server_result:?}",
        started.elapsed()
    );
    match client_result {
        Ok(result) => result?,
        Err(panic) => std::panic::resume_unwind(panic),
    }
    server_result?;
    Ok(())
}
