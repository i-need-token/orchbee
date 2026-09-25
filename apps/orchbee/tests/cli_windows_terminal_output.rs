//! Windows source read/wait lifecycle, strict sample and stdout evidence.
#![cfg(windows)]
#[path = "support/windows_agent_peer.rs"]
#[allow(dead_code)]
mod peer_support;
use orchbee_client_server_protocol::{
    self as obcs, ClientMessage, ServerMessage, TerminalViewerAction as Action,
    TerminalViewerOutcome as Outcome, TerminalViewerRequest, TerminalViewerResult, ViewerId,
    read_client_message, write_server_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_terminal_protocol::*;
use peer_support::*;
use serde_json::Value;
const OBTF: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin");
const ID: &str = "33333333333333333333333333333333";

#[path = "cli_windows_terminal_output/ansi.rs"]
mod ansi;

#[test]
fn source_export_catalog_and_usage_failure_are_offline_machine_contracts() {
    let fixture = Fixture::new();
    let output = fixture.run(&["api", "schema"]);
    assert_eq!(output.status.code(), Some(0));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.api.schema.json"),
    );
    assert_eq!(document["version"], 39);
    assert_eq!(document["result"]["schemas"].as_object().unwrap().len(), 71);
    assert_eq!(
        document["result"]["schemas"]["orchbee.terminal.output"]["$id"],
        "orchbee.terminal.output"
    );
    let output = fixture.run(&["terminal", "read", ID, "--plain", "--format=json"]);
    assert_eq!(output.status.code(), Some(2));
    let failure: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &failure,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
    assert_eq!(failure["failure"]["kind"], "invalid-usage");
}

