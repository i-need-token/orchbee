use super::super::{
    NativeSetupEdit, NativeSetupIntent, NativeSetupStage, NativeSetupStatus, SetupWorkerEvidence,
};
use super::*;
use crate::workbench_client::state::{
    OperationDeadlines, PreparedRefresh,
    tests::{ready_with_cache, snapshot},
};
use crate::workbench_client::workbench_projection::project_preferred_workspace;
use orchbee_workbench_protocol::{
    Snapshot, TerminalContext, WorkbenchProjectId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTerminalContextId,
};
use std::time::Duration;

#[test]
fn conditional_project_delete_reviews_organization_only_and_needs_fresh_draw_and_identity() {
    let (mut state, _) = fixture();
    let original = state.cache.clone();
    let project = original.canonical_snapshot().navigation.projects[0].clone();
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let request = WorkspaceOrganizationRequest::project(&state, &project, deadlines.total())
        .unwrap()
        .with_choice(OrganizationChoice::DeleteIfIdle)
        .unwrap();
    let operation = state
        .plan_sidebar_organization(
            &request,
            WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap(),
            1,
            deadlines,
        )
        .unwrap();
    assert_eq!(
        operation.command.action,
        WorkbenchAction::ProjectDissolveIfIdle {
            project_id: project.project_id.clone()
        }
    );
    state.begin_setup(operation.clone());
    assert!(state.settle_setup_not_applied(&operation));
    state.apply_prepared_refresh(
        PreparedRefresh::DefiniteFailure {
            generation: 1,
            baseline_revision: original.revision(),
        },
        Instant::now(),
    );
    let token = state
        .review_project_delete(&operation, deadlines.total())
        .unwrap();
    let draft = state.setup_draft().unwrap();
    assert_eq!(draft.stage(), NativeSetupStage::ProjectDeleteConfirm);
    assert!(draft.draft.organization.as_ref().unwrap().reviewed_delete());
    assert!(!draft.interactive());
    let next = WorkbenchRequestId::parse(&format!("{:032x}", 91)).unwrap();
    assert!(
        state
            .plan_setup(token, next.clone(), 2, deadlines)
            .is_none()
    );
    assert!(
        !state.arm_project_dissolve(token, deadlines),
        "a reviewed Delete cannot acquire legacy automatic continuation"
    );
    state.acknowledge_setup_draw(Some(token));
    let confirmed = state.plan_setup(token, next.clone(), 2, deadlines).unwrap();
    assert_eq!(
        confirmed.command.action,
        WorkbenchAction::ProjectDissolve {
            project_id: project.project_id
        }
    );
    assert_eq!(confirmed.command.request_id, next);
    assert_ne!(confirmed.command.request_id, operation.command.request_id);
    assert_eq!(state.cache, original);
}

