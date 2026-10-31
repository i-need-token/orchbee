use super::super::*;
use crate::workbench_state::worktree_launch::WorktreeStage;
use crate::workbench_state::worktree_launch::tests::discovery;
use crate::workbench_state::worktree_launch::{WorktreeEdit, WorktreeMode, WorktreeToken};
use crate::{
    tui_hit_map::{FrameHitMap, HitMapBuilder, HitTarget, ModalControl},
    tui_theme::Role,
};
use orchbee_workbench_protocol::{WorkbenchDirectoryPath, WorkbenchRequestId, WorktreeSource};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Modifier};

pub(in crate::workbench_client::ui) fn ready(mode: WorktreeMode) -> WorkbenchClientUi {
    ready_with_project(mode, true)
}
fn ready_with_project(mode: WorktreeMode, with_project: bool) -> WorkbenchClientUi {
    let (mut ui, _, clock, _) = super::super::tests::workspace_setup::ready(with_project);
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
fn draw(
    ui: &mut WorkbenchClientUi,
    locale: crate::localization::Locale,
    width: u16,
    height: u16,
) -> String {
    draw_with_theme(
        ui,
        locale,
        width,
        height,
        &crate::tui_theme::Theme::rgb_test_dark(),
    )
}
fn draw_with_theme(
    ui: &mut WorkbenchClientUi,
    locale: crate::localization::Locale,
    width: u16,
    height: u16,
    theme: &crate::tui_theme::Theme,
) -> String {
    draw_frame_with_theme(ui, locale, width, height, theme).text
}

struct DrawnWorktree {
    text: String,
    buffer: Buffer,
    hits: FrameHitMap,
}

fn visible_row(buffer: &Buffer, width: u16, y: u16) -> String {
    let mut row = String::new();
    let mut x = 0;
    while x < width {
        let symbol = buffer[(x, y)].symbol();
        row.push_str(symbol);
        let columns = crate::terminal_text::display_width(symbol).max(1);
        x = x.saturating_add(u16::try_from(columns).unwrap_or(u16::MAX));
    }
    row
}

fn draw_frame_with_theme(
    ui: &mut WorkbenchClientUi,
    locale: crate::localization::Locale,
    width: u16,
    height: u16,
    theme: &crate::tui_theme::Theme,
) -> DrawnWorktree {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = DrawReceipt::default();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &crate::client_view::ClientView::new(width),
                locale,
                Some(ui.view()),
                theme,
                &mut hits,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    ui.after_close_draw(receipt);
    DrawnWorktree {
        text: text.replace(' ', ""),
        buffer,
        hits: hits.finish(),
    }
}

#[test]
fn worktree_complete_draw_arms_one_exact_plan_and_small_first_draw_refuses() {
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
                let first = draw_with_theme(&mut ui, locale, width, height, &theme);
                let token = if fits {
                    assert!(ui.edit_worktree(token, WorktreeEdit::Paste("feature/猫")));
                    let token = ui.worktrees.launcher.token().unwrap();
                    let drawn = draw_frame_with_theme(&mut ui, locale, width, height, &theme);
                    assert!(drawn.text.contains("feature/猫"));
                    let cancel_label = locale.worktree_cancel_label(WorktreeStage::Branch);
                    let submit_label = locale.worktree_submit_label(WorktreeStage::Branch);
                    assert!(
                        drawn
                            .text
                            .contains(&format!("[{}Esc]", cancel_label.replace(' ', "")))
                    );
                    assert!(
                        drawn
                            .text
                            .contains(&format!("[{}Enter]", submit_label.replace(' ', "")))
                    );

                    let hint_y = (0..height)
                        .find(|&y| {
                            (0..width.saturating_sub(2)).any(|x| {
                                drawn.buffer[(x, y)].symbol() == "E"
                                    && drawn.buffer[(x + 1, y)].symbol() == "s"
                                    && drawn.buffer[(x + 2, y)].symbol() == "c"
                            })
                        })
                        .expect("Worktree cancel button binding");
                    let hint_x = (0..width.saturating_sub(2))
                        .find(|&x| {
                            drawn.buffer[(x, hint_y)].symbol() == "E"
                                && drawn.buffer[(x + 1, hint_y)].symbol() == "s"
                                && drawn.buffer[(x + 2, hint_y)].symbol() == "c"
                        })
                        .unwrap();
                    let footer = visible_row(&drawn.buffer, width, hint_y);
                    assert!(
                        footer.contains(&format!("[ {cancel_label}  Esc ]")),
                        "{locale:?}: {footer:?}"
                    );
                    assert!(
                        footer.contains(&format!("[ {submit_label}  Enter ]")),
                        "{locale:?}: {footer:?}"
                    );
                    let secondary_hints = visible_row(&drawn.buffer, width, hint_y - 1);
                    assert!(!secondary_hints.contains("Esc") && !secondary_hints.contains("Enter"));
                    let key_style = drawn.buffer[(hint_x, hint_y)].style();
                    assert_eq!(key_style.fg, theme.style(Role::TextSecondary).fg);
                    assert!(!key_style.add_modifier.contains(Modifier::BOLD));

                    let target_count = |control| {
                        (0..height)
                            .flat_map(|y| (0..width).map(move |x| (x, y)))
                            .filter(|(x, y)| {
                                matches!(
                                    drawn.hits.hit_target_at(*x, *y),
                                    Some(HitTarget::Worktree {
                                        token: painted,
                                        control: painted_control,
                                    }) if *painted == token && *painted_control == control
                                )
                            })
                            .count()
                    };
                    let focus_width =
                        crate::terminal_text::display_width(theme.glyphs.active_marker).max(1);
                    assert_eq!(
                        target_count(ModalControl::Cancel),
                        focus_width + 4 + crate::terminal_text::display_width(cancel_label) + 5 + 3 // The independent header [x] shares the cancel intent; the button also owns two spaces and Esc.
                    );
                    assert_eq!(
                        target_count(ModalControl::Submit),
                        focus_width + 4 + crate::terminal_text::display_width(submit_label) + 7
                    );
                    assert!(target_count(ModalControl::NextField) > 0);
                    assert!(
                        (0..height)
                            .flat_map(|y| (0..width).map(move |x| (x, y)))
                            .all(|(x, y)| {
                                !matches!(
                                    drawn.hits.hit_target_at(x, y),
                                    Some(HitTarget::Worktree { token: painted, .. })
                                        if *painted != token
                                )
                            })
                    );
                    token
                } else {
                    assert!(!first.contains("feature/猫"));
                    assert!(!ui.worktrees.launcher.visible());
                    assert_eq!(
                        ui.worktrees.launcher.notice(),
                        WorktreeNotice::PresentationUnavailable
                    );
                    token
                };
                let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
                let plan = ui.worktrees.launcher.confirm(
                    token,
                    WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap(),
                    &origin,
                );
                assert_eq!(plan.is_some(), fits);
                assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
            }
        }
    }
}

