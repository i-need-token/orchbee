use super::*;
use crate::workbench_client::state::worktree_launch::{Plan, WorktreeMode, WorktreeStage};
use crate::workbench_client::ui::tests::{
    context_selection, membership, viewport, worktree_launch,
};
use orchbee_workbench_protocol::{
    Directory, PanelNode, PresentationBinding, Project, TerminalContext, TerminalId,
    TerminalPresentationId, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchSnapshotOutcome, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, Workspace, WorktreeCheckoutFacts,
};

fn case() -> (NativeUi, Plan, Snapshot) {
    case_with_mode(WorktreeMode::Open)
}

fn case_with_mode(mode: WorktreeMode) -> (NativeUi, Plan, Snapshot) {
    let mut ui = tests::worktree_launch::ready(mode);
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let token = ui.worktrees.launcher.token().unwrap();
    ui.worktrees.launcher.draw(Some(token));
    let operation = WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap();
    let plan = match mode {
        WorktreeMode::Create => ui.worktrees.launcher.create_with_defaults(
            operation,
            &origin,
            crate::localization::Locale::EnUs,
        ),
        WorktreeMode::Open => ui.worktrees.launcher.confirm(token, operation, &origin),
        WorktreeMode::Remove => unreachable!("entry is only for Create/Open"),
    }
    .unwrap();
    let directory = WorkbenchDirectoryId::parse(&format!("{:032x}", 280)).unwrap();
    let workspace = WorkbenchWorkspaceId::parse(&format!("{:032x}", 281)).unwrap();
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 282)).unwrap();
    let path = WorkbenchDirectoryPath::parse("C:/repo").unwrap();
    let mut snapshot = tests::membership::snapshot(1, 2);
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
fn completed_create_entry_accepts_one_fresh_noncurrent_project_checkout_after_dismissal() {
    use crate::workbench_client::{
        mailbox::{EventDeadline, UiReply, WorkerEvent},
        state::{RefreshDecision, worktree_launch::addressed},
    };
    use orchbee_workbench_protocol::{WorkbenchProjectId, WorktreeIntent, WorktreeSelector};
    let (mut ui, created, mut snapshot) = case_with_mode(WorktreeMode::Create);
    let old_token = ui.worktrees.launcher.token().unwrap();
    let old_outcome = ui.worktrees.launcher.operation().unwrap().outcome.clone();
    let other = WorkbenchProjectId::parse(&format!("{:032x}", 290)).unwrap();
    snapshot.navigation.projects.push(Project {
        project_id: other.clone(),
        label: "noncurrent source".into(),
    });
    let mut source = snapshot.navigation.workspaces[1].clone();
    source.workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 291)).unwrap();
    source.terminal_context_id =
        WorkbenchTerminalContextId::parse(&format!("{:032x}", 292)).unwrap();
    source.project_id = Some(other.clone());
    let mut context = snapshot.terminal_contexts[1].clone();
    context.terminal_context_id = source.terminal_context_id.clone();
    context.tabs[0].tab_id = WorkbenchTabId::parse(&format!("{:032x}", 293)).unwrap();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &mut context.tabs[0].root
    else {
        panic!("one source panel");
    };
    *panel_id = WorkbenchPanelId::parse(&format!("{:032x}", 294)).unwrap();
    primary_presentation.presentation_id =
        TerminalPresentationId::parse(&format!("{:032x}", 295)).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id =
        TerminalId::parse(&format!("{:032x}", 296)).unwrap();
    snapshot
        .navigation
        .workspace_order
        .push(source.workspace_id.clone());
    snapshot.navigation.workspaces.push(source);
    snapshot.terminal_contexts.push(context);

    // Consume the real refresh, old Viewer cleanup and exact membership install.
    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    let source_lease = source_refresh(&mut ui);
    ui.begin_worktree_entry(Some(snapshot.clone()));
    let target = ui.worktrees.entry.as_ref().unwrap().member.clone();
    let wait = publish_snapshot(&ui, &source_lease, snapshot.clone());
    assert!(ui.tick());
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(source_lease).unwrap();
    let context_lease = mailbox.try_recv().unwrap().unwrap();
    let wait = publish_snapshot(&ui, &context_lease, snapshot.clone());
    assert!(ui.tick());
    let Some(UiReply::Refresh(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("accepted context snapshot");
    };
    let RefreshDecision::Cleanup(cleanup) = reply.decision() else {
        panic!("exact old Viewer cleanup");
    };
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
        panic!("exact membership install");
    };
    assert!(matches!(
        reply.decision(),
        RefreshDecision::AttachRecovered(_)
    ));
    mailbox.finish_lease(context_lease).unwrap();
    context_selection::make_desired_ready(&mut ui, 2);
    // The attach/physical-frame boundary is controlled here; entry still consumes
    // its production Ready + exact-member + FrameAvailable admission.
    viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    for _ in 0..6 {
        ui.drive_worktree_entry();
        if ui.worktrees.entry.is_none() {
            break;
        }
    }
    assert!(
        ui.worktrees.entry.is_none(),
        "entry completed, not just queued"
    );
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::Opened);
    assert_eq!(ui.view().focused_member().as_ref(), Some(&target));
    assert_eq!(
        ui.view().pane_projection_status(&target.panel),
        NativeProjectionStatus::FrameAvailable
    );
    ui.worktrees.dismiss_focus();
    assert!(!ui.worktrees.launcher.visible());
    let retained = ui.worktrees.launcher.operation().unwrap();
    assert_eq!(retained.plan.operation_id, created.operation_id);
    assert_eq!(retained.outcome, old_outcome);
    assert!(retained.focus_allowed);

    let addressed = addressed::AddressedWorktreeSource::capture(
        &snapshot,
        &addressed::SidebarWorktreeTarget::Project(other.clone()),
    )
    .unwrap();
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    assert_ne!(addressed.cache.location(), origin.cache.location());
    let sidebar = addressed::SidebarDiscovery {
        addressed,
        sample: crate::workbench_client::primary_directory::RootDirectory::Available(Some(
            WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        )),
        source: created.source.clone(),
        discovery: created.discovery.clone(),
    };
    let checkout = WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap();
    assert!(sidebar.valid(origin.cache.canonical_snapshot()));
    assert!(
        ui.worktrees.launcher.open_sidebar(
            origin.clone(),
            sidebar.clone(),
            WorktreeMode::Open,
            Some(&checkout),
        ),
        "a completed hidden Create must allow a fresh addressed Open"
    );
    let next = ui.worktrees.launcher.token().unwrap();
    assert_ne!(next, old_token);
    assert_eq!(
        ui.worktrees.launcher.view().unwrap().stage(),
        WorktreeStage::Checkout
    );
    assert!(!ui.worktrees.launcher.sidebar_open_ready());
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "no Start before draw"
    );
    let operation = WorkbenchRequestId::parse(&format!("{:032x}", 91)).unwrap();
    ui.worktrees.launcher.draw(Some(old_token));
    assert!(
        ui.worktrees
            .launcher
            .confirm(next, operation.clone(), &origin)
            .is_none()
    );
    ui.worktrees.launcher.draw(Some(next));
    assert!(ui.worktrees.launcher.sidebar_open_ready());
    let opened = ui
        .worktrees
        .launcher
        .confirm(next, operation.clone(), &origin)
        .unwrap();
    assert_eq!(opened.operation_id, operation);
    assert_ne!(opened.operation_id, created.operation_id);
    assert_eq!(opened.project.as_ref(), Some(&other));
    assert_eq!(
        opened.sidebar.as_ref().unwrap().addressed,
        sidebar.addressed
    );
    assert_eq!(opened.discovery.worker, sidebar.discovery.worker);
    assert!(matches!(opened.intent, WorktreeIntent::Open {
        source: ref selected_source,
        selector: WorktreeSelector::Path { path: ref selected }, ..
    } if selected_source == &sidebar.source && selected == &checkout));
    assert!(
        ui.worktrees
            .launcher
            .confirm(next, operation, &origin)
            .is_none()
    );
    assert!(
        !ui.worktrees
            .launcher
            .start_settled(&created.operation_id, Ok(old_outcome.unwrap()))
    );
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::Pending);
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
fn a_retained_entry_owner_prevents_success_replacement_from_both_routes() {
    use crate::workbench_client::state::worktree_launch::addressed;
    let (mut ui, created, snapshot) = case_with_mode(WorktreeMode::Create);
    ui.begin_worktree_entry(Some(snapshot.clone()));
    assert!(ui.worktrees.entry.is_some());
    let state = ui.state.as_mut().unwrap();
    assert!(state.start_pending_refresh(77));
    let refresh = state.prepare_refresh(
        77,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot.clone(),
        },
    );
    state.apply_prepared_refresh(refresh, ui.clock.now());
    let origin = state.worktree_origin().unwrap();
    let addressed = addressed::AddressedWorktreeSource::capture(
        &snapshot,
        &addressed::SidebarWorktreeTarget::Project(
            snapshot.navigation.projects[0].project_id.clone(),
        ),
    )
    .unwrap();
    let sidebar = addressed::SidebarDiscovery {
        addressed,
        sample: crate::workbench_client::primary_directory::RootDirectory::Available(Some(
            WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        )),
        source: created.source.clone(),
        discovery: created.discovery.clone(),
    };
    assert!(sidebar.valid(origin.cache.canonical_snapshot()));
    // Keep the coordinator's entry owner independently of the launcher's known
    // success projection; the coordinator must not infer entryNone from notice.
    ui.worktrees.launcher.entry_started();
    ui.worktrees.launcher.entry_notice(WorktreeNotice::Opened);
    assert!(ui.worktrees.launcher.hidden_opened_result());
    assert!(
        !ui.worktrees
            .open_sidebar(origin.clone(), sidebar.clone(), None)
    );
    assert_eq!(
        ui.worktrees.launcher.operation().unwrap().plan.operation_id,
        created.operation_id
    );
    assert_eq!(
        ui.request_worktree(WorktreeMode::Create),
        crate::workbench_client::ClientActionDisposition::DismissNavigator
    );
    assert_eq!(
        ui.worktrees.launcher.operation().unwrap().plan.operation_id,
        created.operation_id
    );
    assert!(ui.worktrees.launcher.preparation().is_none());
    assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    ui.worktrees.launcher.dismiss();
    ui.worktrees.entry = None;
    assert!(ui.worktrees.open_sidebar(origin, sidebar, None));
    assert!(ui.worktrees.launcher.operation().is_none());
    assert!(!ui.worktrees.launcher.default_create_ready());
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
        crate::workbench_client::mailbox::QueuedCommand::Refresh(_)
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
        crate::workbench_client::ui::ModalPresentation::NotPresented,
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
    lease: &crate::workbench_client::mailbox::CommandLease,
    snapshot: Snapshot,
) -> crate::workbench_client::mailbox::UiWaitKey {
    use crate::workbench_client::mailbox::{MutationResult, QueuedCommand};
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

fn source_refresh(ui: &mut NativeUi) -> crate::workbench_client::mailbox::CommandLease {
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
    use crate::workbench_client::{mailbox::UiReply, state::RefreshDecision};
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
            snapshot.navigation.workspaces[1].primary_directory_id = Some(replacement);
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
    use crate::workbench_client::{mailbox::UiReply, state::RefreshDecision};
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
    use crate::workbench_client::{
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
    use crate::workbench_client::{mailbox::UiReply, state::RefreshDecision};
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
    use crate::workbench_client::mailbox::{AdmissionError, QueuedCommand, Tagged};
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
    use crate::workbench_client::{
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
        launch_failure: None,
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
        ClientPhaseKind::Switching
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
