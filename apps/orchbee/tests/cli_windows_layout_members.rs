//! Actual CLI receipts over controlled Windows peers; no claim of real provider or TUI acceptance.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use peer_support::{Fixture, assert_closed, peer, validate};
use serde_json::{Value, json};

#[path = "cli_windows_layout_members/headless.rs"]
mod headless;
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn panel(panel: u64, primary: u64, child: u64) -> Value {
    json!({"kind":"panel","panelId":id(panel),"rightClick":"panel",
        "primaryPresentation":{"presentationId":id(primary),"label":"Empty","binding":null},
        "childPresentations":[{"presentationId":id(child),"label":"Child","customLabel":"Manual","binding":null}]})
}
fn snapshot() -> Snapshot {
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":"12",
    "navigation":{"projects":[], "standaloneTerminalContextId":id(4), "directories":[], "workspaces":[]},"terminalContexts":[{"terminalContextId":id(4),"groups":[],
    "tabs":[{"tabId":id(5),"label":"Old","pinned":false,"groupId":null,"root":panel(6,8,9)}]}]})).unwrap()
}
fn client() -> Value {
    json!({"clientId":id(7),"heartbeat":"1","viewRevision":"2","availability":"current",
    "view":{"workbenchRevision":"12","phase":"ready","selection":{"terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"zoomed":false,"panelCount":1}}})
}
fn snapshot_reply(stream: &mut LocalStream, snapshot: Snapshot) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot read; no terminal query");
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    )
    .unwrap();
}
fn view_reply(stream: &mut LocalStream, request: &WorkbenchClientViewRequest, outcome: Value) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
fn setup(stream: &mut LocalStream) {
    snapshot_reply(stream, snapshot());
    let WorkbenchRequest::ClientView(request) = read_workbench_request(stream).unwrap() else {
        panic!("Client list");
    };
    assert!(matches!(request.action, ClientViewAction::List {}));
    view_reply(
        stream,
        &request,
        json!({"kind":"listed","clients":[client()]}),
    );
}
#[test]
fn unbound_export_preserves_members_without_runtime_reads_or_focus_changes() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        setup(&mut stream);
        let WorkbenchRequest::ClientView(request) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("layout read");
        };
        let ClientViewAction::Submit { command } = &request.action else {
            panic!();
        };
        assert!(
            matches!(&command.intent,ClientViewIntent::Layout { tab_id,panel_id,.. } if tab_id.as_str()==id(5) && panel_id.as_str()==id(6))
        );
        let rect = json!({"x":0,"y":0,"width":80,"height":24});
        let layout = json!({"terminalContextId":id(4),"tabId":id(5),"sourcePanelId":id(6),"workbenchRevision":"12",
            "viewport":rect,"area":rect,"selected":true,"zoomed":false,"focusedPanelId":id(6),"sourceAvailable":true,
            "neighbors":{"left":null,"right":null,"up":null,"down":null},"splits":[],
            "panels":[{"panelId":id(6),"unzoomed":rect,"terminal":rect,"displayed":rect}]});
        view_reply(
            &mut stream,
            &request,
            json!({"kind":"operation","operation":{"command":command,
            "status":{"kind":"layout-read","layout":layout,"view":client()["view"]}}}),
        );
        snapshot_reply(&mut stream, snapshot());
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "layout",
        "export",
        "--tab",
        &id(5),
        "--client",
        &id(7),
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
        include_str!("../src/cli/schemas/orchbee.layout.export.json"),
    );
    assert_eq!(document["version"], 2);
    let root = &document["result"]["layout"]["root"];
    assert_eq!(root["primary"]["start"], false);
    assert_eq!(root["children"][0]["start"], false);
    assert_eq!(root["children"][0]["label"], "Manual");
    assert_eq!(root["rightClick"], "panel");
}
#[test]
fn apply_records_structure_and_unknown_exact_child_launch_without_compensation_or_secret_leak() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let file = fixture.0.join("members.json");
    std::fs::write(&file,json!({"type":"panel","rightClick":"panel","primary":{"name":"Empty","start":false},
        "children":[{"name":"Child","label":"Manual","start":true,"cwd":fixture.0,
            "command":[fixture.executable(),"--version",""],"env":{"LAYOUT_SECRET":"do-not-print-this"}}]}).to_string()).unwrap();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        setup(&mut stream);
        snapshot_reply(&mut stream, snapshot());
        let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("structure first");
        };
        let WorkbenchAction::TabCreateLayout {
            layout,
            label,
            automatic_label,
            ..
        } = &command.action
        else {
            panic!("one atomic structure");
        };
        assert!(*automatic_label);
        assert_eq!(label.as_str(), "2");
        let WorkbenchLayoutNode::Panel {
            primary,
            children,
            right_click,
        } = layout.root()
        else {
            panic!();
        };
        assert_eq!(primary.label.as_str(), "Empty");
        assert_eq!(children.len(), 1);
        assert_eq!(*right_click, PanelRightClickTarget::Panel);
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::TabLayoutCreated {
                    revision: WorkbenchRevision::new(13),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
                    tab_id: WorkbenchTabId::parse(&id(11)).unwrap(),
                },
            }),
        )
        .unwrap();
        let mut fresh = snapshot();
        fresh.revision = WorkbenchRevision::new(13);
        fresh.terminal_contexts[0].tabs.push(serde_json::from_value(json!({"tabId":id(11),"label":"2","automaticLabel":true,"pinned":false,"groupId":null,"root":panel(12,13,14)})).unwrap());
        snapshot_reply(&mut stream, fresh.clone());
        snapshot_reply(&mut stream, fresh);
        let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("exact child launch");
        };
        assert_eq!(command.expected_revision, WorkbenchRevision::new(13));
        let WorkbenchAction::PanelCreateTerminal {
            target,
            options,
            launch,
            ..
        } = command.action
        else {
            panic!("bind existing member");
        };
        assert!(
            matches!(target,WorkbenchPanelTerminalTarget::Unbound { presentation_id } if presentation_id.as_str()==id(14))
        );
        assert_eq!(options.environment.entries()[0].1, "do-not-print-this");
        assert_eq!(launch.unwrap().arguments(), ["--version", ""]);
        // A lost post-submit reply leaves the exact request uncertain. The CLI must preserve the Tab.
    });
    let output = fixture.run(&[
        "layout",
        "apply",
        "--file",
        file.to_str().unwrap(),
        "--standalone",
        "--client",
        &id(7),
        "--format=json",
    ]);
    handler.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(8),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.layout.apply.json"),
    );
    assert_eq!(document["version"], 2);
    assert_eq!(document["result"]["newTabId"], id(11));
    assert_eq!(document["result"]["effects"].as_array().unwrap().len(), 2);
    assert_eq!(
        document["result"]["effects"][1]["document"]["result"]["kind"],
        "outcome-unknown"
    );
    assert_eq!(document["result"]["recovery"]["outcome"], "preserved");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("do-not-print-this"));
}
