use super::*;

const HEAD40: &str = "0123456789abcdef0123456789abcdef01234567";
const HEAD64: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn read() -> WorkbenchWorktreeListRead {
    WorkbenchWorktreeListRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
        directory_id: WorkbenchDirectoryId::parse("44444444444444444444444444444444").unwrap(),
    }
}

fn result(outcome: WorkbenchWorktreeListOutcome) -> WorkbenchWorktreeListResult {
    WorkbenchWorktreeListResult {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
        outcome,
    }
}

fn utf8(text: &str) -> WorktreeEntryText {
    WorktreeEntryText::Utf8(text.to_owned())
}

fn found(entries: Vec<WorktreeEntry>) -> WorkbenchWorktreeListOutcome {
    WorkbenchWorktreeListOutcome::Found {
        site_id: SiteId::parse("22222222222222222222222222222222").unwrap(),
        workbench_revision: WorkbenchRevision::parse("7").unwrap(),
        directory_id: WorkbenchDirectoryId::parse("44444444444444444444444444444444").unwrap(),
        path: WorkbenchDirectoryPath::parse("/repo").unwrap(),
        entries,
    }
}

fn entry() -> WorktreeEntry {
    WorktreeEntry {
        path: utf8("/repo/main"),
        head: Some(HEAD40.to_owned()),
        branch: Some(utf8("refs/heads/main")),
        bare: false,
        detached: false,
        locked: false,
        prunable: false,
    }
}

fn roundtrip_response(result: WorkbenchWorktreeListResult) -> WorkbenchResponse {
    let encoded = serialize_workbench_response(&WorkbenchResponse::WorktreeList(result)).unwrap();
    parse_workbench_response(&encoded).unwrap()
}

#[test]
fn read_request_roundtrips_through_json() {
    let request = WorkbenchRequest::WorktreeListRead(read());
    let encoded = serialize_workbench_request(&request).unwrap();
    let parsed = parse_workbench_request(&encoded).unwrap();
    assert_eq!(parsed, request);
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["kind"], "workbench.worktree-list.read");
}

#[test]
fn found_outcome_roundtrips_with_every_entry_shape() {
    let entries = vec![
        entry(),
        // An unborn repository: the all-zero HEAD is preserved verbatim.
        WorktreeEntry {
            path: utf8("/repo/unborn"),
            head: Some("0".repeat(64)),
            branch: Some(utf8("refs/heads/trunk")),
            ..entry()
        },
        // The bare repository: head and branch are absent.
        WorktreeEntry {
            path: utf8("/repo/bare.git"),
            head: None,
            branch: None,
            bare: true,
            ..entry()
        },
        // A detached worktree keeps its HEAD but has no branch.
        WorktreeEntry {
            path: utf8("/repo/detached"),
            head: Some(HEAD64.to_owned()),
            branch: None,
            detached: true,
            ..entry()
        },
        // Raw bytes for non-UTF-8 paths and refs, plus flag coverage.
        WorktreeEntry {
            path: WorktreeEntryText::Bytes(b"/repo/\xff".to_vec()),
            head: Some(HEAD40.to_owned()),
            branch: Some(WorktreeEntryText::Bytes(b"refs/heads/\xff".to_vec())),
            locked: true,
            prunable: true,
            ..entry()
        },
    ];
    let request = result(found(entries.clone()));
    match roundtrip_response(request) {
        WorkbenchResponse::WorktreeList(parsed) => {
            let WorkbenchWorktreeListOutcome::Found {
                entries: parsed_entries,
                ..
            } = parsed.outcome
            else {
                panic!("found outcome must stay found");
            };
            assert_eq!(parsed_entries, entries);
        }
        other => panic!("unexpected response variant: {other:?}"),
    }
}

#[test]
fn found_json_keeps_the_verbatim_branch_and_null_head_shapes() {
    let encoded =
        serialize_workbench_response(&WorkbenchResponse::WorktreeList(result(found(vec![
            entry(),
            WorktreeEntry {
                path: utf8("/repo/bare.git"),
                head: None,
                branch: None,
                bare: true,
                ..entry()
            },
        ]))))
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["kind"], "workbench.worktree-list.result");
    let entries = &value["outcome"]["entries"];
    assert_eq!(entries[0]["branch"], "refs/heads/main");
    assert_eq!(entries[0]["head"], HEAD40);
    assert_eq!(entries[1]["head"], serde_json::Value::Null);
    assert_eq!(entries[1]["branch"], serde_json::Value::Null);
}

