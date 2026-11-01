//! Controlled completion order for immediate creation feedback and canonical handoff.
use super::*;
use crate::localization::WorkspaceCreationPhase;

fn rendered_text(buffer: &ratatui::buffer::Buffer) -> String {
    let mut text = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        let mut x = buffer.area.x;
        while x < buffer.area.right() {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            x += crate::terminal_text::display_width(symbol).max(1) as u16;
        }
        text.push('\n');
    }
    text
}

#[test]
fn every_creation_entry_projects_immediately_without_changing_canonical_selection() {
    for entry in ["create", "quick", "project"] {
        let (mut ui, mailbox, clock, snapshot) = ready(true);
        let source = ui.state.as_ref().unwrap().desired().cloned();
        let action = match entry {
            "create" => ClientAction::RequestWorkspaceCreate.into(),
            "quick" => ClientAction::RequestQuickWorkspace.into(),
            _ => crate::workbench_client::InputAction::project_quick_workspace(Box::new(
                crate::workbench_state::workspace_setup::quick::ProjectQuickWorkspaceRequest::capture(
                    ui.state.as_ref().unwrap(),
                    &snapshot.navigation.projects[0],
                    None,
                    clock.now() + crate::workbench_state::OPERATION_TOTAL,
                )
                .unwrap(),
            )),
        };
        assert_eq!(
            ui.apply_client_action(action),
            ClientActionDisposition::LayoutChanged
        );
        finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
        assert!(ui.view().setup_draft().is_none(), "no review is presented");
        assert_eq!(ui.state.as_ref().unwrap().desired(), source.as_ref());
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot
        );

        let preview = ui.view().workspace_creation_preview().unwrap();
        assert_eq!(preview.phase, WorkspaceCreationPhase::Starting);
        assert_eq!(preview.label, Some("项目 space"));
        assert_eq!(
            preview.project_id,
            (entry == "project").then_some(&snapshot.navigation.projects[0].project_id)
        );
        assert!(preview.current && preview.replaces_content() && preview.committed.is_none());
        assert_eq!(ui.state.as_ref().unwrap().desired(), source.as_ref());
        assert!(
            !ui.request_quick_workspace(),
            "one accepted launch remains one operation"
        );
        assert!(ui.state.as_ref().unwrap().invariant_holds());
        mailbox.close(CarrierFailure::WorkerExited);
    }
}

#[test]
fn focus_after_review_acceptance_is_not_rearmed_by_late_commit() {
    let (mut ui, mailbox, clock, snapshot) = ready(false);
    let source = ui.state.as_ref().unwrap().desired().cloned();
    ui.apply_client_action(ClientAction::RequestWorkspaceCreate);
    finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
    assert!(ui.view().workspace_creation_preview().unwrap().current);
    ui.apply_client_action(ClientAction::SelectNextContext);
    let preview = ui.view().workspace_creation_preview().unwrap();
    assert!(!preview.current && !preview.replaces_content());
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("launch")
    };
    assert_eq!(
        mailbox.mark_setup_in_flight(&marker.0).unwrap(),
        MutationResult::Applied
    );
    let (committed, receipt) = launched(&snapshot, &lease);
    let wait = place(&mailbox, &clock, &lease, receipt, committed.clone());
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh")
    };
    assert_eq!(settlement.decision(), &RefreshDecision::Continue);
    assert_eq!(ui.state.as_ref().unwrap().desired(), source.as_ref());
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &committed
    );
    let preview = ui.view().workspace_creation_preview().unwrap();
    assert_eq!(preview.phase, WorkspaceCreationPhase::Committed);
    assert!(!preview.current && !preview.replaces_content());
    assert!(preview.committed.is_some());
    mailbox.finish_lease(lease).unwrap();

    // The previous real identity must never be reused for the next provisional row.
    assert!(ui.request_quick_workspace());
    finish_preparation(&mut ui, &mailbox, &clock, &committed, false);
    assert!(
        ui.view()
            .workspace_creation_preview()
            .unwrap()
            .committed
            .is_none()
    );
    mailbox.close(CarrierFailure::WorkerExited);
}

