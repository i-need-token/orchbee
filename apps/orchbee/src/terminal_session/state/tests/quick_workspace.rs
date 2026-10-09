use super::*;
use crate::localization::Locale;
use crate::terminal_session::state::workspace_setup::quick::{
    ProjectQuickWorkspaceRequest, QuickPreparation, QuickProject, QuickWorkspaceFields,
};
use crate::terminal_session::state::workspace_setup::{
    NativeSetupStatus, SetupOperation, SetupWorkerEvidence,
};
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, project_native_context_tabs,
};
use orchbee_workbench_protocol::{
    Project, WorkbenchAction, WorkbenchDirectoryPath, WorkbenchProjectId,
};

#[test]
fn quick_workspace_project_ownership_uses_source_ids_and_requires_ambiguous_selection() {
    let cwd = WorkbenchDirectoryPath::parse("C:/项目 space").unwrap();
    let empty = projected(1, 1);
    let fields = QuickWorkspaceFields::new(&empty, cwd.clone(), Locale::EnUs);
    assert_eq!(fields.project(), &QuickProject::CreateDefault);
    assert_eq!(fields.name(), "项目 space");
    assert_eq!(fields.directory(), "C:/项目 space");
    let mut snapshot = workspace_setup::workspace_snapshot(1, 0);
    let project = snapshot.navigation.projects[0].project_id.clone();
    let standalone = project_native_standalone_tabs(&snapshot).unwrap();
    assert_eq!(
        QuickWorkspaceFields::new(&standalone, cwd.clone(), Locale::EnUs).project(),
        &QuickProject::Existing(project.clone())
    );
    let mut second = snapshot.navigation.projects[0].clone();
    second.project_id = WorkbenchProjectId::parse(&id(79)).unwrap();
    second.workspaces.clear();
    snapshot.navigation.projects.push(second);
    let standalone = project_native_standalone_tabs(&snapshot).unwrap();
    let mut fields = QuickWorkspaceFields::new(&standalone, cwd.clone(), Locale::EnUs);
    assert_eq!(fields.project(), &QuickProject::Choose);
    assert!(fields.plan().is_none());
    assert!(fields.select_project(&standalone.projects()[1].project_id, &standalone));
    assert_eq!(
        fields.plan().unwrap().project,
        QuickProject::Existing(standalone.projects()[1].project_id.clone())
    );
    let source = workspace_setup::workspace_state();
    assert_eq!(
        QuickWorkspaceFields::new(source.cache(), cwd, Locale::EnUs).project(),
        &QuickProject::Existing(project)
    );
}

#[test]
fn quick_workspace_fields_normalize_relative_and_home_paths_without_mutating_raw_text() {
    let mut fields = QuickWorkspaceFields::new(
        &projected(1, 1),
        WorkbenchDirectoryPath::parse("C:/项目 space").unwrap(),
        Locale::EnUs,
    );
    fields.directory_editor().clear();
    fields.directory_editor().insert("child").unwrap();
    assert_eq!(fields.directory(), "child");
    assert_eq!(fields.plan().unwrap().cwd.as_str(), "C:/项目 space\\child");
    fields.directory_editor().clear();
    fields.directory_editor().insert("~").unwrap();
    assert!(
        fields
            .plan()
            .unwrap()
            .cwd
            .as_str()
            .starts_with(std::env::var("USERPROFILE").unwrap().as_str())
    );
    assert_eq!(fields.directory(), "~");
    fields.directory_editor().clear();
    fields.directory_editor().insert("C:relative").unwrap();
    assert!(fields.plan().is_none());
    fields.directory_editor().clear();
    assert!(fields.plan().is_none());
}

#[test]
fn quick_workspace_name_and_directory_have_independent_atomic_limits() {
    let mut fields = QuickWorkspaceFields::new(
        &projected(1, 1),
        WorkbenchDirectoryPath::parse("C:/").unwrap(),
        Locale::ZhCn,
    );
    assert_eq!(fields.name(), "工作区");
    fields.name_editor().clear();
    fields.name_editor().insert(&"猫".repeat(512)).unwrap();
    assert!(fields.name_editor().insert("猫").is_err());
    assert_eq!(fields.name().chars().count(), 512);
    fields.directory_editor().clear();
    let path = format!("C:/{}x", "猫".repeat(1364));
    assert_eq!(path.len(), 4096);
    fields.directory_editor().insert(&path).unwrap();
    assert!(fields.directory_editor().insert("x").is_err());
    assert_eq!(fields.directory(), path);
    assert!(fields.plan().is_some());
    fields.name_editor().clear();
    assert!(fields.plan().is_none());
}

