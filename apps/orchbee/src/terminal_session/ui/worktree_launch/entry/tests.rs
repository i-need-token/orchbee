use super::*;
use crate::terminal_session::state::worktree_launch::{Plan, WorktreeMode};
use crate::terminal_session::ui::{membership_tests, worktree_launch_tests};
use orchbee_workbench_protocol::{
    Directory, PanelNode, PresentationBinding, Project, TerminalContext, TerminalId,
    TerminalPresentationId, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchSnapshotOutcome, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, Workspace, WorktreeCheckoutFacts,
};

fn case() -> (NativeUi, Plan, Snapshot) {
    let mut ui = worktree_launch_tests::ready(WorktreeMode::Open);
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let token = ui.worktrees.launcher.token().unwrap();
    ui.worktrees.launcher.draw(Some(token));
    let plan = ui
        .worktrees
        .launcher
        .confirm(
            token,
            WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap(),
            &origin,
        )
        .unwrap();
    let directory = WorkbenchDirectoryId::parse(&format!("{:032x}", 280)).unwrap();
    let workspace = WorkbenchWorkspaceId::parse(&format!("{:032x}", 281)).unwrap();
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 282)).unwrap();
    let path = WorkbenchDirectoryPath::parse("C:/repo").unwrap();
    let mut snapshot = membership_tests::snapshot(1, 2);
    let mut tab = snapshot.terminal_contexts[0].tabs[0].clone();
    tab.tab_id = WorkbenchTabId::parse(&format!("{:032x}", 283)).unwrap();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &mut tab.root
    else {
        panic!("one source panel");
    };
    *panel_id = WorkbenchPanelId::parse(&format!("{:032x}", 284)).unwrap();
    primary_presentation.presentation_id =
        TerminalPresentationId::parse(&format!("{:032x}", 285)).unwrap();
    primary_presentation.binding = Some(PresentationBinding {
        terminal_id: TerminalId::parse(&format!("{:032x}", 286)).unwrap(),
        worker_id: plan.discovery.worker.clone(),
    });
    snapshot.navigation.projects.push(Project {
        project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(&format!("{:032x}", 279))
            .unwrap(),
        label: "Default".into(),
    });
    snapshot.navigation.directories.push(Directory {
        directory_id: directory.clone(),
        path: path.clone(),
        worker_id: plan.discovery.worker.clone(),
    });
    snapshot.navigation.workspaces.push(Workspace {
        project_id: Some(snapshot.navigation.projects[0].project_id.clone()),
        workspace_id: workspace.clone(),
        label: "checkout".into(),
        primary_directory_id: Some(directory),
        terminal_context_id: context.clone(),
    });
    snapshot.navigation.workspace_order.push(workspace.clone());
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: context,
        groups: Vec::new(),
        tabs: vec![tab],
    });
    assert!(ui.worktrees.launcher.start_settled(
        &plan.operation_id,
        Ok(WorkbenchWorktreeOutcome::Available {
            project_id: plan.project.clone(),
            attempt: 1,
            status: Box::new(WorktreeOperationStatus::Opened {
                checkout: WorktreeCheckoutFacts {
                    repository_root: path.clone(),
                    common_directory: WorkbenchDirectoryPath::parse("C:/repo/.git").unwrap(),
                    path,
                    branch: None,
                    dirty: false,
                },
                workspace_id: workspace,
                reused: true,
                terminal_outcome: None,
            }),
        },)
    ));
    (ui, plan, snapshot)
}

#[test]
fn worktree_entry_waits_for_inflight_publication_refresh_then_enters_exact_context() {
    let (mut ui, plan, snapshot) = case();
    assert!(ui.state.as_mut().unwrap().start_pending_refresh(77));
    ui.begin_worktree_entry(Some(snapshot.clone()));
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::EntryPending);
    assert!(!ui.drive_worktree_entry());
    assert!(ui.worktrees.entry.is_some());
    assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());

    let state = ui.state.as_mut().unwrap();
    let prepared = state.prepare_refresh(77, WorkbenchSnapshotOutcome::Available { snapshot });
    state.apply_prepared_refresh(prepared, ui.clock.now());
    assert!(plan.origin.unchanged(&state.worktree_origin().unwrap()));
    assert!(ui.drive_worktree_entry());
    assert!(matches!(
        ui.worktrees.entry.as_ref().unwrap().stage,
        EntryStage::Context(Some(_))
    ));
    assert!(ui.state.as_ref().unwrap().context_selecting());
    assert!(!ui.worktrees.launcher.visible());
}

