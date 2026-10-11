use super::workspace_setup_tests::{finish_preparation, finish_preparation_with_directory};
use super::*;
use orchbee_workbench_protocol::Snapshot;

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
fn workspace_create_and_quick_launch_submit_one_ungrouped_shell_without_a_project_form() {
    for with_project in [false, true] {
        for action in [
            ClientAction::RequestWorkspaceCreate,
            ClientAction::RequestQuickWorkspace,
        ] {
            let (mut ui, mailbox, clock, snapshot) =
                super::workspace_setup_tests::ready(with_project);
            ui.apply_client_action(action);
            finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                panic!("WorkspaceLaunch")
            };
            assert_eq!(marker.0.command.expected_revision, snapshot.revision);
            assert!(matches!(&marker.0.command.action,
                WorkbenchAction::WorkspaceLaunch { placement: orchbee_workbench_protocol::WorkspacePlacement::Ungrouped {}, label, options, launch: None, profile: None, .. }
                if label.as_str() == "项目 space" && options.cwd.as_ref().unwrap().as_str() == "C:/项目 space"));
            assert_eq!(
                ui.state.as_ref().unwrap().cache().canonical_snapshot(),
                &snapshot
            );
            assert!(ui.view().setup_draft().is_none());
            assert!(!ui.request_quick_workspace());
            assert!(mailbox.try_recv().unwrap().is_none());
            assert!(ui.state.as_ref().unwrap().invariant_holds());
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn quick_workspace_missing_prepared_directory_refuses_without_creating_or_leaving_the_source() {
    let (mut ui, mailbox, clock, snapshot) = super::workspace_setup_tests::ready(false);
    let source = ui.state.as_ref().unwrap().desired().cloned();
    assert!(ui.request_quick_workspace());
    finish_preparation_with_directory(&mut ui, &mailbox, &clock, &snapshot, false, None);
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(
        ui.view().setup_status(),
        Some(NativeSetupStatus::NotApplied)
    );
    assert_eq!(ui.state.as_ref().unwrap().desired(), source.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn quick_workspace_launch_does_not_require_a_modal_draw_at_small_geometry_in_either_locale() {
    use crate::{
        client_view::ClientView, localization::Locale, terminal_session::TerminalSessionView,
        tui_pointer::HitMapBuilder,
    };
    use ratatui::{Terminal, backend::TestBackend};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (22, 8)] {
            let (mut ui, mailbox, clock, snapshot) = super::workspace_setup_tests::ready(false);
            prepare(&mut ui, &mailbox, &clock, &snapshot, false);
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
            assert!(!receipt.editing_cursor_shown());
            ui.after_close_draw(receipt);
            assert!(ui.view().setup_draft().is_none());
            let lease = mailbox.try_recv().unwrap().unwrap();
            assert!(
                matches!(lease.record(), QueuedCommand::WorkspaceSetup(marker)
                if matches!(marker.0.command.action, WorkbenchAction::WorkspaceLaunch { .. }))
            );
            assert!(mailbox.try_recv().unwrap().is_none());
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}
