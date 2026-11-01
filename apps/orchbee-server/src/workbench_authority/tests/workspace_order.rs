use super::*;
use orchbee_workbench_protocol::{WorkbenchWorkspaceInsertIndex, WorkbenchWorkspaceList};

fn populated() -> (
    WorkbenchAuthority,
    WorkbenchGrant,
    Vec<WorkbenchWorkspaceId>,
) {
    let (mut owner, grant) = empty_authority();
    let mut ids = Vec::new();
    for p in 0..2 {
        let revision = owner.snapshot().revision.value();
        let result = submit(
            &mut owner,
            &grant,
            command(
                10 + revision as u128,
                revision,
                create(&format!("project {p}")),
            ),
        );
        let project_id = project_id(&result);
        for w in 0..2 {
            let revision = owner.snapshot().revision.value();
            let result = submit(
                &mut owner,
                &grant,
                command(
                    10 + revision as u128,
                    revision,
                    WorkbenchAction::WorkspaceCreate {
                        project_id: Some(project_id.clone()),
                        label: WorkbenchLabel::parse(&format!("workspace {p}/{w}")).unwrap(),
                    },
                ),
            );
            let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = result.outcome
            else {
                panic!("workspace")
            };
            ids.push(workspace_id);
        }
    }
    (owner, grant, ids)
}

fn ordered(owner: &WorkbenchAuthority) -> Vec<WorkbenchWorkspaceId> {
    owner
        .snapshot()
        .navigation
        .ordered_workspaces()
        .unwrap()
        .into_iter()
        .map(|(_, workspace)| workspace.workspace_id.clone())
        .collect()
}

#[test]
fn workspace_moves_use_original_boundaries_and_publish_only_effects() {
    // Each row specifies the final identity order for boundaries 0..=4.
    let expected = [
        [
            [0, 1, 2, 3],
            [0, 1, 2, 3],
            [1, 0, 2, 3],
            [1, 2, 0, 3],
            [1, 2, 3, 0],
        ],
        [
            [1, 0, 2, 3],
            [0, 1, 2, 3],
            [0, 1, 2, 3],
            [0, 2, 1, 3],
            [0, 2, 3, 1],
        ],
        [
            [2, 0, 1, 3],
            [0, 2, 1, 3],
            [0, 1, 2, 3],
            [0, 1, 2, 3],
            [0, 1, 3, 2],
        ],
        [
            [3, 0, 1, 2],
            [0, 3, 1, 2],
            [0, 1, 3, 2],
            [0, 1, 2, 3],
            [0, 1, 2, 3],
        ],
    ];
    for (source, cases) in expected.iter().enumerate() {
        for (boundary, expected) in cases.iter().enumerate() {
            let (mut owner, grant, ids) = populated();
            let before = owner.snapshot().clone();
            let request = command(
                100,
                before.revision.value(),
                WorkbenchAction::WorkspaceMove {
                    workspace_id: ids[source].clone(),
                    insert_index: WorkbenchWorkspaceInsertIndex::new(boundary).unwrap(),
                },
            );
            let result = submit(&mut owner, &grant, request.clone());
            assert_eq!(
                ordered(&owner),
                expected.iter().map(|i| ids[*i].clone()).collect::<Vec<_>>()
            );
            let unchanged = expected == &[0, 1, 2, 3];
            assert_eq!(
                owner.snapshot().revision.value(),
                before.revision.value() + u64::from(!unchanged)
            );
            assert_eq!(
                owner.snapshot().navigation.projects,
                before.navigation.projects
            );
            assert_eq!(owner.snapshot().terminal_contexts, before.terminal_contexts);
            if unchanged {
                assert_eq!(
                    owner.snapshot().navigation.workspace_order,
                    before.navigation.workspace_order
                );
                assert!(matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::Unchanged { .. }
                ));
            } else {
                let WorkbenchCommandOutcome::WorkspacesReordered {
                    ref workspace_order,
                    ..
                } = result.outcome
                else {
                    panic!("reorder")
                };
                assert_eq!(workspace_order.ids(), ordered(&owner));
            }
            assert_eq!(submit(&mut owner, &grant, request), result);
        }
    }
}

