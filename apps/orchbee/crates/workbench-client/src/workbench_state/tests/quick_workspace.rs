use super::*;
use crate::localization::Locale;
use crate::workbench_projection::{ClientContextLocation, project_native_context_tabs};
use crate::workbench_state::workspace_setup::quick::{
    ProjectQuickWorkspaceRequest, QuickPreparation, QuickProject, QuickWorkspaceFields,
};
use crate::workbench_state::workspace_setup::{SetupOperation, SetupStatus, SetupWorkerEvidence};
use orchbee_workbench_protocol::{
    Project, WorkbenchAction, WorkbenchDirectoryPath, WorkbenchProjectId,
};

#[test]
fn quick_workspace_defaults_ungrouped_and_selects_only_an_explicit_project() {
    let cwd = WorkbenchDirectoryPath::parse("C:/项目 space").unwrap();
    let empty = projected(1, 1);
    let fields = QuickWorkspaceFields::new(&empty, cwd.clone(), Locale::EnUs);
    assert_eq!(fields.project(), &QuickProject::Ungrouped);
    assert_eq!(fields.name(), "项目 space");
    assert_eq!(fields.directory(), "C:/项目 space");
    let mut snapshot = workspace_setup::workspace_snapshot(1, 0);
    let initial_workspace = project_preferred_workspace(&snapshot).unwrap();
    assert_eq!(
        QuickWorkspaceFields::new(&initial_workspace, cwd.clone(), Locale::EnUs).project(),
        &QuickProject::Ungrouped
    );
    let mut second = snapshot.navigation.projects[0].clone();
    second.project_id = WorkbenchProjectId::parse(&id(79)).unwrap();
    snapshot.navigation.projects.push(second);
    let initial_workspace = project_preferred_workspace(&snapshot).unwrap();
    let mut fields = QuickWorkspaceFields::new(&initial_workspace, cwd.clone(), Locale::EnUs);
    assert_eq!(fields.project(), &QuickProject::Ungrouped);
    assert_eq!(fields.plan().unwrap().project, QuickProject::Ungrouped);
    assert!(fields.select_project(
        &initial_workspace.projects()[1].project_id,
        &initial_workspace
    ));
    assert_eq!(
        fields.plan().unwrap().project,
        QuickProject::Existing(initial_workspace.projects()[1].project_id.clone())
    );
    let source = workspace_setup::workspace_state();
    assert_eq!(
        QuickWorkspaceFields::new(source.cache(), cwd, Locale::EnUs).project(),
        &QuickProject::Ungrouped
    );
}

