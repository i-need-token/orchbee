use super::*;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

fn empty_workspace() -> (WorkbenchAuthority, WorkbenchGrant, WorkbenchCommand) {
    let (mut authority, navigation) = authority();
    let created = submit(
        &mut authority,
        &navigation,
        command(201, 0, create("Project")),
    );
    let project_id = project_id(&created);
    let created = submit(
        &mut authority,
        &navigation,
        command(
            202,
            1,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id.clone()),
                label: WorkbenchLabel::parse("Workspace").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = created.outcome else {
        panic!("expected Workspace")
    };
    let associate = command(
        203,
        2,
        WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id,
            path: WorkbenchDirectoryPath::parse("/tmp/workspace/link/../directory").unwrap(),
        },
    );
    (
        authority,
        navigation.with_directory_association(),
        associate,
    )
}

fn worker() -> WorkerId {
    WorkerId::parse(&identity(800)).unwrap()
}

#[test]
fn association_commits_one_directory_and_exact_replay_preserves_every_owner() {
    let (mut authority, grant, command) = empty_workspace();
    let before = authority.projection.snapshot().clone();
    let cursor = authority.last_identity;
    let entries = authority.ledger.len();
    assert!(
        authority
            .preflight_workspace_directory(Some(&grant), &command)
            .is_none()
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger.len(), entries);
    let accepted = authority.accept_workspace_directory(Some(&grant), command.clone(), worker());
    let WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
        revision,
        directory_id,
        worker_id,
    } = &accepted.outcome
    else {
        panic!("expected accepted directory: {accepted:?}")
    };
    assert_eq!(revision.value(), 3);
    assert_eq!(worker_id, &worker());
    assert_eq!(authority.last_identity, cursor + 1);
    assert_eq!(authority.ledger.len(), entries + 1);
    let settled = authority.projection.snapshot().clone();
    assert_eq!(settled.terminal_contexts, before.terminal_contexts);
    let navigation = &settled.navigation;
    assert_eq!(navigation.directories.len(), 1);
    assert_eq!(navigation.directories[0].directory_id, *directory_id);
    assert_eq!(navigation.directories[0].worker_id, worker());
    assert_eq!(
        navigation.directories[0].path.as_str(),
        "/tmp/workspace/link/../directory"
    );
    assert_eq!(
        navigation.workspaces[0].primary_directory_id.as_ref(),
        Some(directory_id)
    );
    assert_eq!(
        authority.preflight_workspace_directory(Some(&grant), &command),
        Some(accepted.clone())
    );
    assert_eq!(
        authority.accept_workspace_directory(
            Some(&grant),
            command,
            WorkerId::parse(&identity(801)).unwrap()
        ),
        accepted
    );
    assert_eq!(authority.projection.snapshot(), &settled);
    assert_eq!(authority.last_identity, cursor + 1);
    assert_eq!(authority.ledger.len(), entries + 1);
}