#[test]
fn an_admitted_worktree_surface_suspends_at_small_geometry_and_resumes_in_place() {
    let mut ui = ready(WorktreeMode::Create);
    draw(&mut ui, crate::localization::Locale::EnUs, 80, 24);
    let token = ui.worktrees.launcher.token().unwrap();
    assert!(ui.edit_worktree(token, WorktreeEdit::Append("feature/preserved")));
    let token = ui.worktrees.launcher.token().unwrap();
    draw(&mut ui, crate::localization::Locale::EnUs, 80, 24);
    let before = ui.worktrees.launcher.view().unwrap().branch().to_owned();

    draw(&mut ui, crate::localization::Locale::EnUs, 12, 3);
    assert!(ui.worktrees.launcher.visible());
    assert_eq!(ui.worktrees.launcher.token(), Some(token));
    assert!(ui.worktree_capture_claim());
    assert!(ui.worktree_input_context().is_none());
    assert_eq!(
        ui.view().worktree().unwrap().presentation(),
        crate::tui_presentation::PresentationPhase::Suspended
    );
    assert!(ui.edit_worktree(token, WorktreeEdit::Append("-hidden")));
    assert_eq!(ui.worktrees.launcher.view().unwrap().branch(), before);
    ui.submit_worktree(token);
    assert!(ui.worktrees.launcher.operation().is_none());

    draw(&mut ui, crate::localization::Locale::EnUs, 80, 24);
    assert!(ui.edit_worktree(token, WorktreeEdit::Append("-resumed")));
    assert_eq!(
        ui.worktrees.launcher.view().unwrap().branch(),
        "feature/preserved-resumed"
    );
}

