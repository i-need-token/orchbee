//! Actual CLI processes prove strict link reads and effect-free viewer release.
use super::*;

// Independent one-row OBPB/OBRW wire fixture; every generated valid frame is
// admitted by the real decoder, including wide-head/tail shapes.
fn render_window(start: u64, sequence: u64, uri: Option<&str>, wide: bool, text: &str) -> Vec<u8> {
    let frame = decode_terminal_text_frame(OBTF).unwrap();
    let route = frame.route;
    let columns = usize::from(frame.geometry.columns);
    let mut projection = vec![0_u8; 320];
    projection[..8].copy_from_slice(b"OBPB\0\0\x01\0");
    projection[12..14].copy_from_slice(&320_u16.to_le_bytes());
    projection[14..16].copy_from_slice(&frame.geometry.columns.to_le_bytes());
    for at in [16, 18] {
        projection[at..at + 2].copy_from_slice(&1_u16.to_le_bytes());
    }
    projection[20..24].copy_from_slice(&(columns as u32).to_le_bytes());
    projection[24..40].copy_from_slice(route.host_epoch.as_bytes());
    projection[40..56].copy_from_slice(route.attachment_id.as_bytes());
    for (at, value) in [
        (56, sequence),
        (64, u64::MAX),
        (72, route.revision),
        (80, route.generation.get()),
        (88, start),
    ] {
        projection[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    projection[100..102].copy_from_slice(&1_u16.to_le_bytes());
    projection[102..104].copy_from_slice(&1_u16.to_le_bytes());
    projection[104] = 1;
    projection[106] = 1;
    projection[144] = 2;
    for at in [124, 132] {
        projection[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    assert!(wide || (text.is_ascii() && text.len() <= columns));
    let strings = format!("{text}{}", uri.unwrap_or_default());
    projection[140..144].copy_from_slice(&(strings.len() as u32).to_le_bytes());
    for (section, width) in [2, 1, 1, 4, 2, 4, 2, 4, 4, 4, 2, 1, 1, 1, 1, 1]
        .into_iter()
        .enumerate()
    {
        let aligned = (projection.len() + width - 1) & !(width - 1);
        projection.resize(aligned, 0);
        let count = if section <= 2 {
            1
        } else if section == 15 {
            strings.len()
        } else {
            columns
        };
        let length = count * width;
        projection[160 + section * 8..164 + section * 8]
            .copy_from_slice(&(aligned as u32).to_le_bytes());
        projection[164 + section * 8..168 + section * 8]
            .copy_from_slice(&(length as u32).to_le_bytes());
        projection.resize(aligned + length, 0);
        if section == 15 {
            projection[aligned..].copy_from_slice(strings.as_bytes());
        }
        for cell in 0..if (3..=14).contains(&section) {
            columns
        } else {
            0
        } {
            let at = aligned + cell * width;
            match section {
                3 => projection[at..at + 4].copy_from_slice(
                    &(if wide {
                        if cell == 0 { 0 } else { text.len() }
                    } else {
                        cell.min(text.len())
                    } as u32)
                        .to_le_bytes(),
                ),
                4 => projection[at..at + 2].copy_from_slice(
                    &(if wide {
                        if cell == 0 { text.len() as u16 } else { 0 }
                    } else {
                        u16::from(cell < text.len())
                    })
                    .to_le_bytes(),
                ),
                5 => projection[at..at + 4].copy_from_slice(
                    &(if cell == 0 && uri.is_some() {
                        text.len() as u32
                    } else {
                        u32::MAX
                    })
                    .to_le_bytes(),
                ),
                6 => projection[at..at + 2].copy_from_slice(
                    &(if cell == 0 {
                        uri.map_or(0, str::len) as u16
                    } else {
                        0
                    })
                    .to_le_bytes(),
                ),
                12 => {
                    projection[at] = if wide && cell == 0 {
                        1
                    } else if wide && cell == 1 {
                        2
                    } else {
                        0
                    }
                }
                _ => {}
            }
        }
    }
    let length = projection.len() as u32;
    projection[8..12].copy_from_slice(&length.to_le_bytes());
    decode_terminal_projection_buffer(&projection).expect("valid independent render row");
    let mut graphics = vec![0_u8; 40];
    graphics[..8].copy_from_slice(b"OBKG\0\0\x02\x01");
    graphics[8..12].copy_from_slice(&40_u32.to_le_bytes());
    let total = 112 + projection.len() + graphics.len();
    let mut window = vec![0_u8; 112];
    window[..8].copy_from_slice(b"OBRW\0\0\x01\0");
    window[8..12].copy_from_slice(&(total as u32).to_le_bytes());
    window[12..14].copy_from_slice(&112_u16.to_le_bytes());
    window[14..16].copy_from_slice(&frame.geometry.columns.to_le_bytes());
    window[16..32].copy_from_slice(route.host_epoch.as_bytes());
    window[32..48].copy_from_slice(route.attachment_id.as_bytes());
    for (at, value) in [
        (48, sequence),
        (56, route.revision),
        (64, route.generation.get()),
        (72, 10 + u64::from(frame.geometry.rows)),
        (80, start),
    ] {
        window[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    window[88..90].copy_from_slice(&1_u16.to_le_bytes());
    window[92..96].copy_from_slice(&(columns as u32).to_le_bytes());
    window[96..100].copy_from_slice(&((projection.len() + 40) as u32).to_le_bytes());
    window[100] = 3;
    window[101..105].copy_from_slice(&(projection.len() as u32).to_le_bytes());
    window[105..109].copy_from_slice(&40_u32.to_le_bytes());
    window.extend(projection);
    window.extend(graphics);
    decode_terminal_render_row_window(&window).expect("valid independent render window");
    window
}
fn row_request(
    stream: &mut LocalStream,
    start: u64,
    count: u16,
    sequence: u64,
    detail: obcs::ViewerRowDetail,
) -> TerminalViewerRequest {
    let req = request(stream);
    let frame = decode_terminal_text_frame(OBTF).unwrap();
    assert!(
        matches!(req.action(),Action::ReadRows { start_row,row_count,request_sequence,detail:actual,revision,generation,.. }
        if *start_row==start && *row_count==count && request_sequence.get()==sequence && *actual==detail
        && *revision==frame.route.revision && *generation==frame.route.generation)
    );
    req
}

#[test]
fn link_resolve_cli_extends_both_token_ends_and_clips_an_explicit_history_viewport() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |mut stream| {
        bound_snapshot_reply(&mut stream);
        let viewer = observed_frame(&mut stream);
        let full = "x".repeat(usize::from(
            decode_terminal_text_frame(OBTF).unwrap().geometry.columns,
        ));
        let req = row_request(&mut stream, 4, 1, 1, obcs::ViewerRowDetail::Render);
        reply(
            &mut stream,
            &req,
            Outcome::Record(history_record(&render_window(4, 1, None, false, &full))),
        );
        let frame = decode_terminal_text_frame(OBTF).unwrap();
        let total = 10 + u64::from(frame.geometry.rows);
        for (start, count, sequence) in [(4, 2, 2), (0, 4, 3), (6, 10, 4)] {
            let req = row_request(
                &mut stream,
                start,
                count,
                sequence,
                obcs::ViewerRowDetail::Text,
            );
            let rows: Vec<_> = (start..start + u64::from(count))
                .map(|row| match row {
                    2 => (1, "https://"),
                    3..=9 => (3, full.as_str()),
                    10 => (2, "end"),
                    _ => (0, ""),
                })
                .collect();
            let window = obrw_window(
                start,
                total,
                frame.geometry.columns,
                sequence,
                frame.route.revision,
                frame.route.generation.get(),
                &rows,
            );
            reply(&mut stream, &req, Outcome::Record(history_record(&window)));
        }
        detached(&mut stream, &viewer);
    });
    let output = fixture.run(&[
        "terminal",
        "link-resolve",
        ID,
        "--cursor",
        "1:0",
        "--offset-from-bottom",
        "7",
        "--format",
        "json",
    ]);
    handler.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &doc,
        include_str!("../../src/cli/schemas/orchbee.terminal.link-resolve.json"),
    );
    let frame = decode_terminal_text_frame(OBTF).unwrap();
    assert_eq!(
        doc["result"]["url"],
        format!(
            "https://{}end",
            "x".repeat(usize::from(frame.geometry.columns) * 7)
        )
    );
    assert_eq!(doc["result"]["viewportStartRow"], "3");
    assert_eq!(doc["result"]["offsetFromBottom"], "7");
    assert_eq!(doc["result"]["regions"],serde_json::json!((0..frame.geometry.rows).map(|row|serde_json::json!({"row":row,"startCol":0,"endCol":frame.geometry.columns-1})).collect::<Vec<_>>()));
}

#[test]
fn link_resolve_cli_rejects_bad_coordinates_options_and_offsets_before_site_discovery() {
    let fixture = Fixture::new();
    for (cursor, extra) in [
        ("65536:0", vec![]),
        ("0:0", vec!["--offset-from-bottom", "18446744073709551616"]),
        ("0:0", vec!["--motion", "line-end"]),
        ("0:0", vec!["--cursor", "1:1"]),
    ] {
        let mut args = vec![
            "terminal",
            "link-resolve",
            ID,
            "--cursor",
            cursor,
            "--format",
            "json",
        ];
        args.extend(extra);
        let output = fixture.run(&args);
        assert_eq!(output.status.code(), Some(2));
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(doc["failure"]["kind"], "invalid-usage");
    }
}

#[test]
fn link_resolve_cli_returns_plain_offscreen_url_and_proven_no_link_without_input() {
    for present in [true, false] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let req = row_request(&mut stream, 10, 1, 1, obcs::ViewerRowDetail::Render);
            reply(
                &mut stream,
                &req,
                Outcome::Record(history_record(&render_window(
                    10,
                    1,
                    None,
                    false,
                    if present { "a/b " } else { "ordinary" },
                ))),
            );
            let req = row_request(&mut stream, 10, 2, 2, obcs::ViewerRowDetail::Text);
            let frame = decode_terminal_text_frame(OBTF).unwrap();
            let total = 10 + u64::from(frame.geometry.rows);
            let window = obrw_window(
                10,
                total,
                frame.geometry.columns,
                2,
                frame.route.revision,
                frame.route.generation.get(),
                &[
                    (
                        if present { 2 } else { 0 },
                        if present { "a/b " } else { "ordinary" },
                    ),
                    (0, ""),
                ],
            );
            reply(&mut stream, &req, Outcome::Record(history_record(&window)));
            if present {
                let req = row_request(&mut stream, 0, 10, 3, obcs::ViewerRowDetail::Text);
                let mut rows = vec![(0, ""); 10];
                rows[9] = (1, "https://");
                let window = obrw_window(
                    0,
                    total,
                    frame.geometry.columns,
                    3,
                    frame.route.revision,
                    frame.route.generation.get(),
                    &rows,
                );
                reply(&mut stream, &req, Outcome::Record(history_record(&window)));
            }
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal",
            "link-resolve",
            ID,
            "--cursor",
            "0:0",
            "--format",
            "json",
        ]);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.terminal.link-resolve.json"),
        );
        assert_eq!(doc["result"]["viewportStartRow"], "10");
        assert_eq!(doc["result"]["offsetFromBottom"], "0");
        if present {
            assert_eq!(doc["result"]["url"], "https://a/b");
            assert_eq!(
                doc["result"]["regions"],
                serde_json::json!([{"row":0,"startCol":0,"endCol":2}])
            );
        } else {
            assert!(doc["result"]["url"].is_null());
            assert_eq!(doc["result"]["kind"], "none");
        }
    }
}
#[test]
fn link_resolve_cli_returns_whole_explicit_uri_from_a_wide_tail_without_plain_reads() {
    for format in ["json", "text"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let req = row_request(&mut stream, 10, 1, 1, obcs::ViewerRowDetail::Render);
            reply(
                &mut stream,
                &req,
                Outcome::Record(history_record(&render_window(
                    10,
                    1,
                    Some("mailto:user@example.com"),
                    true,
                    "路",
                ))),
            );
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal",
            "link-resolve",
            ID,
            "--cursor",
            "0:1",
            "--format",
            format,
        ]);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        if format == "json" {
            let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.terminal.link-resolve.json"),
            );
            assert_eq!(doc["result"]["url"], "mailto:user@example.com");
            assert_eq!(doc["result"]["kind"], "explicit");
            assert_eq!(doc["result"]["regions"], serde_json::json!([]));
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).contains("mailto:user@example.com"));
        }
    }
}
#[test]
fn link_resolve_cli_refuses_stale_viewport_content_and_misrouted_render_without_retry() {
    for case in [
        "stale",
        "outside",
        "offset",
        "wrong-render-start",
        "host-stale",
        "lost",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = if matches!(case, "stale" | "outside" | "offset") {
                selection::selection_frame_without_ack(&mut stream)
            } else {
                observed_frame(&mut stream)
            };
            if !matches!(case, "stale" | "outside" | "offset") {
                let req = row_request(&mut stream, 10, 1, 1, obcs::ViewerRowDetail::Render);
                if case == "lost" {
                    drop(stream);
                    return;
                }
                let outcome = if case == "host-stale" {
                    Outcome::RowsRejected {
                        reason: obcs::ViewerRowsRejectedReason::StaleGeneration,
                    }
                } else {
                    Outcome::Record(history_record(&render_window(
                        11,
                        1,
                        Some("https://forged"),
                        false,
                        "x",
                    )))
                };
                reply(&mut stream, &req, outcome);
            }
            detached(&mut stream, &viewer);
        });
        let current = inspect_terminal_frame_route(OBTF)
            .unwrap()
            .revision
            .to_string();
        let stale = (inspect_terminal_frame_route(OBTF).unwrap().revision + 1).to_string();
        let output = fixture.run(&[
            "terminal",
            "link-resolve",
            ID,
            "--cursor",
            if case == "outside" { "65535:0" } else { "0:0" },
            "--offset-from-bottom",
            if case == "offset" {
                "18446744073709551615"
            } else {
                "0"
            },
            "--content-revision",
            if case == "stale" { &stale } else { &current },
            "--format",
            "json",
        ]);
        handler.join().unwrap();
        assert_ne!(output.status.code(), Some(0));
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert!(doc.get("result").is_none());
        if case == "wrong-render-start" {
            assert_eq!(doc["failure"]["kind"], "protocol-invalid");
        }
        if case == "host-stale" {
            assert_eq!(doc["failure"]["details"]["reason"], "stale-generation");
        }
    }
}
