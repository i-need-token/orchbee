use super::*;

#[test]
fn plugin_overlay_open_pins_actual_source_member_and_preserves_creation_on_focus_loss() {
    for (child, zoomed, cut) in [
        (false, false, Cut::Complete),
        (true, true, Cut::Complete),
        (false, false, Cut::FocusLost),
        (false, false, Cut::SelectionChanged),
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.0.join("orchbee-plugin.toml");
        std::fs::write(&manifest, "id='tools'\nname='Tools'\nversion='1'\n[[panels]]\nid='dashboard'\ntitle='Dashboard'\nplacement='overlay'\ncommand=['cmd.exe','/d','/c','echo dashboard']\n").unwrap();
        assert!(
            fixture
                .run(&[
                    "plugin",
                    "link",
                    manifest.to_str().unwrap(),
                    "--format=json"
                ])
                .status
                .success()
        );
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut before = snapshot(false, true);
            if child {
                let PanelNode::Split { children, .. } =
                    &mut before.terminal_contexts[0].tabs[0].root
                else {
                    panic!()
                };
                let PanelNode::Panel {
                    child_presentations,
                    ..
                } = &mut children[0].node
                else {
                    panic!()
                };
                child_presentations.push(Presentation {
                    presentation_id: TerminalPresentationId::parse(&id(206)).unwrap(),
                    label: "child".into(),
                    custom_label: None,
                    binding: None,
                    launch_failure: None,
                    agent_not_resumed: None,
                    agent_start_failure: None,
                    launch_program: None,
                });
            }
            snapshot_reply(&mut stream, before.clone());
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::List {}));
            let mut original = record(false, true, false);
            original["view"]["selection"]["zoomed"] = json!(zoomed);
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"listed","clients":[original.clone()]}),
            );
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("passive source member read")
            };
            assert_eq!(
                json!(command.intent),
                json!({"kind":"presentation","panelId":id(6)})
            );
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":{"kind":"presentation-read","view":original["view"],"presentation":{"workbenchRevision":"12","panelId":id(6),"presentation":{"presentationId":id(if child {206} else {106}),"selectionGeneration":"1"}}}}}),
            );
            source::read(
                &mut stream,
                &before,
                Some(&original),
                if child { 206 } else { 106 },
                true,
                false,
            );
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one canonical creation")
            };
            let action = json!(command.action);
            assert_eq!(action["kind"], "panel.split");
            assert_eq!(action["panelId"], id(6));
            assert_eq!(action["direction"], "right");
            assert_eq!(action["label"], "Dashboard");
            assert_eq!(
                action["options"]["pluginPanel"],
                json!({"pluginId":"tools","entrypointId":"dashboard"})
            );
            write_workbench_response(&mut stream, &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion, request_id: command.request_id,
                outcome: serde_json::from_value(json!({"kind":"panel-split-pending","revision":"13","terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"newPanelId":id(20),"splitId":id(30),"firstSlotId":id(31),"secondSlotId":id(32),"presentationId":id(120)})).unwrap(),
            })).unwrap();
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            let mut current = record(true, true, false);
            current["view"]["selection"]["zoomed"] = json!(zoomed);
            if cut == Cut::SelectionChanged {
                current["view"]["selection"]["panelId"] = json!(id(8));
            }
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"found","client":current}),
            );
            if cut == Cut::SelectionChanged {
                assert_closed(&mut stream);
                return;
            }
            let mut fresh = snapshot(true, true);
            let PanelNode::Split { children, .. } = &mut fresh.terminal_contexts[0].tabs[0].root
            else {
                panic!()
            };
            let PanelNode::Split { children, .. } = &mut children[0].node else {
                panic!()
            };
            if child {
                let PanelNode::Panel {
                    child_presentations,
                    ..
                } = &mut children[0].node
                else {
                    panic!()
                };
                let PanelNode::Split {
                    children: prior, ..
                } = &before.terminal_contexts[0].tabs[0].root
                else {
                    panic!()
                };
                let PanelNode::Panel {
                    child_presentations: selected,
                    ..
                } = &prior[0].node
                else {
                    panic!()
                };
                *child_presentations = selected.clone();
            }
            let PanelNode::Panel {
                primary_presentation,
                ..
            } = &mut children[1].node
            else {
                panic!()
            };
            primary_presentation.binding = Some(PresentationBinding {
                worker_id: WorkerId::parse(&id(9)).unwrap(),
                terminal_id: TerminalId::parse(&id(10)).unwrap(),
            });
            snapshot_reply(&mut stream, fresh);
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("one overlay handoff")
            };
            assert_eq!(json!(command.expected_view_revision), "3");
            assert_eq!(
                json!(command.intent),
                json!({"kind":"overlay","terminalContextId":id(4),"tabId":id(5),"panelId":id(20),"presentationId":id(120),"expectedBinding":{"workerId":id(9),"terminalId":id(10)},"returnTo":{"panelId":id(6),"presentationId":id(if child {206} else {106}),"binding":null},"previouslyZoomed":zoomed})
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
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "plugin",
            "panel",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--no-focus",
            "--format=json",
        ]);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../../src/cli/schemas/orchbee.creation.result.json"),
        );
        assert_eq!(document["version"], 1);
        assert_eq!(
            document["result"]["postCreate"]["requested"]["overlay"],
            true
        );
        assert_eq!(document["result"]["postCreate"]["requested"]["focus"], true);
        assert_eq!(document["result"]["outcome"], "committed");
        assert_eq!(
            output.status.code(),
            Some(if cut == Cut::Complete {
                0
            } else if cut == Cut::FocusLost {
                8
            } else {
                3
            }),
            "{document}"
        );
        if cut == Cut::Complete {
            assert_eq!(document["result"]["postCreate"]["focus"]["version"], 1);
        }
    }
}

