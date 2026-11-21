use super::*;
use orchbee_workbench_protocol::{NavigationConfiguration, WorkbenchWorkspaceId};

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
    project_id: Option<WorkbenchProjectId>,
) -> WorkbenchWorkspaceId {
    let result = apply(
        owner,
        grant,
        id,
        WorkbenchAction::WorkspaceCreate {
            project_id,
            label: WorkbenchLabel::parse("Workspace").unwrap(),
        },
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = result.outcome else {
        panic!("Workspace create: {result:?}")
    };
    workspace_id
}

#[test]
fn navigation_configuration_commits_membership_and_order_once_and_replays_without_allocation() {
    let (mut owner, grant) = authority();
    let first = project_id(&apply(&mut owner, &grant, 1, create("First")));
    let second = project_id(&apply(&mut owner, &grant, 2, create("Second")));
    let source = workspace(&mut owner, &grant, 3, Some(first));
    let _peer = workspace(&mut owner, &grant, 4, Some(second.clone()));
    let baseline = owner.snapshot().clone();
    let cursor = owner.last_identity;
    let request = command(
        5,
        baseline.revision.value(),
        WorkbenchAction::NavigationConfigure {
            operation: NavigationConfiguration::MoveWorkspace {
                workspace_id: source.clone(),
                project_id: Some(second.clone()),
                before_workspace_id: None,
            },
        },
    );
    let plan = owner
        .plan(
            &request,
            PureWorkbenchAction::from_action(&request.action).unwrap(),
        )
        .unwrap_or_else(|_| panic!("plan"));
    let prepared = owner
        .prepare_command_commit(request.clone(), plan)
        .unwrap_or_else(|_| panic!("prepare"));
    drop(prepared);
    assert_eq!(owner.snapshot(), &baseline);
    let result = submit(&mut owner, &grant, request.clone());
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::NavigationConfigured {
            revision: WorkbenchRevision::new(baseline.revision.value() + 1)
        }
    );
    assert_eq!(
        owner
            .snapshot()
            .navigation
            .workspace(&source)
            .unwrap()
            .project_id,
        Some(second)
    );
    assert_eq!(owner.last_identity, cursor);
    assert_eq!(
        owner.snapshot().terminal_contexts,
        baseline.terminal_contexts
    );
    let after = owner.snapshot().clone();
    assert_eq!(submit(&mut owner, &grant, request.clone()), result);
    assert_eq!(owner.snapshot(), &after);
    let mut noop = request.clone();
    noop.request_id = WorkbenchRequestId::parse(&identity(6)).unwrap();
    noop.expected_revision = after.revision;
    assert_eq!(
        submit(&mut owner, &grant, noop).outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: after.revision
        }
    );
    let mut stale = request;
    stale.request_id = WorkbenchRequestId::parse(&identity(7)).unwrap();
    assert_eq!(
        submit(&mut owner, &grant, stale).outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: after.revision,
            reason: WorkbenchRejectionReason::RevisionMismatch
        }
    );
    assert_eq!(owner.snapshot(), &after);
    let WorkbenchPublicationOutcome::Published { publications } = publication_outcome(
        &mut owner,
        Some(&grant),
        publication_read(8, baseline.revision.value()),
    ) else {
        panic!("one publication")
    };
    assert_eq!(publications.len(), 1);
    let WorkbenchPublicationDocument::Delta(delta) = &publications[0] else {
        panic!("aggregate delta")
    };
    assert_eq!(
        orchbee_workbench_protocol::apply_workbench_delta(&baseline, delta).unwrap(),
        after
    );
}

#[test]
fn explicit_project_order_survives_creation_regrouping_and_dissolution() {
    let (mut owner, grant) = authority();
    let first = project_id(&apply(&mut owner, &grant, 1, create("First")));
    let second = project_id(&apply(&mut owner, &grant, 2, create("Second")));
    apply(
        &mut owner,
        &grant,
        3,
        WorkbenchAction::NavigationConfigure {
            operation: NavigationConfiguration::MoveProject {
                project_id: second.clone(),
                before_project_id: Some(first.clone()),
            },
        },
    );
    let third = project_id(&apply(&mut owner, &grant, 4, create("Third")));
    assert_eq!(
        owner.snapshot().navigation.project_order,
        [second.clone(), first.clone(), third.clone()]
    );
    let ungrouped = workspace(&mut owner, &grant, 5, None);
    let moved = apply(
        &mut owner,
        &grant,
        6,
        WorkbenchAction::WorkspaceCreateProject {
            workspace_id: ungrouped,
            label: WorkbenchLabel::parse("Fourth").unwrap(),
        },
    );
    let WorkbenchCommandOutcome::WorkspaceProjectChanged {
        project_id: Some(fourth),
        ..
    } = moved.outcome
    else {
        panic!("create project and move")
    };
    assert_eq!(
        owner.snapshot().navigation.project_order,
        [second.clone(), first.clone(), third.clone(), fourth.clone()]
    );
    apply(
        &mut owner,
        &grant,
        7,
        WorkbenchAction::ProjectDissolve { project_id: first },
    );
    assert_eq!(
        owner.snapshot().navigation.project_order,
        [second, third, fourth]
    );
    assert!(owner.snapshot().navigation.ordered_projects().is_ok());
}

/// A Project named from elsewhere keeps its identity and comes last in the display order wherever
/// the identity sorts; naming it again changes nothing. A Project this Session creates sorts last,
/// or is refused when nothing is left above, so an earlier Client's prediction of it holds.
#[test]
fn a_named_project_comes_last_and_is_created_once() {
    let (mut owner, grant) = authority();
    let first = project_id(&apply(&mut owner, &grant, 1, create("First")));
    let named = WorkbenchProjectId::parse(&identity(RANDOM_PROJECT_FLOOR | 1)).unwrap();
    let create_named = || WorkbenchAction::ProjectCreate {
        label: WorkbenchLabel::parse("Named").unwrap(),
        project_id: Some(named.clone()),
    };
    assert_eq!(
        project_id(&apply(&mut owner, &grant, 2, create_named())),
        named
    );
    let navigation = &owner.snapshot().navigation;
    assert_eq!(navigation.projects[0].project_id, named);
    assert_eq!(navigation.project_order, [first.clone(), named.clone()]);
    assert!(matches!(
        apply(&mut owner, &grant, 3, create_named()).outcome,
        WorkbenchCommandOutcome::Unchanged { .. }
    ));
    let third = project_id(&apply(&mut owner, &grant, 4, create("Third")));
    let navigation = &owner.snapshot().navigation;
    assert_eq!(navigation.projects.last().unwrap().project_id, third);
    assert_eq!(navigation.project_order, [first, named, third]);

    let top = WorkbenchProjectId::parse(&identity(u128::MAX)).unwrap();
    apply(
        &mut owner,
        &grant,
        5,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("Top").unwrap(),
            project_id: Some(top),
        },
    );
    assert!(matches!(
        apply(&mut owner, &grant, 6, create("Fifth")).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::IdentityExhausted,
            ..
        }
    ));
}
