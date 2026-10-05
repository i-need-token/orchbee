use super::*;
use crate::terminal_session::state::worktree_launch::{
    WorktreeEdit, WorktreeMode, WorktreeToken, tests::discovery,
};
use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchRequestId, WorktreeSource,
};
use ratatui::{Terminal, backend::TestBackend};

pub(super) fn ready(mode: WorktreeMode) -> NativeUi {
    ready_with_project(mode, true)
}
fn ready_with_project(mode: WorktreeMode, with_project: bool) -> NativeUi {
    let (mut ui, _, clock, _) = super::workspace_setup_tests::ready(with_project);
    let p = ui
        .state
        .as_ref()
        .unwrap()
        .worktree_preparation(
            mode,
            1,
            WorktreeToken::for_test(0),
            OperationDeadlines::from_sample(clock.now()).unwrap(),
        )
        .unwrap();
    assert!(ui.worktrees.launcher.open(p));
    let p = ui.worktrees.launcher.preparation().unwrap().clone();
    assert!(ui.worktrees.launcher.prepared(
        &p,
        Some(WorktreeSource::Directory {
            path: WorkbenchDirectoryPath::parse("C:/repo").unwrap()
        })
    ));
    let mut listing = discovery();
    listing.worker = p.primary.as_ref().unwrap().worker_id().clone();
    assert!(
        ui.worktrees
            .launcher
            .discovered(ui.worktrees.launcher.token().unwrap(), Ok(listing))
    );
    ui
}
fn draw(ui: &mut NativeUi, locale: crate::localization::Locale, width: u16, height: u16) -> String {
    draw_with_theme(
        ui,
        locale,
        width,
        height,
        &crate::tui_theme::Theme::rgb_test_dark(),
    )
}
fn draw_with_theme(
    ui: &mut NativeUi,
    locale: crate::localization::Locale,
    width: u16,
    height: u16,
    theme: &crate::tui_theme::Theme,
) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = NativeDrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &crate::client_view::ClientView::new(width),
                locale,
                Some(crate::terminal_session::TerminalSessionView::Native(
                    ui.view(),
                )),
                theme,
                &mut crate::tui_pointer::HitMapBuilder::new(),
            );
        })
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    ui.after_close_draw(receipt);
    text.replace(' ', "")
}

#[test]
fn worktree_complete_draw_in_both_locales_arms_one_exact_plan_and_small_draw_cancels() {
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        for (width, height, fits) in [
            (120, 40, true),
            (80, 24, true),
            (80, 20, true),
            (50, 10, false),
            (12, 3, false),
        ] {
            for theme in [
                crate::tui_theme::Theme::terminal_native(),
                crate::tui_theme::Theme::rgb_test_dark(),
                crate::tui_theme::Theme::one_light(),
                crate::tui_theme::Theme::one_dark(),
            ] {
                let mut ui = ready(WorktreeMode::Create);
                let token = ui.worktrees.launcher.token().unwrap();
                ui.edit_worktree(token, WorktreeEdit::Paste("feature/猫"));
                let token = ui.worktrees.launcher.token().unwrap();
                let text = draw_with_theme(&mut ui, locale, width, height, &theme);
                if fits {
                    assert!(text.contains("feature/猫"));
                    assert!(
                        text.contains(
                            &locale
                                .worktree_actions(WorktreeStage::Branch)
                                .replace(' ', "")
                        )
                    );
                }
                let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
                let plan = ui.worktrees.launcher.confirm(
                    token,
                    WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap(),
                    &origin,
                    WorkbenchLabel::parse("Default").unwrap(),
                );
                assert_eq!(plan.is_some(), fits);
                assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
            }
        }
    }
}

#[test]
fn worktree_checkout_picker_windows_selection_and_disambiguates_duplicate_branches_by_path() {
    let mut ui = ready(WorktreeMode::Open);
    let token = ui.worktrees.launcher.token().unwrap();
    ui.edit_worktree(token, WorktreeEdit::Next);
    let text = draw(&mut ui, crate::localization::Locale::ZhCn, 80, 24);
    assert!(text.contains("C:/trees/猫"));
    assert!(text.contains("2/2"));
    let view = ui.view();
    assert_eq!(view.worktree().unwrap().selected(), 1);
}

#[test]
fn a_geometry_change_revokes_the_draw_without_submitting_or_changing_source() {
    let mut ui = ready(WorktreeMode::Create);
    let token = ui.worktrees.launcher.token().unwrap();
    ui.edit_worktree(token, WorktreeEdit::Append("feature/one"));
    draw(&mut ui, crate::localization::Locale::EnUs, 120, 40);
    let old = ui.state.as_ref().unwrap().desired().cloned();
    ui.observe_geometry(Geometry::new(100, 24, 8, 16).unwrap());
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    assert!(
        ui.worktrees
            .launcher
            .confirm(
                ui.worktrees.launcher.token().unwrap(),
                WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap(),
                &origin,
                WorkbenchLabel::parse("Default").unwrap()
            )
            .is_none()
    );
    assert_eq!(ui.state.as_ref().unwrap().desired(), old.as_ref());
}

