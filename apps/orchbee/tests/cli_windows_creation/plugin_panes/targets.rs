//! Target Pane and remembered inactive Workspace context through real CLI pipe traffic.
use super::*;

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("orchbee-plugin.toml");
    std::fs::write(&manifest, "id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='tab'\ncommand=['cmd.exe']\n").unwrap();
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
    fixture
}

fn binding(n: u64) -> Value {
    json!({"workerId":"6f7263686265652d6c6f63616c2d7631","terminalId":id(n)})
}

fn scene() -> Snapshot {
    let mut value = json!(snapshot(false, false));
    let current = &mut value["terminalContexts"][0]["tabs"][0]["root"]["children"][0]["node"];
    current["primaryPresentation"]["binding"] = binding(206);
    current["childPresentations"] =
        json!([{"presentationId":id(306),"label":"current child","binding":binding(406)}]);
    value["terminalContexts"][0]["tabs"][0]["root"]["children"][1]["node"]["primaryPresentation"]
        ["binding"] = binding(208);
    let mut focused = panel(18);
    focused["primaryPresentation"]["binding"] = binding(218);
    focused["childPresentations"] =
        json!([{"presentationId":id(318),"label":"workspace child","binding":binding(418)}]);
    value["navigation"]["projects"] = json!([{"projectId":id(13),"label":"project"}]);
    value["navigation"]["workspaces"] = json!([{"workspaceId":id(12),"projectId":id(13),"terminalContextId":id(11),"label":"destination","primaryDirectoryId":null}]);
    value["terminalContexts"].as_array_mut().unwrap().push(json!({"terminalContextId":id(11),"groups":[],"tabs":[
        {"tabId":id(14),"label":"first","groupId":null,"pinned":false,"root":panel(15)},
        {"tabId":id(16),"label":"remembered","groupId":null,"pinned":false,"root":tree(60,panel(17),focused)}]}));
    serde_json::from_value(value).unwrap()
}

fn list(stream: &mut LocalStream, record: Option<&Value>) {
    let request = view_request(stream);
    assert!(matches!(request.action, ClientViewAction::List {}));
    view_reply(
        stream,
        &request,
        json!({"kind":"listed","clients":record.into_iter().collect::<Vec<_>>()}),
    );
}

fn fence(stream: &mut LocalStream, record: &Value, changed: bool) {
    let request = view_request(stream);
    assert_eq!(
        json!(request.action),
        json!({"kind":"get","clientId":id(7)})
    );
    let mut fresh = record.clone();
    if changed {
        fresh["viewRevision"] = json!("3");
    }
    view_reply(stream, &request, json!({"kind":"found","client":fresh}));
}

