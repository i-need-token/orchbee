//! Workbench query composition uses the same admitted pipe without state-changing controls.
#[path = "workbench/metadata.rs"]
mod metadata;
#[path = "workbench/panel_metadata.rs"]
mod panel_metadata;
use super::*;
use orchbee_agent_observation_protocol::{AgentResult, write_agent_result};
#[path = "workbench/runtime.rs"]
mod runtime;
#[path = "workbench/worktrees.rs"]
mod worktrees;

fn page(stream: &mut LocalStream, revision: u64) {
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("Agent list")
    };
    assert_eq!(json!(request)["kind"], "agent.session.list.request");
    assert_eq!(json!(request)["limit"], 100);
    assert_eq!(json!(request)["includeEnded"], false);
    assert!(json!(request)["cursor"].is_null());
    let items=[("primary",106), ("child",306)].map(|(name,presentation)| {
        let detail=client_get::completed();
        let mut item=json!({"sessionId":name,"pendingRequestCount":0,"status":detail["status"]});
        for key in ["executionLocation","lifecycle","observation","parentSessionId","providerId"] {item[key]=detail["snapshot"][key].clone();}
        item["presentationAttachments"]=json!([{"generation":"gen-2","navigable":true,"presentationId":id(presentation),"visibility":"visible"}]);
        item
    });
    write_agent_result(stream,&AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.list.result","requestId":request.request_id(),"result":{"outcome":{"kind":"listed","revision":revision,"items":items,"nextCursor":null}}})).unwrap())).unwrap();
}

fn viewport(stream: &mut LocalStream, case: &str) {
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("viewport read")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"viewports","panelIds":[id(6),id(8)]})
    );
    if case == "viewport-lost" {
        return;
    }
    if case == "viewport-timeout" {
        let command = command.clone();
        view_reply(
            stream,
            &request,
            json!({"kind":"operation","operation":{"command":command,"status":{"kind":"queued"}}}),
        );
        loop {
            let next = match read_workbench_request(stream) {
                Ok(WorkbenchRequest::ClientView(next)) => next,
                Err(WorkbenchWireError::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::UnexpectedEof
                    ) =>
                {
                    break;
                }
                Ok(_) => panic!("viewport timeout peer received a different request family"),
                Err(error) => panic!("viewport timeout request: {error}"),
            };
            assert!(
                matches!(&next.action, ClientViewAction::Read { operation_id } if *operation_id == command.operation_id)
            );
            let reply = WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                protocol_version: ProtocolVersion,
                request_id: next.request_id,
                session_id: next.session_id,
                outcome: serde_json::from_value(json!({"kind":"operation","operation":{"command":command,"status":{"kind":"queued"}}})).unwrap(),
            });
            match write_workbench_response(stream, &reply) {
                Ok(()) => {}
                Err(WorkbenchWireError::Io(error))
                    if error.kind() == std::io::ErrorKind::BrokenPipe =>
                {
                    break;
                }
                Err(error) => panic!("viewport timeout peer: {error}"),
            }
        }
        return;
    }
    let mut view = record(7, 6)["view"].clone();
    if case == "busy" {
        view["phase"] = json!("busy");
    }
    let primary = if case == "wrong-binding" { 207 } else { 206 };
    let panels = json!([
        {"panelId":id(6),"primaryTerminalId":id(primary),"observation":{"state":"observed","mode":"history","source":{"hostEpoch":id(50),"attachmentId":id(51),"generation":"1","revision":"5","columns":80,"alternateScreen":false,"totalRows":"400","startRow":"100"},"scroll":{"offsetFromBottom":"276","maxOffsetFromBottom":"376","viewportRows":24}}},
        {"panelId":id(8),"primaryTerminalId":id(208),"observation":{"state":"unavailable","reason":"not-attached"}}
    ]);
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"viewport-read","view":view,"viewport":{"workbenchRevision":"12","panels":panels}}}}),
    );
}

