//! Public report uncertainty and consistent token batches over real Windows named pipes.
use super::*;
pub(super) fn read_reply(stream: &mut LocalStream, case: &str, turn: u64) {
    let WorkbenchRequest::WorkspaceMetadata(request) = read_workbench_request(stream).unwrap()
    else {
        panic!("metadata read")
    };
    let WorkspaceMetadataAction::Read { workspace_ids } = &request.action else {
        panic!("passive read only")
    };
    if case == "lost" {
        return;
    }
    if case == "total-deadline" {
        std::thread::sleep(Duration::from_millis(1800));
        if turn == 2 {
            return;
        }
    }
    let mut rows: Vec<_> = workspace_ids.iter().map(|id| json!({"workspaceId":id,"tokens":{"summary":"中文\u{202e}","__proto__":"preserved"}})).collect();
    if case == "wrong-row"
        && let Some(row) = rows.first_mut()
    {
        row["workspaceId"] = json!(id(999));
    }
    let outcome = if case == "not-found" {
        json!({"kind":"failed","reason":"not-found"})
    } else {
        json!({"kind":"read","workbenchRevision":if case=="workbench-changed" {"13"} else {"12"},
            "metadataRevision":if (case=="batch-changed" && turn==1) || (case=="fence-changed" && workspace_ids.is_empty()) {"2"} else {"1"}, "workspaces":rows})
    };
    let reply = WorkspaceMetadataResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id,
        site_id: request.site_id,
        outcome: serde_json::from_value(outcome).unwrap(),
    };
    write_workbench_response(stream, &WorkbenchResponse::WorkspaceMetadata(reply)).unwrap();
}
pub(super) fn empty_agents(stream: &mut LocalStream) {
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("Agent read")
    };
    write_agent_result(stream,&AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.list.result","requestId":request.request_id(),"result":{"outcome":{"kind":"listed","revision":5,"items":[],"nextCursor":null}}})).unwrap())).unwrap();
}
#[test]
fn workspace_metadata_query_batches_fail_closed_and_share_original_deadline() {
    for case in [
        "stable",
        "empty",
        "get",
        "batch-changed",
        "fence-changed",
        "wrong-row",
        "not-found",
        "workbench-changed",
        "lost",
        "total-deadline",
    ] {
        let fixture = Fixture::new_limited();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let count = if case == "empty" {
            0
        } else if case == "get" {
            1
        } else {
            10
        };
        let server = peer(&fixture, listener, move |mut stream| {
            super::worktrees::snapshot(&mut stream, count, true);
            empty_agents(&mut stream);
            let batches = if count > 8 { 2 } else { 1 };
            for turn in 0..=batches {
                read_reply(&mut stream, case, turn);
                if case == "lost" {
                    return;
                }
                if matches!(
                    case,
                    "lost" | "wrong-row" | "not-found" | "workbench-changed"
                ) || (case == "batch-changed" && turn == 1)
                    || (case == "fence-changed" && turn == batches)
                    || (case == "total-deadline" && turn == 2)
                {
                    assert_closed(&mut stream);
                    return;
                }
            }
            super::worktrees::snapshot(&mut stream, count, true);
            assert_closed(&mut stream);
        });
        let workspace = id(100);
        let args = if case == "get" {
            vec!["workspace", "get", &workspace, "--format=json"]
        } else {
            vec!["workspace", "list", "--format=json"]
        };
        let start = Instant::now();
        let output = fixture.run(&args);
        let elapsed = start.elapsed();
        server.join().unwrap();
        let doc = document(&output);
        if matches!(case, "stable" | "empty" | "get") {
            assert!(output.status.success(), "{case}: {doc}");
            validate(
                &doc,
                if case == "get" {
                    include_str!("../../../src/cli/schemas/orchbee.workspace.get.json")
                } else {
                    include_str!("../../../src/cli/schemas/orchbee.workspace.list.json")
                },
            );
            assert_eq!(doc["version"], 6);
            assert_eq!(doc["result"]["metadataSource"]["metadataRevision"], "1");
            assert_eq!(
                doc["result"]["metadataSource"]["requestIds"]
                    .as_array()
                    .unwrap()
                    .len(),
                if count > 8 { 3 } else { 2 }
            );
            if count > 0 {
                let row = if case == "get" {
                    &doc["result"]["workspace"]
                } else {
                    &doc["result"]["workspaces"][0]
                };
                assert_eq!(row["tokens"]["__proto__"], "preserved");
            }
        } else {
            assert!(doc.get("result").is_none());
            assert_eq!(
                output.status.code(),
                Some(match case {
                    "batch-changed" | "fence-changed" | "not-found" | "workbench-changed" => 3,
                    "total-deadline" => 7,
                    _ => 5,
                }),
                "{case}: {doc}"
            );
            if matches!(case, "batch-changed" | "fence-changed") {
                validate(
                    &doc,
                    include_str!(
                        "../../../src/cli/schemas/orchbee.cli.failure.workbench-query.json"
                    ),
                );
                assert_eq!(doc["failure"]["details"]["reason"], "metadata-changed");
            }
            if case == "total-deadline" {
                assert!(
                    elapsed >= Duration::from_secs(5) && elapsed < Duration::from_secs(8),
                    "{elapsed:?}"
                );
            }
        }
    }
}
#[test]
fn workspace_metadata_report_preserves_known_and_unknown_outcomes_without_replay() {
    for case in [
        "applied",
        "stale",
        "denied",
        "not-found",
        "lost",
        "wrong-request",
        "malformed",
    ] {
        let fixture = Fixture::new_limited();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            super::worktrees::snapshot(&mut stream, 1, true);
            let WorkbenchRequest::WorkspaceMetadata(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("report")
            };
            let WorkspaceMetadataAction::Report {
                workspace_id,
                source,
                tokens,
                sequence,
                ttl_milliseconds,
            } = &request.action
            else {
                panic!("report")
            };
            assert_eq!(workspace_id.as_str(), id(100));
            assert_eq!(source.as_str(), "hook:codex");
            assert_eq!(json!(tokens), json!({"summary":"中文值","clear":null}));
            assert_eq!(*sequence, Some(WorkbenchRevision::new(u64::MAX)));
            assert_eq!(*ttl_milliseconds, Some(86400000));
            if case == "lost" {
                return;
            }
            if case == "malformed" {
                stream.write_all(&[0; 12]).unwrap();
            } else {
                let outcome = if matches!(case, "denied" | "not-found") {
                    json!({"kind":"failed","reason":case})
                } else {
                    json!({"kind":"reported","workbenchRevision":"12","metadataRevision":"1","workspaceId":workspace_id,"disposition":if case=="stale" {"stale"} else {"applied"}})
                };
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorkspaceMetadata(WorkspaceMetadataResult {
                        protocol_version: ProtocolVersion,
                        request_id: if case == "wrong-request" {
                            WorkbenchRequestId::parse(&id(999)).unwrap()
                        } else {
                            request.request_id
                        },
                        site_id: request.site_id,
                        outcome: serde_json::from_value(outcome).unwrap(),
                    }),
                )
                .unwrap();
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "workspace",
            "report-metadata",
            "work",
            "--source= hook:codex ",
            "--token=summary= 中文\t值 ",
            "--clear-token=clear",
            "--seq=18446744073709551615",
            "--ttl-ms=86400000",
            "--format=json",
        ]);
        server.join().unwrap();
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../../../src/cli/schemas/orchbee.workspace.metadata.json"),
        );
        assert_eq!(doc["workspaceId"], id(100));
        assert_eq!(doc["siteId"], id(2));
        assert_eq!(
            output.status.code(),
            Some(match case {
                "applied" | "stale" => 0,
                "denied" | "not-found" => 3,
                _ => 8,
            }),
            "{case}: {doc}"
        );
        assert_eq!(
            doc["observation"]["kind"],
            if matches!(case, "lost" | "wrong-request" | "malformed") {
                "unknown"
            } else {
                "server"
            }
        );
    }
}