#[test]
fn refused_quick_preparation_cannot_auto_open_when_its_read_finishes() {
    let mut state = ready_with_cache(projected(1, 1));
    let preparation = state
        .begin_quick_preparation(
            11,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
            None,
        )
        .unwrap();
    assert_eq!(
        state.setup_draft().unwrap().presentation(),
        crate::tui_presentation::PresentationPhase::Pending
    );
    assert_eq!(
        state.settle_setup_draft_not_presented(),
        Some(crate::tui_presentation::GeometrySettlement::Refused)
    );
    assert!(state.setup_draft().is_none());
    assert_eq!(
        state.setup_status(),
        Some(SetupStatus::PresentationUnavailable)
    );
    assert_eq!(state.take_quick_preparation(&preparation), Some(false));
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
    assert_eq!(fields.plan().unwrap().cwd.as_str(), "C:\\项目 space\\child");
    // Home expansion reads only the environment snapshot taken at the UI boundary.
    fields.directory_editor().clear();
    fields.directory_editor().insert("~").unwrap();
    assert!(fields.plan().is_none());
    fields.set_environment(vec![(
        "USERPROFILE".to_owned(),
        std::env::var_os("USERPROFILE").unwrap(),
    )]);
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

fn targeted_source() -> (WorkbenchClientState, Snapshot, Project, PaneRef) {
    let mut snapshot = workspace_setup::workspace_snapshot(1, 0);
    snapshot.terminal_contexts[0].tabs.clear();
    snapshot.terminal_contexts[1].tabs = split_panel_baseline().terminal_contexts[0].tabs.clone();
    let mut target = snapshot.navigation.projects[0].clone();
    target.project_id = WorkbenchProjectId::parse(&id(79)).unwrap();
    snapshot.navigation.projects.push(target.clone());
    let location = ClientContextLocation::Workspace {
        workspace_id: snapshot.navigation.workspaces[1].workspace_id.clone(),
        terminal_context_id: snapshot.terminal_contexts[1].terminal_context_id.clone(),
    };
    let mut state = ready_with_cache(project_native_context_tabs(&snapshot, &location).unwrap());
    let peer = attach_nonfirst_leaf(&mut state);
    (state, snapshot, target, peer)
}

fn begin_targeted(state: &mut WorkbenchClientState, target: &Project) -> QuickPreparation {
    let request = ProjectQuickWorkspaceRequest::capture(
        state,
        target,
        None,
        Instant::now() + OPERATION_TOTAL,
    )
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
    state: &mut WorkbenchClientState,
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

fn finish_targeted(state: &mut WorkbenchClientState, preparation: &QuickPreparation) {
    state.finish_quick_preparation(
        preparation,
        Some(WorkbenchDirectoryPath::parse("C:/source").unwrap()),
        Locale::EnUs,
        Vec::new(),
    );
}

fn targeted_plan(state: &mut WorkbenchClientState) -> SetupOperation {
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
        state.phase = ClientPhase::Ready(ReadyAttachment {
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
        assert_eq!(state.setup_status(), Some(SetupStatus::NotApplied));
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
                None,
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
                matches!(state.cache().location(), ClientContextLocation::Workspace { workspace_id, .. }
                if workspace_id == &snapshot.navigation.workspaces[1].workspace_id)
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
        placement,
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
    let project_id = match placement {
        orchbee_workbench_protocol::WorkspacePlacement::Project { project_id } => {
            assert!(
                snapshot
                    .navigation
                    .projects
                    .iter()
                    .any(|p| &p.project_id == project_id)
            );
            Some(project_id.clone())
        }
        orchbee_workbench_protocol::WorkspacePlacement::Ungrouped {} => None,
        _ => panic!("fixture launch placement"),
    };
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
        project_id,
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
    snapshot.navigation.directories.push(Directory {
        directory_id: commit.directory_id.clone(),
        worker_id: primary_presentation
            .binding
            .as_ref()
            .unwrap()
            .worker_id
            .clone(),
        path: options.cwd.clone().unwrap(),
    });
    snapshot.navigation.workspaces.push(Workspace {
        automatic_label: false,
        project_id: commit.project_id.clone(),
        workspace_id: commit.workspace_id.clone(),
        terminal_context_id: commit.terminal_context_id.clone(),
        primary_directory_id: Some(commit.directory_id.clone()),
        label: label.as_str().to_owned(),
    });
    if snapshot.navigation.workspace_order.is_empty() {
        snapshot.navigation.workspace_order = snapshot
            .navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect();
    } else {
        snapshot
            .navigation
            .workspace_order
            .push(commit.workspace_id.clone());
    }
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

fn project_quick_entering(
    before_commit: impl FnOnce(&mut WorkbenchClientState),
) -> (
    WorkbenchClientState,
    PaneKey,
    crate::workbench_state::workspace_setup::SetupKnownCommit,
    Snapshot,
) {
    use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
    let (mut state, snapshot, target, _) = targeted_source();
    let preparation = begin_targeted(&mut state, &target);
    settle_preparation_read(&mut state, &preparation, &snapshot);
    finish_targeted(&mut state, &preparation);
    let operation = targeted_plan(&mut state);
    let (snapshot, commit) = committed_launch_snapshot(&snapshot, &operation);
    state.begin_setup(operation.clone());
    assert!(!state.take_project_quick_terminal_entry());
    before_commit(&mut state);
    let known = operation
        .known_commit(WorkbenchSetupCommit::WorkspaceLaunched(Box::new(
            commit.clone(),
        )))
        .unwrap();
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known.clone())));
    assert!(!state.take_project_quick_terminal_entry());
    let source = state.cache().location().clone();
    let original = state.desired().unwrap().clone();
    let decision = workspace_setup::finish(&mut state, snapshot.clone(), operation.generation);
    if decision == MembershipInstallDecision::Preserved {
        assert_eq!(state.cache().location(), &source);
        assert_eq!(state.desired(), Some(&original));
        assert!(!state.take_project_quick_terminal_entry());
        return (state, original, known, snapshot);
    }
    let MembershipInstallDecision::Attach(attachment) = decision else {
        panic!("exact Started member attaches");
    };
    let key = attachment.selected().key().clone();
    assert_eq!(key.anchor().tab_id(), &commit.tab_id);
    assert_eq!(key.panel(), &commit.panel_id);
    assert!(
        matches!(state.cache().location(), ClientContextLocation::Workspace { workspace_id, terminal_context_id }
        if workspace_id == &commit.workspace_id && terminal_context_id == &commit.terminal_context_id)
    );
    assert_eq!(state.active().unwrap().index(), 1);
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert_eq!(
        state
            .selected_presentation(state.cache().tabs()[1].panel(1).unwrap())
            .presentation_id(),
        &commit.presentation_id
    );
    assert_ne!(state.phase_kind(), ClientPhaseKind::Ready);
    assert!(!state.take_project_quick_terminal_entry());
    (state, key, known, snapshot)
}

fn ready_project_quick(state: &mut WorkbenchClientState, key: &PaneKey) {
    assert!(state.install_projection_t(key));
    assert!(state.record_setup_configuration_t(key, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame_t(key, FrameIdentity::new(2)));
    assert!(!state.take_project_quick_terminal_entry());
    assert!(state.mark_feedback_accepted_t(key, FrameIdentity::new(2)));
    assert!(!state.take_project_quick_terminal_entry());
    assert!(state.publish_ready_t(
        key,
        FrameIdentity::new(2),
        controlling(),
        Some(geometry(80))
    ));
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert!(state.invariant_holds());
}

#[test]
fn project_quick_enters_receipt_member_after_peer_reorders_tabs_and_panels() {
    for cancel_at in ["none", "entering", "ready"] {
        let (mut state, key, known, _) = project_quick_entering(|_| {});
        if cancel_at == "entering" {
            state.cancel_project_quick_entry();
        }
        ready_project_quick(&mut state, &key);
        if cancel_at == "ready" {
            // Input is dispatched before the shell's completion poll.
            state.cancel_project_quick_entry();
        }
        assert_eq!(
            state.take_project_quick_terminal_entry(),
            cancel_at == "none"
        );
        assert!(!state.take_project_quick_terminal_entry());
        assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
        assert!(!state.take_project_quick_terminal_entry());
    }
}

#[test]
fn project_quick_return_retires_mismatched_identity_even_if_restored_before_ready() {
    for mismatch in ["source", "desired", "presentation"] {
        let (mut state, key, _, mut snapshot) = project_quick_entering(|_| {});
        let cache = state.cache().clone();
        if mismatch == "source" {
            state.cache = project_preferred_workspace(&snapshot).unwrap();
        } else if mismatch == "desired" {
            state.desired = Some(PaneKey::new(
                AttachmentKey::new(key.anchor().generation() + 1, key.anchor().tab_id().clone()),
                key.panel().clone(),
            ));
        } else {
            let PanelNode::Split { children, .. } =
                &mut snapshot.terminal_contexts.last_mut().unwrap().tabs[1].root
            else {
                panic!("receipt panel");
            };
            let PanelNode::Panel {
                primary_presentation,
                ..
            } = &mut children[1].node
            else {
                panic!("receipt presentation");
            };
            primary_presentation.presentation_id =
                TerminalPresentationId::parse(&id(7_001)).unwrap();
            state.cache = project_native_context_tabs(&snapshot, cache.location()).unwrap();
        }
        assert!(!state.take_project_quick_terminal_entry());
        state.cache = cache;
        state.desired = Some(key.clone());
        ready_project_quick(&mut state, &key);
        assert!(!state.take_project_quick_terminal_entry());
    }
}

#[test]
fn project_row_and_disclosure_focus_retires_submitted_entering_and_queued_ready_return() {
    use crate::client_view::{ClientAction, ClientActionDisposition, ClientView, Overlay};
    use crate::machine_picker::{Choice, SidebarTarget};

    fn arm(view: &mut ClientView, state: &WorkbenchClientState) {
        for action in [
            ClientAction::ToggleBusinessSidebar.into(),
            crate::workbench_client::InputAction::project_quick_workspace(Box::new(
                ProjectQuickWorkspaceRequest::capture(
                    state,
                    &state.cache().projects()[1],
                    None,
                    Instant::now() + OPERATION_TOTAL,
                )
                .unwrap(),
            )),
        ] {
            let transition = view.decide_action(action.intent());
            view.commit(transition, ClientActionDisposition::KeepNavigator);
        }
        view.machines.available = true;
        view.machines.observe_agent_sidebar(true);
        assert!(view.project_quick_entry_pending());
        assert!(!view.machines.sidebar_navigation_active());
    }

    fn press_project(view: &mut ClientView, state: &mut WorkbenchClientState, leave: bool) {
        let ClientContextLocation::Workspace { .. } = state.cache().location() else {
            panic!("Workspace source or entered receipt");
        };
        let project_id = state.cache().project_id().unwrap();
        let target = SidebarTarget::Project {
            machine: Choice::Local,
            session_id: state.cache().session_id().clone(),
            project_id: project_id.clone(),
        };
        view.machines
            .observe_local(vec![crate::machine_discovery::Context {
                target: crate::workbench_projection::ClientContextTarget {
                    session_id: state.cache().session_id().clone(),
                    location: state.cache().location().clone(),
                },
                project_id: Some(project_id.clone()),
                project: Some("Project".into()),
                workspace: Some("Workspace".into()),
            }]);
        // Both MachineGroupRow and SidebarDisclosure reach this exact
        // Picker owner transition, bypassing ClientAction/commit_pointer.
        assert!(view.machines.sidebar_target_exists(&target));
        assert!(view.machines.focus_sidebar_target(&target));
        assert!(view.machines.toggle_sidebar_group(&target));
        assert!(view.machines.sidebar_navigation_active());
        assert!(view.project_quick_entry_pending());
        // The initial Native shell adapter settles intent before any Ready take.
        view.settle_project_quick_entry_intent();
        if !view.project_quick_entry_pending() {
            state.cancel_project_quick_entry();
        }
        assert!(!view.project_quick_entry_pending());
        if leave {
            view.machines.leave_sidebar_navigation();
        }
    }

    for phase in ["submitted", "entering", "ready"] {
        for leave in [false, true] {
            let mut view = ClientView::new(80);
            let (mut state, key, known, _) = project_quick_entering(|state| {
                arm(&mut view, state);
                if phase == "submitted" {
                    press_project(&mut view, state, leave);
                }
            });
            if phase == "entering" {
                press_project(&mut view, &mut state, leave);
            }
            if phase != "submitted" {
                ready_project_quick(&mut state, &key);
            }
            if phase == "ready" {
                press_project(&mut view, &mut state, leave);
            }
            assert!(!state.take_project_quick_terminal_entry());
            assert!(!view.complete_project_quick_entry());
            assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
            assert_eq!(view.machines.sidebar_navigation_active(), !leave);
            assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
            view.machines.leave_sidebar_navigation();
            view.settle_project_quick_entry_intent();
            assert!(!state.take_project_quick_terminal_entry());
            assert!(!view.complete_project_quick_entry());
            assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
        }
    }
}

#[test]
fn newer_project_quick_submission_cannot_emit_the_previous_operations_return() {
    let (mut state, key, old_known, snapshot) = project_quick_entering(|_| {});
    ready_project_quick(&mut state, &key);
    let project = state.cache().projects()[1].clone();
    let preparation = begin_targeted(&mut state, &project);
    settle_preparation_read(&mut state, &preparation, &snapshot);
    finish_targeted(&mut state, &preparation);
    let mut next = targeted_plan(&mut state);
    next.generation = 13;
    next.command.request_id = workbench_request_id(100);
    next.request =
        crate::local_client::workbench::workspace_setup::WorkbenchSetupRequest::try_from(
            next.command.clone(),
        )
        .unwrap();
    state.begin_setup(next);
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(old_known)));
    assert!(!state.take_project_quick_terminal_entry());
    assert!(state.setup_pending_or_draft());
    assert!(state.invariant_holds());
}

#[test]
fn project_quick_retains_known_receipt_when_started_member_is_unavailable() {
    use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
    for missing in ["binding", "tab", "panel", "presentation"] {
        let (mut state, snapshot, target, _) = targeted_source();
        let preparation = begin_targeted(&mut state, &target);
        settle_preparation_read(&mut state, &preparation, &snapshot);
        finish_targeted(&mut state, &preparation);
        let operation = targeted_plan(&mut state);
        let (mut snapshot, commit) = committed_launch_snapshot(&snapshot, &operation);
        match missing {
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
                if missing == "binding" {
                    primary_presentation.binding = None;
                } else if missing == "panel" {
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
        assert!(!state.take_project_quick_terminal_entry());
        assert!(matches!(
            state.prepare_refresh(
                operation.generation,
                WorkbenchSnapshotOutcome::Available { snapshot }
            ),
            PreparedRefresh::Divergent { .. }
        ));
        assert_eq!(state.desired(), original.as_ref());
        assert!(!state.take_project_quick_terminal_entry());
        assert_eq!(
            state.quick_workspace_recovery().unwrap().workspace,
            Some(commit.workspace_id.as_str())
        );
    }
}

#[test]
fn revoked_launch_landing_keeps_current_focus_and_still_validates_the_known_resources() {
    use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
    for targeted in [false, true] {
        for cancel_after_receipt in [false, true] {
            let (mut state, snapshot, project, _) = targeted_source();
            let preparation = if targeted {
                begin_targeted(&mut state, &project)
            } else {
                state
                    .begin_quick_preparation(
                        11,
                        OperationDeadlines::from_sample(Instant::now()).unwrap(),
                        None,
                    )
                    .unwrap()
            };
            settle_preparation_read(&mut state, &preparation, &snapshot);
            finish_targeted(&mut state, &preparation);
            let operation = targeted_plan(&mut state);
            let (committed, receipt) = committed_launch_snapshot(&snapshot, &operation);
            let location = state.cache().location().clone();
            let focus = state.focus_pane_ref();
            let desired = state.desired().cloned();
            state.begin_setup(operation.clone());
            let known = operation
                .known_commit(WorkbenchSetupCommit::WorkspaceLaunched(Box::new(
                    receipt.clone(),
                )))
                .unwrap();
            if cancel_after_receipt {
                assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known.clone())));
            }
            // The UI revokes the pending landing before attempting a newer context action.
            state.cancel_project_quick_entry();
            assert!(!state.start_context_selection(12, true, operation.deadlines));
            if !cancel_after_receipt {
                assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known.clone())));
            }
            for field in ["directory", "tab", "presentation", "source"] {
                let mut forged = committed.clone();
                match field {
                    "directory" => {
                        forged.navigation.directories[0].path =
                            WorkbenchDirectoryPath::parse("C:/forged").unwrap()
                    }
                    "tab" => forged.terminal_contexts.last_mut().unwrap().tabs.clear(),
                    "presentation" => {
                        let PanelNode::Split { children, .. } =
                            &mut forged.terminal_contexts.last_mut().unwrap().tabs[1].root
                        else {
                            panic!("receipt split")
                        };
                        let PanelNode::Panel {
                            primary_presentation,
                            ..
                        } = &mut children[1].node
                        else {
                            panic!("receipt member")
                        };
                        primary_presentation.presentation_id = presentation_id(9901);
                    }
                    _ => forged.navigation.workspaces[1].terminal_context_id = context_id(9999),
                }
                assert!(
                    matches!(
                        state.prepare_refresh(
                            operation.generation,
                            WorkbenchSnapshotOutcome::Available { snapshot: forged }
                        ),
                        PreparedRefresh::Divergent { .. }
                    ),
                    "{field}"
                );
                assert_eq!(state.desired(), desired.as_ref());
                assert_eq!(state.focus_pane_ref(), focus);
                assert_eq!(
                    state.setup_status(),
                    Some(SetupStatus::CommittedNeedRefresh)
                );
            }
            assert_eq!(
                workspace_setup::finish(&mut state, committed.clone(), operation.generation),
                MembershipInstallDecision::Preserved
            );
            assert_eq!(state.cache().location(), &location);
            assert_eq!(state.cache().canonical_snapshot(), &committed);
            assert_eq!(state.desired(), desired.as_ref());
            assert_eq!(state.focus_pane_ref(), focus);
            assert_eq!(state.setup_status(), Some(SetupStatus::WorkspaceCreated));
            assert!(!state.setup_blocks_structure());
            assert!(!state.take_project_quick_terminal_entry());
            assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
            assert!(!state.take_project_quick_terminal_entry());
            assert!(state.invariant_holds());
        }
    }
}

