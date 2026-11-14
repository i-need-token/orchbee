use super::*;
mod navigation_configuration;
mod tab_configuration;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::workbench_state::workspace_setup::{
    SetupEdit as Edit, SetupOperation, SetupStage as Stage, SetupStatus as Status,
    SetupWorkerEvidence,
};
use orchbee_workbench_protocol::{
    Project, WorkbenchProjectId, WorkbenchSnapshotUnavailableReason, WorkbenchWorkspaceId,
    Workspace,
};

pub(super) fn group_intent(
    state: &WorkbenchClientState,
    label: &str,
) -> crate::workbench_state::workspace_setup::SetupIntent {
    use orchbee_workbench_protocol::{TabConfiguration, TabGroupColor, WorkbenchTabGroupLabel};
    crate::workbench_state::workspace_setup::SetupIntent::TabGroupName(Box::new(
        TabConfiguration::CreateGroup {
            tab_id: state.cache().tabs()[0].tab_id().clone(),
            label: WorkbenchTabGroupLabel::parse(label).unwrap(),
            color: TabGroupColor::Blue,
        },
    ))
}

fn organization_intent(
    state: &WorkbenchClientState,
) -> crate::workbench_state::workspace_setup::SetupIntent {
    let workspace = state.cache().canonical_snapshot().navigation.workspaces[1]
        .workspace_id
        .clone();
    crate::workbench_state::workspace_setup::SetupIntent::Organize(Box::new(
        crate::workbench_state::workspace_setup::WorkspaceOrganizationRequest::workspace(
            state,
            &workspace,
            Instant::now() + OPERATION_TOTAL,
        )
        .unwrap(),
    ))
}

fn project() -> Project {
    Project {
        project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
        label: "Project".into(),
    }
}
pub(super) fn workspace_snapshot(revision: u64, tabs: usize) -> Snapshot {
    let mut snapshot = snapshot(1, revision, None);
    let project = project();
    snapshot.navigation.workspaces.push(Workspace {
        terminal_directory: None,
        automatic_label: false,
        project_id: Some(project.project_id.clone()),
        workspace_id: WorkbenchWorkspaceId::parse(&id(71)).unwrap(),
        label: "Workspace".into(),
        primary_directory_id: None,
        terminal_context_id: context_id(72),
    });
    snapshot.navigation.projects.push(project);
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: context_id(72),
        groups: Vec::new(),
        tabs: (10..10 + tabs).map(tab).collect(),
    });
    snapshot
}
pub(super) fn workspace_state() -> WorkbenchClientState {
    let snapshot = workspace_snapshot(1, 0);
    let mut state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    assert!(state.start_context_selection(1, true, deadlines));
    let PreparedRefresh::Membership { prepared, .. } =
        state.prepare_refresh(1, WorkbenchSnapshotOutcome::Available { snapshot })
    else {
        panic!("Empty workspace selection");
    };
    let old = state.attached_key().unwrap().clone();
    let cleanup = state
        .stage_refresh_membership_cleanup(1, prepared, deadlines)
        .unwrap();
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    assert_eq!(
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, None)
            .unwrap()
            .commit(),
        MembershipInstallDecision::Empty
    );
    assert!(state.invariant_holds());
    state
}

