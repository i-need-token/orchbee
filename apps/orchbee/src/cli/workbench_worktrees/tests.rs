use super::*;
use orchbee_workbench_protocol::WorktreeListStage;

fn id(n: u64) -> String {
    format!("{n:032x}")
}
pub(in crate::cli) fn unavailable(projection: &Projection) -> Option<Document> {
    let rows: Vec<_> = match projection {
        Projection::WorkspaceList(rows) => rows.iter().collect(),
        Projection::WorkspaceGet(doc) => vec![&doc.row],
        _ => return None,
    };
    Some(Document(
        rows.into_iter()
            .map(|row| {
                (
                    row.workspace_id.clone(),
                    Observation::Unavailable {
                        request_id: WorkbenchRequestId::parse(&id(90)).unwrap(),
                        reason: WorktreeQueryFailure::Unsupported {},
                    },
                )
            })
            .collect(),
    ))
}
fn fixture(linked: bool) -> (Snapshot, WorkspaceRow, WorkbenchWorktreeQueryOutcome) {
    let path = if linked {
        "C:/trees/中文/sub"
    } else {
        "C:/repo/sub"
    };
    let checkout = if linked { "C:/trees/中文" } else { "C:/repo" };
    let snapshot:Snapshot = serde_json::from_value(json!({"protocolVersion":1,"siteId":id(1),"revision":"7",
        "navigation":{"projects":[{"projectId":id(2), "label":"project"}], "standaloneTerminalContextId":id(7), "directories":[{"directoryId":id(4),"workerId":id(5),"path":path}], "workspaces":[{"workspaceId":id(3), "label":"work", "primaryDirectoryId":id(4), "terminalContextId":id(6), "projectId":id(2)}]},
        "terminalContexts":[{"terminalContextId":id(6),"groups":[],"tabs":[]},{"terminalContextId":id(7),"groups":[],"tabs":[]}]})).unwrap();
    let Projection::WorkspaceGet(doc) = super::super::results::project(
        &crate::invocation::QueryCommand::WorkspaceGet(crate::invocation::Target::Id(id(3))),
        &snapshot,
    )
    .unwrap() else {
        panic!()
    };
    let entry = |path: &str, linked: bool, selected: bool| {
        json!({"checkout":{"worktreePath":path,
        "head":"0123456789012345678901234567890123456789","branch":"refs/heads/main",
        "bare":false,"detached":false,"locked":false,"prunable":false},"linked":linked,
        "workspaces":if selected {json!([{"projectId":id(2),"workspaceId":id(3),"label":"work"}])} else {json!([])}})
    };
    let mut entries = vec![entry("C:/repo", false, !linked)];
    if linked {
        entries.push(entry(checkout, true, true));
    }
    let outcome=serde_json::from_value(json!({"kind":"found","siteId":id(1),"workbenchRevision":"7","workerId":id(5),
        "source":{"requestedPath":path,"repositoryRoot":"C:/repo","commonDirectory":"C:/repo/.git","checkoutPath":checkout,"workspaceId":id(3)},"entries":entries})).unwrap();
    (snapshot, doc.row, outcome)
}
#[test]
fn live_git_source_keeps_main_linked_and_subdirectory_identity() {
    for linked in [false, true] {
        let (snapshot, row, outcome) = fixture(linked);
        let observed = classify(
            &snapshot,
            &row,
            WorkbenchRequestId::parse(&id(90)).unwrap(),
            outcome,
        )
        .unwrap()
        .fields();
        assert_eq!(observed["worktree"]["repositoryName"], "repo");
        assert_eq!(observed["worktree"]["commonDirectory"], "C:/repo/.git");
        assert_eq!(observed["worktree"]["linked"], linked);
        assert_eq!(
            observed["worktree"]["checkoutPath"],
            if linked { "C:/trees/中文" } else { "C:/repo" }
        );
        assert_eq!(
            observed["worktreeObservation"]["source"]["workbenchRevision"],
            "7"
        );
    }
}
#[test]
fn source_binding_and_revision_mismatches_never_publish_a_summary() {
    let (snapshot, row, outcome) = fixture(true);
    for (pointer, value) in [
        ("/siteId", json!(id(99))),
        ("/workbenchRevision", json!("8")),
        ("/workerId", json!(id(99))),
        ("/source/requestedPath", json!("C:/elsewhere")),
        ("/source/workspaceId", json!(id(99))),
        ("/entries/1/workspaces/0/projectId", json!(id(99))),
        ("/entries/1/workspaces", json!([])),
    ] {
        let mut encoded = json!(outcome);
        *encoded.pointer_mut(pointer).unwrap() = value;
        assert!(
            classify(
                &snapshot,
                &row,
                WorkbenchRequestId::parse(&id(90)).unwrap(),
                serde_json::from_value(encoded).unwrap()
            )
            .is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn plugin_context_preserves_validated_native_checkout_identity_and_absence() {
    let (snapshot, row, outcome) = fixture(true);
    let observed = classify(
        &snapshot,
        &row,
        WorkbenchRequestId::parse(&id(90)).unwrap(),
        outcome,
    )
    .unwrap();
    let document = Document(BTreeMap::from([(row.workspace_id.clone(), observed)]));
    let context = document.plugin_context(&row.workspace_id).unwrap().unwrap();
    assert_eq!(context.checkout_path.as_str(), "C:/trees/中文");
    assert_eq!(context.common_directory.as_str(), "C:/repo/.git");
    assert!(context.linked);
    assert_eq!(context.worker_id.as_str(), id(5));
    assert!(document.plugin_context(&id(99)).is_err());
    let absent = Document(BTreeMap::from([(
        row.workspace_id.clone(),
        Observation::NoDirectory,
    )]));
    assert!(absent.plugin_context(&row.workspace_id).unwrap().is_none());
}
#[test]
fn unavailability_is_explicit_but_trust_transport_and_deadline_fail_whole_read() {
    let (snapshot, row, _) = fixture(false);
    for (reason, expected) in [
        (WorktreeQueryFailure::Unsupported {}, None),
        (
            WorktreeQueryFailure::GitFailed {
                stage: WorktreeListStage::List,
                status: Some(128),
            },
            None,
        ),
        (WorktreeQueryFailure::Denied {}, Some(FailureKind::Denied)),
        (WorktreeQueryFailure::Timeout {}, Some(FailureKind::Timeout)),
        (
            WorktreeQueryFailure::TransportLoss {},
            Some(FailureKind::TransportLoss),
        ),
        (
            WorktreeQueryFailure::MalformedListing {},
            Some(FailureKind::ProtocolInvalid),
        ),
    ] {
        let observed = classify(
            &snapshot,
            &row,
            WorkbenchRequestId::parse(&id(90)).unwrap(),
            WorkbenchWorktreeQueryOutcome::Failed { reason },
        );
        if let Some(kind) = expected {
            assert_eq!(observed.unwrap_err().kind, kind);
        } else {
            let fields = observed.unwrap().fields();
            assert!(fields["worktree"].is_null());
            assert_eq!(fields["worktreeObservation"]["state"], "unavailable");
        }
    }
    assert_eq!(
        Observation::NoDirectory.fields()["worktreeObservation"]["state"],
        "no-directory"
    );
}
