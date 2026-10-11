use super::super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::mailbox::{CommandLease, EventDeadline, UiWaitKey};
use crate::terminal_session::state::workspace_setup::SetupResult;
use orchbee_workbench_protocol::{
    Project, Snapshot, TerminalContext, WorkbenchDirectoryPath, WorkbenchProjectId,
    WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
    Workspace,
};
use std::sync::atomic::AtomicU64;
fn project() -> Project {
    Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 270)).unwrap(),
        label: "Project".into(),
    }
}
pub(in crate::terminal_session::ui) fn ready(
    with_project: bool,
) -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock, Snapshot) {
    let mut snapshot = super::super::tests::membership::snapshot(1, 1);
    if with_project {
        snapshot.navigation.projects.push(project());
    }
    ready_snapshot(snapshot)
}
pub(in crate::terminal_session::ui) fn ready_snapshot(
    snapshot: Snapshot,
) -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock, Snapshot) {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache:
                    crate::terminal_session::workbench_projection::project_native_standalone_tabs(
                        &snapshot,
                    )
                    .unwrap(),
            },
            EventDeadline::Startup(clock.now() + super::super::super::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    state.install_projection_t(&key);
    state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None);
    state.stage_drawn_frame_t(&key, FrameIdentity::new(1));
    state.mark_feedback_accepted_t(&key, FrameIdentity::new(1));
    state.publish_ready_t(&key, FrameIdentity::new(1), ControllerMode::ReadOnly, None);
    (ui, mailbox, clock, snapshot)
}
pub(in crate::terminal_session::ui) fn finish_preparation(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &Snapshot,
    cancelled: bool,
) {
    finish_preparation_with_directory(
        ui,
        mailbox,
        clock,
        snapshot,
        cancelled,
        Some(
            WorkbenchDirectoryPath::parse(if cfg!(windows) {
                "C:/项目 space"
            } else {
                "/tmp/项目 space"
            })
            .unwrap(),
        ),
    );
}

pub(in crate::terminal_session::ui) fn finish_preparation_with_directory(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &Snapshot,
    cancelled: bool,
    cwd: Option<WorkbenchDirectoryPath>,
) {
    let token = ui.view().setup_draft().unwrap().token();
    assert_eq!(
        ui.view().setup_draft().unwrap().stage(),
        NativeSetupStage::PreparingWorkspace
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("read-only preparation");
    };
    let preparation = marker.quick_preparation().unwrap().clone();
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(preparation.generation)
            .unwrap(),
        MutationResult::Applied
    );
    if cancelled {
        ui.cancel_setup_draft(token);
    }
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::QuickWorkspacePrepared {
                preparation: Box::new(preparation.clone()),
                cwd,
                outcome: Box::new(WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                }),
            },
            EventDeadline::PlacementOnly(preparation.deadlines.forward_cutoff()),
            clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Refresh(_))
    ));
    mailbox.finish_lease(lease).unwrap();
    assert!(ui.state.as_ref().unwrap().invariant_holds());
}

fn submit_launch(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &Snapshot,
) -> CommandLease {
    ui.apply_client_action(ClientAction::RequestWorkspaceCreate);
    finish_preparation(ui, mailbox, clock, snapshot, false);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("exact launch")
    };
    assert_eq!(
        mailbox.mark_setup_in_flight(&marker.0).unwrap(),
        MutationResult::Applied
    );
    lease
}