fn targeted_source() -> (NativeSessionState, Snapshot, Project, PaneRef) {
    let mut snapshot = workspace_setup::workspace_snapshot(1, 0);
    snapshot.terminal_contexts[0].tabs.clear();
    snapshot.terminal_contexts[1].tabs = split_panel_baseline().terminal_contexts[0].tabs.clone();
    let mut target = snapshot.navigation.projects[0].clone();
    target.project_id = WorkbenchProjectId::parse(&id(79)).unwrap();
    target.workspaces.clear();
    snapshot.navigation.projects.push(target.clone());
    let location = NativeContextLocation::Workspace {
        project_id: snapshot.navigation.projects[0].project_id.clone(),
        workspace_id: snapshot.navigation.projects[0].workspaces[0]
            .workspace_id
            .clone(),
        terminal_context_id: snapshot.terminal_contexts[1].terminal_context_id.clone(),
    };
    let mut state = ready_with_cache(project_native_context_tabs(&snapshot, &location).unwrap());
    let peer = attach_nonfirst_leaf(&mut state);
    (state, snapshot, target, peer)
}

fn begin_targeted(state: &mut NativeSessionState, target: &Project) -> QuickPreparation {
    let request =
        ProjectQuickWorkspaceRequest::capture(state, target, Instant::now() + OPERATION_TOTAL)
            .unwrap();
    state
        .begin_quick_preparation(
            11,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
            Some(request),
        )
        .unwrap()
}

fn settle_preparation_read(
    state: &mut NativeSessionState,
    preparation: &QuickPreparation,
    snapshot: &Snapshot,
) {
    assert_eq!(state.take_quick_preparation(preparation), Some(true));
    let prepared = state.prepare_refresh(
        preparation.generation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot.clone(),
        },
    );
    assert_eq!(
        state.apply_prepared_refresh(prepared, Instant::now()),
        RefreshDecision::Continue
    );
}

fn finish_targeted(state: &mut NativeSessionState, preparation: &QuickPreparation) {
    state.finish_quick_preparation(
        preparation,
        Some(WorkbenchDirectoryPath::parse("C:/source").unwrap()),
        Locale::EnUs,
    );
}

