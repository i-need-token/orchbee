use super::super::{SetupEdit, SetupIntent, SetupStage, SetupStatus, SetupWorkerEvidence};
use super::*;
use crate::workbench_projection::project_preferred_workspace;
use crate::workbench_state::tests::{ready_with_cache, snapshot};
use crate::workbench_state::{OperationDeadlines, PreparedRefresh};
use orchbee_workbench_protocol::{
    Snapshot, TerminalContext, WorkbenchProjectId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTerminalContextId,
};
use std::time::Duration;

#[test]
fn project_dissolve_runs_directly_as_the_unconditional_dissolve() {
    let (state, _) = fixture();
    let project = state.cache.canonical_snapshot().navigation.projects[0].clone();
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let request = WorkspaceOrganizationRequest::project(&state, &project, deadlines.total())
        .unwrap()
        .with_choice(OrganizationChoice::Dissolve)
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
        WorkbenchAction::ProjectDissolve {
            project_id: project.project_id
        }
    );
}

#[test]
fn empty_project_dissolve_reports_no_retained_workspace() {
    let (mut state, mut snapshot) = fixture();
    let project = snapshot.navigation.projects[0].clone();
    assert!(
        snapshot
            .navigation
            .workspaces
            .iter()
            .all(|workspace| workspace.project_id.as_ref() != Some(&project.project_id))
    );
    let request = WorkspaceOrganizationRequest::project(
        &state,
        &project,
        Instant::now() + Duration::from_secs(30),
    )
    .unwrap()
    .with_choice(OrganizationChoice::Dissolve)
    .unwrap();
    let operation = state
        .plan_sidebar_organization(
            &request,
            WorkbenchRequestId::parse(&format!("{:032x}", 91)).unwrap(),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    snapshot.navigation.projects.clear();
    snapshot.revision = WorkbenchRevision::new(2);
    install(
        &mut state,
        operation,
        WorkbenchSetupCommit::ProjectDissolved {
            revision: snapshot.revision,
            project_id: project.project_id,
        },
        snapshot,
    );
    assert_eq!(
        state.setup_status(),
        Some(SetupStatus::EmptyProjectDissolved)
    );
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
        automatic_label: false,
        workspace_id: workspace_id(),
        project_id: None,
        label: "Other Workspace".into(),
        primary_directory_id: None,
        terminal_context_id: context.clone(),
    });
    snapshot.navigation.workspace_order = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
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
        .open_setup_draft(SetupIntent::Organize(Box::new(request)))
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
    assert!(state.edit_setup_draft(token, SetupEdit::SelectPreviousProject));
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
    assert_eq!(state.setup_status(), Some(SetupStatus::WorkspaceOrganized));
}

#[test]
fn create_project_and_move_is_one_commit_and_dissolve_keeps_workspace_resources() {
    let (mut state, mut snapshot) = fixture();
    open(&mut state);
    let token = state.setup_draft().unwrap().token();
    assert!(state.edit_setup_draft(token, SetupEdit::SelectNextProject));
    let token = state.setup_draft().unwrap().token();
    state.acknowledge_setup_draw(Some(token));
    assert!(state.advance_setup_picker(token));
    assert_eq!(
        state.setup_draft().unwrap().stage(),
        SetupStage::WorkspaceProjectName
    );
    let token = state.setup_draft().unwrap().token();
    state.acknowledge_setup_draw(Some(token));
    assert!(state.edit_setup_draft(token, SetupEdit::Append("New group")));
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
    .unwrap()
    .with_choice(OrganizationChoice::Dissolve)
    .unwrap();
    let operation = state
        .plan_sidebar_organization(
            &request,
            WorkbenchRequestId::parse(&format!("{:032x}", 92)).unwrap(),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
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
        2
    );
    assert_eq!(state.setup_status(), Some(SetupStatus::ProjectDissolved));
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