fn launched(snapshot: &Snapshot, lease: &CommandLease) -> (Snapshot, WorkbenchSetupCommit) {
    use orchbee_workbench_protocol::{
        Directory, PanelNode, WorkbenchDirectoryId, WorkbenchTerminalOutcome,
    };
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("launch")
    };
    let WorkbenchAction::WorkspaceLaunch {
        label,
        options,
        geometry,
        placement,
        ..
    } = &marker.0.command.action
    else {
        panic!("launch")
    };
    let project_id = match placement {
        orchbee_workbench_protocol::WorkspacePlacement::Ungrouped {} => None,
        orchbee_workbench_protocol::WorkspacePlacement::Project { project_id } => {
            Some(project_id.clone())
        }
        _ => panic!("fixture supports default or targeted launch"),
    };
    let mut snapshot = snapshot.clone();
    snapshot.revision = WorkbenchRevision::new(2);
    let mut fresh = super::super::tests::membership::snapshot(2, 1);
    let mut tab = fresh.terminal_contexts[0].tabs.pop().unwrap();
    tab.label = label.as_str().to_owned();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &tab.root
    else {
        panic!("first shell")
    };
    let commit = crate::local_client::workbench::workspace_launch::WorkspaceLaunchCommit {
        revision: snapshot.revision,
        project_id,
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 271)).unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 272)).unwrap(),
        directory_id: WorkbenchDirectoryId::parse(&format!("{:032x}", 273)).unwrap(),
        tab_id: tab.tab_id.clone(),
        panel_id: panel_id.clone(),
        presentation_id: primary_presentation.presentation_id.clone(),
        terminal_outcome: Some(WorkbenchTerminalOutcome::Started {
            geometry: *geometry,
        }),
    };
    snapshot.navigation.directories.push(Directory {
        directory_id: commit.directory_id.clone(),
        path: options.cwd.clone().unwrap(),
        worker_id: primary_presentation
            .binding
            .as_ref()
            .unwrap()
            .worker_id
            .clone(),
    });
    snapshot.navigation.workspaces.push(Workspace {
        workspace_id: commit.workspace_id.clone(),
        project_id: commit.project_id.clone(),
        terminal_context_id: commit.terminal_context_id.clone(),
        label: label.as_str().to_owned(),
        primary_directory_id: Some(commit.directory_id.clone()),
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
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: commit.terminal_context_id.clone(),
        tabs: vec![tab],
        groups: vec![],
    });
    (
        snapshot,
        WorkbenchSetupCommit::WorkspaceLaunched(Box::new(commit)),
    )
}
pub(in crate::terminal_session::ui) fn place(
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    lease: &CommandLease,
    commit: WorkbenchSetupCommit,
    snapshot: Snapshot,
) -> UiWaitKey {
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("setup");
    };
    let known = marker.0.known_commit(commit).unwrap();
    assert!(mailbox.retain_setup_known(&known));
    mailbox
        .publish_setup_result_before(
            marker.0.clone(),
            SetupResult::Committed {
                known: Box::new(known),
                outcome: Box::new(WorkbenchSnapshotOutcome::Available { snapshot }),
            },
            clock,
        )
        .unwrap()
}
#[test]
fn workspace_ui_result_does_not_publish_new_location_until_exact_old_viewer_is_detached() {
    let (mut ui, mailbox, clock, snapshot) = ready(true);
    let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let lease = submit_launch(&mut ui, &mailbox, &clock, &snapshot);
    let (committed, receipt) = launched(&snapshot, &lease);
    let wait = place(&mailbox, &clock, &lease, receipt, committed);
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("cleanup reply");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("must detach");
    };
    assert_eq!(cleanup.old(), &old);
    assert_eq!(ui.view().active_context_position(), Some(1));
    assert_eq!(
        ui.view().setup_status(),
        Some(NativeSetupStatus::CommittedNeedRefresh)
    );
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: old.anchor().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Membership(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("atomic membership install");
    };
    assert!(matches!(
        settlement.decision(),
        RefreshDecision::AttachRecovered(_)
    ));
    assert_eq!(ui.view().active_context_position(), Some(2));
    assert_ne!(ui.view().phase(), NativeDisplayPhase::Ready);
    assert!(!ui.view().workspace_directory_unconfigured());
    assert!(ui.view().setup_draft().is_none());
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn newer_refused_focus_intent_keeps_current_context_when_a_launch_result_arrives() {
    let (mut ui, mailbox, clock, snapshot) = ready(false);
    let source = ui.state.as_ref().unwrap().cache().location().clone();
    let desired = ui.state.as_ref().unwrap().desired().cloned();
    let focus = ui.state.as_ref().unwrap().focus_pane_ref();
    let lease = submit_launch(&mut ui, &mailbox, &clock, &snapshot);
    let (committed, receipt) = launched(&snapshot, &lease);
    ui.apply_client_action(ClientAction::SelectNextContext);
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &source);
    let wait = place(&mailbox, &clock, &lease, receipt, committed.clone());
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("launch refresh")
    };
    assert_eq!(settlement.decision(), &RefreshDecision::Continue);
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &committed
    );
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &source);
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), focus);
    assert_eq!(
        ui.view().setup_status(),
        Some(NativeSetupStatus::WorkspaceCreated)
    );
    assert!(ui.view().setup_draft().is_none());
    #[cfg(windows)]
    assert!(!ui.take_project_quick_terminal_entry());
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[cfg(windows)]
#[test]
fn shell_control_tick_lands_all_launch_entries_unless_newer_focus_revokes_them() {
    use crate::client_view::ClientView;
    use crate::localization::Locale;
    use crate::terminal_session::{ProjectQuickWorkspaceRequest, SessionBackend, TerminalSession};
    use ratatui::layout::Rect;

    for width in [80, 160] {
        for entry in ["create", "quick", "project"] {
            for newer_focus in [false, true] {
                let (ui, mailbox, clock, snapshot) = ready(true);
                let source = ui.state.as_ref().unwrap().cache().location().clone();
                let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
                let (shutdown, _peer) = orchbee_local_ipc::LocalStream::pair().unwrap();
                let mut session = TerminalSession {
                    position_writer: None,
                    position_startup: None,
                    configuration: None,
                    notifications: None,
                    window_title: None,
                    client_control: None,
                    backend: SessionBackend::Native(Box::new(ui)),
                    task: None,
                    shutdown: crate::local_client::test_server::shutdown_handle(shutdown),
                    retirement_redraw_pending: false,
                    origin: None,
                    recovery_shutdown: None,
                };
                let mut view = ClientView::new(width);
                let action = match entry {
                    "create" => ClientAction::RequestWorkspaceCreate,
                    "quick" => ClientAction::RequestQuickWorkspace,
                    _ => ClientAction::RequestProjectQuickWorkspace(Box::new(
                        ProjectQuickWorkspaceRequest::capture(
                            session
                                .backend
                                .native_mut()
                                .unwrap()
                                .state
                                .as_ref()
                                .unwrap(),
                            &snapshot.navigation.projects[0],
                            clock.now() + super::super::super::state::OPERATION_TOTAL,
                        )
                        .unwrap(),
                    )),
                };
                let transition = view.decide_action(action.clone());
                let disposition = session
                    .backend
                    .native_mut()
                    .unwrap()
                    .apply_client_action(action);
                view.commit(transition, disposition);
                finish_preparation(
                    session.backend.native_mut().unwrap(),
                    &mailbox,
                    &clock,
                    &snapshot,
                    false,
                );
                let lease = mailbox.try_recv().unwrap().unwrap();
                let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                    panic!("one launch")
                };
                assert_eq!(
                    mailbox.mark_setup_in_flight(&marker.0).unwrap(),
                    MutationResult::Applied
                );
                let (committed, receipt) = launched(&snapshot, &lease);
                let WorkbenchSetupCommit::WorkspaceLaunched(expected) = &receipt else {
                    unreachable!()
                };
                let expected = expected.clone();
                if newer_focus {
                    let transition = view.decide_action(ClientAction::SelectNextContext);
                    let disposition = session
                        .backend
                        .native_mut()
                        .unwrap()
                        .apply_client_action(ClientAction::SelectNextContext);
                    view.commit(transition, disposition);
                }
                // The real shell tick used to clear default launches because only
                // targeted Project actions armed the ClientView intent marker.
                session.client_control_tick(&mut view, Rect::new(0, 0, width, 24), Locale::EnUs);
                assert_eq!(view.project_quick_entry_pending(), !newer_focus, "{entry}");
                let wait = place(&mailbox, &clock, &lease, receipt, committed.clone());
                session.backend.native_mut().unwrap().tick();
                let Some(UiReply::Refresh(settlement)) =
                    mailbox.take_deposited_reply(&wait).unwrap()
                else {
                    panic!("launch refresh")
                };
                if newer_focus {
                    assert_eq!(settlement.decision(), &RefreshDecision::Continue);
                    let state = session
                        .backend
                        .native_mut()
                        .unwrap()
                        .state
                        .as_ref()
                        .unwrap();
                    assert_eq!(state.cache().location(), &source);
                    assert_eq!(state.desired(), Some(&old));
                    assert_eq!(state.cache().canonical_snapshot(), &committed);
                    assert_eq!(
                        state.setup_status(),
                        Some(NativeSetupStatus::WorkspaceCreated)
                    );
                } else {
                    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
                        panic!("{entry} must enter its receipt")
                    };
                    assert_eq!(cleanup.old(), &old);
                    let wait = mailbox
                        .publish_event_before(
                            WorkerEvent::MembershipDetached {
                                cleanup: cleanup.clone(),
                                detached: old.anchor().clone(),
                            },
                            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                            &clock,
                        )
                        .unwrap();
                    session.backend.native_mut().unwrap().tick();
                    let Some(UiReply::Membership(settlement)) =
                        mailbox.take_deposited_reply(&wait).unwrap()
                    else {
                        panic!("membership")
                    };
                    assert!(matches!(
                        settlement.decision(),
                        RefreshDecision::AttachRecovered(_)
                    ));
                    let state = session
                        .backend
                        .native_mut()
                        .unwrap()
                        .state
                        .as_ref()
                        .unwrap();
                    assert!(
                        matches!(state.cache().location(), crate::terminal_session::NativeContextLocation::Workspace { workspace_id, terminal_context_id }
                        if workspace_id == &expected.workspace_id && terminal_context_id == &expected.terminal_context_id)
                    );
                    assert_eq!(state.desired().unwrap().anchor().tab_id(), &expected.tab_id);
                    assert_eq!(state.desired().unwrap().panel(), &expected.panel_id);
                }
                mailbox.finish_lease(lease).unwrap();
            }
        }
    }
}
