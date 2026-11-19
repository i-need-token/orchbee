//! Process-level startup effects, wire identity and recovery on Windows pipes.
use super::peer_support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use peer_support::*;
use serde_json::{Value, json};

const PANEL: &str = "22222222222222222222222222222222";

fn shell(stream: &mut LocalStream) {
    process_reply(stream, vec![process(10, None, "cmd.exe")]);
}
fn accept_client(stream: &mut LocalStream) {
    use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    orchbee_workbench_client::test_support::local_client::handshake::answer_offer(stream).unwrap();
    assert!(matches!(
        read_message(stream).unwrap(),
        LocalClientMessage::ClientHello
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: std::num::NonZeroU64::new(1).unwrap(),
        },
    )
    .unwrap();
}
#[test]
fn new_tab_and_split_start_forward_cwd_and_guard_focus_before_and_after_creation() {
    use orchbee_workbench_protocol::*;

    let ctx_id = "00000000000000000000000000000001";
    let base_tab_id = "00000000000000000000000000000002";
    let new_tab_id = "00000000000000000000000000000019";
    let client_id = "77777777777777777777777777777777";
    let ready_record = move |rev: u64, view_rev: u64, panel_count: u64| {
        json!({
            "clientId": client_id,
            "heartbeat": "1",
            "viewRevision": view_rev.to_string(),
            "availability": "current",
            "view": {
                "workbenchRevision": rev.to_string(),
                "phase": "ready",
                "selection": {
                    "terminalContextId": ctx_id,
                    "tabId": base_tab_id,
                    "panelId": PANEL,
                    "zoomed": false,
                    "panelCount": panel_count,
                },
            },
        })
    };
    let reply_view =
        move |stream: &mut LocalStream, req: &WorkbenchClientViewRequest, outcome: Value| {
            write_workbench_response(
                stream,
                &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: req.request_id.clone(),
                    session_id: req.session_id.clone(),
                    outcome: serde_json::from_value(outcome).unwrap(),
                }),
            )
            .unwrap();
        };

    // 1. `--focus` with no Clients refuses before creation so no Panel is created.
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let no_client = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::ClientView(req) = read_workbench_request(&mut stream).unwrap() else {
            panic!("client list before creation")
        };
        assert!(matches!(req.action, ClientViewAction::List {}));
        reply_view(&mut stream, &req, json!({"kind": "listed", "clients": []}));
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--panel",
        PANEL,
        "--cwd",
        r"C:\proj",
        "--focus",
        "--format=json",
    ]);
    no_client.join().unwrap();
    assert_eq!(output.status.code(), Some(3));
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        doc["failure"]["details"]["createdPanelId"].is_null(),
        "{doc}"
    );

    // 2. `--cwd <abs>` + `--focus` on `--new-tab` succeeds and submits `ClientViewIntent::Panel`.
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let happy = std::thread::spawn(move || {
        // Connection 1: client discovery, tab creation, client observation + focus.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::ClientView(req) = read_workbench_request(&mut stream).unwrap() else {
            panic!("client list")
        };
        reply_view(
            &mut stream,
            &req,
            json!({"kind": "listed", "clients": [ready_record(9, 2, 1)]}),
        );
        let WorkbenchRequest::Command(cmd) = read_workbench_request(&mut stream).unwrap() else {
            panic!("tab create")
        };
        let action = json!(cmd.action);
        assert_eq!(action["kind"], "tab.create");
        assert_eq!(action["options"]["cwd"], r"C:\proj");
        assert_eq!(
            action["options"]["agentStart"],
            json!(AgentStartSelector::parse("amp").unwrap())
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: cmd.request_id,
                outcome: serde_json::from_value(json!({
                    "kind": "tab-create-pending",
                    "revision": "10",
                    "terminalContextId": ctx_id,
                    "tabId": new_tab_id,
                    "panelId": PANEL,
                    "presentationId": "11111111111111111111111111111111",
                }))
                .unwrap(),
            }),
        )
        .unwrap();
        let WorkbenchRequest::ClientView(get_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client observe")
        };
        reply_view(
            &mut stream,
            &get_req,
            json!({"kind": "found", "client": ready_record(10, 3, 1)}),
        );
        let WorkbenchRequest::ClientView(sub_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client focus submit")
        };
        let ClientViewAction::Submit { command } = &sub_req.action else {
            panic!("client submit")
        };
        assert_eq!(
            json!(command.intent),
            json!({"kind": "panel", "terminalContextId": ctx_id, "tabId": new_tab_id, "panelId": PANEL})
        );
        let mut applied_view = ready_record(10, 4, 1)["view"].clone();
        applied_view["selection"]["tabId"] = json!(new_tab_id);
        reply_view(
            &mut stream,
            &sub_req,
            json!({
                "kind": "operation",
                "operation": {
                    "command": command,
                    "status": {"kind": "applied", "view": applied_view},
                },
            }),
        );
        assert_closed(&mut stream);

        // Connection 2: launched-as-first-command verification returns immediately on interactive shell.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--panel",
        PANEL,
        "--cwd",
        r"C:\proj",
        "--focus",
        "--format=json",
    ]);
    happy.join().unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{doc}");
    assert_eq!(doc["result"]["confirmation"], "launched");

    // 3. `--cwd workspace --no-focus` leaves `options.cwd` unset and skips Client discovery.
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let no_focus = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::Command(cmd) = read_workbench_request(&mut stream).unwrap() else {
            panic!("tab create without client list")
        };
        let action = json!(cmd.action);
        assert_eq!(action["kind"], "tab.create");
        assert!(action["options"]["cwd"].is_null());
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: cmd.request_id,
                outcome: serde_json::from_value(json!({
                    "kind": "tab-create-pending",
                    "revision": "10",
                    "terminalContextId": ctx_id,
                    "tabId": new_tab_id,
                    "panelId": PANEL,
                    "presentationId": "11111111111111111111111111111111",
                }))
                .unwrap(),
            }),
        )
        .unwrap();
        assert_closed(&mut stream);

        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--panel",
        PANEL,
        "--cwd",
        "workspace",
        "--no-focus",
        "--format=json",
    ]);
    no_focus.join().unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{doc}");

    // 4. A split with `--focus` that the Client rejects still starts the Agent in the new Panel:
    //    exit 0, `result.focus` names the miss, and `panelId` is the new Panel, not the reference.
    let new_panel_id = "00000000000000000000000000000029";
    let new_terminal = "66666666666666666666666666666666";
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let rejected = std::thread::spawn(move || {
        // Connection 1: client discovery, split creation, client observation + rejected focus.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::ClientView(req) = read_workbench_request(&mut stream).unwrap() else {
            panic!("client list")
        };
        reply_view(
            &mut stream,
            &req,
            json!({"kind": "listed", "clients": [ready_record(9, 2, 1)]}),
        );
        let WorkbenchRequest::Command(cmd) = read_workbench_request(&mut stream).unwrap() else {
            panic!("panel split")
        };
        let action = json!(cmd.action);
        assert_eq!(action["kind"], "panel.split");
        assert_eq!(action["options"]["cwd"], r"C:\proj");
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: cmd.request_id,
                outcome: serde_json::from_value(json!({
                    "kind": "panel-split-pending",
                    "revision": "10",
                    "terminalContextId": ctx_id,
                    "tabId": base_tab_id,
                    "panelId": PANEL,
                    "newPanelId": new_panel_id,
                    "splitId": "00000000000000000000000000000030",
                    "firstSlotId": "00000000000000000000000000000031",
                    "secondSlotId": "00000000000000000000000000000032",
                    "presentationId": "11111111111111111111111111111111",
                }))
                .unwrap(),
            }),
        )
        .unwrap();
        let WorkbenchRequest::ClientView(get_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client observe")
        };
        reply_view(
            &mut stream,
            &get_req,
            json!({"kind": "found", "client": ready_record(10, 3, 2)}),
        );
        let WorkbenchRequest::ClientView(sub_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client focus submit")
        };
        let ClientViewAction::Submit { command } = &sub_req.action else {
            panic!("client submit")
        };
        assert_eq!(
            json!(command.intent),
            json!({"kind": "panel", "terminalContextId": ctx_id, "tabId": base_tab_id, "panelId": new_panel_id})
        );
        reply_view(
            &mut stream,
            &sub_req,
            json!({
                "kind": "operation",
                "operation": {
                    "command": command,
                    "status": {"kind": "rejected", "reason": "stale-view"},
                },
            }),
        );
        assert_closed(&mut stream);

        // Connection 2: the Agent launches in the new Panel as its first command. The confirmation
        // reads the Workbench after the split, so the snapshot holds the new Panel.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_split_snapshot_reply(&mut stream, 10, new_panel_id, new_terminal);
        // The Agent's shell runs in the new Panel's own terminal, bound in the split snapshot.
        process_sample_for(
            &mut stream,
            Ok(vec![process(10, None, "cmd.exe")]),
            new_terminal,
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--split=right",
        "--panel",
        PANEL,
        "--cwd",
        r"C:\proj",
        "--focus",
        "--format=json",
    ]);
    rejected.join().unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{doc}");
    assert_eq!(doc["result"]["confirmation"], "launched");
    assert_eq!(doc["result"]["panelId"], new_panel_id);
    assert_eq!(doc["result"]["focus"]["stage"], "focus");
    assert_eq!(doc["result"]["focus"]["reason"], "rejected");
}
