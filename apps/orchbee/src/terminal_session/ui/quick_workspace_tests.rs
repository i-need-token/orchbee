use super::*;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::mailbox::{EventDeadline, UiReply};
use orchbee_workbench_protocol::{
    Project, Snapshot, WorkbenchDirectoryPath, WorkbenchProjectId, WorkbenchRevision,
    WorkbenchSnapshotOutcome,
};

fn prepare(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &Snapshot,
    cancelled: bool,
) {
    assert!(ui.request_quick_workspace());
    finish_preparation(ui, mailbox, clock, snapshot, cancelled);
}

pub(super) fn finish_preparation(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &Snapshot,
    cancelled: bool,
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
                cwd: Some(WorkbenchDirectoryPath::parse("C:/项目 space").unwrap()),
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

#[test]
fn quick_workspace_late_preparation_after_escape_preserves_the_original_terminal_without_mutation()
{
    let (mut ui, mailbox, clock, snapshot) = super::workspace_setup_tests::ready(false);
    let old = ui.state.as_ref().unwrap().desired().cloned();
    prepare(&mut ui, &mailbox, &clock, &snapshot, true);
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(ui.state.as_ref().unwrap().desired(), old.as_ref());
    assert!(ui.state.as_ref().unwrap().cache().projects().is_empty());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn quick_workspace_uses_one_form_confirmation_and_continues_a_known_default_project_once() {
    let (mut ui, mailbox, clock, mut snapshot) = super::workspace_setup_tests::ready(false);
    prepare(&mut ui, &mailbox, &clock, &snapshot, false);
    let token = ui.view().setup_draft().unwrap().token();
    assert_eq!(ui.view().setup_draft().unwrap().text(), "项目 space");
    ui.submit_setup_draft(token);
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.submit_setup_draft(token);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("ProjectCreate");
    };
    assert!(
        matches!(&marker.0.command.action, WorkbenchAction::ProjectCreate { label } if !label.as_str().is_empty())
    );
    let label = match &marker.0.command.action {
        WorkbenchAction::ProjectCreate { label } => label.as_str().to_owned(),
        _ => unreachable!(),
    };
    let deadlines = marker.0.deadlines;
    assert_eq!(
        mailbox.mark_setup_in_flight(&marker.0).unwrap(),
        MutationResult::Applied
    );
    let project = WorkbenchProjectId::parse(&format!("{:032x}", 270)).unwrap();
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.projects.push(Project {
        project_id: project.clone(),
        label,
        directories: vec![],
        workspaces: vec![],
    });
    let wait = super::workspace_setup_tests::place(
        &mailbox,
        &clock,
        &lease,
        WorkbenchSetupCommit::ProjectCreated {
            revision: snapshot.revision,
            project_id: project.clone(),
        },
        snapshot,
    );
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Refresh(_))
    ));
    mailbox.finish_lease(lease).unwrap();
    let launch = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = launch.record() else {
        panic!("continued WorkspaceLaunch");
    };
    assert_eq!(marker.0.deadlines, deadlines);
    assert_eq!(
        marker.0.command.expected_revision,
        WorkbenchRevision::new(2)
    );
    assert!(
        matches!(&marker.0.command.action, WorkbenchAction::WorkspaceLaunch {project_id, label, options, ..} if project_id == &project && label.as_str() == "项目 space" && options.cwd.as_ref().unwrap().as_str() == "C:/项目 space")
    );
    assert!(ui.view().setup_draft().is_none());
    assert!(ui.state.as_ref().unwrap().invariant_holds());
    mailbox.close(CarrierFailure::WorkerExited);
}

#[test]
fn quick_workspace_existing_project_launch_keeps_both_fields_and_rejects_stale_field_draw() {
    let (mut ui, mailbox, clock, snapshot) = super::workspace_setup_tests::ready(true);
    prepare(&mut ui, &mailbox, &clock, &snapshot, false);
    let token = ui.view().setup_draft().unwrap().token();
    ui.edit_setup_draft(token, NativeSetupEdit::Clear);
    ui.edit_setup_draft(token, NativeSetupEdit::Append("named"));
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_setup_draft(token, NativeSetupEdit::NextField);
    let path_token = ui.view().setup_draft().unwrap().token();
    assert_ne!(token, path_token);
    assert_eq!(ui.view().setup_draft().unwrap().text(), "C:/项目 space");
    ui.edit_setup_draft(path_token, NativeSetupEdit::Clear);
    ui.edit_setup_draft(path_token, NativeSetupEdit::Append("child"));
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.submit_setup_draft(path_token);
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.after_setup_draw(Some(ModalPresentation::Presented(path_token)));
    ui.submit_setup_draft(path_token);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("WorkspaceLaunch");
    };
    assert!(
        matches!(&marker.0.command.action, WorkbenchAction::WorkspaceLaunch { project_id, label, options, ..} if project_id == &snapshot.navigation.projects[0].project_id && label.as_str() == "named" && options.cwd.as_ref().unwrap().as_str() == "C:/项目 space\\child")
    );
    mailbox.close(CarrierFailure::WorkerExited);
}

#[test]
fn quick_workspace_a_narrow_form_draw_cancels_without_creating_a_project() {
    let (mut ui, mailbox, clock, snapshot) = super::workspace_setup_tests::ready(false);
    prepare(&mut ui, &mailbox, &clock, &snapshot, false);
    ui.after_setup_draw(Some(ModalPresentation::NotPresented));
    assert!(ui.view().setup_draft().is_none());
    assert!(ui.state.as_ref().unwrap().cache().projects().is_empty());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn quick_workspace_actual_shell_draw_shows_both_localized_fields_and_only_authorizes_a_fitting_form()
 {
    use crate::{
        client_view::ClientView, localization::Locale, terminal_session::TerminalSessionView,
        tui_pointer::HitMapBuilder,
    };
    use ratatui::{Terminal, backend::TestBackend};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height, fits) in [(120, 40, true), (22, 8, false)] {
            let (mut ui, mailbox, clock, snapshot) = super::workspace_setup_tests::ready(false);
            prepare(&mut ui, &mailbox, &clock, &snapshot, false);
            let token = ui.view().setup_draft().unwrap().token();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut receipt = NativeDrawReceipt::default();
            terminal
                .draw(|frame| {
                    receipt = crate::tui_render::render_terminal_shell(
                        frame,
                        &ClientView::new(width),
                        locale,
                        Some(TerminalSessionView::Native(ui.view())),
                        &crate::tui_theme::Theme::rgb_test_dark(),
                        &mut HitMapBuilder::new(),
                    );
                })
                .unwrap();
            assert_eq!(receipt.editing_cursor_shown(), fits);
            let text = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            if fits {
                // Wide CJK glyphs leave a padding cell in TestBackend's flattened buffer.
                let visible = text.replace(' ', "");
                assert!(visible.contains(&locale.quick_workspace_name().replace(' ', "")));
                assert!(visible.contains(&locale.quick_workspace_directory().replace(' ', "")));
                assert!(visible.contains(&locale.quick_workspace_actions().replace(' ', "")));
                assert!(visible.contains("C:/项目space"));
            }
            ui.after_close_draw(receipt);
            ui.submit_setup_draft(token);
            assert_eq!(
                mailbox.try_recv().unwrap().is_some(),
                fits,
                "only a successful complete draw authorizes creation"
            );
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}