#[test]
fn workbench_queries_join_only_primary_agents_and_reject_changed_client_without_partial_output() {
    for case in [
        "headless",
        "seen",
        "unseen",
        "stale",
        "changed-view",
        "busy",
        "wrong-binding",
        "viewport-timeout",
        "viewport-lost",
    ] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            if case != "headless" {
                client_get::selected_reply(&mut stream, case == "busy");
            }
            page(&mut stream, 3);
            if case != "headless" {
                let view = client_get::selected_reply(&mut stream, case == "busy");
                let request = view_request(&mut stream);
                let ClientViewAction::Submit { command } = &request.action else {
                    panic!("read attention")
                };
                assert_eq!(
                    json!(command.intent),
                    json!({"kind":"agent-attention","sessionIds":["primary"]})
                );
                let generation = if case == "stale" { 2 } else { 3 };
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"operation","operation":{"command":command,"status":{"kind":"attention-read","view":view,"attention":{"revision":3,"cursor":attention::token(),"outerFocused":false,"entries":[{"sessionId":"primary","state":{"generation":generation,"sequence":11,"stateChangeRevision":3,"seen":case=="seen"}}]}}}}),
                );
                attention::authenticate(&mut stream, true);
                page(&mut stream, 3);
            }
            for _ in 0..2 {
                runtime::unavailable(&mut stream);
            }
            if case != "headless" {
                viewport(&mut stream, case);
            }
            if matches!(case, "wrong-binding" | "viewport-timeout" | "viewport-lost") {
                assert_closed(&mut stream);
                return;
            }
            snapshot_reply(&mut stream);
            if case == "changed-view" {
                let request = view_request(&mut stream);
                let mut client = record(7, 8);
                client["viewRevision"] = json!("3");
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"found","client":client}),
                );
            } else if case != "headless" {
                client_get::selected_reply(&mut stream, case == "busy");
            }
            assert_closed(&mut stream);
        });
        let client = id(7);
        let args = if case == "headless" {
            vec!["panel", "list", "--format=json"]
        } else {
            vec!["panel", "list", "--client", &client, "--format=json"]
        };
        let output = fixture.run(&args);
        server
            .join()
            .unwrap_or_else(|failure| panic!("case={case}; output={output:?}; peer={failure:?}"));
        let doc = document(&output);
        if matches!(case, "wrong-binding" | "viewport-timeout" | "viewport-lost") {
            assert_eq!(
                output.status.code(),
                Some(if case == "viewport-timeout" { 7 } else { 5 })
            );
            assert!(doc.get("result").is_none());
            continue;
        }
        if case == "changed-view" {
            assert_eq!(output.status.code(), Some(3));
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.cli.failure.workbench-query.json"),
            );
            assert_eq!(doc["failure"]["details"]["reason"], "client-changed");
            assert!(doc.get("result").is_none());
            continue;
        }
        assert!(output.status.success(), "{output:?}");
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.panel.list.json"),
        );
        assert_eq!(
            doc["result"]["agents"]["sessions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            doc["result"]["panels"][0]["agentSessionIds"],
            json!(["primary"])
        );
        assert_eq!(doc["result"]["panels"][0]["agentStatus"], json!("done"));
        assert_eq!(doc["result"]["panels"][1]["agentStatus"], "unknown");
        let row = &doc["result"]["panels"][0];
        if case == "headless" {
            assert!(row["scroll"].is_null());
            assert_eq!(row["scrollObservation"]["state"], "no-client");
        } else {
            assert_eq!(row["scroll"]["offsetFromBottom"], "276");
            assert_eq!(
                doc["result"]["client"]["operationIds"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
        }
        assert_eq!(
            doc["result"]["agents"]["requestIds"]
                .as_array()
                .unwrap()
                .len(),
            if case == "headless" { 1 } else { 2 }
        );
    }
}

#[test]
fn human_panel_list_uses_stable_handles_and_the_unified_agent_projection() {
    for (locale, headers, state) in [
        ("en_US.UTF-8", "HANDLE", "Done"),
        ("zh_CN.UTF-8", "句柄", "完成"),
    ] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut canonical = client_get::completed();
            canonical["snapshot"]["providerId"] = json!("codex");
            canonical["snapshot"]["presentationAttachments"] = json!([
                {"generation":id(1),"navigable":true,"presentationId":id(106),"visibility":"visible"},
                {"generation":id(1),"navigable":true,"presentationId":id(306),"visibility":"visible"}
            ]);
            let projection = AgentProjectionFixture::canonical(snapshot(12), 5, &[canonical])
                .with_state(
                    &id(106),
                    Some(peer_support::terminal_agent_state(
                        &id(106),
                        &id(1),
                        &id(2),
                        &id(206),
                        None,
                        "unknown",
                    )),
                )
                .with_state(
                    &id(306),
                    Some(peer_support::terminal_agent_state(
                        &id(306),
                        &id(1),
                        &id(2),
                        &id(406),
                        None,
                        "unknown",
                    )),
                );
            projection.read(&mut stream, false);
            super::metadata_peer::unavailable(&mut stream);
            super::metadata_peer::unavailable(&mut stream);
            projection.confirm(&mut stream, false);
            assert_closed(&mut stream);
        });
        let output = fixture.run_with_env(
            &["panel", "list"],
            None,
            None,
            &[("LC_ALL", std::ffi::OsStr::new(locale))],
        );
        server.join().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3, "{text}");
        assert!(lines[0].contains(headers), "{text}");
        assert!(text.contains("@panel:0x6"), "{text}");
        assert!(text.contains("@panel:0x8"), "{text}");
        assert!(text.contains("codex"), "{text}");
        assert!(text.contains(state), "{text}");
        assert!(
            lines.iter().all(|line| line.chars().count() <= 80),
            "{text}"
        );
    }
}
