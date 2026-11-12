use super::*;

fn leaf(n: u64, label: &str) -> Value {
    json!({"kind":"panel","panelId":id(n),"rightClick":"orchbee",
        "primaryPresentation":{"customLabel":null,"presentationId":id(n+100),"label":label,"binding":null},
        "childPresentations":[]})
}
fn split_root(first: u64, second: u64, ratio: f64) -> Value {
    json!({"kind":"split","splitId":id(50),"orientation":"horizontal","ratio":ratio,
        "children":[{"slotId":id(51),"node":leaf(first,"First")},
                    {"slotId":id(52),"node":leaf(second,"Second")}]})
}
fn split_snapshot() -> Snapshot {
    let mut snapshot = snapshot();
    snapshot.terminal_contexts[0].tabs[0].root =
        serde_json::from_value(split_root(6, 8, 0.3)).unwrap();
    snapshot
}
fn outcome(stream: &mut LocalStream, command: &WorkbenchCommand, outcome: WorkbenchCommandOutcome) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id.clone(),
            outcome,
        }),
    )
    .unwrap();
}

#[test]
fn headless_layout_and_export_only_read_canonical_snapshots() {
    for export in [false, true] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream, split_snapshot());
            if export {
                snapshot_reply(&mut stream, split_snapshot());
            }
            assert_closed(&mut stream);
        });
        let args = if export {
            vec!["layout", "export", "--tab", "@tab:0x5", "--format=json"]
        } else {
            vec!["panel", "layout", "@panel:0x6", "--format=json"]
        };
        let output = fixture.run(&args);
        handler.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if export {
            validate(
                &document,
                include_str!("../../src/cli/schemas/orchbee.layout.export.json"),
            );
            assert_eq!(document["version"], 1);
            assert_eq!(document["source"]["scope"], "shared");
            assert!(document["source"]["clientId"].is_null());
            assert!(document["result"]["layout"]["zoomed"].is_null());
            assert_eq!(document["result"]["layout"]["root"]["ratio"], 0.3);
        } else {
            validate(
                &document,
                include_str!("../../src/cli/schemas/orchbee.panel.structure.json"),
            );
            assert_eq!(document["result"]["scope"], "shared");
            assert_eq!(document["result"]["root"]["ratio"], 0.3);
        }
    }
}

#[test]
fn headless_ratio_and_pair_swap_submit_once_without_client_discovery() {
    for swap in [false, true] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream, split_snapshot());
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("canonical command without a Client request")
            };
            assert_eq!(command.expected_revision.value(), 12);
            let revision = WorkbenchRevision::new(13);
            let result = if swap {
                let WorkbenchAction::PanelSwap {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    target_panel_id,
                } = &command.action
                else {
                    panic!("swap")
                };
                WorkbenchCommandOutcome::PanelSwapped {
                    revision,
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                    target_panel_id: target_panel_id.clone(),
                }
            } else {
                let WorkbenchAction::SplitRatioSet {
                    split,
                    previous_ratio,
                    ratio,
                    ..
                } = &command.action
                else {
                    panic!("shared ratio")
                };
                assert_eq!(split.split_id.as_str(), id(50));
                assert_eq!(previous_ratio.value(), 0.3);
                assert_eq!(ratio.value(), 0.7);
                WorkbenchCommandOutcome::SplitRatioConfigured { revision }
            };
            outcome(&mut stream, &command, result);
            assert_closed(&mut stream);
        });
        let args = if swap {
            vec![
                "panel",
                "swap",
                "@panel:0x6",
                "--with",
                "@panel:0x8",
                "--format=json",
            ]
        } else {
            vec![
                "layout",
                "set-split-ratio",
                "--tab",
                "@tab:0x5",
                "--path",
                "root",
                "--ratio",
                "0.7",
                "--format=json",
            ]
        };
        let output = fixture.run(&args);
        handler.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            if swap {
                include_str!("../../src/cli/schemas/orchbee.panel.swap.json")
            } else {
                include_str!("../../src/cli/schemas/orchbee.layout.ratio.json")
            },
        );
        if swap {
            assert_eq!(document["result"]["outcome"], "swapped");
            assert!(document["result"]["focus"].is_null());
        } else {
            assert_eq!(document["result"]["effect"]["result"]["revision"], "13");
        }
    }
}

