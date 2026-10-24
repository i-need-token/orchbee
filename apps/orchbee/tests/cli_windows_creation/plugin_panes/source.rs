//! Exact source-context reads through actual Windows CLI processes.
use super::*;

fn member(snapshot: &Snapshot, presentation: u64) -> (WorkbenchPanelId, Presentation) {
    fn find(node: &PanelNode, id: &str) -> Option<(WorkbenchPanelId, Presentation)> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } => std::iter::once(primary_presentation)
                .chain(child_presentations)
                .find(|p| p.presentation_id.as_str() == id)
                .map(|p| (panel_id.clone(), p.clone())),
            PanelNode::Split { children, .. } => children.iter().find_map(|s| find(&s.node, id)),
        }
    }
    snapshot
        .terminal_contexts
        .iter()
        .flat_map(|c| &c.tabs)
        .find_map(|t| find(&t.root, &id(presentation)))
        .unwrap()
}

fn passive_member(
    stream: &mut LocalStream,
    source: &Value,
    snapshot: &Snapshot,
    presentation: u64,
) {
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("passive source member")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"presentation","panelId":member(snapshot, presentation).0})
    );
    assert_eq!(
        json!(command.expected_view_revision),
        source["viewRevision"]
    );
    let (panel, selected) = member(snapshot, presentation);
    let physical = selected.binding.map(|binding| json!({"presentationId":id(presentation),"binding":binding,"attachmentGeneration":"7","incarnation":0,"viewport":{"state":"unavailable","reason":"frame-unavailable"}}));
    let selected = physical
        .unwrap_or_else(|| json!({"presentationId":id(presentation),"selectionGeneration":"1"}));
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"presentation-read","view":source["view"],"presentation":{"workbenchRevision":"12", "panelId":panel,"presentation":selected}}}}),
    );
}