#[test]
fn ungrouped_workspace_accepts_a_global_directory_without_creating_a_project() {
    let (mut authority, grant) = authority();
    let grant = grant.with_directory_association();
    let created = submit(
        &mut authority,
        &grant,
        command(
            201,
            0,
            WorkbenchAction::WorkspaceCreate {
                project_id: None,
                label: WorkbenchLabel::parse("ungrouped").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = created.outcome else {
        panic!("expected Workspace");
    };
    let before = authority.snapshot().clone();
    let request = command(
        202,
        before.revision.value(),
        WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id: workspace_id.clone(),
            path: WorkbenchDirectoryPath::parse("C:/ungrouped-cwd").unwrap(),
        },
    );
    let accepted = authority.accept_workspace_directory(Some(&grant), request, worker());
    let WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { directory_id, .. } =
        accepted.outcome
    else {
        panic!("{accepted:?}");
    };
    let after = authority.snapshot();
    assert!(after.navigation.projects.is_empty());
    assert_eq!(after.navigation.workspaces[0].workspace_id, workspace_id);
    assert_eq!(after.navigation.workspaces[0].project_id, None);
    assert_eq!(
        after.navigation.workspaces[0].primary_directory_id,
        Some(directory_id.clone())
    );
    assert_eq!(after.navigation.directories.len(), 1);
    assert_eq!(after.navigation.directories[0].directory_id, directory_id);
    assert_eq!(
        after.navigation.directories[0].path.as_str(),
        "C:/ungrouped-cwd"
    );
    assert_eq!(
        after.navigation.workspace_order,
        before.navigation.workspace_order
    );
    assert_eq!(after.terminal_contexts, before.terminal_contexts);
}

#[test]
fn directory_grant_is_independent_and_denial_precedes_replay() {
    let (mut authority, grant, command) = empty_workspace();
    let session = authority.session_id().clone();
    let before = authority.projection.snapshot().clone();
    for denied in [
        None,
        Some(WorkbenchGrant::read_only(session.clone()).with_directory_association()),
        Some(WorkbenchGrant::manage_navigation(session.clone())),
        Some(WorkbenchGrant::manage_navigation_and_terminal_create(
            session.clone(),
        )),
    ] {
        assert!(matches!(
            authority
                .accept_workspace_directory(denied.as_ref(), command.clone(), worker())
                .outcome,
            WorkbenchCommandOutcome::Denied {}
        ));
        assert_eq!(authority.projection.snapshot(), &before);
    }
    let accepted = authority.accept_workspace_directory(Some(&grant), command.clone(), worker());
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    assert!(matches!(
        authority
            .preflight_workspace_directory(None, &command)
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::Denied {}
    ));
}

#[test]
fn changed_revision_after_validation_does_not_rebase_or_consume_identity() {
    let (mut authority, grant, command) = empty_workspace();
    assert!(
        authority
            .preflight_workspace_directory(Some(&grant), &command)
            .is_none()
    );
    submit(
        &mut authority,
        &grant,
        super::command(204, 2, create("Concurrent project")),
    );
    let before = authority.projection.snapshot().clone();
    let cursor = authority.last_identity;
    let entries = authority.ledger.len();
    let rejected = authority.accept_workspace_directory(Some(&grant), command, worker());
    assert!(matches!(
        rejected.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger.len(), entries);
}

#[test]
fn competing_association_and_request_id_reuse_cannot_overwrite_directory() {
    let (mut authority, grant, command) = empty_workspace();
    let mut conflicting = command.clone();
    if let WorkbenchAction::WorkspacePrimaryDirectoryAccept { path, .. } = &mut conflicting.action {
        *path = WorkbenchDirectoryPath::parse("/another").unwrap();
    }
    let accepted = authority.accept_workspace_directory(Some(&grant), command, worker());
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    let before = authority.projection.snapshot().clone();
    let rejected =
        authority.accept_workspace_directory(Some(&grant), conflicting.clone(), worker());
    assert!(matches!(
        rejected.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    conflicting.request_id = WorkbenchRequestId::parse(&identity(204)).unwrap();
    conflicting.expected_revision = WorkbenchRevision::new(3);
    let rejected = authority.accept_workspace_directory(Some(&grant), conflicting, worker());
    assert!(matches!(
        rejected.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::WorkspaceDirectoryConflict,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &before);
}

#[test]
fn association_capacity_failures_preserve_canonical_state_and_ledger() {
    for exhausted in ["identity", "revision", "ledger"] {
        let (mut authority, grant, mut command) = empty_workspace();
        match exhausted {
            "identity" => authority.last_identity = u128::MAX,
            "revision" => {
                let mut snapshot = authority.projection.snapshot().clone();
                snapshot.revision = WorkbenchRevision::new(u64::MAX);
                authority.projection = WorkbenchProjectionStore::load(
                    &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
                )
                .unwrap();
                command.expected_revision = WorkbenchRevision::new(u64::MAX);
            }
            "ledger" => authority.ledger_bytes = MAXIMUM_LEDGER_BYTES,
            _ => unreachable!(),
        }
        let before = authority.projection.snapshot().clone();
        let cursor = authority.last_identity;
        let entries = authority.ledger.len();
        let bytes = authority.ledger_bytes;
        let result = authority.accept_workspace_directory(Some(&grant), command, worker());
        match exhausted {
            "identity" => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::IdentityExhausted,
                    ..
                }
            )),
            "revision" => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::RevisionExhausted,
                    ..
                }
            )),
            "ledger" => assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::RequestLedgerFull
                }
            )),
            _ => unreachable!(),
        }
        assert_eq!(authority.projection.snapshot(), &before);
        assert_eq!(authority.last_identity, cursor);
        assert_eq!(authority.ledger.len(), entries);
        assert_eq!(authority.ledger_bytes, bytes);
    }
}

