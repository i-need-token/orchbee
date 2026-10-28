//! Actual Windows CLI/cohort/pipe framing evidence; the peer is deliberately not a renderer.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};
const PANEL: &str = "22222222222222222222222222222222";
const PRESENTATION: &str = "11111111111111111111111111111111";
fn reply(stream: &mut LocalStream, request: PaneGraphicsRequest, outcome: PaneGraphicsOutcome) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::PaneGraphics(PaneGraphicsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }),
    )
    .unwrap();
}
fn resolved(mut target: PaneGraphicsTarget) -> PaneGraphicsTarget {
    if target.presentation_id.is_none() {
        target.presentation_id = Some(TerminalPresentationId::parse(PRESENTATION).unwrap());
    }
    target
}
fn request(stream: &mut LocalStream) -> PaneGraphicsRequest {
    let WorkbenchRequest::PaneGraphics(request) = read_workbench_request(stream).unwrap() else {
        panic!("graphics request")
    };
    request
}
#[test]
fn graphics_upload_clear_info_and_unknown_submission_keep_closed_receipts_without_replay() {
    for mode in ["set", "clear", "info", "denied", "lost", "wrong-action"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let request = request(&mut stream);
            let outcome = match &request.action {
                PaneGraphicsAction::Set {
                    target,
                    layer_id,
                    z_index,
                    image,
                } => {
                    assert_eq!(*z_index, -1);
                    assert_eq!(image.format, PaneGraphicsFormat::Bgra);
                    assert_eq!(image.data_base64, "AP//gA==");
                    if mode == "lost" {
                        drop(stream);
                        return;
                    }
                    if mode == "denied" {
                        PaneGraphicsOutcome::Failed {
                            reason: PaneGraphicsFailure::Denied,
                        }
                    } else if mode == "wrong-action" {
                        PaneGraphicsOutcome::Closed {
                            stream_id: WorkbenchRequestId::parse(&"f".repeat(32)).unwrap(),
                            revision: WorkbenchRevision::new(1),
                            removed: false,
                        }
                    } else {
                        PaneGraphicsOutcome::Applied {
                            target: resolved(target.clone()),
                            layer_id: layer_id.clone(),
                            revision: WorkbenchRevision::new(1),
                            changed: true,
                        }
                    }
                }
                PaneGraphicsAction::Clear { target, layer_id } => PaneGraphicsOutcome::Applied {
                    target: resolved(target.clone()),
                    layer_id: layer_id.clone(),
                    revision: WorkbenchRevision::new(0),
                    changed: false,
                },
                PaneGraphicsAction::Info { target, client_id } => {
                    assert!(client_id.is_none());
                    PaneGraphicsOutcome::Info {
                        target: resolved(target.clone()),
                        revision: WorkbenchRevision::new(0),
                        layer_count: 0,
                        client_id: None,
                        capabilities: None,
                        pane_visible: None,
                        drawn_revision: None,
                        file_frame_directory: None,
                    }
                }
                _ => panic!("unexpected write"),
            };
            reply(&mut stream, request, outcome);
            assert_closed(&mut stream);
        });
        let verb = match mode {
            "clear" => "clear",
            "info" => "info",
            _ => "set",
        };
        let mut args = vec![
            "panel",
            "graphics",
            verb,
            "@panel:0x22222222222222222222222222222222",
            "--format=json",
        ];
        let spec = json!({"zIndex":-1,"image":{"format":"bgra","width":1,"height":1,"dataBase64":"AP//gA==","placement":{"viewportColumn":-1,"viewportRow":0,"gridColumns":1,"gridRows":1}}});
        let encoded = serde_json::to_vec(&spec).unwrap();
        let output = if verb == "set" {
            args.push("--spec=-");
            fixture.run_process(&args, None, None, &[], Some(&encoded))
        } else {
            fixture.run(&args)
        };
        assert_eq!(
            output.status.code(),
            Some(match mode {
                "denied" => 3,
                "lost" | "wrong-action" => 8,
                _ => 0,
            }),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.panel.graphics.json"),
        );
        assert_eq!(document["target"]["panelId"], PANEL);
        assert_eq!(
            document["observation"]["kind"],
            if matches!(mode, "lost" | "wrong-action") {
                "unknown"
            } else {
                "server"
            }
        );
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("dataBase64"),
            "upload bytes never echoed"
        );
        handler.join().unwrap();
    }
}
#[test]
fn actual_graphics_raw_pipe_frames_keep_owner_sequence_and_cleanup_on_eof_bad_frame_or_unknown_reply()
 {
    for mode in [
        "eof",
        "truncated",
        "lost",
        "wrong-target",
        "older-sequence",
        "file",
        "file-numeric",
        "file-unavailable",
        "file-external",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let file_root = fixture.0.join("producer");
        let file_path = file_root.join("帧.bgra");
        let expected_file = file_path.to_str().unwrap().to_owned();
        let returned_root = file_root.to_str().unwrap().to_owned();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let open = request(&mut stream);
            let PaneGraphicsAction::StreamOpen {
                stream_id,
                target,
                layer_id,
                z_index,
            } = open.action.clone()
            else {
                panic!("open")
            };
            assert_eq!(z_index, 0);
            assert_eq!(layer_id, "primary");
            let target = resolved(target);
            reply(
                &mut stream,
                open,
                PaneGraphicsOutcome::StreamOpened {
                    stream_id: stream_id.clone(),
                    target: target.clone(),
                    layer_id: layer_id.clone(),
                    z_index,
                    revision: WorkbenchRevision::new(1),
                    file_frame_directory: matches!(mode, "file" | "file-numeric" | "file-external")
                        .then_some(returned_root),
                },
            );
            if !matches!(mode, "truncated" | "file-unavailable" | "file-external") {
                let frame = request(&mut stream);
                let PaneGraphicsAction::StreamFrame {
                    stream_id: owner,
                    sequence,
                    revision,
                    image,
                    damage,
                } = &frame.action
                else {
                    panic!("complete frame")
                };
                assert_eq!(owner, &stream_id);
                assert_eq!(sequence.value(), 1);
                assert_eq!(
                    revision.value(),
                    if mode == "file-numeric" { 8 } else { u64::MAX }
                );
                if matches!(mode, "file" | "file-numeric") {
                    assert_eq!(
                        damage,
                        &[PaneGraphicsDamage {
                            x: 0,
                            y: 0,
                            width: 1,
                            height: 1
                        }]
                    );
                    assert!(
                        matches!(image, PaneGraphicsStreamImage::File { format: PaneGraphicsFormat::Bgra, path, width: 1, height: 1, .. } if path == &expected_file)
                    );
                } else {
                    assert!(damage.is_empty());
                    assert!(
                        matches!(image, PaneGraphicsStreamImage::Inline { image } if image.data_base64=="AP8KgA==")
                    );
                }
                if mode == "lost" {
                    drop(stream);
                    return;
                }
                let mut returned_target = target.clone();
                if mode == "wrong-target" {
                    returned_target.presentation_id =
                        Some(TerminalPresentationId::parse(&"4".repeat(32)).unwrap());
                }
                reply(
                    &mut stream,
                    frame,
                    PaneGraphicsOutcome::FrameApplied {
                        stream_id: stream_id.clone(),
                        target: returned_target,
                        layer_id,
                        sequence: WorkbenchRevision::new(1),
                        frame_revision: WorkbenchRevision::new(if mode == "file-numeric" {
                            8
                        } else {
                            u64::MAX
                        }),
                        revision: WorkbenchRevision::new(2),
                        changed: true,
                    },
                );
            }
            let close = request(&mut stream);
            assert!(
                matches!(&close.action, PaneGraphicsAction::StreamClose { stream_id: owner } if owner==&stream_id)
            );
            reply(
                &mut stream,
                close,
                PaneGraphicsOutcome::Closed {
                    stream_id,
                    revision: WorkbenchRevision::new(3),
                    removed: mode != "truncated",
                },
            );
            assert_closed(&mut stream);
        });
        let mut bytes = b"{\"format\":\"rgba\",\"width\":1,\"height\":1,\"dataLength\":4,\"revision\":\"18446744073709551615\"}\n".to_vec();
        bytes.extend(if mode == "truncated" {
            &[0, 255][..]
        } else {
            &[0, 255, 10, 128][..]
        });
        if mode == "older-sequence" {
            bytes.extend(b"{\"format\":\"rgba\",\"width\":1,\"height\":1,\"dataLength\":4,\"sequence\":\"1\"}\n");
        }
        if matches!(
            mode,
            "file" | "file-numeric" | "file-unavailable" | "file-external"
        ) {
            let path = if mode == "file-external" {
                file_root.join("../outside.bgra")
            } else {
                file_path
            };
            bytes = serde_json::to_vec(&json!({"format":"bgra", "width":1, "height":1,
                "revision":u64::MAX.to_string(), "file":{"path":path.to_str().unwrap()}, "damage":{"x":0,"y":0,"width":1,"height":1}, "transport":"direct-kitty"}))
            .unwrap();
            if mode == "file-numeric" {
                let mut document: Value = serde_json::from_slice(&bytes).unwrap();
                document["sequence"] = json!(1);
                document["revision"] = json!(8);
                bytes = serde_json::to_vec(&document).unwrap();
            }
            bytes.push(b'\n');
        }
        let output = fixture.run_process(
            &[
                "panel",
                "graphics",
                "stream",
                "@panel:0x22222222222222222222222222222222",
                "--format=json",
            ],
            None,
            None,
            &[],
            Some(&bytes),
        );
        assert_eq!(
            output.status.code(),
            Some(match mode {
                "truncated" | "older-sequence" | "file-external" => 2,
                "file-unavailable" => 3,
                "lost" => 8,
                "wrong-target" => 5,
                _ => 0,
            }),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let records = output
            .stdout
            .split(|b| *b == b'\n')
            .filter(|bytes| !bytes.is_empty())
            .map(|bytes| serde_json::from_slice::<Value>(bytes).unwrap())
            .collect::<Vec<_>>();
        for record in &records {
            validate(
                record,
                include_str!("../src/cli/schemas/orchbee.panel.graphics.stream.json"),
            );
        }
        let closed = records.last().unwrap();
        assert_eq!(closed["kind"], "closed");
        assert_eq!(
            closed["cleanup"],
            if mode == "lost" {
                "connection-ended"
            } else {
                "closed"
            }
        );
        if matches!(mode, "truncated" | "file-unavailable" | "file-external") {
            assert_eq!(records.len(), 2, "no unadmitted image submitted");
        }
        handler.join().unwrap();
    }
}
#[test]
fn malformed_input_stays_before_session_discovery() {
    let fixture = Fixture::new();
    let output = fixture.run_process(
        &[
            "panel",
            "graphics",
            "set",
            "@panel:0x22222222222222222222222222222222",
            "--spec=-",
            "--format=json",
        ],
        None,
        None,
        &[],
        Some(b"{\"image\":null}"),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!fixture.0.join("runtime").exists());
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.cli.failure.graphics.json"),
    );
}
