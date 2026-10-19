//! Actual Windows processes prove styled sources, strict records and cleanup.
use super::*;

const RENDER: &[u8] = include_bytes!(
    "../../../../packages/terminal-protocol/fixtures/ghostty-render-row-window-v0.0.3.bin"
);
const SCHEMA: &str = include_str!("../../src/cli/schemas/orchbee.terminal.output.json");

fn render_window() -> Vec<u8> {
    let route = inspect_terminal_frame_route(OBTF).unwrap();
    let mut record = RENDER.to_vec();
    record[16..32].copy_from_slice(route.host_epoch.as_bytes());
    record[32..48].copy_from_slice(route.attachment_id.as_bytes());
    record[48..56].copy_from_slice(&1_u64.to_le_bytes());
    record[56..64].copy_from_slice(&route.revision.to_le_bytes());
    record[64..72].copy_from_slice(&route.generation.get().to_le_bytes());
    record[72..80].copy_from_slice(&16_u64.to_le_bytes());
    record[80..88].copy_from_slice(&10_u64.to_le_bytes());
    let projection = &mut record[112..];
    projection[24..40].copy_from_slice(route.host_epoch.as_bytes());
    projection[40..56].copy_from_slice(route.attachment_id.as_bytes());
    projection[56..64].copy_from_slice(&1_u64.to_le_bytes());
    projection[72..80].copy_from_slice(&route.revision.to_le_bytes());
    projection[80..88].copy_from_slice(&route.generation.get().to_le_bytes());
    projection[88..96].copy_from_slice(&10_u64.to_le_bytes());
    // ConPTY may know the grid while the character pixel dimensions are unknown.
    projection[100..104].fill(0);
    decode_terminal_render_row_window(&record).expect("valid repinned Host fixture");
    record
}

fn render_request(stream: &mut LocalStream, viewer: &ViewerId) -> TerminalViewerRequest {
    let req = request(stream);
    let route = inspect_terminal_frame_route(OBTF).unwrap();
    assert!(matches!(req.action(), Action::ReadRows {
        detail: obcs::ViewerRowDetail::Render, attachment_id, start_row: 10, row_count: 6,
        revision, generation, request_sequence, maximum_bytes,
    } if attachment_id == viewer && *revision == route.revision && *generation == route.generation
        && request_sequence.get() == 1 && *maximum_bytes == obcs::MAXIMUM_READ_ROWS_BYTES));
    req
}

#[test]
fn ansi_visible_agent_json_and_zero_need_no_render_request() {
    for zero in [false, true] {
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
            "--ansi",
            "--source=visible",
            "--lines",
            if zero { "0" } else { "6" },
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
        validate(&doc, SCHEMA);
        assert_eq!(doc["version"], 2);
        assert_eq!(doc["result"]["encoding"], "ansi");
        let text = doc["result"]["content"]["text"].as_str().unwrap();
        if zero {
            assert!(text.is_empty());
        } else {
            assert!(text.contains("中") && text.contains("👩🏽‍💻"));
            assert!(text.contains("\x1b[38;2;") && text.ends_with("\x1b[0m\n"));
        }
        let mut absent = doc.clone();
        absent["result"].as_object_mut().unwrap().remove("encoding");
        let schema: Value = serde_json::from_str(SCHEMA).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(!validator.is_valid(&absent));
        let mut v1 = doc;
        v1["version"] = Value::from(1);
        assert!(!validator.is_valid(&v1));
    }
}

#[test]
fn ansi_recent_and_unwrapped_json_match_the_actual_native_stdout_pipe() {
    for source in ["recent", "recent-unwrapped"] {
        let mut text = None;
        for json in [true, false] {
            let fixture = Fixture::new();
            let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
            let handler = peer(&fixture, listener, |mut stream| {
                bound_snapshot_reply(&mut stream);
                let viewer = observed_frame(&mut stream);
                let req = render_request(&mut stream, &viewer);
                reply(
                    &mut stream,
                    &req,
                    Outcome::Record(history_record(&render_window())),
                );
                detached(&mut stream, &viewer);
            });
            let output = fixture.run(&[
                "terminal",
                "read",
                ID,
                "--ansi",
                "--source",
                source,
                "--lines=6",
                "--format",
                if json { "json" } else { "text" },
            ]);
            handler.join().unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if json {
                let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
                validate(&doc, SCHEMA);
                assert_eq!(doc["result"]["observed"]["windowStart"], "10");
                assert_eq!(doc["result"]["content"]["physicalRows"], 6);
                let value = doc["result"]["content"]["text"].as_str().unwrap();
                assert!(value.contains('中') && value.contains("\x1b[38;2;"));
                text = Some(value.as_bytes().to_vec());
            } else {
                assert_eq!(Some(output.stdout), text);
            }
        }
    }
}

#[test]
fn ansi_malformed_or_rejected_render_never_retries_or_leaks_partial_text() {
    for fault in ["detail", "outer", "nested", "graphics", "limit"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let req = render_request(&mut stream, &viewer);
            let mut window = render_window();
            let outcome = match fault {
                "limit" => Outcome::RowsRejected {
                    reason: obcs::ViewerRowsRejectedReason::LimitExceeded,
                },
                _ => {
                    match fault {
                        "detail" => {
                            let route = inspect_terminal_frame_route(OBTF).unwrap();
                            window = obrw_window(
                                10,
                                16,
                                8,
                                1,
                                route.revision,
                                route.generation.get(),
                                &[(0, "leak"); 6],
                            );
                        }
                        "outer" => window[48] = 2,
                        "nested" => window[112 + 72] ^= 1,
                        "graphics" => {
                            let start = window.len() - 40;
                            window[start + 7] = 2;
                        }
                        _ => unreachable!(),
                    }
                    Outcome::Record(history_record(&window))
                }
            };
            reply(&mut stream, &req, outcome);
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&["terminal", "read", ID, "--ansi", "--lines=6"]);
        handler.join().unwrap();
        assert!(!output.status.success(), "{fault}");
        assert!(output.stdout.is_empty(), "partial output for {fault}");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn ansi_detection_keeps_the_reference_plain_text_source() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |mut stream| {
        bound_snapshot_reply(&mut stream);
        let viewer = observed_frame(&mut stream);
        let req = request(&mut stream);
        assert!(matches!(
            req.action(),
            Action::ReadRows {
                detail: obcs::ViewerRowDetail::Text,
                start_row: 10,
                row_count: 6,
                ..
            }
        ));
        let route = inspect_terminal_frame_route(OBTF).unwrap();
        let window = obrw_window(
            10,
            16,
            8,
            1,
            route.revision,
            route.generation.get(),
            &[
                (0, "n\x1b]x\x07"),
                (0, "cursor"),
                (0, ""),
                (0, ""),
                (0, ""),
                (0, ""),
            ],
        );
        reply(&mut stream, &req, Outcome::Record(history_record(&window)));
        detached(&mut stream, &viewer);
    });
    let output = fixture.run(&[
        "terminal",
        "read",
        ID,
        "--ansi",
        "--source=detection",
        "--format=json",
    ]);
    handler.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(&doc, SCHEMA);
    assert_eq!(doc["version"], 2);
    assert_eq!(
        doc["result"]["content"]["text"],
        "n\\u{1b}]x\\u{7}\ncursor\n"
    );
}
