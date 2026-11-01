use super::*;
use orchbee_workbench_protocol::{
    LaunchOptions, MAXIMUM_PROJECTS, MAXIMUM_WORKSPACES_PER_PROJECT, WorkbenchDirectoryPath,
    WorkbenchWorkspaceId, WorkspacePlacement,
};

fn apply(
    owner: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    id: u128,
    action: WorkbenchAction,
) -> WorkbenchCommandResult {
    let revision = owner.snapshot().revision.value();
    submit(owner, grant, command(id, revision, action))
}

fn workspace(
    owner: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    id: u128,
    project: Option<WorkbenchProjectId>,
) -> WorkbenchWorkspaceId {
    let result = apply(
        owner,
        grant,
        id,
        WorkbenchAction::WorkspaceCreate {
            project_id: project,
            label: WorkbenchLabel::parse("workspace").unwrap(),
        },
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = result.outcome else {
        panic!("{result:?}")
    };
    workspace_id
}

fn launch(owner: &WorkbenchAuthority, id: u128, placement: WorkspacePlacement) -> WorkbenchCommand {
    command(
        id,
        owner.snapshot().revision.value(),
        WorkbenchAction::WorkspaceLaunch {
            automatic_label: false,
            placement,
            label: WorkbenchLabel::parse("new shell").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Box::new(LaunchOptions {
                cwd: Some(WorkbenchDirectoryPath::parse("C:/checkout").unwrap()),
                ..Default::default()
            }),
        },
    )
}

fn install(owner: &mut WorkbenchAuthority, snapshot: Snapshot) {
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
}

#[test]
fn ungrouped_creation_has_one_context_no_project_and_explicit_display_order() {
    let (mut owner, grant) = empty_authority();
    let first = workspace(&mut owner, &grant, 1, None);
    let second = workspace(&mut owner, &grant, 2, None);
    let snapshot = owner.snapshot();
    assert!(snapshot.navigation.projects.is_empty());
    assert!(snapshot.navigation.directories.is_empty());
    assert_eq!(snapshot.navigation.workspace_order, [first, second]);
    assert!(
        snapshot
            .navigation
            .workspaces
            .iter()
            .all(|workspace| workspace.project_id.is_none())
    );
    assert_eq!(snapshot.terminal_contexts.len(), 2);
    assert!(
        snapshot
            .terminal_contexts
            .iter()
            .all(|context| context.tabs.is_empty() && context.groups.is_empty())
    );
}

#[test]
fn regroup_ungroup_create_project_and_dissolve_preserve_every_other_resource_and_replay() {
    let (mut owner, _) = authority();
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(owner.session_id().clone())
        .with_directory_association();
    let first_project = project_id(&apply(&mut owner, &grant, 1, create("first")));
    let second_project = project_id(&apply(&mut owner, &grant, 2, create("second")));
    let source = workspace(&mut owner, &grant, 3, Some(first_project.clone()));
    let peer = workspace(&mut owner, &grant, 4, Some(first_project.clone()));
    let request = command(
        5,
        owner.snapshot().revision.value(),
        WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id: source.clone(),
            path: WorkbenchDirectoryPath::parse("C:/unchanged-cwd").unwrap(),
        },
    );
    assert!(matches!(
        owner
            .accept_workspace_directory(Some(&grant), request, binding(700).worker_id)
            .outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    let context = owner
        .snapshot()
        .navigation
        .workspace(&source)
        .unwrap()
        .terminal_context_id
        .clone();
    let tab = command(
        6,
        owner.snapshot().revision.value(),
        WorkbenchAction::TabCreate {
            terminal_context_id: context,
            label: WorkbenchLabel::parse("bound shell").unwrap(),
            automatic_label: false,
            placement: None,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    let WorkbenchTabCreateBegin::Admitted(admission) =
        owner.begin_terminal_tab_create(Some(&grant), tab)
    else {
        panic!("tab admission")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(700), None)
    else {
        panic!("tab preparation")
    };
    let pending = prepared.commit().into_pending();
    let pending_request = WorkbenchRequestId::parse(&identity(6)).unwrap();

    for (id, project) in [(7, Some(second_project)), (8, None)] {
        let before = owner.snapshot().clone();
        let high = owner.last_identity;
        let request = command(
            id,
            before.revision.value(),
            WorkbenchAction::WorkspaceSetProject {
                workspace_id: source.clone(),
                project_id: project.clone(),
            },
        );
        let accepted = submit(&mut owner, &grant, request.clone());
        assert!(
            matches!(&accepted.outcome, WorkbenchCommandOutcome::WorkspaceProjectChanged { project_id, .. } if *project_id == project)
        );
        let mut expected = before;
        expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
        expected
            .navigation
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.workspace_id == source)
            .unwrap()
            .project_id = project;
        assert_eq!(
            owner.snapshot(),
            &expected,
            "only membership and revision may change"
        );
        assert_eq!(owner.last_identity, high);
        assert_eq!(submit(&mut owner, &grant, request.clone()), accepted);
        let mut conflict = request;
        conflict.action = WorkbenchAction::WorkspaceSetProject {
            workspace_id: peer.clone(),
            project_id: None,
        };
        assert!(matches!(
            submit(&mut owner, &grant, conflict).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
        assert_eq!(owner.snapshot(), &expected);
    }
    let before = owner.snapshot().clone();
    let high = owner.last_identity;
    let created = apply(
        &mut owner,
        &grant,
        9,
        WorkbenchAction::WorkspaceCreateProject {
            workspace_id: source.clone(),
            label: WorkbenchLabel::parse("new grouping").unwrap(),
        },
    );
    let WorkbenchCommandOutcome::WorkspaceProjectChanged {
        project_id: Some(created_project),
        ..
    } = created.outcome
    else {
        panic!("new group")
    };
    let mut expected = before;
    expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
    expected.navigation.projects.push(Project {
        project_id: created_project.clone(),
        label: "new grouping".to_owned(),
    });
    expected
        .navigation
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.workspace_id == source)
        .unwrap()
        .project_id = Some(created_project.clone());
    assert_eq!(owner.snapshot(), &expected);
    assert_eq!(owner.last_identity, high + 1);
    for (id, project) in [(10, created_project), (11, first_project)] {
        let before = owner.snapshot().clone();
        let high = owner.last_identity;
        assert!(matches!(
            apply(
                &mut owner,
                &grant,
                id,
                WorkbenchAction::ProjectDissolve {
                    project_id: project.clone()
                }
            )
            .outcome,
            WorkbenchCommandOutcome::ProjectDissolved { .. }
        ));
        let mut expected = before;
        expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
        expected
            .navigation
            .projects
            .retain(|p| p.project_id != project);
        for workspace in &mut expected.navigation.workspaces {
            if workspace.project_id.as_ref() == Some(&project) {
                workspace.project_id = None;
            }
        }
        assert_eq!(owner.snapshot(), &expected);
        assert_eq!(owner.last_identity, high);
    }
    // Membership edits must not restart, replace, or settle the existing pending terminal.
    let before_settlement = owner.snapshot().clone();
    let settled = owner.prepare_terminal_tab_settlement(pending, WorkbenchTerminalOutcome::Failed {
        reason: orchbee_workbench_protocol::WorkbenchTerminalFailureReason::TerminalCreateFailed,
    }).unwrap().commit();
    assert_eq!(settled.request_id, pending_request);
    assert_eq!(owner.snapshot(), &before_settlement);
}

#[test]
fn invalid_targets_and_ledger_refusal_do_not_allocate_or_partially_move() {
    let (mut owner, grant) = authority();
    let source = workspace(&mut owner, &grant, 1, None);
    for (index, action) in [
        WorkbenchAction::WorkspaceSetProject {
            workspace_id: source.clone(),
            project_id: Some(WorkbenchProjectId::parse(&identity(90)).unwrap()),
        },
        WorkbenchAction::WorkspaceSetProject {
            workspace_id: WorkbenchWorkspaceId::parse(&identity(91)).unwrap(),
            project_id: None,
        },
        WorkbenchAction::WorkspaceCreateProject {
            workspace_id: WorkbenchWorkspaceId::parse(&identity(91)).unwrap(),
            label: WorkbenchLabel::parse("must not appear").unwrap(),
        },
        WorkbenchAction::ProjectDissolve {
            project_id: WorkbenchProjectId::parse(&identity(90)).unwrap(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let before = owner.snapshot().clone();
        let high = owner.last_identity;
        assert!(matches!(
            apply(&mut owner, &grant, 10 + index as u128, action).outcome,
            WorkbenchCommandOutcome::Rejected { .. }
        ));
        assert_eq!(owner.snapshot(), &before);
        assert_eq!(owner.last_identity, high);
    }
    let before = owner.snapshot().clone();
    let high = owner.last_identity;
    owner.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    assert!(matches!(
        apply(
            &mut owner,
            &grant,
            20,
            WorkbenchAction::WorkspaceCreateProject {
                workspace_id: source,
                label: WorkbenchLabel::parse("refused").unwrap()
            }
        )
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::RequestLedgerFull
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, high);
}

#[test]
fn full_project_refuses_new_members_but_equal_membership_is_a_noop_and_project_limit_is_atomic() {
    let (mut owner, grant) = empty_authority();
    let source = workspace(&mut owner, &grant, 1, None);
    let project = project_id(&apply(&mut owner, &grant, 2, create("full")));
    let mut full = owner.snapshot().clone();
    for index in 0..MAXIMUM_WORKSPACES_PER_PROJECT {
        let workspace_id =
            WorkbenchWorkspaceId::parse(&identity(1000 + index as u128 * 2)).unwrap();
        let context =
            WorkbenchTerminalContextId::parse(&identity(1001 + index as u128 * 2)).unwrap();
        full.navigation.workspaces.push(Workspace {
            automatic_label: false,
            workspace_id,
            project_id: Some(project.clone()),
            label: "member".to_owned(),
            primary_directory_id: None,
            terminal_context_id: context.clone(),
        });
        full.terminal_contexts.push(empty_context(context));
    }
    full.navigation.workspace_order = full
        .navigation
        .workspaces
        .iter()
        .map(|w| w.workspace_id.clone())
        .collect();
    install(&mut owner, full.clone());
    owner.last_identity = 10_000;
    for (index, action) in [
        WorkbenchAction::WorkspaceSetProject {
            workspace_id: source.clone(),
            project_id: Some(project.clone()),
        },
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(project.clone()),
            label: WorkbenchLabel::parse("overflow").unwrap(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        assert!(matches!(
            apply(&mut owner, &grant, 10 + index as u128, action).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::ResourceLimit,
                ..
            }
        ));
        assert_eq!(owner.snapshot(), &full);
        assert_eq!(owner.last_identity, 10_000);
    }
    let member = full.navigation.workspaces[1].workspace_id.clone();
    assert!(matches!(
        apply(
            &mut owner,
            &grant,
            12,
            WorkbenchAction::WorkspaceSetProject {
                workspace_id: member,
                project_id: Some(project)
            }
        )
        .outcome,
        WorkbenchCommandOutcome::Unchanged { .. }
    ));
    for index in 1..MAXIMUM_PROJECTS {
        full.navigation.projects.push(Project {
            project_id: WorkbenchProjectId::parse(&identity(20_000 + index as u128)).unwrap(),
            label: "empty".to_owned(),
        });
    }
    install(&mut owner, full.clone());
    owner.last_identity = 50_000;
    assert!(matches!(
        apply(
            &mut owner,
            &grant,
            13,
            WorkbenchAction::WorkspaceCreateProject {
                workspace_id: source,
                label: WorkbenchLabel::parse("overflow").unwrap()
            }
        )
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::ResourceLimit,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &full);
    assert_eq!(owner.last_identity, 50_000);
}

#[test]
fn alongside_an_ungrouped_source_commits_only_that_source_and_new_shell_into_one_project() {
    let (mut owner, _) = empty_authority();
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(owner.session_id().clone())
        .with_directory_association();
    let source = workspace(&mut owner, &grant, 1, None);
    let peer = workspace(&mut owner, &grant, 2, None);
    let before = owner.snapshot().clone();
    let high = owner.last_identity;
    let request = launch(
        &owner,
        3,
        WorkspacePlacement::Alongside {
            workspace_id: source.clone(),
            project_label: WorkbenchLabel::parse("siblings").unwrap(),
        },
    );
    let WorkbenchTabCreateBegin::Admitted(admission) =
        owner.begin_workspace_launch(Some(&grant), request.clone(), binding(700).worker_id)
    else {
        panic!("admit")
    };
    drop(admission);
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, high);
    let WorkbenchTabCreateBegin::Admitted(admission) =
        owner.begin_workspace_launch(Some(&grant), request.clone(), binding(700).worker_id)
    else {
        panic!("admit again")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(700), None)
    else {
        panic!("prepare")
    };
    let pending = prepared.commit().into_pending();
    let after = owner.snapshot().clone();
    assert_eq!(after.revision.value(), before.revision.value() + 1);
    assert_eq!(after.navigation.projects.len(), 1);
    let group = after.navigation.projects[0].project_id.clone();
    assert_eq!(
        after.navigation.workspace(&source).unwrap().project_id,
        Some(group.clone())
    );
    assert_eq!(
        after.navigation.workspace(&peer),
        before.navigation.workspace(&peer)
    );
    let created = after.navigation.workspaces.last().unwrap();
    assert_eq!(created.project_id, Some(group));
    assert_eq!(
        &after.terminal_contexts[..before.terminal_contexts.len()],
        before.terminal_contexts.as_slice()
    );
    assert_eq!(after.terminal_contexts.last().unwrap().tabs.len(), 1);
    assert_eq!(after.navigation.directories.len(), 1);
    assert_eq!(owner.last_identity, high + 7);
    let _settled = owner.prepare_terminal_tab_settlement(pending, WorkbenchTerminalOutcome::Failed { reason: orchbee_workbench_protocol::WorkbenchTerminalFailureReason::TerminalCreateFailed }).unwrap().commit();
    assert_eq!(owner.snapshot(), &after);
    let WorkbenchTabCreateBegin::Ready(replay) =
        owner.begin_workspace_launch(Some(&grant), request, binding(700).worker_id)
    else {
        panic!("replay")
    };
    assert!(matches!(
        replay.outcome,
        WorkbenchCommandOutcome::WorkspaceLaunched {
            project_id: Some(_),
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &after);
}

#[test]
fn global_workspace_capacity_rejects_ungrouped_create_and_launch_without_partial_resources() {
    let (mut owner, _) = authority();
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(owner.session_id().clone())
        .with_directory_association();
    let mut full = owner.snapshot().clone();
    for index in 0..orchbee_workbench_protocol::MAXIMUM_TERMINAL_CONTEXTS - 1 {
        let context =
            WorkbenchTerminalContextId::parse(&identity(1001 + index as u128 * 2)).unwrap();
        full.navigation.workspaces.push(Workspace {
            automatic_label: false,
            workspace_id: WorkbenchWorkspaceId::parse(&identity(1000 + index as u128 * 2)).unwrap(),
            project_id: None,
            label: "ungrouped".to_owned(),
            primary_directory_id: None,
            terminal_context_id: context.clone(),
        });
        full.terminal_contexts.push(empty_context(context));
    }
    full.navigation.workspace_order = full
        .navigation
        .workspaces
        .iter()
        .map(|w| w.workspace_id.clone())
        .collect();
    install(&mut owner, full.clone());
    owner.last_identity = 10_000;
    assert!(matches!(
        apply(
            &mut owner,
            &grant,
            1,
            WorkbenchAction::WorkspaceCreate {
                project_id: None,
                label: WorkbenchLabel::parse("overflow").unwrap(),
            }
        )
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::ResourceLimit,
            ..
        }
    ));
    let request = launch(
        &owner,
        2,
        WorkspacePlacement::NewProject {
            label: WorkbenchLabel::parse("must not appear").unwrap(),
        },
    );
    assert!(matches!(
        owner
            .preflight_workspace_launch(Some(&grant), &request)
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::ResourceLimit,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &full);
    assert_eq!(owner.last_identity, 10_000);
}

#[test]
fn ungrouped_and_new_project_launch_receipts_match_the_atomic_structure() {
    for grouped in [false, true] {
        let (mut owner, _) = empty_authority();
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(owner.session_id().clone())
                .with_directory_association();
        let placement = if grouped {
            WorkspacePlacement::NewProject {
                label: WorkbenchLabel::parse("group").unwrap(),
            }
        } else {
            WorkspacePlacement::Ungrouped {}
        };
        let request = launch(&owner, 1, placement);
        let before = owner.snapshot().clone();
        let WorkbenchTabCreateBegin::Admitted(admission) =
            owner.begin_workspace_launch(Some(&grant), request.clone(), binding(700).worker_id)
        else {
            panic!("admit")
        };
        let WorkbenchTabCreatePreparation::Prepared(prepared) =
            admission.finish(binding(700), None)
        else {
            panic!("prepare")
        };
        let pending = prepared.commit().into_pending();
        let after = owner.snapshot().clone();
        assert_eq!(after.revision.value(), before.revision.value() + 1);
        assert_eq!(after.navigation.projects.len(), usize::from(grouped));
        assert_eq!(after.navigation.workspaces.len(), 1);
        assert_eq!(
            after.navigation.workspace_order,
            [after.navigation.workspaces[0].workspace_id.clone()]
        );
        assert!(before.terminal_contexts.is_empty());
        assert_eq!(after.terminal_contexts.len(), 1);
        assert_eq!(after.terminal_contexts[0].tabs.len(), 1);
        assert_eq!(after.navigation.directories.len(), 1);
        let settled = owner.prepare_terminal_tab_settlement(pending, WorkbenchTerminalOutcome::Failed {
            reason: orchbee_workbench_protocol::WorkbenchTerminalFailureReason::TerminalCreateFailed,
        }).unwrap().commit();
        assert!(
            matches!(&settled.outcome, WorkbenchCommandOutcome::WorkspaceLaunched { project_id, .. }
            if *project_id == after.navigation.workspaces[0].project_id && project_id.is_some() == grouped)
        );
        assert_eq!(owner.snapshot(), &after);
    }
}