#[test]
fn same_tab_move_uses_one_placement_and_preserves_unchanged_receipts() {
    for unchanged in [false, true] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut current = split_snapshot();
            snapshot_reply(&mut stream, current.clone());
            let WorkbenchRequest::ClientView(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one Client selection read")
            };
            assert!(matches!(request.action, ClientViewAction::List {}));
            view_reply(&mut stream, &request, json!({"kind":"listed","clients":[]}));
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one same-Tab placement")
            };
            let WorkbenchAction::PanelMove {
                terminal_context_id,
                tab_id,
                panel_id,
                target_panel_id,
                direction,
            } = &command.action
            else {
                panic!("same-Tab placement retains the current action")
            };
            assert_eq!(command.expected_revision.value(), 12);
            assert_eq!(terminal_context_id.as_str(), id(4));
            assert_eq!(tab_id.as_str(), id(5));
            assert_eq!(panel_id.as_str(), id(6));
            assert_eq!(target_panel_id.as_str(), id(8));
            assert_eq!(*direction, WorkbenchPanelSplitDirection::Right);
            let result = if unchanged {
                WorkbenchCommandOutcome::Unchanged {
                    revision: current.revision,
                }
            } else {
                current.revision = WorkbenchRevision::new(13);
                current.terminal_contexts[0].tabs[0].root =
                    serde_json::from_value(split_root(8, 6, 0.5)).unwrap();
                WorkbenchCommandOutcome::PanelMoved {
                    revision: current.revision,
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                    target_panel_id: target_panel_id.clone(),
                }
            };
            outcome(&mut stream, &command, result);
            snapshot_reply(&mut stream, current);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "panel",
            "move",
            "@panel:0x6",
            "--into-tab",
            "@tab:0x5",
            "--target-panel",
            "@panel:0x8",
            "--direction",
            "right",
            "--no-focus",
            "--format=json",
        ]);
        handler.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../../src/cli/schemas/orchbee.panel.move.json"),
        );
        assert_eq!(document["result"]["outcome"], "completed");
        assert_eq!(
            document["result"]["movement"]["result"]["action"],
            "panel.move"
        );
        assert_eq!(
            document["result"]["movement"]["result"]["outcome"],
            if unchanged { "unchanged" } else { "committed" }
        );
        assert!(document["result"]["focus"].is_null());
    }
}

#[test]
fn headless_apply_builds_and_exports_the_shared_tree_in_one_session() {
    let fixture = Fixture::new();
    let file = fixture.0.join("headless-layout.json");
    std::fs::write(&file, json!({"type":"split","direction":"right","ratio":0.7,
        "first":{"type":"panel","rightClick":"orchbee","primary":{"name":"First","start":false},"children":[]},
        "second":{"type":"panel","rightClick":"orchbee","primary":{"name":"Second","start":false},"children":[]}
    }).to_string()).unwrap();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        let mut current = snapshot();
        let mut creates = 0;
        let mut ratios = 0;
        for _ in 0..24 {
            let Ok(request) = read_workbench_request(&mut stream) else {
                break;
            };
            match request {
                WorkbenchRequest::SnapshotRead(request) => {
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            outcome: WorkbenchSnapshotOutcome::Available {
                                snapshot: current.clone(),
                            },
                        }),
                    )
                    .unwrap();
                }
                WorkbenchRequest::Command(command) => match &command.action {
                    WorkbenchAction::TabCreateLayout {
                        label,
                        automatic_label,
                        ..
                    } => {
                        creates += 1;
                        assert_eq!(creates, 1);
                        current.revision = WorkbenchRevision::new(13);
                        current.terminal_contexts[0].tabs.push(
                            serde_json::from_value(json!({
                                "tabId":id(11),"label":label,"automaticLabel":automatic_label,
                                "pinned":false,"groupId":null,"root":split_root(12,14,0.5),
                            }))
                            .unwrap(),
                        );
                        outcome(
                            &mut stream,
                            &command,
                            WorkbenchCommandOutcome::TabLayoutCreated {
                                revision: current.revision,
                                terminal_context_id: WorkbenchTerminalContextId::parse(&id(4))
                                    .unwrap(),
                                tab_id: WorkbenchTabId::parse(&id(11)).unwrap(),
                            },
                        );
                    }
                    WorkbenchAction::SplitRatioSet {
                        ratio,
                        previous_ratio,
                        ..
                    } => {
                        ratios += 1;
                        assert_eq!(ratios, 1);
                        assert_eq!(previous_ratio.value(), 0.5);
                        assert_eq!(ratio.value(), 0.7);
                        current.revision = WorkbenchRevision::new(14);
                        current.terminal_contexts[0].tabs[1].root =
                            serde_json::from_value(split_root(12, 14, 0.7)).unwrap();
                        outcome(
                            &mut stream,
                            &command,
                            WorkbenchCommandOutcome::SplitRatioConfigured {
                                revision: current.revision,
                            },
                        );
                    }
                    _ => {
                        panic!("no launch, focus or compensating mutation for this unbound layout")
                    }
                },
                _ => panic!("no Client discovery or geometry request"),
            }
        }
        assert_eq!((creates, ratios), (1, 1));
    });
    let output = fixture.run(&[
        "layout",
        "apply",
        "--file",
        file.to_str().unwrap(),
        "--workspace",
        "Workspace",
        "--format=json",
    ]);
    handler.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../../src/cli/schemas/orchbee.layout.apply.json"),
    );
    assert_eq!(document["version"], 1);
    assert_eq!(document["result"]["outcome"], "completed");
    assert_eq!(document["result"]["layout"]["root"]["ratio"], 0.7);
    assert!(document["source"]["clientId"].is_null());
}