#[test]
fn every_failure_outcome_roundtrips() {
    for outcome in [
        WorkbenchWorktreeListOutcome::NotFound {},
        WorkbenchWorktreeListOutcome::Denied {},
        WorkbenchWorktreeListOutcome::Unavailable {
            reason: WorktreeListUnavailableReason::ServerBusy,
        },
        WorkbenchWorktreeListOutcome::Unavailable {
            reason: WorktreeListUnavailableReason::WorkerUnavailable,
        },
        WorkbenchWorktreeListOutcome::Unavailable {
            reason: WorktreeListUnavailableReason::JobDead,
        },
        WorkbenchWorktreeListOutcome::Unavailable {
            reason: WorktreeListUnavailableReason::JobUnknown,
        },
        WorkbenchWorktreeListOutcome::GitFailed {
            stage: WorktreeListStage::Probe,
            status: Some(128),
        },
        WorkbenchWorktreeListOutcome::GitFailed {
            stage: WorktreeListStage::List,
            status: None,
        },
        WorkbenchWorktreeListOutcome::SpawnUnavailable {
            stage: WorktreeListStage::Probe,
        },
        WorkbenchWorktreeListOutcome::SpawnFailed {
            stage: WorktreeListStage::List,
        },
        WorkbenchWorktreeListOutcome::TooLarge {
            origin: WorktreeListTooLargeOrigin::GitProbe,
        },
        WorkbenchWorktreeListOutcome::TooLarge {
            origin: WorktreeListTooLargeOrigin::GitList,
        },
        WorkbenchWorktreeListOutcome::TooLarge {
            origin: WorktreeListTooLargeOrigin::WorkerResult,
        },
        WorkbenchWorktreeListOutcome::TooLarge {
            origin: WorktreeListTooLargeOrigin::WorkbenchResult,
        },
        WorkbenchWorktreeListOutcome::Timeout {},
        WorkbenchWorktreeListOutcome::OwnerIo {
            stage: Some(WorktreeListStage::Probe),
        },
        WorkbenchWorktreeListOutcome::OwnerIo { stage: None },
        WorkbenchWorktreeListOutcome::TransportLoss {},
        WorkbenchWorktreeListOutcome::MalformedListing {},
    ] {
        let response = WorkbenchResponse::WorktreeList(result(outcome));
        let encoded = serialize_workbench_response(&response).unwrap();
        assert_eq!(parse_workbench_response(&encoded).unwrap(), response);
    }
}

#[test]
fn entry_invariants_are_enforced_on_decode() {
    let valid = serde_json::to_value(entry()).unwrap();
    let accept = |mut value: serde_json::Value| {
        // Bare entries legitimately drop head/branch; callers fix the flags.
        let outcome = found(vec![
            serde_json::from_value(std::mem::take(&mut value)).unwrap(),
        ]);
        serialize_workbench_response(&WorkbenchResponse::WorktreeList(result(outcome))).unwrap();
    };
    accept(valid.clone());

    let reject = |mut entry_value: serde_json::Value, patch: serde_json::Value| {
        entry_value.as_object_mut().unwrap().extend(
            patch
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        let document = serde_json::json!({
            "kind": "workbench.worktree-list.result",
            "protocolVersion": 1,
            "requestId": "11111111111111111111111111111111",
            "outcome": {
                "kind": "found",
                "siteId": "22222222222222222222222222222222",
                "workbenchRevision": "7",
                "directoryId": "44444444444444444444444444444444",
                "path": "/repo",
                "entries": [entry_value],
            },
        });
        assert!(
            parse_workbench_response(&serde_json::to_vec(&document).unwrap()).is_err(),
            "{entry_value} must be rejected"
        );
    };

    // Both path spellings at once.
    reject(
        valid.clone(),
        serde_json::json!({"worktreePathBytesHex": "2f61"}),
    );
    // Missing path entirely.
    let mut missing_path = valid.clone();
    missing_path.as_object_mut().unwrap().remove("worktreePath");
    reject(missing_path, serde_json::json!({}));
    // Both branch spellings at once.
    reject(
        valid.clone(),
        serde_json::json!({"branchBytesHex": "726566732f68656164732f6d61696e"}),
    );
    // Malformed hex and odd-length hex.
    reject(valid.clone(), serde_json::json!({"branchBytesHex": "zz"}));
    reject(valid.clone(), serde_json::json!({"branchBytesHex": "abc"}));
    // Bad HEAD values: uppercase, wrong length, non-bare null.
    reject(valid.clone(), serde_json::json!({"head": "A".repeat(40)}));
    reject(valid.clone(), serde_json::json!({"head": "0".repeat(39)}));
    reject(valid.clone(), serde_json::json!({"head": null}));
    // Bare entries cannot carry head/branch/detached.
    reject(valid.clone(), serde_json::json!({"bare": true}));
    reject(
        valid.clone(),
        serde_json::json!({"bare": true, "head": null, "branch": null, "detached": true}),
    );
    // Detached entries cannot carry a branch.
    reject(valid.clone(), serde_json::json!({"detached": true}));
    // A relative path.
    reject(valid.clone(), serde_json::json!({"worktreePath": "repo/x"}));
    // Unknown fields are denied.
    reject(valid.clone(), serde_json::json!({"surprise": 1}));
}

#[test]
fn the_dedicated_32kib_result_bound_is_enforced_both_ways() {
    let big = "/".to_owned() + &"a".repeat(64 * 1024);
    let response = WorkbenchResponse::WorktreeList(result(found(vec![WorktreeEntry {
        path: utf8(&big),
        ..entry()
    }])));
    assert!(matches!(
        serialize_workbench_response(&response),
        Err(ContractError::EncodedValueTooLarge { .. })
    ));
}
