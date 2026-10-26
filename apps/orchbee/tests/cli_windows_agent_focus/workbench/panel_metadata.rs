//! Public reads must correlate batches, bindings and the final metadata fence.
use super::*;
fn snapshot(stream: &mut LocalStream, count: u64) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot")
    };
    let tabs:Vec<_>=(0..count).map(|n|json!({"tabId":id(1000+n),"label":"tab","pinned":false,"groupId":null,"root":panel(6+2*n)})).collect();
    let snapshot:Snapshot=serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":"12","navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},"terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":tabs}]})).unwrap();
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
fn reply(stream: &mut LocalStream, case: &str, turn: u64, count: u64) {
    let WorkbenchRequest::PanelMetadata(request) = read_workbench_request(stream).unwrap() else {
        panic!("metadata read")
    };
    let PanelMetadataAction::Read { panel_ids } = &request.action else {
        panic!("passive read only")
    };
    let expected: Vec<_> = if turn == 0 {
        (0..count.min(4)).map(|n| id(6 + 2 * n)).collect()
    } else if turn == 1 && count > 4 {
        vec![id(14)]
    } else {
        vec![]
    };
    assert_eq!(
        panel_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        expected
    );
    if case == "lost" {
        return;
    }
    if matches!(case, "total-deadline" | "report-total-deadline") {
        std::thread::sleep(Duration::from_millis(1800));
        if turn == 2 {
            return;
        }
    }
    let mut rows:Vec<_>=panel_ids.iter().map(|panel|{
 let n=u64::from_str_radix(panel.as_str(),16).unwrap();
 json!({"panelId":panel,"binding":{"workerId":"6f7263686265652d6c6f63616c2d7631","terminalId":id(n+200)},"tokens":{"branch":"中文","__proto__":"preserved"},"presentations":[{"source":"plain","agent":null,"appliesToSource":null,"title":{"value":"标题","order":"1"},"displayAgent":null,"stateLabels":{}},{"source":"guarded","agent":"codex","appliesToSource":"hook-a","title":{"value":"guarded","order":"2"},"displayAgent":null,"stateLabels":{}}]})
 }).collect();
    if !rows.is_empty() {
        match case {
            "wrong-row" => rows[0]["panelId"] = json!(id(999)),
            "wrong-binding" => rows[0]["binding"]["terminalId"] = json!(id(999)),
            _ => {}
        }
    }
    let outcome = match case {
        "denied" => json!({"kind":"failed","reason":"denied"}),
        "not-found" => json!({"kind":"failed","reason":"not-found"}),
        _ => {
            json!({"kind":"read","workbenchRevision":if case=="workbench-changed"{"13"}else{"12"},"metadataRevision":if (case=="batch-changed"&&turn==1)||(case=="fence-changed"&&panel_ids.is_empty()){"3"}else{"2"},"panels":rows})
        }
    };
    let response = PanelMetadataResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id,
        site_id: request.site_id,
        outcome: serde_json::from_value(outcome).unwrap(),
    };
    write_workbench_response(stream, &WorkbenchResponse::PanelMetadata(response)).unwrap();
}
fn report_reply(stream: &mut LocalStream, case: &str, turn: u64, count: u64) {
    let WorkbenchRequest::PanelAgent(request) = read_workbench_request(stream).unwrap() else {
        panic!("report read")
    };
    let PanelAgentAction::Read { panel_ids } = &request.action else {
        panic!("read only")
    };
    let expected: Vec<_> = if turn == 0 {
        (0..count.min(4)).map(|n| id(6 + 2 * n)).collect()
    } else if turn == 1 && count > 4 {
        vec![id(14)]
    } else {
        vec![]
    };
    assert_eq!(
        panel_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        expected
    );
    if case == "report-lost" {
        return;
    }
    if case == "report-total-deadline" {
        std::thread::sleep(Duration::from_millis(1800));
        return;
    }
    let mut panels: Vec<_> = panel_ids.iter().map(|panel| {
        let n = u64::from_str_radix(panel.as_str(), 16).unwrap();
        json!({"panelId":panel,"binding":{"workerId":"6f7263686265652d6c6f63616c2d7631","terminalId":id(n+200)},
            "report":{"agent":"codex","state":"blocked","authoritySource":"hook-a","message":"中文\n\u{1b}[31m"}})
    }).collect();
    if !panels.is_empty() {
        match case {
            "report-wrong-row" => panels[0]["panelId"] = json!(id(999)),
            "report-wrong-binding" => panels[0]["binding"]["terminalId"] = json!(id(999)),
            _ => {}
        }
    }
    let outcome = match case {
        "report-denied" => json!({"kind":"failed","reason":"denied"}),
        "report-not-found" => json!({"kind":"failed","reason":"not-found"}),
        _ => {
            json!({"kind":"read","workbenchRevision":if case=="report-workbench-changed"{"13"}else{"12"},
            "reportRevision":if (case=="report-batch-changed" && turn==1)||(case=="report-fence-changed" && panel_ids.is_empty()){"4"}else{"3"},"panels":panels})
        }
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
#[test]
fn panel_metadata_queries_enforce_exact_batches_binding_fences_and_original_deadline() {
    for case in [
        "stable",
        "get",
        "empty",
        "batch-changed",
        "fence-changed",
        "wrong-row",
        "wrong-binding",
        "not-found",
        "denied",
        "workbench-changed",
        "lost",
        "total-deadline",
        "report-batch-changed",
        "report-fence-changed",
        "report-wrong-row",
        "report-wrong-binding",
        "report-not-found",
        "report-denied",
        "report-workbench-changed",
        "report-lost",
        "report-total-deadline",
    ] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let count = if case == "empty" {
            0
        } else if case == "get" {
            1
        } else {
            5
        };
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot(&mut stream, count);
            super::metadata::empty_agents(&mut stream);
            for _ in 0..count {
                runtime::unavailable(&mut stream);
            }
            let batches = if count > 4 { 2 } else { 1 };
            for turn in 0..batches {
                reply(&mut stream, case, turn, count);
                if case == "lost" {
                    return;
                }
                if matches!(
                    case,
                    "wrong-row" | "wrong-binding" | "not-found" | "denied" | "workbench-changed"
                ) || (case == "batch-changed" && turn == 1)
                {
                    assert_closed(&mut stream);
                    return;
                }
            }
            for turn in 0..batches {
                report_reply(&mut stream, case, turn, count);
                if case == "report-lost" {
                    return;
                }
                if matches!(
                    case,
                    "report-wrong-row"
                        | "report-wrong-binding"
                        | "report-not-found"
                        | "report-denied"
                        | "report-workbench-changed"
                        | "report-total-deadline"
                ) || (case == "report-batch-changed" && turn == 1)
                {
                    assert_closed(&mut stream);
                    return;
                }
            }
            reply(&mut stream, case, batches, count);
            if matches!(case, "fence-changed" | "total-deadline") {
                assert_closed(&mut stream);
                return;
            }
            report_reply(&mut stream, case, batches, count);
            if case == "report-fence-changed" {
                assert_closed(&mut stream);
                return;
            }
            snapshot(&mut stream, count);
            assert_closed(&mut stream);
        });
        let target = id(6);
        let args = if case == "get" {
            vec!["panel", "get", &target, "--format=json"]
        } else {
            vec!["panel", "list", "--format=json"]
        };
        let start = Instant::now();
        let output = fixture.run(&args);
        let elapsed = start.elapsed();
        assert!(
            server.join().is_ok(),
            "{case}: peer failed after {elapsed:?}; CLI output: {output:?}"
        );
        let doc = document(&output);
        if matches!(case, "stable" | "get" | "empty") {
            assert!(output.status.success(), "{case}: {doc}");
            validate(
                &doc,
                if case == "get" {
                    include_str!("../../../src/cli/schemas/orchbee.panel.get.json")
                } else {
                    include_str!("../../../src/cli/schemas/orchbee.panel.list.json")
                },
            );
            assert_eq!(doc["version"], 1);
            if count > 0 {
                let row = if case == "get" {
                    &doc["result"]["panel"]
                } else {
                    &doc["result"]["panels"][0]
                };
                let md = &row["metadata"];
                assert_eq!(md["presentation"]["title"], "guarded");
                assert_eq!(md["presentations"].as_array().unwrap().len(), 2);
                assert_eq!(
                    md["context"],
                    json!({"agent":"codex","authoritySource":"hook-a"})
                );
                assert_eq!(
                    md["source"]["requestIds"].as_array().unwrap().len(),
                    if count > 4 { 3 } else { 2 }
                );
                assert_eq!(md["tokens"]["__proto__"], "preserved");
                assert_eq!(row["reportedAgent"]["view"]["state"], "blocked");
                assert_eq!(row["reportedAgent"]["source"]["reportRevision"], "3");
                assert_eq!(
                    row["reportedAgent"]["source"]["requestIds"]
                        .as_array()
                        .unwrap()
                        .len(),
                    if count > 4 { 3 } else { 2 }
                );
            }
            let schema = if case == "get" {
                include_str!("../../../src/cli/schemas/orchbee.panel.get.json")
            } else {
                include_str!("../../../src/cli/schemas/orchbee.panel.list.json")
            };
            let schema: Value = serde_json::from_str(schema).unwrap();
            let validator = jsonschema::validator_for(&schema).unwrap();
            assert!(validator.is_valid(&doc));
        } else {
            assert!(doc.get("result").is_none());
            assert_eq!(
                output.status.code(),
                Some(match case {
                    "batch-changed"
                    | "fence-changed"
                    | "not-found"
                    | "denied"
                    | "workbench-changed"
                    | "report-batch-changed"
                    | "report-fence-changed"
                    | "report-not-found"
                    | "report-denied"
                    | "report-workbench-changed" => 3,
                    "total-deadline" | "report-total-deadline" => 7,
                    _ => 5,
                }),
                "{case}: {doc}"
            );
            if matches!(case, "total-deadline" | "report-total-deadline") {
                assert!(
                    elapsed >= Duration::from_secs(5) && elapsed < Duration::from_secs(8),
                    "{elapsed:?}"
                );
            }
        }
    }
}
