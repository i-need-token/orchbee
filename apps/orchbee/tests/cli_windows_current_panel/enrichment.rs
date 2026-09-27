//! Current selection is pinned before joins, including a move away and back.
use super::*;
use orchbee_agent_observation_protocol::{
    AgentRequest, AgentResult, read_agent_request, write_agent_result,
};

pub(super) fn read(stream: &mut LocalStream, panel: u64, client: Option<u64>, case: &str) {
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("read-only Agent list")
    };
    assert_eq!(json!(request)["kind"], "agent.session.list.request");
    write_agent_result(
        stream,
        &AgentResult::Read(
            serde_json::from_value(json!({
                "kind":"agent.session.list.result", "requestId":request.request_id(),
                "result":{"outcome":{"kind":"listed","revision":3,"items":[],"nextCursor":null}}
            }))
            .unwrap(),
        ),
    )
    .unwrap();
    let request = viewer(stream);
    assert_eq!(
        json!(request.action()),
        json!({"kind":"attach-binding", "siteId":id(2),
        "workerId":"6f7263686265652d6c6f63616c2d7631", "terminalId":id(panel+200)})
    );
    answer(
        stream,
        &request,
        c::TerminalViewerOutcome::Unavailable {
            reason: c::ViewerUnavailableReason::TerminalRuntimeUnavailable,
        },
    );
    if let Some(client) = client {
        let request = view_request(stream);
        let ClientViewAction::Submit { command } = &request.action else {
            panic!("read-only viewport operation")
        };
        assert_eq!(command.client_id.as_str(), id(client));
        assert_eq!(json!(command.expected_view_revision), "2");
        assert_eq!(json!(command.minimum_workbench_revision), "12");
        assert_eq!(
            json!(command.intent),
            json!({"kind":"viewports","panelIds":[id(panel)]})
        );
        let mut view =
            record(client, if case == "viewport-moved" { 8 } else { panel })["view"].clone();
        if case == "busy" {
            view["phase"] = json!("busy");
        }
        let observation = json!({"state":"observed","mode":"history",
            "source":{"hostEpoch":id(50),"attachmentId":id(51),"generation":"1","revision":"5",
                "columns":80,"alternateScreen":false,"totalRows":"400","startRow":"100"},
            "scroll":{"offsetFromBottom":"276","maxOffsetFromBottom":"376","viewportRows":24}});
        view_reply(
            stream,
            &request,
            json!({"kind":"operation","operation":{"command":command,
            "status":{"kind":"viewport-read","view":view,"viewport":{"workbenchRevision":"12",
                "panels":[{"panelId":id(panel),"primaryTerminalId":id(panel+200),"observation":observation}]}}}}),
        );
        if case == "viewport-moved" {
            return;
        }
    }
    snapshot_reply(stream);
    if let Some(client) = client {
        let request = view_request(stream);
        assert!(
            matches!(&request.action, ClientViewAction::Get { client_id } if client_id.as_str() == id(client))
        );
        let mut selected = record(client, if case == "final-moved" { 8 } else { panel });
        if case == "busy" {
            selected["view"]["phase"] = json!("busy");
        }
        if matches!(case, "final-moved" | "away-and-back") {
            selected["viewRevision"] = json!("4");
        }
        view_reply(stream, &request, json!({"kind":"found","client":selected}));
    }
}

#[test]
fn current_selection_cannot_move_during_joins_even_if_it_moves_back() {
    for case in ["viewport-moved", "final-moved", "away-and-back"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            list_reply(&mut stream, vec![record(10, 6)]);
            read(&mut stream, 6, Some(10), case);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["panel", "current", "--format=json"]);
        server.join().unwrap();
        assert_eq!(output.status.code(), Some(3), "{case}: {output:?}");
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.workbench-query.json"),
        );
        assert_eq!(doc["failure"]["details"]["reason"], "client-changed");
        assert!(doc.get("result").is_none());
    }
}

#[test]
fn enriched_current_schema_requires_complete_facts_and_keeps_legacy_v3_closed() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let server = peer(&fixture, listener, move |mut stream| {
        snapshot_reply(&mut stream);
        read(&mut stream, 6, None, "stable");
        assert_closed(&mut stream);
    });
    let output = fixture.run(&["panel", "current", "--panel", &id(6), "--format=json"]);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    let doc = document(&output);
    let schema: Value = serde_json::from_str(include_str!(
        "../../src/cli/schemas/orchbee.panel.current.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    assert!(validator.is_valid(&doc));
    for field in [
        "agentSessionIds",
        "agentStatus",
        "cwd",
        "cwdSource",
        "foregroundCwd",
        "terminalTitle",
        "terminalTitleStripped",
        "terminalObservation",
        "scroll",
        "scrollObservation",
    ] {
        let mut missing = doc.clone();
        missing["result"]["panel"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(!validator.is_valid(&missing), "required {field}");
    }
    let mut legacy = doc.clone();
    legacy["version"] = json!(3);
    assert!(!validator.is_valid(&legacy));
    legacy["result"].as_object_mut().unwrap().remove("agents");
    legacy["result"]["panel"]
        .as_object_mut()
        .unwrap()
        .retain(|key, _| {
            [
                "panelId",
                "rightClick",
                "tabId",
                "context",
                "slotPath",
                "primaryPresentation",
                "childPresentations",
            ]
            .contains(&key.as_str())
        });
    assert!(validator.is_valid(&legacy));
    legacy["version"] = json!(4);
    assert!(!validator.is_valid(&legacy));
}

#[test]
fn current_can_observe_busy_history_without_relaxing_process_or_membership_guards() {
    for case in ["busy", "process", "wrong-tab"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut client = record(10, 6);
            client["view"]["phase"] = json!("busy");
            if case == "wrong-tab" {
                client["view"]["selection"]["tabId"] = json!(id(99));
            }
            list_reply(&mut stream, vec![client]);
            if case == "busy" {
                read(&mut stream, 6, Some(10), case);
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "panel",
            if case == "process" {
                "process-info"
            } else {
                "current"
            },
            "--format=json",
        ]);
        server.join().unwrap();
        let doc = document(&output);
        if case == "busy" {
            assert!(output.status.success(), "{output:?}");
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.panel.current.json"),
            );
            assert_eq!(doc["result"]["client"]["view"]["phase"], "busy");
            assert_eq!(doc["result"]["panel"]["scroll"]["offsetFromBottom"], "276");
            assert_eq!(doc["result"]["panel"]["focused"], true);
        } else {
            assert!(!output.status.success(), "{output:?}");
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.cli.failure.client-control.json"),
            );
            assert_eq!(
                doc["failure"]["details"]["reason"],
                if case == "process" {
                    "client-not-ready"
                } else {
                    "client-panel-unavailable"
                }
            );
        }
    }
}