fn workspace_id() -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&format!("{:032x}", 81)).unwrap()
}
fn project_id(n: u128) -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&format!("{n:032x}")).unwrap()
}
fn fixture() -> (WorkbenchClientState, Snapshot) {
    let mut snapshot = snapshot(1, 1, None);
    snapshot.navigation.projects.push(Project {
        project_id: project_id(70),
        label: "Existing".into(),
    });
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 82)).unwrap();
    snapshot.navigation.workspaces.push(Workspace {
        workspace_id: workspace_id(),
        project_id: None,
        label: "Other Workspace".into(),
        primary_directory_id: None,
        terminal_context_id: context.clone(),
    });
    snapshot.navigation.workspace_order.push(workspace_id());
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: context,
        tabs: vec![],
        groups: vec![],
    });
    (
        ready_with_cache(project_preferred_workspace(&snapshot).unwrap()),
        snapshot,
    )
}
fn open(state: &mut WorkbenchClientState) {
    let request = WorkspaceOrganizationRequest::workspace(
        state,
        &workspace_id(),
        Instant::now() + Duration::from_secs(30),
    )
    .unwrap();
    let token = state
        .open_setup_draft(NativeSetupIntent::Organize(Box::new(request)))
        .unwrap();
    state.acknowledge_setup_draw(Some(token));
}
fn plan(state: &mut WorkbenchClientState) -> SetupOperation {
    let token = state.setup_draft().unwrap().token();
    state.acknowledge_setup_draw(Some(token));
    state
        .plan_setup(
            token,
            WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap(),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap()
}
fn install(
    state: &mut WorkbenchClientState,
    operation: SetupOperation,
    commit: WorkbenchSetupCommit,
    snapshot: Snapshot,
) {
    let focus = state.focus_pane_ref();
    let desired = state.desired().cloned();
    let location = state.cache().location().clone();
    let known = operation.known_commit(commit).unwrap();
    state.begin_setup(operation);
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    let PreparedRefresh::Membership { prepared, .. } =
        state.prepare_refresh(1, WorkbenchSnapshotOutcome::Available { snapshot })
    else {
        panic!("grouping refresh")
    };
    assert!(prepared.preserves_selection());
    let anchor = state.desired().map(|pane| pane.anchor().clone());
    state
        .prepare_refresh_membership_install(1, prepared, anchor, state.setup_deadlines())
        .unwrap()
        .commit();
    assert_eq!(state.cache().location(), &location);
    assert_eq!(state.focus_pane_ref(), focus);
    assert_eq!(state.desired(), desired.as_ref());
    assert!(state.invariant_holds());
}

#[test]
fn move_noncurrent_workspace_changes_only_grouping_and_preserves_selected_terminal() {
    let (mut state, mut snapshot) = fixture();
    let contexts = snapshot.terminal_contexts.clone();
    open(&mut state);
    let token = state.setup_draft().unwrap().token();
    assert!(state.edit_setup_draft(token, NativeSetupEdit::SelectPreviousProject));
    let operation = plan(&mut state);
    assert!(
        matches!(&operation.command.action, WorkbenchAction::WorkspaceSetProject { workspace_id: workspace, project_id: Some(project) } if workspace == &workspace_id() && project == &project_id(70))
    );
    snapshot.navigation.workspaces[1].project_id = Some(project_id(70));
    snapshot.revision = WorkbenchRevision::new(2);
    install(
        &mut state,
        operation,
        WorkbenchSetupCommit::WorkspaceProjectChanged {
            revision: snapshot.revision,
            workspace_id: workspace_id(),
            project_id: Some(project_id(70)),
            created: false,
            unchanged: false,
        },
        snapshot,
    );
    assert_eq!(
        state.cache().canonical_snapshot().terminal_contexts,
        contexts
    );
    assert_eq!(
        state.setup_status(),
        Some(NativeSetupStatus::WorkspaceOrganized)
    );
}

#[test]
fn create_project_and_move_is_one_commit_and_dissolve_keeps_workspace_resources() {
    let (mut state, mut snapshot) = fixture();
    open(&mut state);
    let token = state.setup_draft().unwrap().token();
    assert!(state.edit_setup_draft(token, NativeSetupEdit::SelectNextProject));
    let token = state.setup_draft().unwrap().token();
    state.acknowledge_setup_draw(Some(token));
    assert!(state.advance_setup_picker(token));
    assert_eq!(
        state.setup_draft().unwrap().stage(),
        NativeSetupStage::WorkspaceProjectName
    );
    let token = state.setup_draft().unwrap().token();
    state.acknowledge_setup_draw(Some(token));
    assert!(state.edit_setup_draft(token, NativeSetupEdit::Append("New group")));
    let operation = plan(&mut state);
    assert!(
        matches!(&operation.command.action, WorkbenchAction::WorkspaceCreateProject { workspace_id: workspace, label } if workspace == &workspace_id() && label.as_str() == "New group")
    );
    let group = Project {
        project_id: project_id(100),
        label: "New group".into(),
    };
    snapshot.navigation.projects.push(group.clone());
    snapshot.navigation.workspaces[1].project_id = Some(group.project_id.clone());
    snapshot.revision = WorkbenchRevision::new(2);
    install(
        &mut state,
        operation,
        WorkbenchSetupCommit::WorkspaceProjectChanged {
            revision: snapshot.revision,
            workspace_id: workspace_id(),
            project_id: Some(group.project_id.clone()),
            created: true,
            unchanged: false,
        },
        snapshot.clone(),
    );

    // A separate Client may dissolve the group; selection identity remains unchanged.
    let mut state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
    let request = WorkspaceOrganizationRequest::project(
        &state,
        &group,
        Instant::now() + Duration::from_secs(30),
    )
    .unwrap();
    let token = state
        .open_setup_draft(NativeSetupIntent::Organize(Box::new(request)))
        .unwrap();
    assert!(state.arm_project_dissolve(
        token,
        OperationDeadlines::from_sample(Instant::now()).unwrap()
    ));
    let operation = plan(&mut state);
    snapshot
        .navigation
        .projects
        .retain(|project| project.project_id != group.project_id);
    snapshot.navigation.workspaces[1].project_id = None;
    snapshot.revision = WorkbenchRevision::new(3);
    install(
        &mut state,
        operation,
        WorkbenchSetupCommit::ProjectDissolved {
            revision: snapshot.revision,
            project_id: group.project_id,
        },
        snapshot,
    );
    assert_eq!(
        state
            .cache()
            .canonical_snapshot()
            .navigation
            .workspaces
            .len(),
        1
    );
    assert_eq!(
        state.setup_status(),
        Some(NativeSetupStatus::ProjectDissolved)
    );
}

#[test]
fn grouping_receipts_require_exact_identity_and_noop_revision() {
    let (mut state, _) = fixture();
    open(&mut state);
    let operation = plan(&mut state);
    let receipt = WorkbenchSetupCommit::WorkspaceProjectChanged {
        revision: WorkbenchRevision::new(1),
        workspace_id: workspace_id(),
        project_id: None,
        created: false,
        unchanged: true,
    };
    assert!(operation.known_commit(receipt.clone()).is_some());
    let WorkbenchSetupCommit::WorkspaceProjectChanged {
        mut revision,
        workspace_id,
        project_id,
        created,
        unchanged,
    } = receipt
    else {
        unreachable!()
    };
    revision = WorkbenchRevision::new(revision.value() + 1);
    assert!(
        operation
            .known_commit(WorkbenchSetupCommit::WorkspaceProjectChanged {
                revision,
                workspace_id,
                project_id,
                created,
                unchanged
            })
            .is_none()
    );
}