fn request(stream: &mut LocalStream) -> TerminalViewerRequest {
    let ClientMessage::TerminalViewer(request) = read_client_message(stream).unwrap() else {
        panic!("read-only viewer request")
    };
    request
}
fn reply(stream: &mut LocalStream, request: &TerminalViewerRequest, outcome: Outcome) {
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .unwrap();
}
fn observed_frame(stream: &mut LocalStream) -> ViewerId {
    let route = inspect_terminal_frame_route(OBTF).unwrap();
    let viewer = ViewerId::parse(&route.attachment_id.to_text()).unwrap();
    let req = request(stream);
    assert!(
        matches!(req.action(), Action::AttachBinding { terminal_id, .. } if terminal_id.as_str()==ID)
    );
    reply(
        stream,
        &req,
        Outcome::Attached {
            attachment_id: viewer.clone(),
            host_epoch: obcs::HostEpoch::parse(&route.host_epoch.to_text()).unwrap(),
        },
    );
    next_frame(stream, &viewer, route.sequence.get(), route.revision);
    viewer
}
fn next_frame(stream: &mut LocalStream, viewer: &ViewerId, sequence: u64, revision: u64) {
    let req = request(stream);
    assert!(matches!(req.action(),Action::Poll { attachment_id } if attachment_id == viewer));
    let mut bytes = OBTF.to_vec();
    bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
    bytes[64..72].copy_from_slice(&revision.to_le_bytes());
    bytes[80..88].copy_from_slice(&10_u64.to_le_bytes());
    let route = inspect_terminal_frame_route(&bytes).unwrap();
    let frame = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        optional_message: false,
        payload_encoding: PayloadEncoding::TerminalFrame,
        correlation_id: None,
        host_epoch: Some(route.host_epoch),
        payload: &bytes,
    })
    .unwrap();
    reply(stream, &req, Outcome::Record(frame));
    let req = request(stream);
    assert!(
        matches!(req.action(),Action::FrameApplied { attachment_id, sequence, generation, revision } if attachment_id==viewer && *sequence==route.sequence && *generation==route.generation && *revision==route.revision)
    );
    reply(stream, &req, Outcome::FeedbackAccepted {});
}
fn detached(stream: &mut LocalStream, viewer: &ViewerId) {
    let req = request(stream);
    assert!(matches!(req.action(),Action::Detach { attachment_id } if attachment_id==viewer));
    reply(stream, &req, Outcome::Detached {});
    assert_closed(stream);
}
fn obrw_window(
    start_row: u64,
    total_rows: u64,
    columns: u16,
    sequence: u64,
    revision: u64,
    generation: u64,
    rows: &[(u8, &str)],
) -> Vec<u8> {
    const IDENTITY_ASCII: u8 = 0x04;
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    let mut body = Vec::new();
    let mut entry_count = 0_u32;
    for (flags, text) in rows {
        assert!(text.is_ascii() && text.len() <= usize::from(columns));
        body.push(flags | IDENTITY_ASCII);
        body.push(0); // prompt: none
        body.extend_from_slice(&[0; 2]); // reserved
        body.extend_from_slice(&(text.len() as u32).to_le_bytes());
        body.extend_from_slice(&0_u32.to_le_bytes()); // no explicit mappings
        body.extend_from_slice(text.as_bytes());
        entry_count += text.len() as u32;
    }
    let total = 112 + body.len();
    let mut record = Vec::with_capacity(total);
    record.extend_from_slice(b"OBRW");
    record.extend_from_slice(&[0, 0]); // format major/minor
    record.push(2); // detail: text
    record.push(0); // flags
    record.extend_from_slice(&(total as u32).to_le_bytes());
    record.extend_from_slice(&112_u16.to_le_bytes()); // header bytes
    record.extend_from_slice(&columns.to_le_bytes());
    record.extend_from_slice(route.host_epoch.as_bytes());
    record.extend_from_slice(route.attachment_id.as_bytes());
    record.extend_from_slice(&sequence.to_le_bytes());
    record.extend_from_slice(&revision.to_le_bytes());
    record.extend_from_slice(&generation.to_le_bytes());
    record.extend_from_slice(&total_rows.to_le_bytes());
    record.extend_from_slice(&start_row.to_le_bytes());
    record.extend_from_slice(&(rows.len() as u16).to_le_bytes()); // row count
    record.extend_from_slice(&[0; 2]); // reserved
    record.extend_from_slice(&entry_count.to_le_bytes());
    record.extend_from_slice(&(body.len() as u32).to_le_bytes()); // body bytes
    record.push(3); // format patch
    record.extend_from_slice(&0_u32.to_le_bytes()); // projection bytes: text only
    record.extend_from_slice(&0_u32.to_le_bytes()); // graphics bytes: text only
    record.extend_from_slice(&[0; 3]); // reserved tail
    record.extend_from_slice(&body);
    decode_terminal_text_row_window(&record).expect("generated window must decode");
    record
}

/// Wrap an OBRW record in its OBTP `RowsSnapshot` response envelope — the
/// byte payload a `record` outcome answering `ReadRows` carries.
fn history_record(obrw: &[u8]) -> Vec<u8> {
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    // The OBTP correlation is the Server-issued operation identity: opaque to
    // the CLI and checked by the routing hop, so any valid value suffices.
    let correlation = CorrelationId::from_bytes([0xdd; 16]).expect("correlation id");
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind: 46,
        payload_encoding: PayloadEncoding::RowWindow,
        host_epoch: Some(route.host_epoch),
        correlation_id: Some(correlation),
        optional_message: false,
        payload: obrw,
    })
    .expect("history record")
}

#[test]
fn windows_source_read_exports_plain_unicode_and_zero_without_history_requests() {
    for (source, lines, expected) in [
        ("visible", "2", "12345UDE\n^P.\n"),
        ("visible", "0", ""),
        ("recent", "0", ""),
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal", "read", ID, "--source", source, "--lines", lines, "--plain",
        ]);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
    }
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |mut stream| {
        bound_agent_reply(&mut stream, session_detail("sess-alpha"));
        let viewer = observed_frame(&mut stream);
        detached(&mut stream, &viewer);
    });
    let output = fixture.run(&[
        "agent",
        "read",
        "--name",
        "alpha",
        "--source=visible",
        "--plain",
    ]);
    handler.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "A中e\u{301}👩🏽\u{200d}💻x\n界\nH\nFBITVKOS\n12345UDE\n^P.\n"
    );
}

