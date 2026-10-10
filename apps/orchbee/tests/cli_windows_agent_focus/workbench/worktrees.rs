//! Workspace reads preserve one connection, original deadline and exact Git-source membership.
use super::*;

pub(super) fn snapshot(stream: &mut LocalStream, count: u64, no_directory: bool) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!()
    };
    let mut contexts = vec![json!({"terminalContextId":id(4),"groups":[],"tabs":[]})];
    let rows:Vec<_>=(0..count).map(|n| {
        contexts.push(json!({"terminalContextId":id(200+n),"groups":[],"tabs":[]}));
        json!({"workspaceId":id(100+n),"projectId":id(30),"label":"work","primaryDirectoryId":if no_directory {Value::Null} else {json!(id(31))},"terminalContextId":id(200+n)})
    }).collect();
    let value = json!({"protocolVersion":1,"siteId":id(2),"revision":"12",
        "navigation":{"standaloneTerminalContextId":id(4),"projects":[{"projectId":id(30),"label":"project"}],"directories":[{"directoryId":id(31),"workerId":id(32),"path":"C:/repo/sub"}],"workspaces":rows},"terminalContexts":contexts});
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: serde_json::from_value(value).unwrap(),
            },
        }),
    )
    .unwrap();
}
fn reply(stream: &mut LocalStream, request: WorkbenchWorktreeQuery, case: &str, n: u64) {
    let outcome = if case == "unavailable" {
        json!({"kind":"failed","reason":{"kind":"git-failed","stage":"probe","status":128}})
    } else {
        json!({"kind":"found","siteId":id(2),"workbenchRevision":if case=="changed-revision" {"13"} else {"12"},
            "workerId":id(if case=="wrong-worker" {33} else {32}),
            "source":{"requestedPath":if case=="wrong-path" {"C:/other"} else {"C:/repo/sub"},"repositoryRoot":"C:/repo","commonDirectory":"C:/repo/.git","checkoutPath":"C:/repo","workspaceId":id(100+n)},
            "entries":[{"checkout":{"worktreePath":"C:/repo","head":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","branch":"refs/heads/main","bare":false,"detached":false,"locked":false,"prunable":false},"linked":false,"workspaces":[{"workspaceId":id(100+n),"projectId":id(30),"label":"work"}]}]})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::WorktreeQuery(WorkbenchWorktreeQueryResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
#[test]
fn workspace_queries_join_git_without_trust_and_fail_closed_on_changed_or_lost_sources() {
    for case in [
        "found",
        "unavailable",
        "no-directory",
        "wrong-worker",
        "wrong-path",
        "changed-revision",
        "lost",
        "total-deadline",
    ] {
        let fixture = FocusFixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let count = if case == "total-deadline" { 3 } else { 1 };
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot(&mut stream, count, case == "no-directory");
            page(&mut stream, 3);
            if case != "no-directory" {
                for n in 0..count {
                    let WorkbenchRequest::WorktreeQuery(request) =
                        read_workbench_request(&mut stream).unwrap()
                    else {
                        panic!("one read-only Git query")
                    };
                    assert!(!request.trust_repository);
                    assert_eq!(
                        json!(request.source),
                        json!({"kind":"workspace","workspaceId":id(100+n)})
                    );
                    if case == "lost" {
                        return;
                    }
                    if case == "total-deadline" {
                        std::thread::sleep(Duration::from_millis(1800));
                        if n == 2 {
                            assert_closed(&mut stream);
                            return;
                        }
                    }
                    reply(&mut stream, request, case, n);
                }
            }
            if matches!(case, "wrong-worker" | "wrong-path" | "changed-revision") {
                assert_closed(&mut stream);
                return;
            }
            for _ in 0..2 {
                super::metadata::read_reply(&mut stream, "stable", 0);
            }
            snapshot(&mut stream, count, case == "no-directory");
            assert_closed(&mut stream);
        });
        let started = Instant::now();
        let output = fixture.run(&["workspace", "list", "--format=json"]);
        let elapsed = started.elapsed();
        server.join().unwrap();
        let doc = document(&output);
        if matches!(case, "found" | "unavailable" | "no-directory") {
            assert!(output.status.success(), "{case}: {doc}");
            validate(
                &doc,
                include_str!("../../../src/cli/schemas/orchbee.workspace.list.json"),
            );
            let row = &doc["result"]["workspaces"][0];
            assert_eq!(doc["version"], 7);
            assert_eq!(
                row["worktreeObservation"]["state"],
                match case {
                    "found" => "observed",
                    other => other,
                }
            );
            if case == "found" {
                assert_eq!(row["worktree"]["checkoutPath"], "C:/repo");
                assert_eq!(row["worktree"]["linked"], false);
            } else {
                assert!(row["worktree"].is_null());
            }
        } else {
            assert_eq!(
                output.status.code(),
                Some(match case {
                    "changed-revision" => 3,
                    "total-deadline" => 7,
                    _ => 5,
                }),
                "{case}: {doc}"
            );
            assert!(doc.get("result").is_none());
            if case == "total-deadline" {
                assert!(
                    elapsed >= Duration::from_secs(5) && elapsed < Duration::from_secs(8),
                    "{elapsed:?}"
                );
            }
        }
    }
}