fn source_reports(
    stream: &mut LocalStream,
    panel: &WorkbenchPanelId,
    presentation: u64,
    binding: Option<PresentationBinding>,
    fence: bool,
) {
    let WorkbenchRequest::PanelMetadata(request) = read_workbench_request(stream).unwrap() else {
        panic!("read metadata")
    };
    assert_eq!(
        json!(request.action),
        if fence {
            json!({"kind":"read","panelIds":[]})
        } else {
            json!({"kind":"read-presentations","presentationIds":[id(presentation)]})
        }
    );
    let outcome = if fence {
        json!({"kind":"read","workbenchRevision":"12","metadataRevision":"5","panels":[]})
    } else {
        json!({"kind":"presentations-read","workbenchRevision":"12","metadataRevision":"5","presentations":[{"presentationId":id(presentation),"panel":{"panelId":panel,"binding":binding,"tokens":{},"presentations":[]}}]})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::PanelMetadata(PanelMetadataResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
    let WorkbenchRequest::PanelAgent(request) = read_workbench_request(stream).unwrap() else {
        panic!("read Agent report")
    };
    assert_eq!(
        json!(request.action),
        if fence {
            json!({"kind":"read","panelIds":[]})
        } else {
            json!({"kind":"read-presentations","presentationIds":[id(presentation)]})
        }
    );
    let report=binding.as_ref().map(|_|json!({"agent":"codex","state":"blocked","authoritySource":"selected-hook","message":"waiting","session":null}));
    let outcome = if fence {
        json!({"kind":"read","workbenchRevision":"12","reportRevision":"5","panels":[]})
    } else {
        json!({"kind":"presentations-read","workbenchRevision":"12","reportRevision":"5","presentations":[{"presentationId":id(presentation),"panel":{"panelId":panel,"binding":binding,"report":report}}]})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::PanelAgent(PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}

pub(super) fn read(
    stream: &mut LocalStream,
    snapshot: &Snapshot,
    source: Option<&Value>,
    presentation: u64,
    original_client: bool,
    changed: bool,
) {
    use orchbee_agent_observation_protocol::{
        AgentRequest, AgentResult, read_agent_request, write_agent_result,
    };
    if let Some(source) = source {
        let request = view_request(stream);
        assert!(matches!(request.action, ClientViewAction::List {}));
        view_reply(
            stream,
            &request,
            json!({"kind":"listed","clients":[source]}),
        );
        passive_member(stream, source, snapshot, presentation);
    }
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("read-only Agent list")
    };
    assert_eq!(json!(request)["kind"], "agent.session.list.request");
    write_agent_result(stream,&AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.list.result","requestId":request.request_id(),"result":{"outcome":{"kind":"listed","revision":3,"items":[],"nextCursor":null}}})).unwrap())).unwrap();
    let (panel, selected) = member(snapshot, presentation);
    let binding = selected.binding;
    if let Some(binding) = &binding {
        terminal_metadata_peer::sample_for(
            stream,
            "stable",
            "C:\\cmd.exe",
            Some(binding.terminal_id.as_str()),
        );
    }
    source_reports(stream, &panel, presentation, binding, false);
    source_reports(stream, &panel, presentation, None, true);
    snapshot_reply(stream, snapshot.clone());
    if let Some(source) = source {
        passive_member(stream, source, snapshot, presentation);
        if original_client {
            let request = view_request(stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            let mut fresh = source.clone();
            if changed {
                fresh["viewRevision"] = json!("3");
            }
            view_reply(stream, &request, json!({"kind":"found","client":fresh}));
        }
    }
}

#[test]
fn plugin_pane_source_context_tracks_child_observations_and_fails_before_creation_on_source_change()
{
    for case in [
        "caller-child",
        "client-child",
        "changed",
        "overlay-member-changed",
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.0.join("orchbee-plugin.toml");
        std::fs::write(&manifest,"id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='tab'\ncommand=['cmd.exe']\n").unwrap();
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
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut before = snapshot(false, false);
            let PanelNode::Split { children, .. } = &mut before.terminal_contexts[0].tabs[0].root
            else {
                panic!()
            };
            let PanelNode::Panel {
                primary_presentation,
                child_presentations,
                ..
            } = &mut children[0].node
            else {
                panic!()
            };
            primary_presentation.binding = Some(PresentationBinding {
                worker_id: WorkerId::parse("6f7263686265652d6c6f63616c2d7631").unwrap(),
                terminal_id: TerminalId::parse(&id(206)).unwrap(),
            });
            child_presentations.push(Presentation {
                presentation_id: TerminalPresentationId::parse(&id(306)).unwrap(),
                label: "child".into(),
                custom_label: None,
                binding: Some(PresentationBinding {
                    worker_id: WorkerId::parse("6f7263686265652d6c6f63616c2d7631").unwrap(),
                    terminal_id: TerminalId::parse(&id(406)).unwrap(),
                }),
                launch_failure: None,
            });
            snapshot_reply(&mut stream, before.clone());
            let selected = record(false, false, false);
            if case != "caller-child" {
                let request = view_request(&mut stream);
                assert!(matches!(request.action, ClientViewAction::List {}));
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"listed","clients":[selected]}),
                );
            }
            if case == "overlay-member-changed" {
                passive_member(&mut stream, &selected, &before, 106);
            }
            read(
                &mut stream,
                &before,
                (case != "caller-child").then_some(&selected),
                306,
                !matches!(case, "caller-child" | "overlay-member-changed"),
                case == "changed",
            );
            if matches!(case, "changed" | "overlay-member-changed") {
                assert_closed(&mut stream);
                return;
            }
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one creation")
            };
            let WorkbenchAction::TabCreate { options, .. } = &command.action else {
                panic!()
            };
            let context = options.plugin_context.as_ref().unwrap();
            assert_eq!(context.focused_pane_id.as_deref(), Some(id(6).as_str()));
            assert_eq!(context.focused_pane_cwd.as_deref(), Some(r"C:\frame 项目"));
            assert_eq!(context.focused_pane_agent.as_deref(), Some("codex"));
            assert_eq!(context.focused_pane_status.as_deref(), Some("blocked"));
            assert!(context.selected_text.is_none());
            assert!(options.cwd.is_none());
            write_workbench_response(&mut stream,&WorkbenchResponse::Command(WorkbenchCommandResult {protocol_version:ProtocolVersion,request_id:command.request_id,outcome:serde_json::from_value(json!({"kind":"tab-create-pending","revision":"13","terminalContextId":id(4),"tabId":id(19),"panelId":id(20),"presentationId":id(120)})).unwrap()})).unwrap();
            assert_closed(&mut stream);
        });
        let terminal = id(406);
        let mut args = vec![
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--no-focus",
            "--format=json",
        ];
        if case == "overlay-member-changed" {
            args.extend(["--placement", "overlay"]);
        }
        let output = fixture.run_with_caller(
            &args,
            (case == "caller-child").then_some(terminal.as_str()),
            (case == "caller-child").then_some("6f7263686265652d6c6f63616c2d7631"),
        );
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if matches!(case, "changed" | "overlay-member-changed") {
            assert!(!output.status.success(), "{document}");
            assert!(document.get("result").is_none());
            assert_eq!(document["failure"]["details"]["reason"], "client-changed");
        } else {
            assert!(output.status.success(), "{document}");
            validate(
                &document,
                include_str!("../../../src/cli/schemas/orchbee.creation.result.json"),
            );
            assert_eq!(document["version"], 3);
        }
    }
}

