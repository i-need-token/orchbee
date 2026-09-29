use super::*;

#[test]
fn plugin_pane_open_submits_catalog_identity_once_and_preserves_canonical_receipts() {
    for (split, focus, lost, zoom, cut) in [
        (false, false, false, false, Cut::Complete),
        (true, true, false, false, Cut::Complete),
        (true, false, true, false, Cut::Complete),
        (true, false, false, true, Cut::Complete),
        (true, false, false, true, Cut::FocusLost),
        (true, false, false, true, Cut::SelectionChanged),
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.0.join("orchbee-plugin.toml");
        std::fs::write(&manifest, "id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='tab'\ncommand=['cmd.exe','/d','/c','echo dashboard']\n").unwrap();
        let linked = fixture.run(&[
            "plugin",
            "link",
            manifest.to_str().unwrap(),
            "--format=json",
        ]);
        assert!(linked.status.success(), "{linked:?}");
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream, snapshot(false, split));
            if !split || focus || zoom {
                let request = view_request(&mut stream);
                assert!(matches!(request.action, ClientViewAction::List {}));
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"listed","clients":[record(false,split,false)]}),
                );
            }
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!()
            };
            let action = json!(command.action);
            assert_eq!(
                action["kind"],
                if split { "panel.split" } else { "tab.create" }
            );
            assert_eq!(action["label"], "Dashboard");
            assert_eq!(
                action["options"]["pluginPane"],
                json!({"pluginId":"tools","entrypointId":"dashboard"})
            );
            assert_eq!(
                action["options"]["environment"],
                json!([["LITERAL", "hello world"]])
            );
            assert!(action.get("launch").is_none() && action.get("profile").is_none());
            assert!(action["options"].get("cwd").is_none());
            if lost {
                return;
            }
            let outcome = if split {
                json!({"kind":"panel-split-pending","revision":"13","terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"newPanelId":id(20),"splitId":id(30),"firstSlotId":id(31),"secondSlotId":id(32),"presentationId":id(120)})
            } else {
                json!({"kind":"tab-create-pending","revision":"13","terminalContextId":id(4),"tabId":id(19),"panelId":id(20),"presentationId":id(120)})
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id,
                    outcome: serde_json::from_value(outcome).unwrap(),
                }),
            )
            .unwrap();
            if focus || zoom {
                if !observe(&mut stream, split, cut) {
                    return;
                }
                if zoom {
                    let request = view_request(&mut stream);
                    let ClientViewAction::Submit { command } = &request.action else {
                        panic!("combined focus and zoom");
                    };
                    assert_eq!(
                        command.minimum_workbench_revision,
                        WorkbenchRevision::new(13)
                    );
                    assert_eq!(json!(command.expected_view_revision), "3");
                    assert_eq!(
                        json!(command.intent),
                        json!({"kind":"zoom","terminalContextId":id(4),"tabId":id(5),"panelId":id(20),"enabled":true})
                    );
                    if cut == Cut::FocusLost {
                        return;
                    }
                    let mut view = record(true, true, true)["view"].clone();
                    view["selection"]["zoomed"] = json!(true);
                    view_reply(
                        &mut stream,
                        &request,
                        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"applied","view":view}}}),
                    );
                } else {
                    assert!(submit(&mut stream, split, false, Cut::Complete));
                }
            }
            assert_closed(&mut stream);
        });
        let panel = id(6);
        let mut args = vec![
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--env",
            "LITERAL=hello world",
            "--format=json",
        ];
        if split {
            args.extend([
                "--placement",
                if zoom { "zoomed" } else { "split" },
                "--target-pane",
                &panel,
                "--workspace",
                "ignored-for-splits",
            ]);
        } else {
            args.extend(["--target-pane", &panel]);
        }
        if !focus {
            args.push("--no-focus");
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if lost {
            assert_eq!(output.status.code(), Some(8), "{document}");
        } else {
            if cut == Cut::FocusLost {
                assert_eq!(output.status.code(), Some(8), "{document}");
            } else if cut == Cut::SelectionChanged {
                assert!(!output.status.success(), "{document}");
                assert_eq!(document["postCreate"]["failure"], "denied");
            } else {
                assert!(output.status.success(), "{document}");
            }
            assert_eq!(document["version"], if zoom { 4 } else { 3 });
            if zoom {
                assert_eq!(document["postCreate"]["requested"]["zoom"], true);
                assert_eq!(document["postCreate"]["requested"]["focus"], true);
                assert_eq!(document["result"]["outcome"], "committed");
                let mut legacy = document.clone();
                legacy["version"] = json!(3);
                let schema: Value = serde_json::from_str(include_str!(
                    "../../src/cli/schemas/orchbee.creation.result.json"
                ))
                .unwrap();
                assert!(
                    !jsonschema::validator_for(&schema)
                        .unwrap()
                        .is_valid(&legacy)
                );
            }
            validate(
                &document,
                include_str!("../../src/cli/schemas/orchbee.creation.result.json"),
            );
            assert_eq!(document["directory"]["policy"], "plugin-root");
            let mut legacy = document.clone();
            legacy["version"] = json!(2);
            let schema: Value = serde_json::from_str(include_str!(
                "../../src/cli/schemas/orchbee.creation.result.json"
            ))
            .unwrap();
            assert!(
                !jsonschema::validator_for(&schema)
                    .unwrap()
                    .is_valid(&legacy)
            );
        }
    }
}