#[test]
fn pending_creation_never_forwards_typing_to_the_hidden_source_terminal() {
    let (mut ui, mailbox, clock, _) = super::super::viewport::controlling_ui();
    let snapshot = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    assert!(ui.request_quick_workspace());
    finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
    assert!(
        ui.view()
            .workspace_creation_preview()
            .unwrap()
            .replaces_content()
    );
    assert!(!ui.forward(b"must not reach the old shell\r"));
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::WorkspaceSetup(_)));
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_none());
    mailbox.close(CarrierFailure::WorkerExited);
}

#[test]
fn definite_refusal_removes_the_preview_and_unknown_or_failed_commits_keep_recovery_facts() {
    for failure in ["refused", "unknown", "committed"] {
        let (mut ui, mailbox, clock, snapshot) = ready(false);
        let source = ui.state.as_ref().unwrap().desired().cloned();
        let lease = submit_launch(&mut ui, &mailbox, &clock, &snapshot);
        let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
            panic!("launch")
        };
        let operation = marker.0.clone();
        if failure == "refused" {
            assert!(mailbox.retain_setup_not_applied(&operation));
            let wait = mailbox
                .publish_setup_result_before(operation, SetupResult::NotApplied, &clock)
                .unwrap();
            ui.tick();
            assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
            assert!(ui.view().workspace_creation_preview().is_none());
            assert_eq!(ui.view().setup_status(), Some(SetupStatus::NotApplied));
        } else {
            let evidence = if failure == "unknown" {
                crate::workbench_state::workspace_setup::SetupWorkerEvidence::Unknown(operation)
            } else {
                let (_, commit) = launched(&snapshot, &lease);
                crate::workbench_state::workspace_setup::SetupWorkerEvidence::Known(
                    operation.known_commit(commit).unwrap(),
                )
            };
            assert!(ui.finalize_setup_worker(Some(evidence)));
            ui.retire(CarrierFailure::OutcomeUnknown);
            assert!(ui.view().workspace_creation_preview().is_none());
            let recovery = ui.view().quick_workspace_recovery().unwrap();
            assert_eq!(recovery.workspace.is_some(), failure == "committed");
            assert_eq!(recovery.unknown.is_some(), failure == "unknown");
            assert!(!ui.request_quick_workspace());
        }
        assert_eq!(ui.state.as_ref().unwrap().desired(), source.as_ref());
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot
        );
        mailbox.close(CarrierFailure::WorkerExited);
    }
}

#[test]
fn loading_projection_renders_both_locales_and_all_tiers_without_a_modal_or_old_tab_hits() {
    use crate::{
        client_view::ClientView,
        localization::Locale,
        tui_hit_map::{HitMapBuilder, HitTarget},
        tui_theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                let (mut ui, mailbox, clock, snapshot) = ready(false);
                let mut view = ClientView::new(width);
                view.machines.available = true;
                ui.apply_client_action(ClientAction::RequestWorkspaceCreate);
                finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &view,
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut hits,
                        );
                    })
                    .unwrap();
                let text = rendered_text(terminal.backend().buffer());
                assert!(
                    !text.contains(locale.setup_cancel_label()),
                    "creation has no modal cancel surface"
                );
                if width >= 60 {
                    let title = match locale {
                        Locale::EnUs => "Tab 1",
                        Locale::ZhCn => "标签页 1",
                    };
                    assert!(text.lines().next().unwrap().contains(title), "{text}");
                }
                if width == 120 {
                    let name = locale.sidebar_navigation_action(
                        crate::machine_picker::SidebarCommand::NewWorkspace,
                    );
                    assert!(
                        text.contains(name),
                        "the sidebar retains its Workspace name"
                    );
                    assert!(text.contains(
                        locale.workspace_creation_progress(WorkspaceCreationPhase::Starting)
                    ));
                }
                let hits = hits.finish();
                assert!((0..height).all(|y| (0..width).all(|x| !matches!(
                    hits.hit_target_at(x, y),
                    Some(HitTarget::Tab(_) | HitTarget::Pane { .. })
                ))));
                assert!(!terminal.backend().cursor_visible());
                mailbox.close(CarrierFailure::WorkerExited);
            }
        }
    }
}
