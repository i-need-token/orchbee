use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::mailbox::{CommandLease, EventDeadline, UiWaitKey};
use crate::terminal_session::state::workspace_setup::SetupResult;
use orchbee_workbench_protocol::{
    Project, Snapshot, TerminalContext, WorkbenchProjectId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTerminalContextId, WorkbenchWorkspaceId, Workspace,
};
use std::sync::atomic::AtomicU64;
fn project() -> Project {
    Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 270)).unwrap(),
        label: "Project".into(),
        workspaces: Vec::new(),
        directories: Vec::new(),
    }
}
pub(super) fn ready(
    with_project: bool,
) -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock, Snapshot) {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let mut snapshot = super::membership_tests::snapshot(1, 1);
    if with_project {
        snapshot.navigation.projects.push(project());
    }
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                cache:
                    crate::terminal_session::workbench_projection::project_native_standalone_tabs(
                        &snapshot,
                    )
                    .unwrap(),
            },
            EventDeadline::Startup(clock.now() + super::super::state::OPERATION_TOTAL),
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
fn submit(ui: &mut NativeUi, mailbox: &CommandMailbox, existing: bool) -> CommandLease {
    ui.apply_client_action(ClientAction::RequestWorkspaceCreate);
    let mut token = ui.view().setup_draft().unwrap().token();
    if existing {
        ui.edit_setup_draft(token, NativeSetupEdit::SelectPreviousProject);
        token = ui.view().setup_draft().unwrap().token();
    }
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.submit_setup_draft(token);
    token = ui.view().setup_draft().unwrap().token();
    ui.edit_setup_draft(
        token,
        NativeSetupEdit::Append(if existing { "Workspace" } else { "Project" }),
    );
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.submit_setup_draft(token);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("exact setup marker");
    };
    assert_eq!(
        mailbox.mark_setup_in_flight(&marker.0).unwrap(),
        MutationResult::Applied
    );
    lease
}
fn place(
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
fn project_ui_commit_preserves_terminal_and_requires_a_new_complete_draw_for_workspace_submit() {
    let (mut ui, mailbox, clock, mut snapshot) = ready(false);
    let old = ui.state.as_ref().unwrap().desired().cloned();
    let lease = submit(&mut ui, &mailbox, false);
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.projects.push(project());
    let wait = place(
        &mailbox,
        &clock,
        &lease,
        WorkbenchSetupCommit::ProjectCreated {
            revision: snapshot.revision,
            project_id: project().project_id,
        },
        snapshot,
    );
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh result");
    };
    assert_eq!(settlement.decision(), &RefreshDecision::Continue);
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(ui.state.as_ref().unwrap().desired(), old.as_ref());
    let draft = ui.view().setup_draft().unwrap();
    assert_eq!(draft.stage(), NativeSetupStage::WorkspaceLabel);
    assert!(draft.known_project_created());
    let token = draft.token();
    ui.edit_setup_draft(token, NativeSetupEdit::Append("Workspace"));
    ui.submit_setup_draft(token);
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.cancel_setup_draft(token);
    assert_eq!(ui.state.as_ref().unwrap().cache().projects().len(), 1);
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Ready);
}
#[test]
fn workspace_ui_result_does_not_publish_new_location_until_exact_old_viewer_is_detached() {
    let (mut ui, mailbox, clock, mut snapshot) = ready(true);
    let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let lease = submit(&mut ui, &mailbox, true);
    let workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 271)).unwrap();
    let terminal_context_id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 272)).unwrap();
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.projects[0].workspaces.push(Workspace {
        workspace_id: workspace_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        label: "Workspace".into(),
        primary_directory_id: None,
    });
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: terminal_context_id.clone(),
        tabs: Vec::new(),
        groups: Vec::new(),
    });
    let wait = place(
        &mailbox,
        &clock,
        &lease,
        WorkbenchSetupCommit::WorkspaceCreated {
            revision: snapshot.revision,
            workspace_id,
            terminal_context_id,
        },
        snapshot,
    );
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
    assert_eq!(
        settlement.decision(),
        &RefreshDecision::SettledWithoutAttachment
    );
    assert_eq!(ui.view().active_context_position(), Some(2));
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    assert!(ui.view().workspace_directory_unconfigured());
    assert!(ui.view().setup_draft().is_none());
    mailbox.finish_lease(lease).unwrap();
}
