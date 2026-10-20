//! Actual Windows binaries prove copy reads, strict batching and viewer release.
use super::*;

#[test]
fn copy_empty_search_and_scan_limit_need_no_rows_and_always_release_the_viewer() {
    for empty in [true, false] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = copy_frame(&mut stream, if empty { 10 } else { 70000 }, false);
            detached(&mut stream, &viewer);
        });
        let revision = inspect_terminal_frame_route(OBTF)
            .unwrap()
            .revision
            .to_string();
        let output = fixture.run(&[
            "terminal",
            "copy-search",
            ID,
            "--cursor",
            "0:0",
            "--query",
            if empty { "" } else { "x" },
            "--search-direction",
            "forward",
            "--content-revision",
            &revision,
            "--format",
            "json",
        ]);
        handler.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if empty {
            assert_eq!(output.status.code(), Some(0));
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.terminal.copy-search.json"),
            );
            assert_eq!(doc["result"]["total"], "0");
            assert!(doc["result"]["current"].is_null());
            assert!(doc["result"]["currentGlobal"].is_null());
        } else {
            assert_eq!(output.status.code(), Some(3));
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
            );
            assert_eq!(doc["failure"]["details"]["reason"], "limit-exceeded");
            assert!(doc.get("result").is_none());
        }
    }
}

#[test]
fn copy_motion_cli_returns_nearby_targets_in_long_history_and_extends_unfinished_words() {
    for case in ["near", "forward", "backward"] {
        let extend = case != "near";
        let backward = case == "backward";
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let frame = decode_terminal_text_frame(OBTF).unwrap();
            let total = if extend {
                128
            } else {
                70000 + u64::from(frame.geometry.rows)
            };
            let viewer = copy_frame(&mut stream, total - u64::from(frame.geometry.rows), true);
            let full = "x".repeat(usize::from(frame.geometry.columns));
            let chunks = if backward {
                [(64, 1_u64), (0, 2)]
            } else {
                [(0, 1_u64), (64, 2)]
            };
            for (start, sequence) in chunks {
                let req = request(&mut stream);
                assert!(
                    matches!(req.action(),Action::ReadRows {start_row,row_count,request_sequence,revision,generation,..}
                    if *start_row==start && *row_count==64 && request_sequence.get()==sequence
                    && *revision==frame.route.revision && *generation==frame.route.generation)
                );
                let mut rows = vec![(0, ""); 64];
                if backward {
                    if start == 64 {
                        rows.fill((3, &full));
                        rows[63] = (2, "xx");
                    } else {
                        rows[63] = (1, &full);
                    }
                } else if !extend {
                    rows[0] = (0, "a.b");
                } else if start == 0 {
                    rows[0] = (0, "x");
                    rows[63] = (1, &full);
                } else {
                    rows[0] = (2, "xx");
                }
                let body = obrw_window(
                    start,
                    total,
                    frame.geometry.columns,
                    sequence,
                    frame.route.revision,
                    frame.route.generation.get(),
                    &rows,
                );
                reply(&mut stream, &req, Outcome::Record(history_record(&body)));
                if !extend {
                    break;
                }
            }
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal",
            "copy-motion",
            ID,
            "--cursor",
            if backward { "127:1" } else { "0:0" },
            "--motion",
            if backward {
                "previous-word-start"
            } else if extend {
                "next-word-end"
            } else {
                "next-word-start"
            },
            "--format",
            "json",
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
            include_str!("../../src/cli/schemas/orchbee.terminal.copy-motion.json"),
        );
        assert_eq!(
            doc["result"]["cursor"],
            if backward {
                serde_json::json!({"row":63,"col":0})
            } else if extend {
                serde_json::json!({"row":64,"col":1})
            } else {
                serde_json::json!({"row":0,"col":1})
            }
        );
    }
}