#[test]
fn workspace_block_moves_preserve_caller_order_and_membership() {
    let (mut owner, grant, ids) = populated();
    let before = owner.snapshot().clone();
    let action = WorkbenchAction::WorkspaceMoveBlock {
        workspace_ids: WorkbenchWorkspaceList::new(vec![ids[3].clone(), ids[1].clone()]).unwrap(),
        before_workspace_id: Some(ids[0].clone()),
    };
    let request = command(100, before.revision.value(), action.clone());
    let result = submit(&mut owner, &grant, request.clone());
    assert_eq!(
        ordered(&owner),
        [
            ids[3].clone(),
            ids[1].clone(),
            ids[0].clone(),
            ids[2].clone()
        ]
    );
    assert_eq!(
        owner.snapshot().navigation.projects,
        before.navigation.projects
    );
    assert_eq!(owner.snapshot().terminal_contexts, before.terminal_contexts);
    assert_eq!(submit(&mut owner, &grant, request), result);
    let revision = owner.snapshot().revision.value();
    assert!(matches!(
        submit(&mut owner, &grant, command(101, revision, action)).outcome,
        WorkbenchCommandOutcome::Unchanged { .. }
    ));
    assert_eq!(owner.snapshot().revision.value(), revision);
    let project_id = before.navigation.projects[0].project_id.clone();
    let created = submit(
        &mut owner,
        &grant,
        command(
            102,
            revision,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id),
                label: WorkbenchLabel::parse("appended").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = created.outcome else {
        panic!("created")
    };
    assert_eq!(ordered(&owner).last(), Some(&workspace_id));
}

#[test]
fn workspace_move_refusals_preserve_all_state() {
    let (mut owner, grant, ids) = populated();
    let before = owner.snapshot().clone();
    let unknown = WorkbenchWorkspaceId::parse(&identity(900)).unwrap();
    let cases = [
        (
            WorkbenchAction::WorkspaceMove {
                workspace_id: ids[0].clone(),
                insert_index: WorkbenchWorkspaceInsertIndex::new(5).unwrap(),
            },
            WorkbenchRejectionReason::WorkspaceMoveIndexOutOfBounds,
        ),
        (
            WorkbenchAction::WorkspaceMove {
                workspace_id: unknown.clone(),
                insert_index: WorkbenchWorkspaceInsertIndex::new(0).unwrap(),
            },
            WorkbenchRejectionReason::WorkspaceUnavailable,
        ),
        (
            WorkbenchAction::WorkspaceMoveBlock {
                workspace_ids: WorkbenchWorkspaceList::new(vec![ids[1].clone(), ids[3].clone()])
                    .unwrap(),
                before_workspace_id: Some(ids[1].clone()),
            },
            WorkbenchRejectionReason::WorkspaceMoveTargetInBlock,
        ),
        (
            WorkbenchAction::WorkspaceMoveBlock {
                workspace_ids: WorkbenchWorkspaceList::new(vec![ids[1].clone(), unknown.clone()])
                    .unwrap(),
                before_workspace_id: None,
            },
            WorkbenchRejectionReason::WorkspaceUnavailable,
        ),
        (
            WorkbenchAction::WorkspaceMoveBlock {
                workspace_ids: WorkbenchWorkspaceList::new(vec![ids[1].clone()]).unwrap(),
                before_workspace_id: Some(unknown),
            },
            WorkbenchRejectionReason::WorkspaceUnavailable,
        ),
    ];
    for (i, (action, reason)) in cases.into_iter().enumerate() {
        let outcome = submit(
            &mut owner,
            &grant,
            command(100 + i as u128, before.revision.value(), action),
        )
        .outcome;
        assert_eq!(
            outcome,
            WorkbenchCommandOutcome::Rejected {
                revision: before.revision,
                reason
            }
        );
        assert_eq!(owner.snapshot(), &before);
    }
    let action = WorkbenchAction::WorkspaceMove {
        workspace_id: ids[0].clone(),
        insert_index: WorkbenchWorkspaceInsertIndex::new(4).unwrap(),
    };
    assert!(matches!(
        submit(&mut owner, &grant, command(200, 0, action.clone())).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    let read_only = WorkbenchGrant::read_only(before.session_id.clone());
    assert!(matches!(
        submit(
            &mut owner,
            &read_only,
            command(201, before.revision.value(), action)
        )
        .outcome,
        WorkbenchCommandOutcome::Denied { .. }
    ));
    assert_eq!(owner.snapshot(), &before);
}

#[test]
fn workspace_order_supports_the_complete_1024_member_domain() {
    let (mut owner, grant) = authority();
    let mut source = owner.snapshot().clone();
    source.navigation.workspaces.clear();
    source.terminal_contexts.clear();
    let mut ids = Vec::new();
    for project_index in 0..4 {
        let project = Project {
            project_id: WorkbenchProjectId::parse(&identity(10 + project_index)).unwrap(),
            label: format!("project {project_index}"),
        };
        for index in 0..256 {
            let value = project_index * 256 + index;
            let workspace_id = WorkbenchWorkspaceId::parse(&identity(1000 + value)).unwrap();
            let terminal_context_id =
                WorkbenchTerminalContextId::parse(&identity(10000 + value)).unwrap();
            ids.push(workspace_id.clone());
            source.navigation.workspaces.push(Workspace {
                automatic_label: false,
                project_id: Some(project.project_id.clone()),
                workspace_id,
                terminal_context_id: terminal_context_id.clone(),
                label: format!("workspace {value}"),
                primary_directory_id: None,
            });
            source
                .terminal_contexts
                .push(empty_context(terminal_context_id));
        }
        source.navigation.projects.push(project);
    }
    source.navigation.workspace_order = ids.clone();
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(source.clone())).unwrap(),
    )
    .unwrap();
    owner.last_identity = 20000;
    ids.reverse();
    let order = WorkbenchWorkspaceList::new(ids.clone()).unwrap();
    let request = command(
        100,
        source.revision.value(),
        WorkbenchAction::WorkspaceMoveBlock {
            workspace_ids: order.clone(),
            before_workspace_id: None,
        },
    );
    let encoded = serialize_workbench_request(&WorkbenchRequest::Command(request.clone())).unwrap();
    assert!(encoded.len() > orchbee_workbench_protocol::MAXIMUM_ENCODED_INTENT_BYTES);
    let result = submit(&mut owner, &grant, request.clone());
    assert!(
        matches!(&result.outcome,WorkbenchCommandOutcome::WorkspacesReordered {workspace_order,..} if workspace_order == &order)
    );
    assert_eq!(ordered(&owner), ids);
    assert_eq!(
        owner.snapshot().navigation.projects,
        source.navigation.projects
    );
    assert_eq!(owner.snapshot().terminal_contexts, source.terminal_contexts);
    assert_eq!(submit(&mut owner, &grant, request), result);
    assert!(
        serialize_workbench_response(&WorkbenchResponse::Command(result))
            .unwrap()
            .len()
            > orchbee_workbench_protocol::MAXIMUM_ENCODED_INTENT_BYTES
    );
}

#[test]
fn workspace_order_obeys_ledger_revision_and_request_identity_fences() {
    let (mut owner, grant, ids) = populated();
    let before = owner.snapshot().clone();
    let action = WorkbenchAction::WorkspaceMove {
        workspace_id: ids[0].clone(),
        insert_index: WorkbenchWorkspaceInsertIndex::new(4).unwrap(),
    };
    owner.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    let full = submit(
        &mut owner,
        &grant,
        command(100, before.revision.value(), action.clone()),
    );
    assert!(matches!(
        full.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::RequestLedgerFull
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    owner.ledger_bytes = 0;
    let request = command(100, before.revision.value(), action.clone());
    submit(&mut owner, &grant, request.clone());
    let settled = owner.snapshot().clone();
    let different = command(
        100,
        before.revision.value(),
        WorkbenchAction::WorkspaceMove {
            workspace_id: ids[1].clone(),
            insert_index: WorkbenchWorkspaceInsertIndex::new(4).unwrap(),
        },
    );
    assert!(matches!(
        submit(&mut owner, &grant, different).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &settled);
    let mut exhausted = before;
    exhausted.revision = WorkbenchRevision::new(u64::MAX);
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(exhausted.clone())).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        submit(&mut owner, &grant, command(200, u64::MAX, action)).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionExhausted,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &exhausted);
    let noop = WorkbenchAction::WorkspaceMove {
        workspace_id: ids[0].clone(),
        insert_index: WorkbenchWorkspaceInsertIndex::new(0).unwrap(),
    };
    assert!(matches!(
        submit(&mut owner, &grant, command(201, u64::MAX, noop)).outcome,
        WorkbenchCommandOutcome::Unchanged { .. }
    ));
}