#[test]
fn worktree_capture_claim_tracks_visibility_and_context_keeps_live_frame_continuity() {
    let mut ui = ready(WorktreeMode::Create);
    let token = ui.worktrees.launcher.token().unwrap();
    assert!(ui.view().worktree_capture_claim());
    assert!(ui.worktree_capture_claim());
    assert!(ui.view().worktree_input_context().is_none());
    assert!(ui.worktree_input_context().is_none());

    ui.worktrees.launcher.draw(Some(token));
    assert_eq!(ui.view().worktree_input_context().unwrap().token, token);
    assert_eq!(ui.worktree_input_context().unwrap().token, token);

    assert!(ui.edit_worktree(token, WorktreeEdit::Append("feature/live")));
    let edited = ui.worktrees.launcher.token().unwrap();
    assert_ne!(edited, token);
    assert!(ui.worktree_capture_claim());
    assert_eq!(ui.worktree_input_context().unwrap().token, edited);

    assert!(ui.edit_worktree(edited, WorktreeEdit::NextField));
    let next_field = ui.worktrees.launcher.token().unwrap();
    assert!(ui.worktree_capture_claim());
    assert!(ui.worktree_input_context().is_none());

    ui.worktrees.launcher.draw(Some(next_field));
    assert_eq!(ui.worktree_input_context().unwrap().token, next_field);
    ui.after_worktree_draw(None);
    assert!(ui.worktree_capture_claim());
    assert!(ui.worktree_input_context().is_none());
    assert_eq!(
        ui.view().worktree().unwrap().presentation(),
        crate::tui_presentation::PresentationPhase::Admitted
    );

    ui.dismiss_worktree(next_field);
    assert!(!ui.worktree_capture_claim());
    assert!(ui.worktree_input_context().is_none());
}

#[test]
fn worktree_checkout_picker_windows_selection_and_disambiguates_duplicate_branches_by_path() {
    let mut ui = ready(WorktreeMode::Open);
    draw(&mut ui, crate::localization::Locale::ZhCn, 80, 24);
    let token = ui.worktrees.launcher.token().unwrap();
    assert!(ui.edit_worktree(token, WorktreeEdit::Next));
    let text = draw(&mut ui, crate::localization::Locale::ZhCn, 80, 24);
    assert!(text.contains(r"C:\trees\猫"));
    assert!(text.contains("2/2"));
    let view = ui.view();
    assert_eq!(view.worktree().unwrap().selected(), 1);
}