#[test]
fn worktree_entry_requests_missing_membership_once_and_waits_without_reissuing() {
    let (mut ui, _, snapshot) = case();
    ui.begin_worktree_entry(Some(snapshot));
    assert!(ui.drive_worktree_entry());
    assert!(matches!(
        ui.worktrees.entry.as_ref().unwrap().stage,
        EntryStage::RefreshRequested
    ));
    let lease = ui.mailbox.as_ref().unwrap().try_recv().unwrap().unwrap();
    assert!(matches!(
        lease.record(),
        crate::terminal_session::mailbox::QueuedCommand::Refresh(_)
    ));
    assert!(!ui.drive_worktree_entry());
    assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::EntryPending);
}

#[test]
fn worktree_entry_rechecks_source_after_waiting_and_never_selects_a_changed_origin() {
    let (mut ui, _, snapshot) = case();
    assert!(ui.state.as_mut().unwrap().start_pending_refresh(77));
    ui.begin_worktree_entry(Some(snapshot.clone()));
    ui.observe_geometry(orchbee_client_server_protocol::Geometry::new(100, 24, 8, 16).unwrap());
    let state = ui.state.as_mut().unwrap();
    let prepared = state.prepare_refresh(77, WorkbenchSnapshotOutcome::Available { snapshot });
    state.apply_prepared_refresh(prepared, ui.clock.now());
    assert!(ui.drive_worktree_entry());
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::EntryFailed);
    assert!(ui.worktrees.entry.is_none());
    assert!(!ui.state.as_ref().unwrap().context_selecting());
    assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    assert!(ui.worktrees.launcher.operation().unwrap().outcome.is_some());
}

#[test]
fn worktree_entry_small_draw_during_refresh_cancels_focus_and_keeps_canonical_receipt() {
    let (mut ui, _, snapshot) = case();
    assert!(ui.state.as_mut().unwrap().start_pending_refresh(77));
    ui.begin_worktree_entry(Some(snapshot.clone()));
    ui.after_worktree_draw(Some(
        crate::terminal_session::ui::ModalPresentation::NotPresented,
    ));
    assert!(ui.worktrees.entry.is_none());
    assert!(!ui.worktrees.launcher.operation().unwrap().focus_allowed);
    let state = ui.state.as_mut().unwrap();
    let prepared = state.prepare_refresh(77, WorkbenchSnapshotOutcome::Available { snapshot });
    state.apply_prepared_refresh(prepared, ui.clock.now());
    assert!(!ui.drive_worktree_entry());
    assert!(!ui.state.as_ref().unwrap().context_selecting());
    assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    assert!(ui.worktrees.launcher.operation().unwrap().outcome.is_some());
}

#[test]
fn worktree_entry_matches_server_directory_spelling_without_relaxing_worker_or_path() {
    let (ui, _, mut snapshot) = case();
    snapshot.navigation.directories[0].path = WorkbenchDirectoryPath::parse(r"c:\REPO\").unwrap();
    let op = ui.worktrees.launcher.operation().unwrap();
    assert!(entry(op, &snapshot, ui.clock.now()).is_some());
    snapshot.navigation.directories[0].path = WorkbenchDirectoryPath::parse("C:/foreign").unwrap();
    assert!(entry(op, &snapshot, ui.clock.now()).is_none());
    snapshot.navigation.directories[0].path = WorkbenchDirectoryPath::parse("C:/repo").unwrap();
    snapshot.navigation.directories[0].worker_id =
        orchbee_workbench_protocol::WorkerId::parse(&format!("{:032x}", 9999)).unwrap();
    assert!(entry(op, &snapshot, ui.clock.now()).is_none());
}

fn publish_snapshot(
    ui: &NativeUi,
    lease: &crate::terminal_session::mailbox::CommandLease,
    snapshot: Snapshot,
) -> crate::terminal_session::mailbox::UiWaitKey {
    use crate::terminal_session::mailbox::{MutationResult, QueuedCommand};
    let mailbox = ui.mailbox.as_ref().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("refresh lease");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    mailbox
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available { snapshot },
            marker.deadline(),
            &ui.clock,
        )
        .unwrap()
}