#[test]
fn default_names_avoid_global_collisions_and_reserve_suffix_space_without_editing_explicit_names() {
    for basename in ["source".to_owned(), "猫".repeat(512)] {
        let suffixed = |ordinal| {
            format!(
                "{} ({ordinal})",
                basename.chars().take(508).collect::<String>()
            )
        };
        let mut snapshot = workspace_setup::workspace_snapshot(1, 0);
        snapshot.navigation.workspaces[1].label = basename.clone();
        let mut second = snapshot.navigation.workspaces[1].clone();
        second.workspace_id =
            orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&id(81)).unwrap();
        second.terminal_context_id = context_id(82);
        second.project_id = None;
        second.label = suffixed(2);
        snapshot.terminal_contexts.push(TerminalContext {
            terminal_context_id: second.terminal_context_id.clone(),
            groups: vec![],
            tabs: vec![],
        });
        snapshot.navigation.workspaces.push(second);
        let baseline = snapshot.clone();
        let cwd = WorkbenchDirectoryPath::parse(&format!("C:/{basename}")).unwrap();
        for targeted in [false, true] {
            let mut state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
            let preparation = if targeted {
                begin_targeted(&mut state, &snapshot.navigation.projects[0])
            } else {
                state
                    .begin_quick_preparation(
                        11,
                        OperationDeadlines::from_sample(Instant::now()).unwrap(),
                        None,
                    )
                    .unwrap()
            };
            settle_preparation_read(&mut state, &preparation, &snapshot);
            state.finish_quick_preparation(
                &preparation,
                Some(cwd.clone()),
                Locale::EnUs,
                Vec::new(),
            );
            let operation = targeted_plan(&mut state);
            let WorkbenchAction::WorkspaceLaunch { label, .. } = operation.command.action else {
                panic!("launch")
            };
            assert_eq!(label.as_str(), suffixed(3));
            assert!(label.as_str().chars().count() <= 512);
            assert_eq!(state.cache().canonical_snapshot(), &baseline);
        }
        let cache = project_preferred_workspace(&snapshot).unwrap();
        let mut fields = QuickWorkspaceFields::new(&cache, cwd, Locale::EnUs);
        fields.name_editor().clear();
        fields.name_editor().insert(&basename).unwrap();
        assert_eq!(
            fields.plan().unwrap().label.as_str(),
            basename,
            "explicit labels are not rewritten"
        );
        let removed = snapshot.navigation.workspaces.remove(1);
        snapshot
            .terminal_contexts
            .retain(|context| context.terminal_context_id != removed.terminal_context_id);
        let cache = project_preferred_workspace(&snapshot).unwrap();
        assert_eq!(
            QuickWorkspaceFields::new(
                &cache,
                WorkbenchDirectoryPath::parse(&format!("C:/{basename}")).unwrap(),
                Locale::EnUs
            )
            .name(),
            basename
        );
    }
}
