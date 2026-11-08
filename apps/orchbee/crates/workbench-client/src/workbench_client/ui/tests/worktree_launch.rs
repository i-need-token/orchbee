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
    listing.worker = p.origin.primary_binding().unwrap().worker_id().clone();
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
            // A small window drops the margin and spacing before it refuses.
            (50, 10, true),
            (40, 6, false),
            (12, 3, false),
        ] {
            for theme in [
                crate::tui_theme::Theme::terminal_native(),
                crate::tui_theme::Theme::rgb_test_dark(),
                crate::tui_theme::Theme::orchbee_light(),
                crate::tui_theme::Theme::orchbee_dark(),
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
                    let submit_label = locale.worktree_submit_label(WorktreeMode::Create);
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
                    // The button owns its brackets, two spaces and Esc; the frame's [x] is its own
                    // Close target with the same effect.
                    assert_eq!(
                        target_count(ModalControl::Cancel),
                        4 + crate::terminal_text::display_width(cancel_label) + 5
                    );
                    assert_eq!(target_count(ModalControl::Close), 3);
                    assert_eq!(
                        target_count(ModalControl::Submit),
                        4 + crate::terminal_text::display_width(submit_label) + 7
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
    let view = ui.view();
    assert_eq!(view.worktree().unwrap().selected(), 1);
}

#[test]
fn reading_the_repository_is_stated_only_until_the_checkout_picker_is_drawn() {
    use crate::localization::Locale;
    for (locale, reading) in [
        (Locale::EnUs, "Reading repository…"),
        (Locale::ZhCn, "正在读取仓库…"),
    ] {
        let (mut ui, _, clock, _) = super::super::tests::workspace_setup::ready(true);
        let p = ui
            .state
            .as_ref()
            .unwrap()
            .worktree_preparation(
                WorktreeMode::Open,
                1,
                WorktreeToken::for_test(0),
                OperationDeadlines::from_sample(clock.now()).unwrap(),
            )
            .unwrap();
        assert!(ui.worktrees.launcher.open(p));
        assert_eq!(ui.view().status(locale, 80).text, reading);
        let p = ui.worktrees.launcher.preparation().unwrap().clone();
        assert!(ui.worktrees.launcher.prepared(
            &p,
            Some(WorktreeSource::Directory {
                path: WorkbenchDirectoryPath::parse("C:/repo").unwrap()
            })
        ));
        assert_eq!(ui.view().status(locale, 80).text, reading);
        let mut listing = discovery();
        listing.worker = p.origin.primary_binding().unwrap().worker_id().clone();
        assert!(
            ui.worktrees
                .launcher
                .discovered(ui.worktrees.launcher.token().unwrap(), Ok(listing))
        );
        assert_eq!(
            ui.view().worktree().unwrap().stage(),
            WorktreeStage::Checkout
        );
        let text = draw(&mut ui, locale, 80, 24);
        assert!(text.contains(r"C:\trees\猫"), "the picker list is drawn");
        assert_ne!(ui.view().status(locale, 80).text, reading);
        assert!(!text.contains(&reading.replace(' ', "")));
    }
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
fn a_result_without_an_operation_offers_only_close() {
    use crate::localization::Locale;
    let targets = |drawn: &DrawnWorktree, wanted: ModalControl| {
        (0..24)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .filter(|(x, y)| {
                matches!(
                    drawn.hits.hit_target_at(*x, *y),
                    Some(HitTarget::Worktree { control, .. }) if *control == wanted
                )
            })
            .count()
    };
    for notice in [
        WorktreeNotice::SourceUnavailable,
        WorktreeNotice::ListingUnavailable,
        WorktreeNotice::NotRepository,
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let (mut ui, _, clock, _) = super::super::tests::workspace_setup::ready(true);
            let p = ui
                .state
                .as_ref()
                .unwrap()
                .worktree_preparation(
                    WorktreeMode::Open,
                    1,
                    WorktreeToken::for_test(0),
                    OperationDeadlines::from_sample(clock.now()).unwrap(),
                )
                .unwrap();
            assert!(ui.worktrees.launcher.open(p));
            let p = ui.worktrees.launcher.preparation().unwrap().clone();
            if notice == WorktreeNotice::SourceUnavailable {
                assert!(ui.worktrees.launcher.prepared(&p, None));
            } else {
                assert!(ui.worktrees.launcher.prepared(
                    &p,
                    Some(WorktreeSource::Directory {
                        path: WorkbenchDirectoryPath::parse("C:/repo").unwrap()
                    })
                ));
                let token = ui.worktrees.launcher.token().unwrap();
                assert!(ui.worktrees.launcher.discovered(token, Err(notice)));
            }
            assert_eq!(ui.worktrees.launcher.notice(), notice);
            assert!(ui.worktrees.launcher.operation().is_none());
            assert_eq!(
                ui.worktrees.launcher.view().unwrap().stage(),
                WorktreeStage::Result
            );
            let drawn = draw_frame_with_theme(
                &mut ui,
                locale,
                80,
                24,
                &crate::tui_theme::Theme::rgb_test_dark(),
            );
            let close = locale.worktree_cancel_label(WorktreeStage::Result);
            assert!(drawn.text.contains(&format!("[{close}Esc]")), "{notice:?}");
            assert_eq!(targets(&drawn, ModalControl::Abandon), 0, "{notice:?}");
            assert_eq!(targets(&drawn, ModalControl::Submit), 0, "{notice:?}");
            assert!(targets(&drawn, ModalControl::Cancel) > 0, "{notice:?}");
        }
    }

    // An unknown result names the status command as its only recovery.
    let mut ui = ready_with_project(WorktreeMode::Open, false);
    draw(&mut ui, Locale::EnUs, 80, 24);
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let operation = WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap();
    let token = ui.worktrees.launcher.token().unwrap();
    ui.worktrees
        .launcher
        .confirm(token, operation.clone(), &origin)
        .unwrap();
    assert!(ui.worktrees.launcher.start_settled(&operation, Err(true)));
    let drawn = draw_frame_with_theme(
        &mut ui,
        Locale::EnUs,
        80,
        24,
        &crate::tui_theme::Theme::rgb_test_dark(),
    );
    // The command is complete: it widens the dialog instead of truncating.
    assert!(
        drawn
            .text
            .contains(&format!("orchbeeworktreestatus{}", operation.as_str())),
        "{}",
        drawn.text
    );
    // The outcome is still observed: the primary stays disabled and Cancel reads Close.
    assert_eq!(
        ui.view().worktree().unwrap().stage(),
        WorktreeStage::Waiting
    );
    let close = Locale::EnUs.worktree_cancel_label(WorktreeStage::Waiting);
    assert!(drawn.text.contains(&format!("[{close}Esc]")));
}

#[test]
fn removal_confirmation_shows_full_target_scope_and_retained_branch_without_auto_submitting() {
    for (locale, title, branch, cancel, remove) in [
        (
            crate::localization::Locale::EnUs,
            "Remove Worktree?",
            "Keeps branch same.",
            "Cancel",
            "Remove",
        ),
        (
            crate::localization::Locale::ZhCn,
            "移除工作树？",
            "保留分支 same。",
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
                // A small window drops the margin and spacing before it refuses.
                (50, 10, true),
                (40, 5, false),
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
        // The removal target is still a canonical Workspace, so its receipt stays recoverable.
        let (mut ui, ..) = super::move_membership::ready_ui_with_snapshot(
            origin.cache.canonical_snapshot().clone(),
        );
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

/// A failed removal of the fixture Workspace, accepted from a drawn confirmation.
fn failed_removal(
    outcome: orchbee_workbench_protocol::WorkbenchWorktreeOutcome,
) -> (
    crate::workbench_state::worktree_launch::Launcher,
    crate::workbench_state::worktree_launch::Origin,
    crate::workbench_state::worktree_launch::Plan,
) {
    use crate::workbench_state::worktree_launch::tests::{confirm, removal};
    let (mut launcher, origin, _) = removal::fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    assert!(launcher.start_settled(&plan.operation_id, Ok(outcome)));
    (launcher, origin, plan)
}

/// Replaces the fixture failure reason and resumability, with the Workspace close held.
fn removal_failure(
    reason: orchbee_workbench_protocol::WorktreeOperationFailure,
    resumable: bool,
) -> orchbee_workbench_protocol::WorkbenchWorktreeOutcome {
    use orchbee_workbench_protocol::{
        WorkbenchWorkspaceId, WorkbenchWorktreeOutcome, WorktreeOperationStatus,
        WorktreeWorkspaceDisposition,
    };
    let mut outcome =
        crate::workbench_state::worktree_launch::tests::removal::failure(1, resumable);
    let WorkbenchWorktreeOutcome::Available { status, .. } = &mut outcome else {
        unreachable!("fixture failure is available")
    };
    let WorktreeOperationStatus::Failed {
        reason: actual,
        progress,
        ..
    } = status.as_mut()
    else {
        unreachable!("fixture failure is failed")
    };
    *actual = reason;
    progress.workspace = WorktreeWorkspaceDisposition::Closing {
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 80)).unwrap(),
        close_request_id: WorkbenchRequestId::parse(&format!("{:032x}", 81)).unwrap(),
    };
    outcome
}

#[test]
fn removal_failures_name_their_cause_and_a_dead_end_points_to_the_held_close() {
    use orchbee_workbench_protocol::{
        WorktreeGitError, WorktreeGitStage, WorktreeOperationFailure,
    };
    let git = |stage, error| WorktreeOperationFailure::Git {
        stage,
        error,
        status: None,
        diagnostics: Vec::new(),
    };
    let close = format!("orchbeeworkspaceclose{:032x}--confirm", 80);
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let english = locale == crate::localization::Locale::EnUs;
        for (reason, resumable, expected, guidance) in [
            (
                git(WorktreeGitStage::Select, WorktreeGitError::CheckoutNotFound),
                false,
                if english {
                    "Git no longer lists this checkout"
                } else {
                    "Git 已不再将此检出登记为工作树。"
                },
                true,
            ),
            (
                git(WorktreeGitStage::Mutate, WorktreeGitError::Io),
                true,
                if english {
                    "The checkout directory is in use"
                } else {
                    "检出目录正被占用或无法删除。"
                },
                false,
            ),
            (
                git(WorktreeGitStage::Mutate, WorktreeGitError::GitFailed),
                true,
                if english {
                    "Git could not remove the checkout."
                } else {
                    "Git 未能移除检出目录。"
                },
                false,
            ),
        ] {
            let (launcher, origin, _) = failed_removal(removal_failure(reason, resumable));
            let (mut ui, ..) = super::move_membership::ready_ui_with_snapshot(
                origin.cache.canonical_snapshot().clone(),
            );
            ui.worktrees.launcher = launcher;
            let text = draw(&mut ui, locale, 120, 40);
            assert!(
                text.contains(&expected.replace(' ', "")),
                "{locale:?}: {text}"
            );
            assert!(!text.contains("Gitrefused") && !text.contains("Git拒绝移除"));
            assert_eq!(text.contains(&close), guidance, "{locale:?}: {text}");
        }
    }
}

#[test]
fn a_failed_removal_whose_workspace_closed_never_blocks_the_launcher() {
    use crate::workbench_state::worktree_launch::tests::removal;
    // While the Workspace exists, Close keeps the recoverable receipt and W reopens it.
    let (launcher, origin, plan) = failed_removal(removal::failure(1, true));
    let (mut ui, ..) =
        super::move_membership::ready_ui_with_snapshot(origin.cache.canonical_snapshot().clone());
    ui.worktrees.launcher = launcher;
    draw(&mut ui, crate::localization::Locale::EnUs, 120, 40);
    ui.dismiss_worktree(ui.worktrees.launcher.token().unwrap());
    assert!(!ui.worktrees.launcher.visible());
    assert_eq!(
        ui.request_worktree(WorktreeMode::Open),
        ClientActionDisposition::DismissNavigator
    );
    assert_eq!(
        ui.worktrees.launcher.operation().unwrap().plan.operation_id,
        plan.operation_id
    );

    // After the Workspace closed elsewhere, Close and W both discard the stale receipt.
    let target = plan.removal.as_ref().unwrap().workspace_id.clone();
    for hidden in [false, true] {
        let (launcher, ..) = failed_removal(removal::failure(1, true));
        let mut ui = ready(WorktreeMode::Open);
        assert!(
            ui.state
                .as_ref()
                .unwrap()
                .cache()
                .canonical_snapshot()
                .navigation
                .workspace(&target)
                .is_none()
        );
        ui.worktrees.launcher = launcher;
        draw(&mut ui, crate::localization::Locale::EnUs, 120, 40);
        if hidden {
            ui.worktrees.launcher.dismiss();
            ui.request_worktree(WorktreeMode::Open);
        } else {
            ui.dismiss_worktree(ui.worktrees.launcher.token().unwrap());
        }
        assert!(
            ui.worktrees.launcher.operation().is_none(),
            "hidden {hidden}"
        );
        assert!(!ui.worktrees.launcher.reopen());
        assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    }
}

#[test]
fn abandoning_a_retained_removal_needs_a_drawn_confirmation_and_frees_the_launcher() {
    use crate::workbench_state::worktree_launch::tests::removal;
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        // The abandon step is a secondary button whose ellipsis promises a confirmation.
        let (hint, confirm, notice) = match locale {
            // The notice's first line; the rest wraps.
            crate::localization::Locale::EnUs => (
                "[StopTracking…x]",
                "[StopTrackingEnter]",
                "The removal continues;",
            ),
            crate::localization::Locale::ZhCn => {
                ("[停止跟踪…x]", "[停止跟踪Enter]", "移除会继续进行，")
            }
        };
        let (launcher, origin, _) = failed_removal(removal::failure(1, true));
        let (mut ui, ..) = super::move_membership::ready_ui_with_snapshot(
            origin.cache.canonical_snapshot().clone(),
        );
        ui.worktrees.launcher = launcher;
        let text = draw(&mut ui, locale, 120, 40);
        assert!(text.contains(hint), "{locale:?}: {text}");
        let token = ui.worktrees.launcher.token().unwrap();
        assert!(ui.edit_worktree(token, WorktreeEdit::Append("x")));
        assert_eq!(
            ui.worktrees.launcher.view().unwrap().stage(),
            WorktreeStage::AbandonRemove
        );
        // Esc returns to the retained receipt instead of abandoning it.
        let token = ui.worktrees.launcher.token().unwrap();
        let text = draw(&mut ui, locale, 120, 40);
        assert!(
            text.contains(confirm) && text.contains(&notice.replace(' ', "")),
            "{locale:?}: {text}"
        );
        ui.dismiss_worktree(token);
        assert_eq!(
            ui.worktrees.launcher.view().unwrap().stage(),
            WorktreeStage::Result
        );
        draw(&mut ui, locale, 120, 40);
        let token = ui.worktrees.launcher.token().unwrap();
        assert!(ui.edit_worktree(token, WorktreeEdit::Append("x")));
        let token = ui.worktrees.launcher.token().unwrap();
        ui.submit_worktree(token);
        assert_eq!(
            ui.worktrees.launcher.view().unwrap().stage(),
            WorktreeStage::AbandonRemove,
            "an undrawn confirmation cannot abandon"
        );
        draw(&mut ui, locale, 120, 40);
        ui.submit_worktree(token);
        assert!(ui.worktrees.launcher.operation().is_none());
        assert!(!ui.worktrees.launcher.visible());
        assert!(!ui.worktrees.launcher.reopen());
        assert!(ui.mailbox.as_ref().unwrap().try_recv().unwrap().is_none());
    }

    // A failure that Close already releases offers no abandon step.
    let (launcher, origin, _) = failed_removal(removal::failure(1, false));
    let (mut ui, ..) =
        super::move_membership::ready_ui_with_snapshot(origin.cache.canonical_snapshot().clone());
    ui.worktrees.launcher = launcher;
    let text = draw(&mut ui, crate::localization::Locale::EnUs, 120, 40);
    assert!(!text.contains("xAbandon"));
    let token = ui.worktrees.launcher.token().unwrap();
    assert!(ui.edit_worktree(token, WorktreeEdit::Append("x")));
    assert_eq!(
        ui.worktrees.launcher.view().unwrap().stage(),
        WorktreeStage::Result
    );
}

#[test]
fn pending_removal_status_is_read_again_with_bounded_backoff_until_it_settles() {
    use crate::local_client::{LocalClient, test_support::server::FakeServer};
    use crate::workbench_state::worktree_launch::tests::removal;
    use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
    use orchbee_workbench_protocol::{
        ProtocolVersion, WorkbenchRequest, WorkbenchResponse, WorkbenchWorktreeAction,
        WorkbenchWorktreeOutcome, WorkbenchWorktreeResult, WorktreeOperationPhase,
        WorktreeOperationStatus, read_workbench_request, write_workbench_response,
    };
    use std::io::{self, Read};
    use std::sync::{atomic::Ordering, mpsc};
    use std::time::Duration;
    let pending = |phase| {
        let WorkbenchWorktreeOutcome::Available {
            project_id,
            attempt,
            status,
        } = removal::failure(1, true)
        else {
            unreachable!("fixture failure is available")
        };
        let WorktreeOperationStatus::Failed { progress, .. } = *status else {
            unreachable!("fixture failure is failed")
        };
        WorkbenchWorktreeOutcome::Available {
            project_id,
            attempt,
            status: Box::new(WorktreeOperationStatus::Pending { phase, progress }),
        }
    };
    let replies = vec![
        pending(WorktreeOperationPhase::Closing),
        pending(WorktreeOperationPhase::Removing),
        removal::failure(1, true),
    ];
    let expected = replies.clone();
    let (launcher, origin, plan) =
        failed_removal(pending(WorktreeOperationPhase::InspectingRemove));
    let operation = plan.operation_id.clone();
    let (reads, observed) = mpsc::channel();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        crate::local_client::test_support::handshake::answer_offer(&mut stream)?;
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: std::num::NonZeroU64::new(index as u64 + 1).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        if index == 0 {
            return Ok(());
        }
        for outcome in &replies {
            let WorkbenchRequest::WorktreeOperation(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("observation sends only Worktree reads")
            };
            assert_eq!(
                request.action,
                WorkbenchWorktreeAction::Read {
                    operation_id: operation.clone()
                }
            );
            let _observed = reads.send(());
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    operation_id: operation.clone(),
                    outcome: outcome.clone(),
                }),
            )
            .map_err(io::Error::other)?;
        }
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint()).unwrap();
    let (mut ui, _mailbox, _clock, elapsed) =
        super::move_membership::ready_ui_with_snapshot(origin.cache.canonical_snapshot().clone());
    ui.worktrees.set_origin(client.origin());
    ui.worktrees.launcher = launcher;
    let applied = |ui: &mut WorkbenchClientUi, outcome: &WorkbenchWorktreeOutcome| {
        let until = std::time::Instant::now() + Duration::from_secs(3);
        while ui.worktrees.launcher.operation().unwrap().outcome.as_ref() != Some(outcome) {
            assert!(std::time::Instant::now() < until, "bounded read reply");
            ui.drive_worktree();
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    // A new change subscription reads once; the Server publishes no change for Pending.
    ui.drive_worktree();
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    applied(&mut ui, &expected[0]);
    for (delay, reply) in [(250, &expected[1]), (500, &expected[2])] {
        elapsed.fetch_add(delay - 1, Ordering::SeqCst);
        ui.drive_worktree();
        assert!(
            observed.recv_timeout(Duration::from_millis(50)).is_err(),
            "read before {delay} ms"
        );
        elapsed.fetch_add(1, Ordering::SeqCst);
        ui.drive_worktree();
        observed.recv_timeout(Duration::from_secs(3)).unwrap();
        applied(&mut ui, reply);
    }
    assert!(!ui.worktrees.launcher.pending());
    assert_eq!(ui.worktrees.launcher.notice(), WorktreeNotice::Failed);
    assert!(ui.worktrees.launcher.view().unwrap().can_resume_remove());
    // A settled failure is not polled again.
    elapsed.fetch_add(10_000, Ordering::SeqCst);
    ui.drive_worktree();
    assert!(observed.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(ui.worktrees.close());
    server.finish().unwrap();
}

#[test]
fn worktree_keys_in_a_workspace_without_a_ready_view_say_why_and_start_nothing() {
    use crate::workbench_client::mailbox::EventDeadline;
    use crate::workbench_state::ClientPhaseKind;
    // Bootstrapped but never attached, like a Workspace whose terminals ended
    // while its close waits on a failed removal.
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(Arc::new(
        std::sync::atomic::AtomicU64::new(0),
    ));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_preferred_workspace(
                    &super::membership::snapshot(2, 1),
                )
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    assert_eq!(
        ui.state.as_ref().unwrap().phase_kind(),
        ClientPhaseKind::Starting
    );
    for mode in [WorktreeMode::Create, WorktreeMode::Open] {
        assert_eq!(
            ui.request_worktree(mode),
            ClientActionDisposition::KeepNavigator
        );
        assert_eq!(
            ui.view().notice(),
            Some(crate::localization::WorkbenchClientNotice::WorkspaceNotReady)
        );
        assert!(!ui.worktrees.launcher.visible());
        assert!(
            ui.state
                .as_ref()
                .unwrap()
                .pending_refresh_generation()
                .is_none()
        );
    }
}