fn label_operation(
    state: &mut WorkbenchClientState,
    label: &str,
    generation: u64,
) -> SetupOperation {
    let token = state.setup_draft().unwrap().token();
    assert!(
        state
            .plan_setup(
                token,
                WorkbenchRequestId::parse(&id(90 + u128::from(generation))).unwrap(),
                generation,
                OperationDeadlines::from_sample(Instant::now()).unwrap()
            )
            .is_none()
    );
    state.acknowledge_setup_draw(Some(token));
    assert!(state.edit_setup_draft(
        token,
        Edit::Line(crate::terminal_text::line_editor::LineKey::KillToStart)
    ));
    assert!(state.edit_setup_draft(token, Edit::Append(label)));
    let operation = state
        .plan_setup(
            token,
            WorkbenchRequestId::parse(&id(90 + u128::from(generation))).unwrap(),
            generation,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    state.begin_setup(operation.clone());
    operation
}
pub(super) fn finish(
    state: &mut WorkbenchClientState,
    snapshot: Snapshot,
    generation: u64,
) -> MembershipInstallDecision {
    let PreparedRefresh::Membership { prepared, .. } =
        state.prepare_refresh(generation, WorkbenchSnapshotOutcome::Available { snapshot })
    else {
        panic!("validated setup membership");
    };
    let desired = if prepared.preserves_selection() {
        state.desired().map(|pane| pane.anchor().clone())
    } else {
        prepared
            .selected_tab_id()
            .map(|tab| AttachmentKey::new(generation + 30, tab.clone()))
    };
    let deadlines = state.setup_deadlines().unwrap();
    if !prepared.preserves_selection() && state.has_attachments() {
        let old = state.attached_key().unwrap().clone();
        let cleanup = state
            .stage_refresh_membership_cleanup(generation, prepared, deadlines)
            .unwrap();
        let prepared = state
            .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
            .unwrap();
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, desired)
            .unwrap()
            .commit()
    } else {
        state
            .prepare_refresh_membership_install(generation, prepared, desired, Some(deadlines))
            .unwrap()
            .commit()
    }
}
#[test]
fn organization_picker_keeps_empty_projects_and_rejects_old_target_draw() {
    let mut baseline = workspace_snapshot(1, 0);
    baseline.navigation.workspaces[1].project_id = None;
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let original = state.open_setup_draft(organization_intent(&state)).unwrap();
    let draft = state.setup_draft().unwrap();
    assert_eq!(draft.project_count(), 3);
    assert_eq!(draft.selected_project(), 1);
    assert!(draft.project_at(1).unwrap().ungrouped());
    assert_eq!(draft.project_at(0).unwrap().label(), Some("Project"));
    state.acknowledge_setup_draw(Some(original));
    assert!(state.edit_setup_draft(original, Edit::SelectPreviousProject));
    let current = state.setup_draft().unwrap().token();
    assert_ne!(current, original);
    state.acknowledge_setup_draw(Some(original));
    assert!(
        state
            .plan_setup(
                current,
                workbench_request_id(98),
                1,
                OperationDeadlines::from_sample(Instant::now()).unwrap()
            )
            .is_none()
    );
    state.acknowledge_setup_draw(Some(current));
    let operation = state
        .plan_setup(
            current,
            workbench_request_id(98),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(
        matches!(operation.command.action, orchbee_workbench_protocol::WorkbenchAction::WorkspaceSetProject { project_id: Some(ref id), .. } if id == &project().project_id)
    );
}
#[test]
fn group_commit_refresh_failure_recovers_by_read_and_does_not_rearm_an_old_receipt() {
    let mut state = ready(1);
    state
        .open_setup_draft(group_intent(&state, "Group"))
        .unwrap();
    let operation = label_operation(&mut state, "Group", 1);
    let group_id = orchbee_workbench_protocol::WorkbenchTabGroupId::parse(&id(70)).unwrap();
    let known = operation
        .known_commit(WorkbenchSetupCommit::TabsConfigured {
            revision: WorkbenchRevision::new(2),
            group_id: Some(group_id.clone()),
            unchanged: false,
        })
        .unwrap();
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known.clone())));
    let prepared = state.prepare_refresh(
        1,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    );
    assert_eq!(
        state.apply_prepared_refresh(prepared, Instant::now()),
        RefreshDecision::Continue
    );
    assert_eq!(state.setup_status(), Some(Status::TabsCommittedNeedRefresh));
    assert!(state.setup_draft().is_none());
    assert!(!state.start_context_selection(2, true, operation.deadlines));
    assert!(state.start_pending_refresh(2));
    state.reset_setup_recovery_deadlines(operation.deadlines);
    let mut snapshot = snapshot(1, 2, None);
    snapshot.terminal_contexts[0]
        .groups
        .push(orchbee_workbench_protocol::TabGroup {
            group_id: group_id.clone(),
            label: "Group".into(),
            color: orchbee_workbench_protocol::TabGroupColor::Blue,
        });
    snapshot.terminal_contexts[0].tabs[0].group_id = Some(group_id);
    assert_eq!(
        finish(&mut state, snapshot, 2),
        MembershipInstallDecision::Preserved
    );
    assert!(state.setup_draft().is_none());
    assert!(!state.setup_blocks_structure());
    state.clear_settled_setup_status();
    assert_eq!(state.setup_status(), None);
    state
        .open_setup_draft(group_intent(&state, "Next"))
        .unwrap();
    let token = state.setup_draft().unwrap().token();
    assert!(
        state
            .plan_setup(token, workbench_request_id(99), 3, operation.deadlines)
            .is_none()
    );
    let queued = label_operation(&mut state, "Next", 3);
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    assert_eq!(state.setup_status(), None);
    assert!(state.setup_blocks_structure());
    assert!(state.retain_setup_worker(SetupWorkerEvidence::NotApplied(queued)));
    assert!(!state.setup_blocks_structure());
    assert_eq!(state.setup_status(), Some(Status::TabsNotApplied));
}
#[test]
fn setup_draft_and_known_recovery_exclude_existing_structural_actions() {
    let mut state = ready(2);
    let token = state
        .open_setup_draft(group_intent(&state, "Group"))
        .unwrap();
    assert_eq!(
        state.plan_selection(ClientAction::SelectNextTab),
        SelectionPlan::Busy
    );
    assert!(state.open_rename_draft().is_none());
    assert!(!state.phase_allows_refresh());
    assert!(
        state
            .open_setup_draft(
                crate::workbench_state::workspace_setup::SetupIntent::RenameLocation(
                    crate::workbench_state::workspace_setup::LocationLabelTarget::Workspace,
                )
            )
            .is_none()
    );
    state.cancel_setup_draft(token);
    assert!(!state.setup_blocks_structure());
    assert!(state.open_rename_draft().is_some());
    assert!(
        state
            .open_setup_draft(group_intent(&state, "Group"))
            .is_none()
    );
}
#[test]
fn known_and_definite_worker_returns_survive_retirement_without_becoming_unknown() {
    let mut state = workspace_state();
    let token = state
        .open_setup_draft(
            crate::workbench_state::workspace_setup::SetupIntent::RenameLocation(
                crate::workbench_state::workspace_setup::LocationLabelTarget::Workspace,
            ),
        )
        .unwrap();
    state.acknowledge_setup_draw(Some(token));
    state.edit_setup_draft(token, Edit::Append("/missing"));
    let operation = state
        .plan_setup(
            token,
            WorkbenchRequestId::parse(&id(97)).unwrap(),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    state.begin_setup(operation.clone());
    assert!(state.retain_setup_worker(SetupWorkerEvidence::NotApplied(operation.clone())));
    state.retire(CarrierFailure::Deadline);
    assert_eq!(
        state.setup_status(),
        Some(Status::LocationLabel {
            target: crate::workbench_state::workspace_setup::LocationLabelTarget::Workspace,
            status: crate::localization::LocationLabelStatus::NotApplied,
        })
    );
    assert!(state.retain_setup_worker(SetupWorkerEvidence::NotApplied(operation)));
    assert!(!state.setup_blocks_structure());
}
#[test]
fn setup_draft_edits_graphemes_and_the_picker_stays_inert_to_text() {
    use crate::workbench_state::workspace_setup::SetupIntent;
    let mut state = workspace_state();
    // The picker stage owns no text field: text/paste/nav edits are inert and
    // project selection only moves through the selection edits.
    let picker = state.open_setup_draft(organization_intent(&state)).unwrap();
    state.acknowledge_setup_draw(Some(picker));
    assert!(!state.edit_setup_draft(picker, Edit::Append("x")));
    assert!(!state.edit_setup_draft(picker, Edit::Paste("[]")));
    assert!(!state.edit_setup_draft(picker, Edit::Home));
    assert!(!state.edit_setup_draft(picker, Edit::Backspace));
    assert_eq!(state.setup_draft().unwrap().text(), "");
    assert_eq!(
        state.setup_draft().unwrap().stage(),
        Stage::ChooseWorkspaceProject
    );
    state.cancel_setup_draft(picker);

    // A rename stage prefills the exact label.
    let rename = state
        .open_setup_draft(SetupIntent::RenameLocation(
            crate::workbench_state::workspace_setup::LocationLabelTarget::Workspace,
        ))
        .unwrap();
    state.acknowledge_setup_draw(Some(rename));
    assert_eq!(state.setup_draft().unwrap().stage(), Stage::WorkspaceRename);
    assert_eq!(state.setup_draft().unwrap().text(), "Workspace");
}

#[test]
fn setup_first_geometry_block_refuses_the_open_attempt_without_a_hidden_owner() {
    let mut state = workspace_state();
    let token = state
        .open_setup_draft(
            crate::workbench_state::workspace_setup::SetupIntent::RenameLocation(
                crate::workbench_state::workspace_setup::LocationLabelTarget::Workspace,
            ),
        )
        .unwrap();
    assert_eq!(
        state.setup_draft().unwrap().presentation(),
        crate::tui_presentation::PresentationPhase::Pending
    );
    state.acknowledge_setup_draw(None);
    assert_eq!(
        state.setup_draft().unwrap().presentation(),
        crate::tui_presentation::PresentationPhase::Pending
    );

    assert_eq!(
        state.settle_setup_draft_not_presented(),
        Some(crate::tui_presentation::GeometrySettlement::Refused)
    );
    assert!(state.setup_draft().is_none());
    assert_eq!(state.setup_status(), Some(Status::PresentationUnavailable));
    assert!(!state.edit_setup_draft(token, Edit::Append("C:/hidden")));
}

#[test]
fn setup_geometry_suspension_preserves_the_exact_draft_and_requires_a_fresh_draw() {
    let mut state = workspace_state();
    let token = state
        .open_setup_draft(
            crate::workbench_state::workspace_setup::SetupIntent::RenameLocation(
                crate::workbench_state::workspace_setup::LocationLabelTarget::Workspace,
            ),
        )
        .unwrap();
    state.acknowledge_setup_draw(Some(token));
    assert_eq!(
        state.setup_draft().unwrap().presentation(),
        crate::tui_presentation::PresentationPhase::Admitted
    );
    assert!(state.edit_setup_draft(token, Edit::Append("C:/draft")));
    assert!(state.edit_setup_draft(token, Edit::Left));
    let cursor = state
        .setup_draft()
        .unwrap()
        .field_window(40)
        .unwrap()
        .cursor_column();

    assert_eq!(
        state.settle_setup_draft_not_presented(),
        Some(crate::tui_presentation::GeometrySettlement::Suspended)
    );
    let suspended = state.setup_draft().unwrap();
    assert_eq!(
        suspended.presentation(),
        crate::tui_presentation::PresentationPhase::Suspended
    );
    assert_eq!(suspended.token(), token);
    assert_eq!(suspended.text(), "C:/draft");
    assert_eq!(suspended.field_window(40).unwrap().cursor_column(), cursor);
    assert!(!state.edit_setup_draft(token, Edit::Append("-hidden")));
    assert!(
        state
            .plan_setup(
                token,
                workbench_request_id(98),
                1,
                OperationDeadlines::from_sample(Instant::now()).unwrap()
            )
            .is_none()
    );

    state.acknowledge_setup_draw(None);
    assert!(!state.edit_setup_draft(token, Edit::Append("-covered")));
    state.acknowledge_setup_draw(Some(token));
    assert!(state.edit_setup_draft(token, Edit::Append("-resumed")));
    assert_eq!(state.setup_draft().unwrap().text(), "C:/draf-resumedt");
}

#[test]
fn setup_child_escape_restores_the_project_picker_and_discards_only_child_edits() {
    let mut state = workspace_state();
    let parent = state.open_setup_draft(organization_intent(&state)).unwrap();
    state.acknowledge_setup_draw(Some(parent));
    let new_project = state.setup_draft().unwrap().project_count() - 1;
    assert!(state.choose_setup_project(parent, new_project));
    assert!(state.advance_setup_picker(parent));

    let child = state.setup_draft().unwrap().token();
    assert_eq!(
        state.setup_draft().unwrap().presentation(),
        crate::tui_presentation::PresentationPhase::Admitted
    );
    state.acknowledge_setup_draw(Some(child));
    assert!(state.edit_setup_draft(child, Edit::Append("Temporary Project")));
    state.cancel_setup_draft(child);

    let restored = state.setup_draft().unwrap();
    assert_eq!(restored.stage(), Stage::ChooseWorkspaceProject);
    assert_eq!(restored.selected_project(), new_project);
    assert_eq!(restored.text(), "");
    assert_ne!(restored.token(), child);
    let restored_token = restored.token();
    state.acknowledge_setup_draw(Some(restored_token));
    assert!(state.advance_setup_picker(restored_token));
    assert_eq!(
        state.setup_draft().unwrap().stage(),
        Stage::WorkspaceProjectName
    );
    assert_eq!(state.setup_draft().unwrap().text(), "");
}