fn copy_frame(stream: &mut LocalStream, history: u64, acknowledge: bool) -> ViewerId {
    let route = inspect_terminal_frame_route(OBTF).unwrap();
    let viewer = ViewerId::parse(&route.attachment_id.to_text()).unwrap();
    let req = request(stream);
    assert!(
        matches!(req.action(),Action::AttachBinding {terminal_id,..} if terminal_id.as_str()==ID)
    );
    reply(
        stream,
        &req,
        Outcome::Attached {
            attachment_id: viewer.clone(),
            host_epoch: obcs::HostEpoch::parse(&route.host_epoch.to_text()).unwrap(),
            host_terminal_id: obcs::HostTerminalId::parse(&route.host_epoch.to_text()).unwrap(),
        },
    );
    let req = request(stream);
    assert!(matches!(req.action(),Action::Poll{attachment_id} if attachment_id==&viewer));
    let mut bytes = OBTF.to_vec();
    bytes[80..88].copy_from_slice(&history.to_le_bytes());
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
    if !acknowledge {
        return viewer;
    }
    let req = request(stream);
    assert!(
        matches!(req.action(),Action::FrameApplied{attachment_id,sequence,revision,generation}
        if attachment_id==&viewer && *sequence==route.sequence && *revision==route.revision && *generation==route.generation)
    );
    reply(stream, &req, Outcome::FeedbackAccepted {});
    viewer
}
#[test]
fn copy_motion_and_search_cli_publish_real_closed_results_without_input_effects() {
    for (verb, format, previous) in [
        ("copy-motion", "json", false),
        ("copy-search", "json", false),
        ("copy-search", "json", true),
        ("copy-search", "text", false),
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let frame = decode_terminal_text_frame(OBTF).unwrap();
            let total = 10 + u64::from(frame.geometry.rows);
            let req = request(&mut stream);
            assert!(
                matches!(req.action(),Action::ReadRows{detail:obcs::ViewerRowDetail::Text,
                start_row:0,row_count,request_sequence,..} if u64::from(*row_count)==total && request_sequence.get()==1)
            );
            let mut rows = vec![(0, ""); total as usize];
            rows[0] = (0, "a.b x x");
            rows[2] = (0, "X x");
            let body = obrw_window(
                0,
                total,
                frame.geometry.columns,
                1,
                frame.route.revision,
                frame.route.generation.get(),
                &rows,
            );
            reply(&mut stream, &req, Outcome::Record(history_record(&body)));
            detached(&mut stream, &viewer);
        });
        let revision = inspect_terminal_frame_route(OBTF)
            .unwrap()
            .revision
            .to_string();
        let mut args = vec![
            "terminal",
            verb,
            ID,
            "--cursor",
            "0:0",
            "--content-revision",
            &revision,
            "--format",
            format,
        ];
        if verb == "copy-motion" {
            args.extend(["--motion", "next-word-start"]);
        } else {
            args.extend(["--query", "x", "--search-direction", "forward"]);
            if previous {
                args.extend(["--previous", "2:2-2:2"]);
            }
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        if format == "json" {
            let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
            if verb == "copy-motion" {
                validate(
                    &doc,
                    include_str!("../../src/cli/schemas/orchbee.terminal.copy-motion.json"),
                );
                assert_eq!(
                    doc["result"]["cursor"],
                    serde_json::json!({"row":0,"col":1})
                );
            } else {
                validate(
                    &doc,
                    include_str!("../../src/cli/schemas/orchbee.terminal.copy-search.json"),
                );
                assert_eq!(doc["result"]["total"], "4");
                assert_eq!(doc["result"]["currentGlobal"], "0");
                assert_eq!(
                    doc["result"]["matches"][0]["start"],
                    serde_json::json!({"row":0,"col":4})
                );
            }
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("Matches: 4, current: 1"));
            assert!(text.contains("* 0:4–0:4"));
        }
    }
}
#[test]
fn copy_search_cli_keeps_one_frame_and_strict_sequences_and_wraps_across_windows() {
    for case in ["consistent", "wrong-sequence", "wrong-boundary"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let frame = decode_terminal_text_frame(OBTF).unwrap();
            let chunk = (MAXIMUM_FRAME_CELLS / usize::from(frame.geometry.columns)).min(4096);
            let total = chunk as u64 + 1;
            let viewer = copy_frame(&mut stream, total - u64::from(frame.geometry.rows), true);
            for (start, count, sequence) in [(0, chunk as u16, 1_u64), (chunk as u64, 1, 2)] {
                let req = request(&mut stream);
                assert!(
                    matches!(req.action(),Action::ReadRows{attachment_id,start_row,row_count,revision,generation,request_sequence,..}
                    if attachment_id==&viewer && *start_row==start && *row_count==count
                    && *revision==frame.route.revision && *generation==frame.route.generation
                    && request_sequence.get()==sequence)
                );
                let mut rows = vec![(0, ""); usize::from(count)];
                if start == 0 {
                    rows[chunk - 1] = (1, "x");
                } else {
                    rows[0] = (if case == "wrong-boundary" { 0 } else { 2 }, "x");
                }
                let seq = if case == "wrong-sequence" {
                    sequence + 1
                } else {
                    sequence
                };
                let body = obrw_window(
                    start,
                    total,
                    frame.geometry.columns,
                    seq,
                    frame.route.revision,
                    frame.route.generation.get(),
                    &rows,
                );
                reply(&mut stream, &req, Outcome::Record(history_record(&body)));
                if case == "wrong-sequence" {
                    break;
                }
            }
            detached(&mut stream, &viewer);
        });
        let revision = inspect_terminal_frame_route(OBTF)
            .unwrap()
            .revision
            .to_string();
        let output = fixture.run(&[
            "terminal",
            "copy-search",
            ID,
            "--cursor",
            "0:0",
            "--query",
            "x",
            "--search-direction",
            "forward",
            "--content-revision",
            &revision,
            "--format",
            "json",
        ]);
        handler.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if case == "consistent" {
            assert_eq!(output.status.code(), Some(0));
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.terminal.copy-search.json"),
            );
            assert_eq!(doc["result"]["total"], "2");
        } else {
            assert_eq!(output.status.code(), Some(5));
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
            );
            assert_eq!(doc["failure"]["kind"], "protocol-invalid");
            assert!(doc.get("result").is_none());
        }
    }
}
#[test]
fn copy_search_cli_refuses_stale_content_and_outside_previous_before_rows_and_releases() {
    for case in ["stale", "outside-previous"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = super::selection::selection_frame_without_ack(&mut stream);
            detached(&mut stream, &viewer);
        });
        let route = inspect_terminal_frame_route(OBTF).unwrap();
        let revision = if case == "stale" {
            route.revision + 1
        } else {
            route.revision
        }
        .to_string();
        let mut args = vec![
            "terminal",
            "copy-search",
            ID,
            "--cursor",
            "0:0",
            "--query",
            "x",
            "--search-direction",
            "forward",
            "--content-revision",
            &revision,
            "--format",
            "json",
        ];
        if case == "outside-previous" {
            args.extend(["--previous", "0:0-4294967295:0"]);
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(if case == "stale" { 4 } else { 3 })
        );
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert!(doc.get("result").is_none());
    }
}