#[test]
fn windows_source_read_pins_merged_range_and_reports_exact_text() {
    for source in ["recent", "recent-unwrapped", "detection"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let req = request(&mut stream);
            let route = inspect_terminal_frame_route(OBTF).unwrap();
            let (start, count) = if source == "detection" {
                (10, 6)
            } else {
                (14, 2)
            };
            assert!(
                matches!(req.action(),Action::ReadRows { detail: orchbee_client_server_protocol::ViewerRowDetail::Text, attachment_id,start_row,row_count,revision,generation,request_sequence,maximum_bytes }
                if attachment_id==&viewer && *start_row==start && *row_count==count && *revision==route.revision && *generation==route.generation && request_sequence.get()==1 && *maximum_bytes==obcs::MAXIMUM_READ_ROWS_BYTES)
            );
            let rows = if source == "detection" {
                vec![
                    (0, "older"),
                    (0, "near"),
                    (1, "HEAD"),
                    (2, "TAIL"),
                    (0, ""),
                    (0, ""),
                ]
            } else {
                vec![(1, "HEAD"), (2, "TAIL")]
            };
            let window = obrw_window(
                start,
                16,
                8,
                1,
                route.revision,
                route.generation.get(),
                &rows,
            );
            reply(&mut stream, &req, Outcome::Record(history_record(&window)));
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal",
            "read",
            ID,
            "--source",
            source,
            "--lines",
            "2",
            "--format=json",
        ]);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.terminal.output.json"),
        );
        assert_eq!(doc["result"]["selection"]["source"], source);
        assert_eq!(
            doc["result"]["content"]["text"],
            if source == "recent-unwrapped" {
                "HEADTAIL\n"
            } else {
                "HEAD\nTAIL\n"
            }
        );
        assert_eq!(doc["result"]["observed"]["totalRows"], "16");
    }
}

#[test]
fn windows_source_read_stale_or_mismatched_rows_detach_without_retry() {
    for malformed in [false, true] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let req = request(&mut stream);
            assert!(matches!(
                req.action(),
                Action::ReadRows {
                    start_row: 14,
                    row_count: 2,
                    ..
                }
            ));
            let outcome = if malformed {
                let route = inspect_terminal_frame_route(OBTF).unwrap();
                Outcome::Record(history_record(&obrw_window(
                    13,
                    16,
                    8,
                    1,
                    route.revision,
                    route.generation.get(),
                    &[(0, "x"), (0, "y")],
                )))
            } else {
                Outcome::RowsRejected {
                    reason: obcs::ViewerRowsRejectedReason::StaleRevision,
                }
            };
            reply(&mut stream, &req, outcome);
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal",
            "read",
            ID,
            "--source=recent",
            "--lines=2",
            "--format=json",
        ]);
        handler.join().unwrap();
        assert_eq!(output.status.code(), Some(if malformed { 5 } else { 4 }));
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(
            doc["failure"]["kind"],
            if malformed {
                "protocol-invalid"
            } else {
                "service-unavailable"
            }
        );
    }
}

fn wait_document(output: std::process::Output, version: u8) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.terminal.wait-output.json"),
    );
    assert_eq!(doc["version"], version);
    doc
}

