use super::*;

#[test]
fn selection_read_cli_reads_a_pinned_inclusive_range_and_releases_its_viewer() {
    for format in ["json", "text"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let req = request(&mut stream);
            let frame = decode_terminal_text_frame(OBTF).unwrap();
            let route = frame.route;
            assert!(
                matches!(req.action(),Action::ReadRows {attachment_id,start_row,row_count,revision,generation,request_sequence,detail,..}
                if attachment_id==&viewer && *start_row==0 && *row_count==2 && *revision==route.revision
                    && *generation==route.generation && request_sequence.get()==1 && *detail==obcs::ViewerRowDetail::Text)
            );
            let obrw = obrw_window(
                0,
                10 + u64::from(frame.geometry.rows),
                frame.geometry.columns,
                1,
                route.revision,
                route.generation.get(),
                &[(1, "alpha  "), (2, "beta")],
            );
            reply(&mut stream, &req, Outcome::Record(history_record(&obrw)));
            detached(&mut stream, &viewer);
        });
        let output = fixture.run(&[
            "terminal",
            "selection-read",
            ID,
            "--anchor",
            "1:2",
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
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        if format == "json" {
            let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.terminal.selection.json"),
            );
            assert_eq!(doc["result"]["text"], "lpha  bet");
            assert_eq!(
                doc["result"]["anchor"],
                serde_json::json!({"row":1,"col":2})
            );
            assert_eq!(
                doc["result"]["contentRevision"],
                inspect_terminal_frame_route(OBTF)
                    .unwrap()
                    .revision
                    .to_string()
            );
        } else {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("Selection: \"lpha  bet\"")
            );
        }
    }
}

pub(super) fn selection_frame_without_ack(stream: &mut LocalStream) -> ViewerId {
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
    assert!(matches!(req.action(),Action::Poll {attachment_id} if attachment_id==&viewer));
    let frame = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        optional_message: false,
        payload_encoding: PayloadEncoding::TerminalFrame,
        correlation_id: None,
        host_epoch: Some(route.host_epoch),
        payload: OBTF,
    })
    .unwrap();
    reply(stream, &req, Outcome::Record(frame));
    viewer
}

#[test]
fn selection_read_cli_refuses_stale_content_and_forged_or_rejected_rows_without_retry() {
    for case in [
        "stale-revision",
        "stale-generation",
        "outside",
        "wrong-start",
        "host-stale",
        "lost",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            if matches!(case, "stale-revision" | "stale-generation" | "outside") {
                let viewer = selection_frame_without_ack(&mut stream);
                detached(&mut stream, &viewer);
                return;
            }
            let viewer = observed_frame(&mut stream);
            let req = request(&mut stream);
            let Action::ReadRows {
                row_count,
                revision,
                generation,
                ..
            } = req.action()
            else {
                panic!("one row read")
            };
            if case == "lost" {
                drop(stream);
                return;
            }
            if case == "host-stale" {
                reply(
                    &mut stream,
                    &req,
                    Outcome::RowsRejected {
                        reason: obcs::ViewerRowsRejectedReason::StaleRevision,
                    },
                );
            } else {
                let frame = decode_terminal_text_frame(OBTF).unwrap();
                assert_eq!(*row_count, 1);
                let bad = obrw_window(
                    1,
                    10 + u64::from(frame.geometry.rows),
                    frame.geometry.columns,
                    1,
                    *revision,
                    generation.get(),
                    &[(0, "wrong")],
                );
                reply(&mut stream, &req, Outcome::Record(history_record(&bad)));
            }
            detached(&mut stream, &viewer);
        });
        let mut args = vec![
            "terminal",
            "selection-read",
            ID,
            "--anchor",
            "0:0",
            "--cursor",
            "0:0",
            "--format=json",
        ];
        let revision = inspect_terminal_frame_route(OBTF)
            .unwrap()
            .revision
            .wrapping_add(1)
            .to_string();
        let generation = inspect_terminal_frame_route(OBTF)
            .unwrap()
            .generation
            .get()
            .wrapping_add(1)
            .to_string();
        if case == "stale-revision" {
            args.extend(["--content-revision", revision.as_str()]);
        }
        if case == "stale-generation" {
            args.extend(["--generation", generation.as_str()]);
        }
        if case == "outside" {
            args[4] = "4294967295:0";
            args[6] = "4294967295:0";
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert!(doc.get("result").is_none(), "{case}:{doc}");
        match case {
            "stale-revision" | "stale-generation" | "host-stale" => {
                assert_eq!(output.status.code(), Some(4), "{case}:{doc}");
                assert_eq!(
                    doc["failure"]["details"]["reason"],
                    if case == "stale-generation" {
                        "stale-generation"
                    } else {
                        "stale-revision"
                    }
                );
            }
            "outside" => assert_eq!(output.status.code(), Some(3), "{doc}"),
            _ => assert_eq!(output.status.code(), Some(5), "{case}:{doc}"),
        }
    }
}
