//! Actual CLI named-pipe traces: child selection and publication fences.
use super::*;

pub(super) fn read(
    stream: &mut LocalStream,
    client: u64,
    panel: u64,
    child: bool,
    generation: u64,
    case: &str,
) {
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("passive Presentation read");
    };
    assert_eq!(command.client_id.as_str(), id(client));
    assert_eq!(json!(command.expected_view_revision), "2");
    assert_eq!(json!(command.minimum_workbench_revision), "12");
    assert_eq!(
        json!(command.intent),
        json!({"kind":"presentation", "panelId":id(panel)})
    );
    let mut view = record(client, panel)["view"].clone();
    if case == "view-moved" {
        view["selection"]["panelId"] = json!(id(8));
    }
    let member_panel = if case == "wrong-owner" { 8 } else { panel };
    let mut member = json!({"presentationId":id(member_panel + if child {300} else {100}),
        "binding":{"terminalId":id(member_panel + if child {400} else {200}), "workerId":"6f7263686265652d6c6f63616c2d7631"},
        "attachmentGeneration":generation.to_string(), "incarnation":0,
        "viewport":{"state":"unavailable","reason":"frame-unavailable"}});
    if case == "missing" {
        member = Value::Null;
    }
    if case == "incarnation" {
        member["incarnation"] = json!(1);
    }
    if case == "scroll-change" {
        member["viewport"] = json!({"state":"pending", "phase":"entering"});
    }
    let mut command = json!(command);
    if case == "wrong-command" {
        command["operationId"] = json!(id(999));
    }
    view_reply(
        stream,
        &request,
        json!({"kind":"operation", "operation":{"command":command,
        "status":{"kind":"presentation-read", "view":view, "presentation":{
            "workbenchRevision":"12", "panelId":id(panel), "presentation":member}}}}),
    );
}

#[test]
fn process_info_uses_exact_child_and_rejects_member_changes_before_publication() {
    for case in [
        "stable",
        "scroll-change",
        "away-and-back",
        "incarnation",
        "rebound",
        "wrong-owner",
        "missing",
        "view-moved",
        "wrong-command",
        "workbench-changed",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            list_reply(&mut stream, vec![record(10, 6)]);
            let invalid_first = matches!(
                case,
                "wrong-owner" | "missing" | "view-moved" | "wrong-command"
            );
            read(
                &mut stream,
                10,
                6,
                true,
                7,
                if invalid_first { case } else { "stable" },
            );
            if !invalid_first {
                process_read(&mut stream, 406, false);
                snapshot_reply_at(
                    &mut stream,
                    if case == "workbench-changed" { 13 } else { 12 },
                );
                if case != "workbench-changed" {
                    read(
                        &mut stream,
                        10,
                        6,
                        case != "rebound",
                        if case == "away-and-back" { 8 } else { 7 },
                        case,
                    );
                }
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["panel", "process-info", "--format=json"]);
        server.join().unwrap();
        let doc = document(&output);
        if matches!(case, "stable" | "scroll-change") {
            assert!(output.status.success(), "{case}: {output:?}");
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.panel.process-info.json"),
            );
            assert_eq!(doc["result"]["terminal"]["terminalId"], id(406));
            assert_eq!(
                doc["result"]["terminal"]["boundTo"]["presentationId"],
                id(306)
            );
        } else {
            assert!(!output.status.success(), "{case}: {output:?}");
            assert!(doc.get("result").is_none());
            if matches!(
                case,
                "away-and-back" | "incarnation" | "rebound" | "view-moved" | "workbench-changed"
            ) {
                validate(
                    &doc,
                    include_str!("../../src/cli/schemas/orchbee.cli.failure.workbench-query.json"),
                );
                assert_eq!(
                    doc["failure"]["details"]["reason"],
                    if case == "workbench-changed" {
                        "workbench-changed"
                    } else {
                        "client-changed"
                    }
                );
            } else if case == "missing" {
                validate(
                    &doc,
                    include_str!("../../src/cli/schemas/orchbee.cli.failure.client-control.json"),
                );
                assert_eq!(
                    doc["failure"]["details"]["reason"],
                    "client-panel-unavailable"
                );
            } else {
                validate(
                    &doc,
                    include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
                );
            }
        }
    }
}