#[test]
fn a_geometry_change_revokes_the_draw_without_submitting_or_changing_source() {
    let mut ui = ready(WorktreeMode::Create);
    draw(&mut ui, crate::localization::Locale::EnUs, 120, 40);
    let token = ui.worktrees.launcher.token().unwrap();
    assert!(ui.edit_worktree(token, WorktreeEdit::Append("feature/one")));
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
            .confirm(token, operation.clone(), &origin)
            .unwrap();
        ui.dismiss_worktree(token);
        assert!(!ui.worktrees.launcher.visible());
        assert!(
            ui.worktrees.launcher.start_settled(
                &operation,
                Ok(
                    orchbee_workbench_protocol::WorkbenchWorktreeOutcome::Available {
                        project_id: plan.project,
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
            ClientActionDisposition::DismissNavigator
        );
        let retained = ui.worktrees.launcher.operation().unwrap();
        assert_eq!(retained.plan.operation_id, operation);
        assert!(!retained.focus_allowed);
        assert_eq!(
            ui.worktrees.launcher.notice(),
            WorktreeNotice::EntryDeferred
        );
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
fn unknown_worktree_result_retains_one_operation_and_can_be_hidden_without_replay() {
    let mut ui = ready_with_project(WorktreeMode::Open, false);
    draw(&mut ui, crate::localization::Locale::EnUs, 80, 24);
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let operation = WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap();
    let token = ui.worktrees.launcher.token().unwrap();
    let plan = ui
        .worktrees
        .launcher
        .confirm(token, operation.clone(), &origin)
        .unwrap();
    assert!(plan.project.is_none());
    assert!(ui.worktrees.launcher.start_settled(&operation, Err(true)));
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::Unknown);
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let text = draw(&mut ui, locale, 80, 24);
        assert!(text.contains("orchbeeworktreestatus"));
        assert!(text.contains(operation.as_str()));
    }
    let token = ui.worktrees.launcher.token().unwrap();
    ui.dismiss_worktree(token);
    assert!(ui.worktree_input_context().is_none());
    assert_eq!(
        ui.worktrees.launcher.operation().unwrap().plan.operation_id,
        operation
    );
    assert!(ui.worktrees.launcher.reopen());
    assert!(!ui.worktrees.launcher.operation().unwrap().focus_allowed);
    assert_eq!(
        ui.worktrees.launcher.begin_read().unwrap().operation_id,
        operation
    );
    assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
}

#[test]
fn removal_confirmation_shows_full_target_scope_and_retained_branch_without_auto_submitting() {
    for (locale, title, branch, cancel, remove) in [
        (
            crate::localization::Locale::EnUs,
            "Remove worktree: Task 猫",
            "Branch kept: same",
            "Cancel",
            "Remove",
        ),
        (
            crate::localization::Locale::ZhCn,
            "移除工作树：Task 猫",
            "保留分支：same",
            "取消",
            "移除",
        ),
    ] {
        for theme in [
            crate::tui_theme::Theme::terminal_native(),
            crate::tui_theme::Theme::rgb_test_dark(),
        ] {
            for (width, height, fits) in [
                (120, 40, true),
                (80, 24, true),
                (80, 20, true),
                (50, 18, true),
                (50, 10, false),
                (12, 3, false),
            ] {
                let (launcher, origin, _) =
                    crate::workbench_state::worktree_launch::tests::removal::fixture();
                let mut ui = ready(WorktreeMode::Open);
                ui.worktrees.launcher = launcher;
                let token = ui.worktrees.launcher.token().unwrap();
                let drawn = draw_frame_with_theme(&mut ui, locale, width, height, &theme);
                if fits {
                    assert!(
                        drawn.text.contains(&title.replace(' ', "")),
                        "{locale:?} {width}x{height}: {}",
                        drawn.text
                    );
                    assert!(drawn.text.contains(r"C:\trees\猫"));
                    assert!(drawn.text.contains(&branch.replace(' ', "")));
                    assert!(drawn.text.contains(&format!("[{cancel}Esc]")));
                    assert!(drawn.text.contains(&format!("[{remove}Enter]")));
                    assert_eq!(
                        ui.worktrees.launcher.view().unwrap().stage(),
                        WorktreeStage::Remove
                    );
                    assert!(ui.worktrees.launcher.operation().is_none());
                    assert!(!ui.worktrees.launcher.sidebar_open_ready());
                    assert!(
                        ui.worktrees
                            .launcher
                            .confirm(
                                token,
                                WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap(),
                                &origin
                            )
                            .is_some()
                    );
                } else {
                    assert!(ui.worktree_input_context().is_none());
                    assert!(ui.worktrees.launcher.operation().is_none());
                    assert_eq!(
                        ui.worktrees.launcher.notice(),
                        WorktreeNotice::PresentationUnavailable
                    );
                    for y in 0..height {
                        for x in 0..width {
                            assert!(!matches!(
                                drawn.hits.hit_target_at(x, y),
                                Some(HitTarget::Worktree { .. })
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn dirty_removal_offers_resume_and_canceling_resume_restores_the_retained_result() {
    use crate::workbench_state::worktree_launch::tests::{confirm, removal};
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let (mut launcher, origin, _) = removal::fixture();
        let plan = confirm(&mut launcher, &origin).unwrap();
        assert!(launcher.start_settled(&plan.operation_id, Ok(removal::failure(1, true))));
        let mut ui = ready(WorktreeMode::Open);
        ui.worktrees.launcher = launcher;
        let text = draw(&mut ui, locale, 120, 40);
        let expected = match locale {
            crate::localization::Locale::EnUs => "Checkout has changes or untracked files.",
            crate::localization::Locale::ZhCn => "检出目录含修改或未跟踪文件。",
        };
        assert!(text.contains(&expected.replace(' ', "")));
        let token = ui.worktrees.launcher.token().unwrap();
        ui.submit_worktree(token);
        assert_eq!(
            ui.worktrees.launcher.view().unwrap().stage(),
            WorktreeStage::ResumeRemove
        );
        let token = ui.worktrees.launcher.token().unwrap();
        draw(&mut ui, locale, 120, 40);
        ui.dismiss_worktree(token);
        assert_eq!(
            ui.worktrees.launcher.view().unwrap().stage(),
            WorktreeStage::Result
        );
        assert_eq!(
            ui.worktrees.launcher.operation().unwrap().outcome,
            Some(removal::failure(1, true))
        );
        let restored = ui.worktrees.launcher.token().unwrap();
        assert_ne!(restored, token);
        ui.dismiss_worktree(restored);
        assert!(ui.worktree_input_context().is_none());
        assert_eq!(
            ui.request_worktree(WorktreeMode::Open),
            ClientActionDisposition::DismissNavigator
        );
        assert_eq!(
            ui.worktrees.launcher.operation().unwrap().plan.operation_id,
            plan.operation_id
        );
        assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    }
}