fn submit_creation(
    stream: &mut LocalStream,
    pane: Option<u64>,
    tab: Option<u64>,
    workspace: bool,
    split: bool,
) {
    let WorkbenchRequest::Command(command) = read_workbench_request(stream).unwrap() else {
        panic!("one creation");
    };
    assert_eq!(command.expected_revision, WorkbenchRevision::new(12));
    let options = match &command.action {
        WorkbenchAction::PanelSplit {
            panel_id, options, ..
        } => {
            assert!(split);
            assert_eq!(panel_id.as_str(), id(pane.unwrap()));
            options
        }
        WorkbenchAction::TabCreate {
            terminal_context_id,
            options,
            ..
        } => {
            assert!(!split);
            assert_eq!(
                terminal_context_id.as_str(),
                id(if workspace { 11 } else { 4 })
            );
            options
        }
        _ => panic!("plugin terminal creation"),
    };
    let context = options.plugin_context.as_ref().unwrap();
    assert_eq!(context.workspace_id, workspace.then(|| id(12)));
    assert_eq!(
        context.workspace_label.as_deref(),
        workspace.then_some("destination")
    );
    assert_eq!(context.tab_id, tab.map(id));
    assert_eq!(context.focused_pane_id, pane.map(id));
    assert!(context.selected_text.is_none());
    if pane.is_some() {
        assert_eq!(context.focused_pane_cwd.as_deref(), Some("C:\\frame 项目"));
        assert_eq!(context.focused_pane_agent.as_deref(), Some("codex"));
        assert_eq!(context.focused_pane_status.as_deref(), Some("blocked"));
    } else {
        assert!(
            context.focused_pane_cwd.is_none()
                && context.focused_pane_agent.is_none()
                && context.focused_pane_status.is_none()
        );
    }
    let outcome = if split {
        json!({"kind":"panel-split-pending","revision":"13","terminalContextId":id(4),"tabId":id(5),"panelId":pane.map(id),"newPanelId":id(20),"splitId":id(30),"firstSlotId":id(31),"secondSlotId":id(32),"presentationId":id(120)})
    } else {
        json!({"kind":"tab-create-pending","revision":"13","terminalContextId":id(if workspace {11} else {4}),"tabId":id(19),"panelId":id(20),"presentationId":id(120)})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}

fn create(
    stream: &mut LocalStream,
    pane: Option<u64>,
    tab: Option<u64>,
    workspace: bool,
    split: bool,
) {
    submit_creation(stream, pane, tab, workspace, split);
    assert_closed(stream);
}

#[test]
fn explicit_plugin_split_reads_target_primary_and_preserves_matching_caller_or_client_child() {
    for case in [
        "other-client",
        "other-caller",
        "headless",
        "matching-client",
        "matching-caller",
        "client-changed",
    ] {
        let fixture = fixture();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let before = scene();
            snapshot_reply(&mut stream, before.clone());
            let source = record(false, false, false);
            if case == "matching-caller" {
                source::read(&mut stream, &before, None, 306, false, false);
            } else if case == "matching-client" {
                source::read(&mut stream, &before, Some(&source), 306, false, false);
            } else {
                list(&mut stream, (case != "headless").then_some(&source));
                source::read(&mut stream, &before, None, 108, false, false);
                if case != "headless" {
                    fence(&mut stream, &source, case == "client-changed");
                }
            }
            if case == "client-changed" {
                assert_closed(&mut stream);
            } else {
                create(
                    &mut stream,
                    Some(if case.starts_with("matching") { 6 } else { 8 }),
                    Some(5),
                    false,
                    true,
                );
            }
        });
        let target = id(if case.starts_with("matching") { 6 } else { 8 });
        let args = [
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--placement",
            "split",
            "--target-pane",
            &target,
            "--no-focus",
            "--format=json",
        ];
        let output = if case.ends_with("caller") {
            fixture.run_with_caller(
                &args,
                Some(&id(406)),
                Some("6f7263686265652d6c6f63616c2d7631"),
            )
        } else {
            fixture.run(&args)
        };
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            output.status.success(),
            case != "client-changed",
            "{case}: {document}"
        );
        if case != "client-changed" {
            validate(
                &document,
                include_str!("../../../src/cli/schemas/orchbee.creation.result.json"),
            );
        }
    }
}

fn memory(stream: &mut LocalStream, source: &Value, case: &str, second: bool) {
    let tab = match case {
        "no-tab" => None,
        "wrong-tab" => Some(5),
        "changed-tab" if second => Some(14),
        _ => Some(16),
    };
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("passive navigation read");
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"selections","terminalContextIds":[id(11)]})
    );
    assert_eq!(
        json!(command.expected_view_revision),
        source["viewRevision"]
    );
    let revision = if case == "stale-nav" { "13" } else { "12" };
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"navigation-read","view":source["view"],"navigation":{"workbenchRevision":revision,"contexts":[{"terminalContextId":id(11),"activeTabId":tab.map(id)}]}}}}),
    );
    if tab.is_none() || matches!(case, "wrong-tab" | "stale-nav") {
        return;
    }
    let (anchor, focus, panels, splits) = if tab == Some(14) {
        (15, Some(15), vec![15], vec![])
    } else {
        (
            17,
            match case {
                "no-focus" => None,
                "wrong-pane" => Some(6),
                "changed-focus" if second => Some(17),
                _ => Some(18),
            },
            vec![17, if case == "wrong-pane" { 6 } else { 18 }],
            vec![
                json!({"splitId":id(60),"firstSlotId":id(61),"secondSlotId":id(62),"orientation":"horizontal","ratio":0.5,"area":null,"divider":null}),
            ],
        )
    };
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("passive layout read");
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"layout","terminalContextId":id(11),"tabId":tab.map(id),"panelId":id(anchor)})
    );
    let panels: Vec<_> = panels
        .into_iter()
        .map(|n| json!({"panelId":id(n),"unzoomed":null,"terminal":null,"displayed":null}))
        .collect();
    let revision = if case == "stale-layout" { "13" } else { "12" };
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"layout-read","view":source["view"],"layout":{"terminalContextId":id(11),"tabId":tab.map(id),"sourcePanelId":id(anchor),"workbenchRevision":revision,"viewport":{"x":0,"y":0,"width":120,"height":40},"area":{"x":0,"y":0,"width":120,"height":40},"selected":false,"zoomed":false,"focusedPanelId":focus.map(id),"sourceAvailable":false,"neighbors":{"left":null,"right":null,"up":null,"down":null},"panels":panels,"splits":splits}}}}),
    );
}

