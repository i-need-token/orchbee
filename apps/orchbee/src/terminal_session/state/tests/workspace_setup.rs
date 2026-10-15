use super::*;
mod navigation_configuration;
mod tab_configuration;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::state::workspace_setup::{
    NativeSetupEdit as Edit, NativeSetupStage as Stage, NativeSetupStatus as Status,
    SetupOperation, SetupWorkerEvidence,
};
use orchbee_workbench_protocol::{
    Directory, Project, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchProjectId,
    WorkbenchSnapshotUnavailableReason, WorkbenchWorkspaceId, Workspace,
};

pub(super) fn group_intent(
    state: &NativeSessionState,
    label: &str,
) -> crate::terminal_session::state::workspace_setup::NativeSetupIntent {
    use orchbee_workbench_protocol::{TabConfiguration, TabGroupColor, WorkbenchTabGroupLabel};
    crate::terminal_session::state::workspace_setup::NativeSetupIntent::TabGroupName(Box::new(
        TabConfiguration::CreateGroup {
            tab_id: state.cache().tabs()[0].tab_id().clone(),
            label: WorkbenchTabGroupLabel::parse(label).unwrap(),
            color: TabGroupColor::Blue,
        },
    ))
}

fn organization_intent(
    state: &NativeSessionState,
) -> crate::terminal_session::state::workspace_setup::NativeSetupIntent {
    let workspace = state.cache().canonical_snapshot().navigation.workspaces[0]
        .workspace_id
        .clone();
    crate::terminal_session::state::workspace_setup::NativeSetupIntent::Organize(Box::new(
        crate::terminal_session::WorkspaceOrganizationRequest::workspace(
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
pub(super) fn workspace_state() -> NativeSessionState {
    let snapshot = workspace_snapshot(1, 0);
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
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

#[test]
fn ux_menu_directory_refusal_query_preserves_original_draft_admission() {
    use crate::terminal_session::NativeContextLocation;
    use crate::terminal_session::state::workspace_setup::{
        DirectoryConfigurationUnavailable, NativeSetupIntent,
    };
    let mut empty = workspace_state();
    assert_eq!(empty.directory_configuration_unavailable(), None);
    let token = empty
        .open_setup_draft(NativeSetupIntent::ConfigureDirectory)
        .unwrap();
    assert_eq!(
        empty.directory_configuration_unavailable(),
        Some(DirectoryConfigurationUnavailable::Busy)
    );
    assert!(
        empty
            .open_setup_draft(NativeSetupIntent::ConfigureDirectory)
            .is_none()
    );
    empty.cancel_setup_draft(token);
    assert_eq!(empty.directory_configuration_unavailable(), None);

    let snapshot = workspace_snapshot(1, 1);
    let location = NativeContextLocation::Workspace {
        workspace_id: snapshot.navigation.workspaces[0].workspace_id.clone(),
        terminal_context_id: snapshot.navigation.workspaces[0]
            .terminal_context_id
            .clone(),
    };
    let cache = crate::terminal_session::workbench_projection::project_native_context_tabs(
        &snapshot, &location,
    )
    .unwrap();
    let mut nonempty = ready_with_cache(cache);
    assert_eq!(
        nonempty.directory_configuration_unavailable(),
        Some(DirectoryConfigurationUnavailable::EmptyRequired)
    );
    assert!(
        nonempty
            .open_setup_draft(NativeSetupIntent::ConfigureDirectory)
            .is_none()
    );

    let mut standalone = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    assert_eq!(
        standalone.directory_configuration_unavailable(),
        Some(DirectoryConfigurationUnavailable::WorkspaceRequired)
    );
    assert!(
        standalone
            .open_setup_draft(NativeSetupIntent::ConfigureDirectory)
            .is_none()
    );
}
fn label_operation(state: &mut NativeSessionState, label: &str, generation: u64) -> SetupOperation {
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
    assert!(state.edit_setup_draft(token, Edit::Clear));
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
    state: &mut NativeSessionState,
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
    baseline.navigation.workspaces[0].project_id = None;
    let mut state = ready_with_cache(project_native_standalone_tabs(&baseline).unwrap());
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
fn directory_literal_byte_limit_completeness_and_exact_owner_receipt() {
    let mut state = workspace_state();
    let token = state
        .open_setup_draft(
            crate::terminal_session::state::workspace_setup::NativeSetupIntent::ConfigureDirectory,
        )
        .unwrap();
    state.acknowledge_setup_draw(Some(token));
    assert!(state.edit_setup_draft(token, Edit::Append(&format!("/{}", "中".repeat(1365)))));
    assert_eq!(state.setup_draft().unwrap().text().len(), 4096);
    assert!(state.edit_setup_draft(token, Edit::Append("x")));
    assert!(state.setup_draft().unwrap().faulted());
    state.edit_setup_draft(token, Edit::Clear);
    let path = "/tmp/literal ~ $HOME space";
    state.edit_setup_draft(token, Edit::Append(path));
    state.edit_setup_draft(token, Edit::Completeness(false));
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let request_id = WorkbenchRequestId::parse(&id(98)).unwrap();
    assert!(
        state
            .plan_setup(token, request_id.clone(), 1, deadlines)
            .is_none()
    );
    state.edit_setup_draft(token, Edit::Completeness(true));
    let operation = state.plan_setup(token, request_id, 1, deadlines).unwrap();
    state.begin_setup(operation.clone());
    let directory = Directory {
        directory_id: WorkbenchDirectoryId::parse(&id(80)).unwrap(),
        worker_id: WorkerId::parse(&id(81)).unwrap(),
        path: WorkbenchDirectoryPath::parse(path).unwrap(),
    };
    let known = operation
        .known_commit(WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
            revision: WorkbenchRevision::new(2),
            directory_id: directory.directory_id.clone(),
            worker_id: directory.worker_id.clone(),
        })
        .unwrap();
    state.retain_setup_worker(SetupWorkerEvidence::Known(known));
    let mut candidate = workspace_snapshot(2, 0);
    candidate.navigation.workspaces[0].primary_directory_id = Some(directory.directory_id.clone());
    candidate.navigation.directories.push(directory);
    assert_eq!(
        finish(&mut state, candidate, 1),
        MembershipInstallDecision::Preserved
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
    assert!(state.desired().is_none());
    assert_eq!(state.workspace_directory_path(), Some(path));
    assert!(state.open_setup_draft(crate::terminal_session::state::workspace_setup::NativeSetupIntent::ConfigureDirectory).is_none());
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
    assert!(state.open_setup_draft(crate::terminal_session::state::workspace_setup::NativeSetupIntent::ConfigureDirectory).is_none());
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
            crate::terminal_session::state::workspace_setup::NativeSetupIntent::ConfigureDirectory,
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
    assert_eq!(state.setup_status(), Some(Status::NotApplied));
    assert!(state.retain_setup_worker(SetupWorkerEvidence::NotApplied(operation)));
    assert!(!state.setup_blocks_structure());
}
#[test]
fn directory_commit_cannot_claim_an_existing_directory_identity_at_a_later_revision() {
    let mut state = workspace_state();
    let mut baseline = workspace_snapshot(2, 0);
    let directory = Directory {
        directory_id: WorkbenchDirectoryId::parse(&id(80)).unwrap(),
        worker_id: WorkerId::parse(&id(81)).unwrap(),
        path: WorkbenchDirectoryPath::parse("/literal").unwrap(),
    };
    baseline.navigation.directories.push(directory.clone());
    assert!(state.start_pending_refresh(2));
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        2,
        WorkbenchSnapshotOutcome::Available {
            snapshot: baseline.clone(),
        },
    ) else {
        panic!("empty refresh");
    };
    state
        .prepare_refresh_membership_install(2, prepared, None, None)
        .unwrap()
        .commit();
    let token = state
        .open_setup_draft(
            crate::terminal_session::state::workspace_setup::NativeSetupIntent::ConfigureDirectory,
        )
        .unwrap();
    state.acknowledge_setup_draw(Some(token));
    state.edit_setup_draft(token, Edit::Append("/literal"));
    let operation = state
        .plan_setup(
            token,
            WorkbenchRequestId::parse(&id(99)).unwrap(),
            3,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    state.begin_setup(operation.clone());
    let known = operation
        .known_commit(WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
            revision: WorkbenchRevision::new(3),
            directory_id: directory.directory_id.clone(),
            worker_id: directory.worker_id,
        })
        .unwrap();
    state.retain_setup_worker(SetupWorkerEvidence::Known(known));
    baseline.revision = WorkbenchRevision::new(4);
    baseline.navigation.workspaces[0].primary_directory_id = Some(directory.directory_id);
    assert!(matches!(
        state.prepare_refresh(
            3,
            WorkbenchSnapshotOutcome::Available { snapshot: baseline }
        ),
        PreparedRefresh::Divergent { .. }
    ));
    assert!(state.workspace_directory_unconfigured());
    assert_eq!(state.setup_status(), Some(Status::CommittedNeedRefresh));
}

#[test]
fn setup_draft_edits_graphemes_and_the_picker_stays_inert_to_text() {
    use crate::terminal_session::state::workspace_setup::NativeSetupIntent;
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

    // DirectoryPath: byte budget, empty prefill, full editing vocabulary.
    let token = state
        .open_setup_draft(NativeSetupIntent::ConfigureDirectory)
        .unwrap();
    state.acknowledge_setup_draw(Some(token));
    assert!(state.edit_setup_draft(token, Edit::Append("/var/tmp")));
    assert!(state.edit_setup_draft(token, Edit::Left));
    assert!(state.edit_setup_draft(token, Edit::Delete));
    assert_eq!(state.setup_draft().unwrap().text(), "/var/tm");
    assert!(state.edit_setup_draft(token, Edit::Home));
    assert!(state.edit_setup_draft(token, Edit::Append("~")));
    assert_eq!(state.setup_draft().unwrap().text(), "~/var/tm");
    assert!(state.edit_setup_draft(token, Edit::End));
    assert!(state.edit_setup_draft(token, Edit::Paste("/中")));
    assert_eq!(state.setup_draft().unwrap().text(), "~/var/tm/中");
    // The window's cursor column is a display cell inside the field width.
    let window = state.setup_draft().unwrap().field_window(8).unwrap();
    assert!(window.cursor_column() < 8);
    // An over-budget paste rejects atomically: text and cursor unchanged.
    let before = state.setup_draft().unwrap().text().to_owned();
    assert!(state.edit_setup_draft(token, Edit::Paste(&"x".repeat(4096))));
    assert_eq!(state.setup_draft().unwrap().text(), before);
    assert!(state.setup_draft().unwrap().faulted());
    assert!(state.edit_setup_draft(token, Edit::Backspace));
    assert!(!state.setup_draft().unwrap().faulted());
    state.cancel_setup_draft(token);

    // A rename stage prefills the exact label with the cursor at its end.
    let rename = state
        .open_setup_draft(NativeSetupIntent::RenameLocation(
            crate::terminal_session::state::workspace_setup::NativeLocationLabelTarget::Workspace,
        ))
        .unwrap();
    state.acknowledge_setup_draw(Some(rename));
    assert_eq!(state.setup_draft().unwrap().stage(), Stage::WorkspaceRename);
    assert_eq!(state.setup_draft().unwrap().text(), "Workspace");
    assert!(state.edit_setup_draft(rename, Edit::Append("2")));
    assert_eq!(state.setup_draft().unwrap().text(), "Workspace2");
}

#[test]
fn setup_first_geometry_block_refuses_the_open_attempt_without_a_hidden_owner() {
    let mut state = workspace_state();
    let token = state
        .open_setup_draft(
            crate::terminal_session::state::workspace_setup::NativeSetupIntent::ConfigureDirectory,
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
            crate::terminal_session::state::workspace_setup::NativeSetupIntent::ConfigureDirectory,
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