#[test]
fn plugin_pane_focus_close_require_exact_membership_and_never_repeat_effects() {
    for case in [
        "focus",
        "close",
        "ordinary",
        "stale",
        "rebound",
        "close-lost",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut snapshot = snapshot(false, false);
            let PanelNode::Split { children, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root
            else {
                panic!()
            };
            let PanelNode::Panel {
                primary_presentation,
                ..
            } = &mut children[0].node
            else {
                panic!()
            };
            let binding = PresentationBinding {
                worker_id: WorkerId::parse(&id(300)).unwrap(),
                terminal_id: TerminalId::parse(&id(301)).unwrap(),
            };
            primary_presentation.binding = Some(binding.clone());
            snapshot_reply(&mut stream, snapshot);
            let WorkbenchRequest::Plugin(request) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("membership first")
            };
            assert_eq!(
                request.action,
                PluginAction::PaneGet {
                    panel_id: WorkbenchPanelId::parse(&id(6)).unwrap()
                }
            );
            let outcome = if case == "ordinary" {
                PluginOutcome::Failed {
                    reason: PluginFailure::NotFound,
                }
            } else {
                PluginOutcome::PaneFound {
                    revision: WorkbenchRevision::new(if case == "stale" { 13 } else { 12 }),
                    pane: Box::new(PluginPaneInfo {
                        selector: PluginPaneSelector::new("tools".into(), "dashboard".into())
                            .unwrap(),
                        terminal_context_id: WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
                        tab_id: WorkbenchTabId::parse(&id(5)).unwrap(),
                        panel_id: WorkbenchPanelId::parse(&id(6)).unwrap(),
                        presentation_id: TerminalPresentationId::parse(&id(106)).unwrap(),
                        binding: if case == "rebound" {
                            PresentationBinding {
                                terminal_id: TerminalId::parse(&id(302)).unwrap(),
                                ..binding
                            }
                        } else {
                            binding
                        },
                    }),
                }
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Plugin(PluginResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome,
                }),
            )
            .unwrap();
            if matches!(case, "ordinary" | "stale" | "rebound") {
                assert_closed(&mut stream);
                return;
            }
            if case == "focus" {
                let request = view_request(&mut stream);
                assert!(matches!(request.action, ClientViewAction::List {}));
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"listed","clients":[record(false,false,false)]}),
                );
                let request = view_request(&mut stream);
                let ClientViewAction::Submit { command } = &request.action else {
                    panic!()
                };
                assert_eq!(
                    command.minimum_workbench_revision,
                    WorkbenchRevision::new(12)
                );
                assert!(
                    matches!(&command.intent, ClientViewIntent::Panel { panel_id, .. } if panel_id.as_str() == id(6))
                );
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"operation","operation":{"command":command,"status":{"kind":"applied","view":record(false,false,false)["view"]}}}),
                );
            } else {
                let WorkbenchRequest::Command(command) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!()
                };
                assert_eq!(command.expected_revision, WorkbenchRevision::new(12));
                let WorkbenchAction::PanelClose {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                } = command.action
                else {
                    panic!()
                };
                assert_eq!(panel_id.as_str(), id(6));
                if case == "close-lost" {
                    return;
                }
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Command(WorkbenchCommandResult {
                        protocol_version: ProtocolVersion,
                        request_id: command.request_id.clone(),
                        outcome: WorkbenchCommandOutcome::PanelCloseAccepted {
                            close_request_id: command.request_id,
                            terminal_context_id,
                            tab_id,
                            panel_id,
                            accepted_revision: WorkbenchRevision::new(12),
                        },
                    }),
                )
                .unwrap();
            }
            assert_closed(&mut stream);
        });
        let panel = id(6);
        let output = fixture.run(&[
            "plugin",
            "pane",
            if case == "focus" { "focus" } else { "close" },
            &panel,
            "--format=json",
        ]);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if matches!(case, "focus" | "close") {
            assert!(output.status.success(), "{case}: {document}");
        } else if case == "close-lost" {
            assert_eq!(output.status.code(), Some(8), "{document}");
        } else {
            assert!(!output.status.success(), "{case}: {document}");
            assert!(document["failure"].is_object());
        }
    }
}