fn source_refresh(ui: &mut NativeUi) -> crate::terminal_session::mailbox::CommandLease {
    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    mailbox
        .try_enqueue_refresh_for(
            ui.state.as_mut().unwrap(),
            ui.clock.now() + Duration::from_secs(2),
            |state, generation| {
                state
                    .start_pending_refresh(generation)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            finish_action,
        )
        .unwrap();
    mailbox.try_recv().unwrap().unwrap()
}

#[test]
fn worktree_entry_real_refresh_reply_admits_context_once_before_source_lease_ack() {
    use crate::terminal_session::{mailbox::UiReply, state::RefreshDecision};
    let (mut ui, _, snapshot) = case();
    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    let source = source_refresh(&mut ui);
    ui.begin_worktree_entry(Some(snapshot.clone()));
    let target = ui.worktrees.entry.as_ref().unwrap().member.clone();
    let wait = publish_snapshot(&ui, &source, snapshot.clone());
    assert!(ui.tick());
    assert!(ui.state.as_ref().unwrap().context_selecting());
    assert!(matches!(
        ui.worktrees.entry.as_ref().unwrap().stage,
        EntryStage::Context(Some(_))
    ));
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Refresh(reply)) if *reply.decision() == RefreshDecision::Continue
    ));
    assert!(mailbox.try_recv().unwrap().is_none(), "one active lease");
    mailbox.finish_lease(source).unwrap();
    let context = mailbox.try_recv().unwrap().unwrap();
    let wait = publish_snapshot(&ui, &context, snapshot);
    assert!(ui.tick());
    let Some(UiReply::Refresh(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("context reply");
    };
    let RefreshDecision::Cleanup(cleanup) = reply.decision() else {
        panic!("existing viewer cleanup remains owned by membership");
    };
    assert_eq!(
        cleanup.old(),
        ui.state.as_ref().unwrap().active().unwrap().key().anchor()
    );
    assert_eq!(ui.worktrees.entry.as_ref().unwrap().member, target);
    assert!(mailbox.try_recv().unwrap().is_none());
    mailbox.finish_lease(context).unwrap();
}

fn replace_target(snapshot: &mut Snapshot, change: usize) {
    snapshot.revision = orchbee_workbench_protocol::WorkbenchRevision::new(3);
    match change {
        0 => snapshot.terminal_contexts[1].tabs.clear(),
        1 => {
            let PanelNode::Panel {
                primary_presentation,
                ..
            } = &mut snapshot.terminal_contexts[1].tabs[0].root
            else {
                panic!("target panel");
            };
            primary_presentation.binding.as_mut().unwrap().terminal_id =
                TerminalId::parse(&format!("{:032x}", 9999)).unwrap();
        }
        2 => {
            let replacement = WorkbenchDirectoryId::parse(&format!("{:032x}", 9999)).unwrap();
            snapshot.navigation.directories[0].directory_id = replacement.clone();
            snapshot.navigation.workspaces[0].primary_directory_id = Some(replacement);
        }
        _ => panic!("bounded target replacements"),
    }
}

#[test]
fn worktree_entry_rejects_replaced_target_in_accepted_source_refresh_without_focus_change() {
    for change in 0..3 {
        let (mut ui, _, mut snapshot) = case();
        let mailbox = ui.mailbox.as_ref().unwrap().clone();
        let location = ui.state.as_ref().unwrap().cache().location().clone();
        let desired = ui.state.as_ref().unwrap().desired().cloned();
        let source = source_refresh(&mut ui);
        ui.begin_worktree_entry(Some(snapshot.clone()));
        replace_target(&mut snapshot, change);
        let wait = publish_snapshot(&ui, &source, snapshot);
        assert!(ui.tick());
        mailbox.take_deposited_reply(&wait).unwrap().unwrap();
        mailbox.finish_lease(source).unwrap();
        assert!(ui.worktrees.entry.is_none());
        assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::EntryFailed);
        assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
        assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(ui.worktrees.launcher.operation().unwrap().outcome.is_some());
        assert!(!ui.retirement_requested());
    }
}

