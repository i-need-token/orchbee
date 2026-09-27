use super::*;
use crate::{
    cli::terminal_stream::tests::{COHORT, SNAPSHOT, handshake, line, reply, request},
    local_client::test_server::FakeServer,
};
use orchbee_local_ipc::ChildOutputReader;
use orchbee_terminal_protocol::{
    CorrelationId, Envelope, HistoryAnchorStatus, HistoryChanged, HistoryControl,
    decode_terminal_text_frame, encode_history_control, encode_transport_envelope,
};
use std::{io, num::NonZeroU64};
const ROWS: &[u8] = include_bytes!(
    "../../../../../../packages/terminal-protocol/fixtures/terminal-row-window-v0.0.3.bin"
);
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn history_demands_exact_windows_flush_before_ack_and_restore_before_live() -> TestResult {
    let window = decode_terminal_text_row_window(ROWS)?;
    let (mut output, child_output) = ChildOutputReader::pipe()?;
    let epoch = window.host_epoch;
    let server = FakeServer::start(move |mut stream| {
        handshake(&mut stream)?;
        for sequence in [8_u64, 9] {
            let enter = request(&mut stream)?;
            assert!(matches!(
                enter.action(),
                Action::SetHistoryDemand {
                    demand: HistoryDemand::ViewingHistory,
                    ..
                }
            ));
            reply(&mut stream, enter, Outcome::FeedbackAccepted {})?;
            let pause = request(&mut stream)?;
            assert!(matches!(
                pause.action(),
                Action::SetProjectionDemand {
                    demand: ProjectionDemand::Paused,
                    ..
                }
            ));
            reply(&mut stream, pause, Outcome::FeedbackAccepted {})?;
            let read = request(&mut stream)?;
            assert!(
                matches!(read.action(), Action::ReadViewportRows { request_sequence, generation, revision: 42, row_count: 2, .. }
                if request_sequence.get() == sequence && generation.get() == 3)
            );
            let mut bytes = ROWS.to_vec();
            bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
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
            reply(&mut stream, read, Outcome::Record(bytes))?;
            let ack = request(&mut stream)?;
            assert!(
                matches!(ack.action(), Action::RowsApplied { request_sequence, generation, revision: 42, .. }
                if request_sequence.get() == sequence && generation.get() == 3)
            );
            let emitted = line(&mut output)?;
            assert_eq!(emitted["kind"], "history");
            assert_eq!(emitted["sequence"], sequence.to_string());
            assert_eq!(emitted["startRow"], "100");
            assert_eq!(emitted["totalRows"], "125");
            let schema: Value =
                serde_json::from_str(include_str!("../../schemas/orchbee.terminal.stream.json"))?;
            jsonschema::draft202012::new(&schema)
                .map_err(io::Error::other)?
                .validate(&emitted)
                .map_err(|e| io::Error::other(e.to_string()))?;
            reply(&mut stream, ack, Outcome::FeedbackAccepted {})?;
            let bottom = request(&mut stream)?;
            assert!(matches!(
                bottom.action(),
                Action::SetHistoryDemand {
                    demand: HistoryDemand::FollowingBottom,
                    ..
                }
            ));
            reply(&mut stream, bottom, Outcome::FeedbackAccepted {})?;
            let active = request(&mut stream)?;
            assert!(matches!(
                active.action(),
                Action::SetProjectionDemand {
                    demand: ProjectionDemand::Active,
                    ..
                }
            ));
            reply(&mut stream, active, Outcome::FeedbackAccepted {})?;
        }
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let viewer = ViewerId::parse(&window.attachment_id.to_text())?;
    let gate = Gate {
        cancelled: &|| false,
        wake: None,
        total: Some(Instant::now() + STEP),
    };
    let mut relay = relay::Relay::with_output(child_output)?;
    let mut frame = decode_terminal_text_frame(SNAPSHOT)?;
    frame.geometry.columns = window.columns;
    frame.geometry.rows = 2;
    frame.scrollback_rows = window.total_rows - 2;
    let projection = TerminalProjection::new(window.attachment_id, window.host_epoch);
    let mut history = History {
        sequence: 7,
        ..History::default()
    };
    for sequence in [1, 2] {
        history
            .scroll(23, &frame, &mut client, &viewer, &gate)
            .unwrap();
        let changed = encode_history_control(&HistoryControl::HistoryChanged(HistoryChanged {
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
        history.event(&record, &projection).unwrap();
        assert_eq!(
            history.event(&record, &projection).unwrap_err().kind,
            FailureKind::ProtocolInvalid
        );
        history
            .advance(&mut client, &viewer, &gate, Some(&mut relay))
            .unwrap();
        assert_eq!(history.record().unwrap()["kind"], "history");
        history
            .advance(&mut client, &viewer, &gate, Some(&mut relay))
            .unwrap();
        history.resume(&mut client, &viewer, &gate).unwrap();
        assert!(
            history.active(),
            "recovery must await a subsequent live frame"
        );
        history.frame().unwrap();
        assert!(!history.active());
    }
    drop(relay);
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn absent_history_does_not_pause_live_projection_or_acquire_history_demand() -> TestResult {
    for (alternate, retained) in [(true, 100), (false, 0)] {
        let server = FakeServer::start(|mut stream| {
            handshake(&mut stream)?;
            assert!(
                request(&mut stream).is_err(),
                "no history operation may be sent"
            );
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let mut frame = decode_terminal_text_frame(SNAPSHOT)?;
        frame.alternate_screen = alternate;
        frame.scrollback_rows = retained;
        let viewer = ViewerId::parse(&frame.route.attachment_id.to_text())?;
        let gate = Gate {
            cancelled: &|| false,
            wake: None,
            total: Some(Instant::now() + STEP),
        };
        let mut history = History::default();
        history
            .scroll(3, &frame, &mut client, &viewer, &gate)
            .unwrap();
        assert!(!history.active());
        drop(client);
        server.finish()?;
    }
    Ok(())
}