#[test]
fn plugin_tab_reads_destination_remembered_nonfirst_focus_and_refuses_invalid_or_changed_memory() {
    for case in [
        "remembered",
        "other-caller",
        "headless",
        "no-tab",
        "no-focus",
        "wrong-tab",
        "wrong-pane",
        "stale-nav",
        "stale-layout",
        "changed-tab",
        "changed-focus",
        "changed-client",
    ] {
        let fixture = fixture();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let fails =
            case.starts_with("wrong") || case.starts_with("stale") || case.starts_with("changed");
        let handler = peer(&fixture, listener, move |mut stream| {
            let before = scene();
            snapshot_reply(&mut stream, before.clone());
            let source = record(false, false, false);
            list(&mut stream, (case != "headless").then_some(&source));
            if case != "headless" {
                memory(&mut stream, &source, case, false);
            }
            if case.starts_with("wrong") || case.starts_with("stale") {
                assert_closed(&mut stream);
                return;
            }
            let pane = (!matches!(case, "headless" | "no-tab" | "no-focus")).then_some(18);
            if pane.is_some() {
                source::read(&mut stream, &before, None, 118, false, false);
            } else {
                snapshot_reply(&mut stream, before);
            }
            if case != "headless" {
                memory(&mut stream, &source, case, true);
                if matches!(case, "changed-tab" | "changed-focus") {
                    assert_closed(&mut stream);
                    return;
                }
                fence(&mut stream, &source, case == "changed-client");
            }
            if fails {
                assert_closed(&mut stream);
            } else {
                create(
                    &mut stream,
                    pane,
                    (!matches!(case, "headless" | "no-tab")).then_some(16),
                    true,
                    false,
                );
            }
        });
        let workspace = id(12);
        let args = [
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
        ];
        let output = if case == "other-caller" {
            fixture.run_with_caller(
                &args,
                Some(&id(406)),
                Some("6f7263686265652d6c6f63616c2d7631"),
            )
        } else {
            fixture.run(&args)
        };
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.success(), !fails, "{case}: {document}");
        if !fails {
            validate(
                &document,
                include_str!("../../../src/cli/schemas/orchbee.creation.result.json"),
            );
        }
    }
}

#[test]
fn plugin_tab_destination_preserves_matching_workspace_client_or_caller_child() {
    for caller in [true, false] {
        let fixture = fixture();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let before = scene();
            snapshot_reply(&mut stream, before.clone());
            let mut record = record(false, false, false);
            record["view"]["selection"] = json!({"terminalContextId":id(11),"tabId":id(16),"panelId":id(18),"zoomed":false,"panelCount":2});
            source::read(
                &mut stream,
                &before,
                (!caller).then_some(&record),
                318,
                false,
                false,
            );
            create(&mut stream, Some(18), Some(16), true, false);
        });
        let workspace = id(12);
        let args = [
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
        ];
        let output = if caller {
            fixture.run_with_caller(
                &args,
                Some(&id(418)),
                Some("6f7263686265652d6c6f63616c2d7631"),
            )
        } else {
            fixture.run(&args)
        };
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "caller={caller}: {document}");
        validate(
            &document,
            include_str!("../../../src/cli/schemas/orchbee.creation.result.json"),
        );
    }
}

#[test]
fn plugin_zoomed_different_target_reads_that_pane_before_forced_client_focus() {
    let fixture = fixture();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        let before = scene();
        snapshot_reply(&mut stream, before.clone());
        let original = record(false, false, false);
        list(&mut stream, Some(&original));
        list(&mut stream, Some(&original));
        source::read(&mut stream, &before, None, 108, false, false);
        fence(&mut stream, &original, false);
        fence(&mut stream, &original, false);
        submit_creation(&mut stream, Some(8), Some(5), false, true);
        assert!(observe(&mut stream, true, Cut::Complete));
        let request = view_request(&mut stream);
        let ClientViewAction::Submit { command } = &request.action else {
            panic!("forced focus and zoom");
        };
        assert_eq!(json!(command.expected_view_revision), "3");
        assert_eq!(
            command.minimum_workbench_revision,
            WorkbenchRevision::new(13)
        );
        assert_eq!(
            json!(command.intent),
            json!({"kind":"zoom","terminalContextId":id(4),"tabId":id(5),"panelId":id(20),"enabled":true})
        );
        let mut view = record(true, true, true)["view"].clone();
        view["selection"]["zoomed"] = json!(true);
        view_reply(
            &mut stream,
            &request,
            json!({"kind":"operation","operation":{"command":command,"status":{"kind":"applied","view":view}}}),
        );
        assert_closed(&mut stream);
    });
    let target = id(8);
    let output = fixture.run(&[
        "plugin",
        "pane",
        "open",
        "--plugin",
        "tools",
        "--entrypoint",
        "dashboard",
        "--placement",
        "zoomed",
        "--target-pane",
        &target,
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
    assert_eq!(document["version"], 4);
    assert_eq!(document["postCreate"]["requested"]["focus"], true);
    assert_eq!(document["postCreate"]["requested"]["zoom"], true);
}