#[test]
fn former_project_directory_limit_is_allowed_and_encoded_capacity_still_refuses_atomically() {
    use orchbee_workbench_protocol::{
        Directory, MAXIMUM_DIRECTORIES_PER_PROJECT, MAXIMUM_ENCODED_PROJECTION_BYTES,
        WorkbenchDirectoryId,
    };
    for encoded_limit in [false, true] {
        let (mut authority, grant, mut command) = empty_workspace();
        let mut snapshot = authority.projection.snapshot().clone();
        let count = MAXIMUM_DIRECTORIES_PER_PROJECT;
        snapshot.navigation.directories = (0..count)
            .map(|index| Directory {
                directory_id: WorkbenchDirectoryId::parse(&identity(10_000 + index as u128))
                    .unwrap(),
                worker_id: worker(),
                path: WorkbenchDirectoryPath::parse("/").unwrap(),
            })
            .collect();
        if encoded_limit {
            let initial_size =
                serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
                    .unwrap()
                    .len();
            let mut remaining = MAXIMUM_ENCODED_PROJECTION_BYTES - 1_024 - initial_size;
            for directory in &mut snapshot.navigation.directories {
                let added = remaining.min(4_095);
                directory.path =
                    WorkbenchDirectoryPath::parse(&format!("/{}", "x".repeat(added))).unwrap();
                remaining -= added;
            }
            assert_eq!(remaining, 0);
            if let WorkbenchAction::WorkspacePrimaryDirectoryAccept { path, .. } =
                &mut command.action
            {
                *path = WorkbenchDirectoryPath::parse(&format!("/{}", "y".repeat(4_095))).unwrap();
            }
        }
        authority.projection = WorkbenchProjectionStore::load(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
        )
        .unwrap();
        authority.last_identity = 20_000;
        let entries = authority.ledger.len();
        let bytes = authority.ledger_bytes;
        let outcome = authority
            .accept_workspace_directory(Some(&grant), command, worker())
            .outcome;
        if !encoded_limit {
            assert!(matches!(
                outcome,
                WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
            ));
            assert_eq!(authority.snapshot().navigation.directories.len(), count + 1);
            continue;
        }
        assert!(
            matches!(
                outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::ResourceLimit,
                    ..
                }
            ),
            "encoded={encoded_limit}: {outcome:?}"
        );
        assert_eq!(authority.projection.snapshot(), &snapshot);
        assert_eq!(authority.last_identity, 20_000);
        assert_eq!(authority.ledger.len(), entries);
        assert_eq!(authority.ledger_bytes, bytes);
    }
}

#[test]
fn exact_association_replay_survives_both_full_ledger_limits() {
    let (mut authority, grant, command) = empty_workspace();
    let accepted = authority.accept_workspace_directory(Some(&grant), command.clone(), worker());
    let before = authority.projection.snapshot().clone();
    let project_id = before.navigation.projects[0].project_id.clone();
    for index in authority.ledger.len()..MAXIMUM_LEDGER_ENTRIES {
        let unchanged = submit(
            &mut authority,
            &grant,
            super::command(
                1_000 + index as u128,
                3,
                WorkbenchAction::ProjectRename {
                    project_id: project_id.clone(),
                    label: WorkbenchLabel::parse("Project").unwrap(),
                },
            ),
        );
        assert!(matches!(
            unchanged.outcome,
            WorkbenchCommandOutcome::Unchanged { .. }
        ));
    }
    assert_eq!(authority.ledger.len(), MAXIMUM_LEDGER_ENTRIES);
    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    let cursor = authority.last_identity;
    let mut new_command = command.clone();
    new_command.request_id = WorkbenchRequestId::parse(&identity(9_000)).unwrap();
    assert!(matches!(
        authority
            .accept_workspace_directory(Some(&grant), new_command, worker())
            .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::RequestLedgerFull
        }
    ));
    assert_eq!(
        authority.preflight_workspace_directory(Some(&grant), &command),
        Some(accepted.clone())
    );
    assert_eq!(
        authority.accept_workspace_directory(Some(&grant), command, worker()),
        accepted
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger.len(), MAXIMUM_LEDGER_ENTRIES);
    assert_eq!(authority.ledger_bytes, MAXIMUM_LEDGER_BYTES);
}