#[test]
fn plugin_overlay_target_overrides_and_popup_canonical_geometry_are_refused_before_runtime() {
    for extra in [
        vec!["--placement", "overlay", "--workspace", "any"],
        vec![
            "--placement",
            "overlay",
            "--target-panel",
            "00000000000000000000000000000001",
        ],
        vec!["--placement", "overlay", "--direction", "down"],
        vec!["--placement", "popup", "--cols", "60"],
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.0.join("orchbee-plugin.toml");
        std::fs::write(&manifest, "id='tools'\nname='Tools'\nversion='1'\n[[panels]]\nid='dashboard'\ntitle='Dashboard'\ncommand=['cmd.exe']\n").unwrap();
        assert!(
            fixture
                .run(&[
                    "plugin",
                    "link",
                    manifest.to_str().unwrap(),
                    "--format=json"
                ])
                .status
                .success()
        );
        let mut args = vec![
            "plugin",
            "panel",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--format=json",
        ];
        args.extend(extra);
        let output = fixture.run(&args);
        assert!(!output.status.success());
        assert!(!fixture.0.join("runtime").exists());
    }
}

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
        std::fs::write(&manifest, "id='tools'\nname='Tools'\nversion='1'\n[[panels]]\nid='dashboard'\ntitle='Dashboard'\nplacement='tab'\ncommand=['cmd.exe','/d','/c','echo dashboard']\n").unwrap();
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
            source::read(
                &mut stream,
                &snapshot(false, split),
                Some(&record(false, split, false)),
                106,
                !split || focus || zoom,
                false,
            );
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!()
            };
            let action = json!(command.action);
            assert_eq!(action["options"]["pluginContext"]["tabId"], id(5));
            assert_eq!(action["options"]["pluginContext"]["focusedPanelId"], id(6));
            assert!(
                action["options"]["pluginContext"]
                    .get("focusedPanelCwd")
                    .is_none()
            );
            assert!(
                action["options"]["pluginContext"]
                    .get("selectedText")
                    .is_none()
            );
            assert_eq!(
                action["kind"],
                if split { "panel.split" } else { "tab.create" }
            );
            assert_eq!(action["label"], "Dashboard");
            assert_eq!(
                action["options"]["pluginPanel"],
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
            "panel",
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
                "--target-panel",
                &panel,
                "--workspace",
                "ignored-for-splits",
            ]);
        } else {
            args.extend(["--target-panel", &panel]);
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
                assert_eq!(document["result"]["postCreate"]["failure"], "denied");
            } else {
                assert!(output.status.success(), "{document}");
            }
            assert_eq!(document["version"], 1);
            if zoom {
                assert_eq!(
                    document["result"]["postCreate"]["requested"]["maximize"],
                    true
                );
                assert_eq!(document["result"]["postCreate"]["requested"]["focus"], true);
                assert_eq!(document["result"]["outcome"], "committed");
            }
            validate(
                &document,
                include_str!("../../src/cli/schemas/orchbee.creation.result.json"),
            );
            assert_eq!(document["result"]["directory"]["policy"], "plugin-root");
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
                PluginAction::PanelGet {
                    panel_id: WorkbenchPanelId::parse(&id(6)).unwrap()
                }
            );
            let outcome = if case == "ordinary" {
                PluginOutcome::Failed {
                    reason: PluginFailure::NotFound,
                }
            } else {
                PluginOutcome::PanelFound {
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
                    session_id: request.session_id,
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
            "panel",
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

#[path = "plugin_panes/source.rs"]
mod source;

#[path = "plugin_panes/targets.rs"]
mod targets;

#[path = "plugin_panes/popup.rs"]
mod popup;