#[test]
fn plugin_pane_explicit_destination_preserves_headless_and_empty_workspace_source() {
    for headless in [true, false] {
        let fixture = Fixture::new();
        let manifest = fixture.0.join("orchbee-plugin.toml");
        std::fs::write(&manifest,"id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='tab'\ncommand=['cmd.exe']\n").unwrap();
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
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut before = snapshot(false, false);
            before.navigation.projects.push(
                serde_json::from_value(json!({"projectId":id(13),"label":"project"})).unwrap(),
            );
            before.navigation.workspaces.push(serde_json::from_value(json!({"workspaceId":id(12),"projectId":id(13),"terminalContextId":id(11),"label":"empty source","primaryDirectoryId":null})).unwrap());
            before.terminal_contexts.push(
                serde_json::from_value(json!({"terminalContextId":id(11),"groups":[],"tabs":[]}))
                    .unwrap(),
            );
            snapshot_reply(&mut stream, before.clone());
            let mut source = record(false, false, false);
            source["view"]["selection"] = json!({"terminalContextId":id(11),"tabId":null,"panelId":null,"zoomed":false,"panelCount":0});
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::List {}));
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"listed","clients":if headless {vec![]} else {vec![source.clone()]}}),
            );
            snapshot_reply(&mut stream, before);
            if !headless {
                let request = view_request(&mut stream);
                assert!(matches!(request.action, ClientViewAction::Get { .. }));
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"found","client":source}),
                );
            }
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one creation")
            };
            let WorkbenchAction::TabCreate {
                terminal_context_id,
                options,
                ..
            } = &command.action
            else {
                panic!()
            };
            assert_eq!(terminal_context_id.as_str(), id(11));
            let context = options.plugin_context.as_ref().unwrap();
            assert_eq!(context.workspace_id.as_deref(), Some(id(12)).as_deref());
            assert_eq!(context.workspace_label.as_deref(), Some("empty source"));
            assert!(
                context.tab_id.is_none()
                    && context.focused_pane_id.is_none()
                    && context.focused_pane_cwd.is_none()
                    && context.focused_pane_agent.is_none()
                    && context.focused_pane_status.is_none()
                    && context.selected_text.is_none()
            );
            write_workbench_response(&mut stream,&WorkbenchResponse::Command(WorkbenchCommandResult {protocol_version:ProtocolVersion,request_id:command.request_id,outcome:serde_json::from_value(json!({"kind":"tab-create-pending","revision":"13","terminalContextId":id(11),"tabId":id(19),"panelId":id(20),"presentationId":id(120)})).unwrap()})).unwrap();
            assert_closed(&mut stream);
        });
        let workspace = id(12);
        let output = fixture.run(&[
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--workspace",
            &workspace,
            "--no-focus",
            "--format=json",
        ]);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "{document}");
        validate(
            &document,
            include_str!("../../../src/cli/schemas/orchbee.creation.result.json"),
        );
        assert_eq!(document["version"], 3);
    }
}