#[test]
fn hidden_completion_reopens_the_same_receipt_without_new_preparation_or_focus() {
    for mode in [WorktreeMode::Create, WorktreeMode::Open] {
        let mut ui = ready(WorktreeMode::Open);
        draw(&mut ui, crate::localization::Locale::EnUs, 80, 24);
        let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
        let operation = WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap();
        let token = ui.worktrees.launcher.token().unwrap();
        let plan = ui
            .worktrees
            .launcher
            .confirm(
                token,
                operation.clone(),
                &origin,
                WorkbenchLabel::parse("Default").unwrap(),
            )
            .unwrap();
        ui.dismiss_worktree(token);
        assert!(!ui.worktrees.launcher.visible());
        assert!(
            ui.worktrees.launcher.start_settled(
                &operation,
                Ok(
                    orchbee_workbench_protocol::WorkbenchWorktreeOutcome::Available {
                        project_id: plan.project.unwrap(),
                        attempt: 1,
                        status: Box::new(
                            orchbee_workbench_protocol::WorktreeOperationStatus::Opened {
                                checkout: orchbee_workbench_protocol::WorktreeCheckoutFacts {
                                    repository_root: WorkbenchDirectoryPath::parse("C:/repo")
                                        .unwrap(),
                                    common_directory: WorkbenchDirectoryPath::parse("C:/repo/.git")
                                        .unwrap(),
                                    path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
                                    branch: None,
                                    dirty: false,
                                },
                                workspace_id:
                                    orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                                        &format!("{:032x}", 80),
                                    )
                                    .unwrap(),
                                reused: true,
                                terminal_outcome: None,
                            }
                        ),
                    }
                ),
            )
        );
        assert_eq!(
            ui.request_worktree(mode),
            SessionActionDisposition::DismissNavigator
        );
        let retained = ui.worktrees.launcher.operation().unwrap();
        assert_eq!(retained.plan.operation_id, operation);
        assert!(!retained.focus_allowed);
        assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::Opened);
        assert_eq!(
            ui.worktrees.launcher.view().unwrap().stage(),
            WorktreeStage::Result
        );
        assert_eq!(
            ui.state.as_ref().unwrap().worktree_origin().unwrap(),
            origin
        );
        assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    }
}

#[test]
fn unknown_default_project_result_can_be_explicitly_closed_without_replaying_creation() {
    let mut ui = ready_with_project(WorktreeMode::Open, false);
    draw(&mut ui, crate::localization::Locale::EnUs, 80, 24);
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let operation = WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap();
    let token = ui.worktrees.launcher.token().unwrap();
    let plan = ui
        .worktrees
        .launcher
        .confirm(
            token,
            operation.clone(),
            &origin,
            WorkbenchLabel::parse("Default").unwrap(),
        )
        .unwrap();
    assert!(plan.project.is_none());
    ui.worktrees
        .launcher
        .retain_project_request(WorkbenchRequestId::parse(&format!("{:032x}", 91)).unwrap());
    assert!(
        ui.worktrees
            .launcher
            .project_settled(&operation, None, None, Some(&origin), true,)
            .is_none()
    );
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::Unknown);
    assert!(ui.worktrees.launcher.begin_read().is_none());
    assert!(
        ui.worktrees
            .launcher
            .view()
            .unwrap()
            .project_request()
            .is_some()
    );
    assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());

    assert!(
        ui.worktrees
            .launcher
            .view()
            .unwrap()
            .operation_id()
            .is_none()
    );
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let text = draw(&mut ui, locale, 80, 24);
        assert!(
            text.contains(
                &locale
                    .worktree_stage_notice(WorktreeNotice::Unknown, WorktreeStage::ProjectResult,)
                    .replace(' ', "")
            )
        );
        assert!(!text.contains("orchbeeworktreestatus"));
    }
    ui.dismiss_worktree(token);
    assert!(ui.worktrees.launcher.operation().is_none());
    assert!(ui.worktree_input_context().is_none());
    let deadlines = OperationDeadlines::from_sample(ui.clock.now()).unwrap();
    assert!(
        ui.mailbox
            .as_ref()
            .unwrap()
            .try_enqueue_worktree_preparation_for(
                ui.state.as_mut().unwrap(),
                &mut ui.worktrees.launcher,
                WorktreeMode::Create,
                deadlines,
                &ui.clock,
            )
    );
    assert_eq!(
        ui.worktrees.launcher.view().unwrap().stage(),
        WorktreeStage::Preparing
    );
    assert_ne!(ui.worktrees.launcher.token(), Some(token));
    assert!(ui.worktrees.launcher.operation().is_none());
}