fn targeted_plan(state: &mut NativeSessionState) -> SetupOperation {
    let token = state.setup_draft().unwrap().token();
    state.acknowledge_setup_draw(Some(token));
    state
        .plan_setup(
            token,
            workbench_request_id(99),
            12,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap()
}

#[test]
fn project_quick_rejects_changed_focus_with_identical_context_before_prepare_or_confirm() {
    for during_preparation in [false, true] {
        let (mut state, snapshot, target, peer) = targeted_source();
        let preparation = begin_targeted(&mut state, &target);
        settle_preparation_read(&mut state, &preparation, &snapshot);
        if !during_preparation {
            finish_targeted(&mut state, &preparation);
            let operation = targeted_plan(&mut state);
            assert_eq!(
                operation.quick.unwrap().project,
                QuickProject::Targeted(preparation.target.clone().unwrap())
            );
        }
        let before = state.cache().clone();
        // Inject an independently changed source; modal input normally prevents this focus action.
        let selected = SelectedTab::new_panel(peer.pane().clone(), 0, 1);
        state.desired = Some(peer.pane().clone());
        state.phase = SessionPhase::Ready(ReadyAttachment {
            selected,
            mode: controlling(),
        });
        assert_eq!(state.cache(), &before);
        assert!(state.invariant_holds());
        if during_preparation {
            finish_targeted(&mut state, &preparation);
        } else {
            assert!(state.setup_draft().is_none());
            assert!(state.settle_stale_quick_workspace());
        }
        assert!(state.setup_draft().is_none());
        assert_eq!(state.setup_status(), Some(NativeSetupStatus::NotApplied));
        assert!(!state.setup_blocks_structure());
        assert!(state.invariant_holds());
    }
}

#[test]
fn project_quick_never_substitutes_a_removed_or_same_id_replaced_target() {
    for stage in 0..3 {
        for removed in [false, true] {
            let (mut state, mut snapshot, target, _) = targeted_source();
            let request = ProjectQuickWorkspaceRequest::capture(
                &state,
                &target,
                Instant::now() + OPERATION_TOTAL,
            )
            .unwrap();
            let preparation = if stage > 0 {
                let preparation = begin_targeted(&mut state, &target);
                settle_preparation_read(&mut state, &preparation, &snapshot);
                if stage == 2 {
                    finish_targeted(&mut state, &preparation);
                }
                Some(preparation)
            } else {
                None
            };
            if removed {
                snapshot.navigation.projects.pop();
            } else {
                snapshot.navigation.projects[1].label = "Replacement".into();
            }
            state.cache = project_native_context_tabs(&snapshot, state.cache().location()).unwrap();
            match stage {
                0 => assert!(
                    state
                        .begin_quick_preparation(
                            11,
                            OperationDeadlines::from_sample(Instant::now()).unwrap(),
                            Some(request)
                        )
                        .is_none()
                ),
                1 => finish_targeted(&mut state, preparation.as_ref().unwrap()),
                _ => assert!(state.settle_stale_quick_workspace()),
            }
            assert!(state.setup_draft().is_none());
            assert!(!state.setup_blocks_structure());
            assert!(
                matches!(state.cache().location(), NativeContextLocation::Workspace { workspace_id, .. }
                if workspace_id == &snapshot.navigation.projects[0].workspaces[0].workspace_id)
            );
        }
    }
}

fn committed_launch_snapshot(
    snapshot: &Snapshot,
    operation: &SetupOperation,
) -> (
    Snapshot,
    crate::local_client::workbench::workspace_launch::WorkspaceLaunchCommit,
) {
    use orchbee_workbench_protocol::{
        Directory, WorkbenchDirectoryId, WorkbenchTerminalOutcome, WorkbenchWorkspaceId, Workspace,
    };
    let WorkbenchAction::WorkspaceLaunch {
        project_id,
        label,
        options,
        geometry,
        ..
    } = &operation.command.action
    else {
        panic!("targeted launch");
    };
    let mut snapshot = snapshot.clone();
    snapshot.revision = WorkbenchRevision::new(3);
    let target = snapshot
        .navigation
        .projects
        .iter_mut()
        .find(|p| &p.project_id == project_id)
        .unwrap();
    let created = tab(20);
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        panic!("single first shell");
    };
    let commit = crate::local_client::workbench::workspace_launch::WorkspaceLaunchCommit {
        revision: WorkbenchRevision::new(2),
        project_id: project_id.clone(),
        workspace_id: WorkbenchWorkspaceId::parse(&id(80)).unwrap(),
        directory_id: WorkbenchDirectoryId::parse(&id(81)).unwrap(),
        terminal_context_id: context_id(82),
        tab_id: created.tab_id.clone(),
        panel_id: panel_id.clone(),
        presentation_id: primary_presentation.presentation_id.clone(),
        terminal_outcome: Some(WorkbenchTerminalOutcome::Started {
            geometry: *geometry,
        }),
    };
    target.directories.push(Directory {
        directory_id: commit.directory_id.clone(),
        worker_id: primary_presentation
            .binding
            .as_ref()
            .unwrap()
            .worker_id
            .clone(),
        path: options.cwd.clone().unwrap(),
    });
    target.workspaces.push(Workspace {
        workspace_id: commit.workspace_id.clone(),
        terminal_context_id: commit.terminal_context_id.clone(),
        primary_directory_id: Some(commit.directory_id.clone()),
        label: label.as_str().to_owned(),
    });
    let mut reordered = created.clone();
    reordered.root = PanelNode::Split {
        ratio: Default::default(),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&id(4_020)).unwrap(),
        children: Box::new([
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&id(4_001)).unwrap(),
                node: tab(22).root,
            },
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&id(4_014)).unwrap(),
                node: created.root,
            },
        ]),
    };
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: commit.terminal_context_id.clone(),
        groups: vec![],
        tabs: vec![tab(21), reordered],
    });
    (snapshot, commit)
}