#[test]
fn windows_source_wait_matches_visible_frame_without_row_read_and_forever_has_no_total_ceiling() {
    for forever in [false, true] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            detached(&mut stream, &viewer);
        });
        let mut args = vec![
            "terminal",
            "wait-output",
            ID,
            "--source=visible",
            "--lines=2",
            "--match=^P.",
            "--format=json",
        ];
        if forever {
            args.push("--forever");
        }
        let doc = wait_document(fixture.run(&args), 3);
        handler.join().unwrap();
        let result = &doc["result"];
        assert_eq!(
            result["selection"],
            serde_json::json!({"source":"visible","lines":2})
        );
        assert_eq!(result["match"]["rowStart"], "15");
        assert_eq!(result["observed"]["requestedRows"], 6);
        assert_eq!(result["observed"]["completedObservations"], 1);
        assert_eq!(result["observed"]["windowTruncated"], false);
        assert_eq!(
            result["observed"]["timeoutMs"],
            if forever {
                Value::Null
            } else {
                Value::from(30_000)
            }
        );
        // Version-specific constraints cannot be bypassed by re-labelling v3.
        let schema: Value = serde_json::from_str(include_str!(
            "../src/cli/schemas/orchbee.terminal.wait-output.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let mut invalid = doc.clone();
        invalid["version"] = Value::from(1);
        assert!(!validator.is_valid(&invalid));
        invalid = doc;
        invalid["result"]
            .as_object_mut()
            .unwrap()
            .remove("selection");
        assert!(!validator.is_valid(&invalid));
    }
}

#[test]
fn windows_source_wait_repinning_never_counts_stale_samples_or_repeats_row_sequence() {
    for source in ["recent", "recent-unwrapped", "detection"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let route = inspect_terminal_frame_route(OBTF).unwrap();
            for (index, reason) in [
                obcs::ViewerRowsRejectedReason::StaleRevision,
                obcs::ViewerRowsRejectedReason::StaleGeneration,
                obcs::ViewerRowsRejectedReason::HistoryEvicted,
            ]
            .into_iter()
            .enumerate()
            {
                let req = request(&mut stream);
                assert!(
                    matches!(req.action(), Action::ReadRows { request_sequence, revision, .. } if request_sequence.get() == index as u64 + 1 && *revision == route.revision + index as u64)
                );
                reply(&mut stream, &req, Outcome::RowsRejected { reason });
                next_frame(
                    &mut stream,
                    &viewer,
                    route.sequence.get() + index as u64 + 1,
                    route.revision + index as u64 + 1,
                );
            }
            let req = request(&mut stream);
            let (start, rows) = if source == "detection" {
                (
                    10,
                    vec![
                        (0, "older"),
                        (0, "near"),
                        (1, "HEAD"),
                        (2, "TAIL"),
                        (0, ""),
                        (0, ""),
                    ],
                )
            } else {
                (14, vec![(1, "HEAD"), (2, "TAIL")])
            };
            assert!(
                matches!(req.action(), Action::ReadRows { start_row, row_count, revision, request_sequence, .. } if *start_row == start && usize::from(*row_count) == rows.len() && *revision == route.revision + 3 && request_sequence.get() == 4)
            );
            let window = obrw_window(
                start,
                16,
                8,
                4,
                route.revision + 3,
                route.generation.get(),
                &rows,
            );
            reply(&mut stream, &req, Outcome::Record(history_record(&window)));
            detached(&mut stream, &viewer);
        });
        let pattern = if source == "detection" {
            "^HEAD$"
        } else {
            "^HEADTAIL$"
        };
        let doc = wait_document(
            fixture.run(&[
                "terminal",
                "wait-output",
                ID,
                "--source",
                source,
                "--lines=2",
                "--regex",
                pattern,
                "--format=json",
            ]),
            3,
        );
        handler.join().unwrap();
        assert_eq!(doc["result"]["observed"]["completedObservations"], 1);
        assert_eq!(doc["result"]["observed"]["windowTruncated"], false);
        assert_eq!(doc["result"]["match"]["endsMidLine"], source == "detection");
        assert_eq!(
            doc["result"]["match"]["matched"],
            if source == "detection" {
                "HEAD"
            } else {
                "HEADTAIL"
            }
        );
    }
}