#[test]
fn worktree_entry_fences_target_replacement_in_context_snapshot_before_cleanup() {
    use crate::terminal_session::{mailbox::UiReply, state::RefreshDecision};
    for change in 0..3 {
        let (mut ui, _, snapshot) = case();
        let mailbox = ui.mailbox.as_ref().unwrap().clone();
        let location = ui.state.as_ref().unwrap().cache().location().clone();
        let desired = ui.state.as_ref().unwrap().desired().cloned();
        let source = source_refresh(&mut ui);
        ui.begin_worktree_entry(Some(snapshot.clone()));
        let wait = publish_snapshot(&ui, &source, snapshot.clone());
        assert!(ui.tick());
        mailbox.take_deposited_reply(&wait).unwrap().unwrap();
        mailbox.finish_lease(source).unwrap();
        let context = mailbox.try_recv().unwrap().unwrap();
        let mut changed = snapshot;
        replace_target(&mut changed, change);
        let wait = publish_snapshot(&ui, &context, changed);
        assert!(ui.tick());
        assert!(matches!(
            mailbox.take_deposited_reply(&wait).unwrap(),
            Some(UiReply::Refresh(reply)) if *reply.decision() == RefreshDecision::Continue
        ));
        mailbox.finish_lease(context).unwrap();
        assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
        assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
        assert!(!ui.state.as_ref().unwrap().context_selecting());
        assert!(ui.worktrees.entry.is_none());
        assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::EntryFailed);
        assert!(ui.worktrees.launcher.operation().unwrap().outcome.is_some());
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(!ui.retirement_requested());
    }
}

fn insert_new_first_tab(snapshot: &mut Snapshot) {
    let mut tab = snapshot.terminal_contexts[1].tabs[0].clone();
    tab.tab_id = WorkbenchTabId::parse(&format!("{:032x}", 9100)).unwrap();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &mut tab.root
    else {
        panic!("target panel");
    };
    *panel_id = WorkbenchPanelId::parse(&format!("{:032x}", 9101)).unwrap();
    primary_presentation.presentation_id =
        TerminalPresentationId::parse(&format!("{:032x}", 9102)).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id =
        TerminalId::parse(&format!("{:032x}", 9104)).unwrap();
    snapshot.terminal_contexts[1].tabs.insert(0, tab);
    snapshot.revision = orchbee_workbench_protocol::WorkbenchRevision::new(3);
}

#[test]
fn worktree_entry_reordered_target_selects_frozen_member_through_actual_cleanup_install() {
    use crate::terminal_session::{
        mailbox::{EventDeadline, UiReply, WorkerEvent},
        state::RefreshDecision,
    };
    let (mut ui, _, mut snapshot) = case();
    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    let source = source_refresh(&mut ui);
    ui.begin_worktree_entry(Some(snapshot.clone()));
    let target = ui.worktrees.entry.as_ref().unwrap().member.clone();
    let binding = ui.worktrees.entry.as_ref().unwrap().binding.clone();
    insert_new_first_tab(&mut snapshot);
    let wait = publish_snapshot(&ui, &source, snapshot.clone());
    assert!(ui.tick());
    assert!(ui.state.as_ref().unwrap().context_selecting());
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(source).unwrap();
    let context = mailbox.try_recv().unwrap().unwrap();
    let wait = publish_snapshot(&ui, &context, snapshot);
    assert!(ui.tick());
    let Some(UiReply::Refresh(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("context reply");
    };
    let RefreshDecision::Cleanup(cleanup) = reply.decision() else {
        panic!("old viewer cleanup");
    };
    let cleanup = cleanup.clone();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                detached: cleanup.old().clone(),
                cleanup: cleanup.clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &ui.clock,
        )
        .unwrap();
    assert!(ui.tick());
    let Some(UiReply::Membership(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("membership install");
    };
    let RefreshDecision::AttachRecovered(attach) = reply.decision() else {
        panic!("exact target attach");
    };
    assert_eq!(attach.selected().key().anchor().tab_id(), &target.tab);
    assert_eq!(attach.selected().key().panel(), &target.panel);
    assert_eq!(attach.binding(), Some(&binding));
    assert_eq!(ui.view().focused_member().as_ref(), Some(&target));
    assert_eq!(
        ui.state.as_ref().unwrap().cache().tab_index(&target.tab),
        Some(1)
    );
    mailbox.finish_lease(context).unwrap();
    assert!(!ui.retirement_requested());
}

#[test]
fn worktree_entry_fence_leaves_a_foreign_context_generation_untouched() {
    use crate::terminal_session::{mailbox::UiReply, state::RefreshDecision};
    let (mut ui, _, snapshot) = case();
    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    let source = source_refresh(&mut ui);
    ui.begin_worktree_entry(Some(snapshot.clone()));
    let wait = publish_snapshot(&ui, &source, snapshot.clone());
    assert!(ui.tick());
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(source).unwrap();
    let context = mailbox.try_recv().unwrap().unwrap();
    ui.worktrees.entry.as_mut().unwrap().stage = EntryStage::Context(Some(9999));
    let mut changed = snapshot;
    replace_target(&mut changed, 1);
    let wait = publish_snapshot(&ui, &context, changed);
    assert!(ui.tick());
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Refresh(reply)) if matches!(reply.decision(), RefreshDecision::Cleanup(_))
    ));
    mailbox.finish_lease(context).unwrap();
    assert!(!ui.retirement_requested());
}