#[test]
fn project_quick_enters_receipt_member_after_peer_reorders_tabs_and_panels() {
    use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
    let (mut state, snapshot, target, _) = targeted_source();
    let preparation = begin_targeted(&mut state, &target);
    settle_preparation_read(&mut state, &preparation, &snapshot);
    finish_targeted(&mut state, &preparation);
    let operation = targeted_plan(&mut state);
    let (snapshot, commit) = committed_launch_snapshot(&snapshot, &operation);
    state.begin_setup(operation.clone());
    let known = operation
        .known_commit(WorkbenchSetupCommit::WorkspaceLaunched(Box::new(
            commit.clone(),
        )))
        .unwrap();
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    let MembershipInstallDecision::Attach(attachment) =
        workspace_setup::finish(&mut state, snapshot, operation.generation)
    else {
        panic!("exact Started member attaches");
    };
    let key = attachment.selected().key().clone();
    assert_eq!(key.anchor().tab_id(), &commit.tab_id);
    assert_eq!(key.panel(), &commit.panel_id);
    assert!(
        matches!(state.cache().location(), NativeContextLocation::Workspace { project_id, workspace_id, terminal_context_id }
        if project_id == &commit.project_id && workspace_id == &commit.workspace_id && terminal_context_id == &commit.terminal_context_id)
    );
    assert_eq!(state.active().unwrap().index(), 1);
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert_eq!(
        state
            .selected_presentation(state.cache().tabs()[1].panel(1).unwrap())
            .presentation_id(),
        &commit.presentation_id
    );
    assert_ne!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(2)));
    assert!(state.publish_ready_t(
        &key,
        FrameIdentity::new(2),
        controlling(),
        Some(geometry(80))
    ));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.invariant_holds());
}

#[test]
fn project_quick_retains_known_receipt_when_started_member_is_unavailable() {
    use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
    for missing in ["started", "tab", "panel", "presentation"] {
        let (mut state, snapshot, target, _) = targeted_source();
        let preparation = begin_targeted(&mut state, &target);
        settle_preparation_read(&mut state, &preparation, &snapshot);
        finish_targeted(&mut state, &preparation);
        let operation = targeted_plan(&mut state);
        let (mut snapshot, mut commit) = committed_launch_snapshot(&snapshot, &operation);
        match missing {
            "started" => commit.terminal_outcome = None,
            "tab" => snapshot
                .terminal_contexts
                .last_mut()
                .unwrap()
                .tabs
                .retain(|tab| tab.tab_id != commit.tab_id),
            _ => {
                let PanelNode::Split { children, .. } =
                    &mut snapshot.terminal_contexts.last_mut().unwrap().tabs[1].root
                else {
                    panic!("reordered receipt panel");
                };
                let PanelNode::Panel {
                    panel_id,
                    primary_presentation,
                    ..
                } = &mut children[1].node
                else {
                    panic!("receipt panel");
                };
                if missing == "panel" {
                    *panel_id = WorkbenchPanelId::parse(&id(7_000)).unwrap();
                } else {
                    primary_presentation.presentation_id =
                        TerminalPresentationId::parse(&id(7_001)).unwrap();
                }
            }
        }
        let original = state.desired().cloned();
        state.begin_setup(operation.clone());
        let known = operation
            .known_commit(WorkbenchSetupCommit::WorkspaceLaunched(Box::new(
                commit.clone(),
            )))
            .unwrap();
        assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
        assert!(matches!(
            state.prepare_refresh(
                operation.generation,
                WorkbenchSnapshotOutcome::Available { snapshot }
            ),
            PreparedRefresh::Divergent { .. }
        ));
        assert_eq!(state.desired(), original.as_ref());
        assert_eq!(
            state.quick_workspace_recovery().unwrap().workspace,
            Some(commit.workspace_id.as_str())
        );
    }
}