#[test]
fn windows_zero_source_wait_does_not_match_empty_regex_and_reports_one_complete_sample() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |mut stream| {
        bound_snapshot_reply(&mut stream);
        let viewer = observed_frame(&mut stream);
        while let Ok(ClientMessage::TerminalViewer(req)) = read_client_message(&mut stream) {
            assert!(
                matches!(req.action(), Action::Poll { attachment_id } if attachment_id == &viewer)
            );
            reply(&mut stream, &req, Outcome::Empty {});
        }
    });
    let output = fixture.run(&[
        "terminal",
        "wait-output",
        ID,
        "--source=visible",
        "--lines=0",
        "--regex=^$",
        "--timeout=1500",
        "--format=json",
    ]);
    handler.join().unwrap();
    assert_eq!(output.status.code(), Some(7));
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
    assert_eq!(doc["failure"]["kind"], "timeout");
    assert_eq!(
        doc["failure"]["details"]["observed"]["completedObservations"],
        1
    );
    assert_eq!(doc["failure"]["details"]["cleanup"], "requested");
}

#[test]
fn windows_legacy_wait_keeps_v1_v2_without_source_selection() {
    for regex in [false, true] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let req = request(&mut stream);
            let route = inspect_terminal_frame_route(OBTF).unwrap();
            assert!(matches!(
                req.action(),
                Action::ReadRows {
                    start_row: 0,
                    row_count: 16,
                    ..
                }
            ));
            let rows = vec![(0, "hit"); 16];
            reply(
                &mut stream,
                &req,
                Outcome::Record(history_record(&obrw_window(
                    0,
                    16,
                    8,
                    1,
                    route.revision,
                    route.generation.get(),
                    &rows,
                ))),
            );
            detached(&mut stream, &viewer);
        });
        let doc = wait_document(
            fixture.run(&[
                "terminal",
                "wait-output",
                ID,
                if regex { "--regex=hit" } else { "--match=hit" },
                "--format=json",
            ]),
            if regex { 2 } else { 1 },
        );
        handler.join().unwrap();
        assert!(doc["result"].get("selection").is_none());
        assert_eq!(doc["result"]["observed"]["timeoutMs"], 30_000);
        assert_eq!(doc["result"]["observed"]["requestedRows"], 1000);
    }
}

#[test]
fn windows_forever_wait_keeps_snapshot_io_bounded_and_preserves_transport_failure() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |mut stream| {
        assert!(matches!(
            orchbee_workbench_protocol::read_workbench_request(&mut stream).unwrap(),
            orchbee_workbench_protocol::WorkbenchRequest::SnapshotRead(_)
        ));
        // Hold the response: an unbounded total must still retire this I/O.
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "terminal",
        "wait-output",
        ID,
        "--forever",
        "--match=x",
        "--format=json",
    ]);
    handler.join().unwrap();
    assert_eq!(output.status.code(), Some(5));
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
    assert_eq!(doc["failure"]["kind"], "transport-loss");
    assert_eq!(doc["failure"]["details"]["stage"], "snapshot");
    assert!(doc["failure"]["details"].get("timeoutMs").is_none());
}

#[test]
fn windows_source_wait_mismatched_window_fails_with_confirmed_cleanup() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |mut stream| {
        bound_snapshot_reply(&mut stream);
        let viewer = observed_frame(&mut stream);
        let req = request(&mut stream);
        assert!(matches!(
            req.action(),
            Action::ReadRows {
                start_row: 14,
                row_count: 2,
                ..
            }
        ));
        let route = inspect_terminal_frame_route(OBTF).unwrap();
        reply(
            &mut stream,
            &req,
            Outcome::Record(history_record(&obrw_window(
                13,
                16,
                8,
                1,
                route.revision,
                route.generation.get(),
                &[(0, "hit"), (0, "tail")],
            ))),
        );
        detached(&mut stream, &viewer);
    });
    let output = fixture.run(&[
        "terminal",
        "wait-output",
        ID,
        "--source=recent",
        "--lines=2",
        "--match=hit",
        "--format=json",
    ]);
    handler.join().unwrap();
    assert_eq!(output.status.code(), Some(5));
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
    assert_eq!(doc["failure"]["kind"], "protocol-invalid");
    assert_eq!(doc["failure"]["details"]["cleanup"], "confirmed");
    assert!(doc["failure"]["details"].get("matchObserved").is_none());
}