#[test]
fn worktree_entry_failed_context_admission_keeps_result_visible_and_does_not_own_navigation() {
    use crate::terminal_session::mailbox::{AdmissionError, QueuedCommand, Tagged};
    let (mut ui, _, snapshot) = case();
    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    let location = ui.state.as_ref().unwrap().cache().location().clone();
    let source = source_refresh(&mut ui);
    ui.begin_worktree_entry(Some(snapshot.clone()));
    let wait = publish_snapshot(&ui, &source, snapshot);
    let event = mailbox.take_event_before(&ui.clock).unwrap().unwrap();
    assert!(ui.apply_event(&mailbox, event));
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(source).unwrap();
    let state = ui.state.as_ref().unwrap();
    let key = state.pane_ref_for(state.desired().unwrap()).unwrap();
    let geometry = mailbox.latest_geometry().unwrap();
    let mut queued = 0;
    loop {
        match mailbox.try_enqueue(QueuedCommand::Resize(Tagged::new(key.clone(), geometry))) {
            Ok(_) => queued += 1,
            Err(AdmissionError::Full) => break,
            result => panic!("expected bounded resize queue: {result:?}"),
        }
    }
    assert!(queued > 0);
    assert!(ui.drive_worktree_entry());
    assert!(ui.worktrees.entry.is_none());
    assert!(ui.worktrees.launcher.visible());
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::EntryFailed);
    assert!(!ui.state.as_ref().unwrap().context_selecting());
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
    for _ in 0..queued {
        let lease = mailbox.try_recv().unwrap().unwrap();
        assert!(matches!(lease.record(), QueuedCommand::Resize(_)));
        mailbox.finish_lease(lease).unwrap();
    }
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(!ui.retirement_requested());
}

#[test]
fn worktree_entry_current_context_child_uses_existing_primary_presentation_switch() {
    use crate::terminal_session::{
        mailbox::{CommandMailbox, EventDeadline, QueuedCommand, WorkerEvent},
        state::{ControllerMode, FrameIdentity},
        workbench_projection::project_native_context_tabs,
    };
    use orchbee_workbench_protocol::Presentation;
    use std::sync::Arc;
    let (original, _, mut snapshot) = case();
    let op = original.worktrees.launcher.operation().unwrap();
    let mut entry = entry(op, &snapshot, original.clock.now()).unwrap();
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut snapshot.terminal_contexts[1].tabs[0].root
    else {
        panic!("target panel");
    };
    let child = TerminalPresentationId::parse(&format!("{:032x}", 9900)).unwrap();
    child_presentations.push(Presentation {
        presentation_id: child.clone(),
        label: "child".into(),
        custom_label: None,
        binding: Some(PresentationBinding {
            terminal_id: TerminalId::parse(&format!("{:032x}", 9901)).unwrap(),
            worker_id: op.plan.discovery.worker.clone(),
        }),
    });
    let cache = project_native_context_tabs(&snapshot, &entry.location).unwrap();
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        original
            .state
            .as_ref()
            .unwrap()
            .worktree_origin()
            .unwrap()
            .geometry,
    ));
    let mut ui = NativeUi::new(mailbox.clone(), original.clock.clone());
    let mut selected = entry.member.clone();
    selected.presentation = child;
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: Some(selected),
                cache,
            },
            EventDeadline::Startup(entry.deadline),
            &ui.clock,
        )
        .unwrap();
    assert!(ui.tick());
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let pane = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&pane));
    assert!(state.record_setup_configuration_t(&pane, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&pane, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&pane, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&pane, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    assert_ne!(ui.view().focused_member().as_ref(), Some(&entry.member));
    ui.worktrees = original.worktrees;
    ui.worktrees.launcher.entry_started();
    entry.stage = EntryStage::Panel;
    let target = entry.member.clone();
    let binding = entry.binding.clone();
    ui.worktrees.entry = Some(entry);
    assert!(ui.drive_worktree_entry());
    assert_eq!(
        ui.state.as_ref().unwrap().phase_kind(),
        SessionPhaseKind::Switching
    );
    assert_eq!(ui.view().focused_member().as_ref(), Some(&target));
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("existing exact presentation switch");
    };
    assert_eq!(marker.target().binding(), Some(&binding));
    mailbox.finish_lease(lease).unwrap();
    assert!(!ui.retirement_requested());
}
