//! The shell keeps draft effects local and preserves committed/unknown receipts.
use super::test_support::{draft_basis, ready, settings_snapshot};
use super::*;
use crate::{
    client_view::{ClientView, KeyCode, Modifiers},
    tui_presentation::PresentationPhase,
    workbench_client::DrawReceipt,
};
use ratatui::{Terminal, backend::TestBackend};

fn present_settings(ui: &mut WorkbenchClientUi) {
    let shell = ClientView::new(120);
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut receipt = DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &shell,
                crate::localization::Locale::EnUs,
                Some(ui.view()),
                &crate::tui_theme::Theme::terminal_native(),
                &mut crate::tui_hit_map::HitMapBuilder::new(),
            );
        })
        .unwrap();
    ui.after_close_draw(receipt);
}

#[test]
fn settings_input_context_is_projected_once_from_the_current_client_view() {
    let mut ui = ready();
    let presented = ui.view().settings_input_context().unwrap();
    assert_eq!(ui.settings_input_context(), Some(presented));
    assert_eq!(
        presented.readiness,
        crate::native_settings::SettingsInputReadiness::Interactive
    );

    ui.after_settings_draw(None);
    let recovery = ui.view().settings_input_context().unwrap();
    assert_eq!(ui.settings_input_context(), Some(recovery));
    assert_eq!(recovery.token, presented.token);
    assert_eq!(recovery.phase, PresentationPhase::Admitted);
    assert_eq!(
        recovery.readiness,
        crate::native_settings::SettingsInputReadiness::Recovery
    );
    assert_eq!(recovery.remaining_text_bytes, 0);

    let Some(View::Draft(draft)) = ui.settings.editor.view() else {
        panic!("retained draft");
    };
    let draw = draft.presentation(4);
    ui.after_settings_draw(Some(ModalPresentation::Presented(draw)));
    assert_eq!(
        ui.settings_input_context().unwrap().readiness,
        crate::native_settings::SettingsInputReadiness::Interactive
    );
}

#[test]
fn settings_input_guards_consume_recovery_and_reject_expired_snapshots() {
    let mut ui = ready();
    let selected = ui.settings_input_context().unwrap();
    assert!(
        ui.capture_settings_key_for(selected, KeyEvent::press(KeyCode::Right, Modifiers::NONE))
    );
    let current = ui.settings_input_context().unwrap();
    assert_ne!(current.token, selected.token);
    let Some(View::Draft(after_first)) = ui.settings.editor.view() else {
        panic!("draft");
    };
    let values = after_first.values.clone();
    assert!(
        !ui.capture_settings_key_for(selected, KeyEvent::press(KeyCode::Right, Modifiers::NONE))
    );
    assert!(matches!(
        ui.settings.editor.view(),
        Some(View::Draft(draft)) if draft.token == current.token && draft.values == &values
    ));

    ui.after_settings_draw(None);
    let recovery = ui.settings_input_context().unwrap();
    assert_eq!(
        recovery.readiness,
        crate::native_settings::SettingsInputReadiness::Recovery
    );
    assert!(ui.capture_settings_key_for(recovery, KeyEvent::press(KeyCode::Down, Modifiers::NONE)));
    assert!(ui.capture_settings_paste_for(recovery, Some("ignored")));
    assert!(matches!(
        ui.settings.editor.view(),
        Some(View::Draft(draft)) if draft.token == recovery.token && draft.values == &values
    ));
    assert!(ui.capture_settings_key_for(recovery, KeyEvent::press(KeyCode::Esc, Modifiers::NONE)));
    assert!(ui.settings_input_context().is_none());
}

#[test]
fn settings_paste_guard_uses_the_current_interactive_text_snapshot() {
    let mut ui = ready();
    ui.settings
        .editor
        .edit(Edit::Select(Row::CompletionThreshold));
    let row = ui.settings_input_context().unwrap();
    assert!(ui.capture_settings_key_for(row, KeyEvent::press(KeyCode::Enter, Modifiers::NONE)));
    let text = ui.settings_input_context().unwrap();
    assert!(text.remaining_text_bytes > 0);
    assert!(ui.capture_settings_paste_for(text, Some("1")));
    let changed = ui.settings_input_context().unwrap();
    assert_ne!(changed.token, text.token);
    assert_eq!(changed.remaining_text_bytes, text.remaining_text_bytes - 1);
    assert!(!ui.capture_settings_paste_for(text, Some("2")));
    assert_eq!(ui.settings_input_context(), Some(changed));
}

#[test]
fn settings_owns_its_hints_without_repeating_them_in_global_status() {
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        for (width, height) in [(120, 40), (80, 24)] {
            let mut ui = ready();
            ui.observe_geometry(
                orchbee_client_server_protocol::Geometry::new(width, height, 8, 16).unwrap(),
            );
            super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
            let mut shell = ClientView::new(width);
            {
                shell.machines.available = true;
            }
            shell.startup_notice(crate::client_view::Notice::UnknownPrefix);
            let theme = crate::tui_theme::Theme::orchbee_light();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    let mut hits = crate::tui_hit_map::HitMapBuilder::new();
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &shell,
                        locale,
                        Some(ui.view()),
                        &theme,
                        &mut hits,
                    );
                    shell.machines.render(frame, locale, &theme, &mut hits);
                })
                .unwrap();
            let row: String = (crate::tui_layout::project_shell_areas(
                ratatui::layout::Rect::new(0, 0, width, height),
                shell.sidebar_surface(),
            )
            .status
            .x..width)
                .map(|x| terminal.backend().buffer()[(x, height - 1)].symbol())
                .collect();
            // Settings may reach the last row; no global status repeats its hints there.
            let notice = locale.shell_notice(Some(crate::client_view::Notice::UnknownPrefix));
            assert!(
                !row.replace(' ', "").contains(&notice.replace(' ', "")),
                "{row:?}"
            );
            assert!(!row.contains("Ctrl+"), "{row:?}");
            assert!(!terminal.backend().cursor_visible());
        }
    }
}

#[test]
fn captures_whole_shell_settings_alignment_frames() {
    for (width, height, locale, theme, name) in [
        (
            120,
            40,
            crate::localization::Locale::EnUs,
            crate::tui_theme::Theme::resolve(
                (&orchbee_workbench_protocol::UserSettingsValues::default()).into(),
                None,
                crate::tui_theme::ColorLevel::Truecolor,
            ),
            "alignment-settings-draft-en-us-120x40-default",
        ),
        (
            58,
            12,
            crate::localization::Locale::ZhCn,
            crate::tui_theme::Theme::terminal_native(),
            "alignment-settings-narrow-zh-cn-58x12-terminal-native",
        ),
    ] {
        let mut ui = ready();
        ui.observe_geometry(
            orchbee_client_server_protocol::Geometry::new(width, height, 8, 16).unwrap(),
        );
        super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
        let shell = ClientView::new(width);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut receipt = DrawReceipt::default();
        terminal
            .draw(|frame| {
                receipt = crate::tui_render::render_terminal_shell(
                    frame,
                    &shell,
                    locale,
                    Some(ui.view()),
                    &theme,
                    &mut crate::tui_hit_map::HitMapBuilder::new(),
                );
            })
            .unwrap();
        ui.after_close_draw(receipt);
        assert!(ui.settings_active());
        let Some(View::Draft(draft)) = ui.settings.editor.view() else {
            panic!("presented Settings draft");
        };
        let token = draft.token;
        assert_eq!(
            ui.settings.editor.scroll(token, true, 1),
            Some(token),
            "a no-op upward scroll proves the current draft frame was admitted"
        );
        crate::tui_render::test_capture::capture_screen(&mut terminal, name, locale, &theme);
    }
}

#[test]
fn settings_enter_tab_and_release_follow_canonical_host_keys() {
    use orchbee_workbench_protocol::SettingsLocale;

    let mut ui = ready();
    present_settings(&mut ui);
    ui.capture_settings_key(KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL));
    assert!(matches!(ui.settings.editor.view(), Some(View::Draft(draft))
            if draft.values.locale == SettingsLocale::System
                && !draft.dirty
                && draft.picker.is_some_and(|picker| picker.row() == Row::Language && picker.cursor() == 0)));
    present_settings(&mut ui);
    ui.capture_settings_key(KeyEvent::press(KeyCode::Char('i'), Modifiers::CTRL));
    assert_eq!(
        ui.settings.editor.appearance_preview().unwrap().locale,
        SettingsLocale::EnUs
    );
    assert!(matches!(ui.settings.editor.view(), Some(View::Draft(draft))
            if draft.selected == Row::Language && draft.values.locale == SettingsLocale::System && !draft.dirty));
    let mut enter_release = KeyEvent::press(KeyCode::Enter, Modifiers::NONE);
    enter_release.kind = crate::client_view::KeyKind::Release;
    ui.capture_settings_key(enter_release);
    assert!(
        matches!(ui.settings.editor.view(), Some(View::Draft(draft)) if draft.picker.is_some() && !draft.dirty)
    );
    ui.capture_settings_key(KeyEvent::press(
        KeyCode::Char('i'),
        Modifiers::CTRL.with_shift(),
    ));
    assert_eq!(
        ui.settings.editor.appearance_preview().unwrap().locale,
        SettingsLocale::System
    );
    ui.capture_settings_key(KeyEvent::press(KeyCode::Char('i'), Modifiers::CTRL));
    present_settings(&mut ui);
    ui.capture_settings_key(KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL));
    assert!(matches!(ui.settings.editor.view(), Some(View::Draft(draft))
        if draft.picker.is_none() && draft.dirty && draft.values.locale == SettingsLocale::EnUs));
    present_settings(&mut ui);
    ui.capture_settings_key(KeyEvent::press(KeyCode::Char('i'), Modifiers::CTRL));
    assert!(
        matches!(ui.settings.editor.view(), Some(View::Draft(draft)) if draft.selected == Row::Theme)
    );
    ui.capture_settings_key(KeyEvent::press(
        KeyCode::Char('i'),
        Modifiers::CTRL.with_shift(),
    ));
    assert!(
        matches!(ui.settings.editor.view(), Some(View::Draft(draft)) if draft.selected == Row::Language)
    );
    let before_release = ui.settings.editor.view().unwrap().token();
    let mut release = KeyEvent::press(KeyCode::Esc, Modifiers::NONE);
    release.kind = crate::client_view::KeyKind::Release;
    ui.capture_settings_key(release);
    assert!(ui.settings_active());
    assert_eq!(ui.settings.editor.view().unwrap().token(), before_release);
}
#[test]
fn conflict_and_unknown_receipts_stay_visible_while_the_user_inspects_the_retained_draft() {
    for notice in [Notice::SourceChanged, Notice::Unknown] {
        let mut ui = ready();
        ui.settings.note(notice, ui.clock.now());
        ui.capture_settings_key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
        assert_eq!(ui.settings.notice(ui.clock.now()), Some(notice));
        assert!(ui.settings_active());
    }
}
#[test]
fn retained_drafts_render_their_failure_instead_of_covering_it_with_key_hints() {
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        for notice in [Notice::SourceChanged, Notice::Unknown, Notice::Failed] {
            let mut ui = ready();
            ui.settings.note(notice, ui.clock.now());
            let shell = ClientView::new(120);
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            let theme = crate::tui_theme::Theme::orchbee_light();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &shell,
                        locale,
                        Some(ui.view()),
                        &theme,
                        &mut crate::tui_hit_map::HitMapBuilder::new(),
                    );
                })
                .unwrap();
            let status = crate::tui_layout::project_shell_areas(
                ratatui::layout::Rect::new(0, 0, 120, 40),
                shell.sidebar_surface(),
            )
            .status;
            let row: String = (status.x..status.right())
                .map(|x| terminal.backend().buffer()[(x, 39)].symbol())
                .collect();
            assert_eq!(
                row.replace(' ', ""),
                format!(
                    "{} {}",
                    theme.glyphs.status_bullet,
                    locale.settings_notice(notice)
                )
                .replace(' ', "")
            );
            let marker_role = match notice {
                Notice::Failed => crate::tui_theme::Role::StatusError,
                Notice::SourceChanged | Notice::Unknown => crate::tui_theme::Role::StatusWarning,
                _ => unreachable!(),
            };
            let marker = (status.x..status.right())
                .find(|x| {
                    terminal.backend().buffer()[(*x, status.y)].symbol()
                        == theme.glyphs.status_bullet
                })
                .expect("the notice floats as the status chip");
            assert_eq!(
                terminal.backend().buffer()[(marker, status.y)].fg,
                theme.color(marker_role),
                "the notice marker conveys its severity"
            );
            assert_eq!(
                terminal.backend().buffer()[(marker + 2, status.y)].fg,
                theme.color(crate::tui_theme::Role::TextPrimary),
                "the message keeps the primary reading color"
            );
        }
    }
}
#[test]
fn observation_publishes_live_values_without_erasing_dirty_drafts_or_unknown_receipts() {
    for notice in [None, Some(Notice::Unknown)] {
        let mut ui = ready();
        ui.settings.editor.edit(Edit::Adjust(true));
        if let Some(notice) = notice {
            ui.settings.note(notice, ui.clock.now());
        }
        let mut values = draft_basis().values;
        values.locale = orchbee_workbench_protocol::SettingsLocale::ZhCn;
        let latest = settings_snapshot(2, &values);
        assert!(ui.settings.observe_snapshot(latest.clone(), ui.clock.now()));
        assert_eq!(
            ui.settings.notice(ui.clock.now()),
            Some(notice.unwrap_or(Notice::SourceChanged))
        );
        assert!(matches!(ui.settings.editor.view(), Some(View::Draft(draft))
                if draft.values.locale == orchbee_workbench_protocol::SettingsLocale::EnUs
                    && draft.basis_values.locale == orchbee_workbench_protocol::SettingsLocale::System
                    && draft.effective_values.locale == orchbee_workbench_protocol::SettingsLocale::ZhCn
                    && draft.conflict));
        assert_eq!(ui.take_settings_snapshot(), Some(latest.clone()));
        assert!(!ui.settings.observe_snapshot(latest, ui.clock.now()));
    }
}

#[test]
fn coordinator_maps_unknown_save_to_a_fresh_read_requirement() {
    let mut ui = ready();
    ui.settings.editor.edit(Edit::Select(Row::Sound));
    ui.settings.editor.edit(Edit::Activate);
    ui.settings.editor.edit(Edit::Select(Row::Save));
    let token = ui.settings.editor.view().unwrap().token();
    ui.settings.editor.draw(token);
    let submission = ui.settings.editor.edit(Edit::Activate).unwrap();

    ui.fail_settings(submission.token, Notice::Unknown);
    assert_eq!(ui.settings.notice(ui.clock.now()), Some(Notice::Unknown));
    assert!(matches!(
        ui.settings.editor.view(),
        Some(View::Draft(draft))
            if draft.reconciliation == Some(Reconciliation::OutcomeUnknown)
                && !draft.enabled(Row::Save)
    ));

    // The unknown Save may have landed: a later snapshot carries its value.
    let mut passive = draft_basis().values;
    passive.sound_enabled = !passive.sound_enabled;
    assert!(
        ui.settings
            .observe_snapshot(settings_snapshot(2, &passive), ui.clock.now())
    );
    assert!(matches!(
        ui.settings.editor.view(),
        Some(View::Draft(draft))
            if draft.reconciliation == Some(Reconciliation::OutcomeUnknown)
                && draft.conflict
                && !draft.enabled(Row::Save)
    ));
}
#[test]
fn settings_draft_cancel_and_small_draw_have_no_live_application() {
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let mut ui = ready();
        let shell = ClientView::new(80);
        let before = shell.clone();
        ui.capture_settings_key(KeyEvent::press(KeyCode::Right, Modifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut receipt = DrawReceipt::default();
        terminal
            .draw(|frame| {
                receipt = crate::tui_render::render_terminal_shell(
                    frame,
                    &shell,
                    locale,
                    Some(ui.view()),
                    &crate::tui_theme::Theme::terminal_native(),
                    &mut crate::tui_hit_map::HitMapBuilder::new(),
                );
            })
            .unwrap();
        ui.after_close_draw(receipt);
        assert_eq!(shell, before);
        assert!(ui.take_settings_snapshot().is_none());
        ui.after_settings_draw(Some(ModalPresentation::NotPresented));
        assert!(
            ui.settings_active(),
            "a small frame retains the draft without arming it"
        );
        assert_eq!(
            ui.settings.editor.presentation(),
            PresentationPhase::Suspended
        );
        assert_eq!(ui.settings.notice(ui.clock.now()), Some(Notice::TooSmall));
        assert!(ui.settings.editor.open().is_none());

        let mut resumed = DrawReceipt::default();
        terminal
            .draw(|frame| {
                resumed = crate::tui_render::render_terminal_shell(
                    frame,
                    &shell,
                    locale,
                    Some(ui.view()),
                    &crate::tui_theme::Theme::terminal_native(),
                    &mut crate::tui_hit_map::HitMapBuilder::new(),
                );
            })
            .unwrap();
        ui.after_close_draw(resumed);
        assert_eq!(
            ui.settings.editor.presentation(),
            PresentationPhase::Admitted
        );
        assert!(ui.settings.notice(ui.clock.now()).is_none());
        assert!(matches!(
            ui.settings.editor.view(),
            Some(View::Draft(draft)) if draft.dirty
        ));
    }
}

#[test]
fn initial_too_small_settings_is_refused_without_cancelling_its_read() {
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let (mut ui, _, _, _) = super::super::tests::workspace_setup::ready(true);
        let token = ui.settings.editor.open().unwrap();
        assert_eq!(
            ui.view().settings_presentation(),
            Some(PresentationPhase::Pending)
        );
        let shell = ClientView::new(120);
        let mut terminal = Terminal::new(TestBackend::new(120, 9)).unwrap();
        let mut receipt = DrawReceipt::default();
        let mut hits = crate::tui_hit_map::HitMapBuilder::new();
        terminal
            .draw(|frame| {
                receipt = crate::tui_render::render_terminal_shell(
                    frame,
                    &shell,
                    locale,
                    Some(ui.view()),
                    &crate::tui_theme::Theme::terminal_native(),
                    &mut hits,
                );
            })
            .unwrap();
        let map = hits.finish();
        assert!((0..9).all(|y| {
            (0..120).all(|x| {
                !matches!(
                    map.hit_target_at(x, y),
                    Some(
                        crate::tui_hit_map::HitTarget::SettingsRow { .. }
                            | crate::tui_hit_map::HitTarget::SettingsControl { .. }
                    )
                )
            })
        }));
        ui.after_close_draw(receipt);

        assert!(!ui.settings_active());
        assert_eq!(ui.view().settings_presentation(), None);
        assert_eq!(
            ui.settings.notice(ui.clock.now()),
            Some(Notice::PresentationUnavailable)
        );
        let mut candidate = None;
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &shell,
                    locale,
                    Some(ui.view()),
                    &crate::tui_theme::Theme::terminal_native(),
                    &mut crate::tui_hit_map::HitMapBuilder::new(),
                );
                candidate = Some(frame.buffer_mut().clone());
            })
            .unwrap();
        let candidate = candidate.expect("settled Settings frame");
        let candidate_status: String = (0..120).map(|x| candidate[(x, 8)].symbol()).collect();
        let expected = locale
            .settings_notice(Notice::PresentationUnavailable)
            .replace(' ', "");
        assert!(
            candidate_status.replace(' ', "").contains(&expected),
            "the settled candidate must carry the localized refusal: {candidate_status:?}"
        );
        let hidden_by_candidate_wide_glyph = |x: u16| {
            (0..x).rev().take(4).any(|start| {
                let width = crate::terminal_text::display_width(candidate[(start, 8)].symbol());
                width > 1 && usize::from(start) + width > usize::from(x)
            })
        };
        let sink = terminal.backend().buffer();
        let mut hidden_differences = Vec::new();
        for x in 0..120 {
            let candidate_cell = &candidate[(x, 8)];
            let sink_cell = &sink[(x, 8)];
            if candidate_cell != sink_cell {
                assert!(
                    hidden_by_candidate_wide_glyph(x),
                    "the TestBackend sink diverged at visible cell {x}: candidate={candidate_cell:?} sink={sink_cell:?}"
                );
                assert_eq!(candidate_cell.symbol(), " ");
                hidden_differences.push(x);
            }
        }
        if locale == crate::localization::Locale::ZhCn {
            assert!(
                !hidden_differences.is_empty(),
                "the locked TestBackend must expose its CJK trailing-cell limitation"
            );
        } else {
            assert!(hidden_differences.is_empty());
        }
        let visible_sink_status: String = (0..120)
            .filter(|x| !hidden_by_candidate_wide_glyph(*x))
            .map(|x| sink[(x, 8)].symbol())
            .collect();
        assert!(
            visible_sink_status.replace(' ', "").contains(&expected),
            "visible sink cells must retain the localized refusal: {visible_sink_status:?}"
        );
        assert!(
            ui.settings.editor.open().is_none(),
            "the accepted read still owns the coordinator"
        );
        assert!(!ui.settings.editor.loaded(token, draft_basis()));
        assert!(ui.settings.editor.open().is_some());
    }
}
#[test]
fn settings_pending_threshold_and_hidden_completion_do_not_reopen_a_surface() {
    let mut ui = ready();
    ui.settings.editor.edit(Edit::Cancel);
    let token = ui.settings.editor.open().unwrap();
    for (millis, expected) in [(199, false), (200, true), (201, true)] {
        ui.settings.started = ui.clock.now().checked_sub(Duration::from_millis(millis));
        ui.drive_settings();
        assert_eq!(ui.settings.pending, expected);
    }
    ui.capture_settings_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!ui.settings_active());
    assert!(ui.settings.editor.open().is_none());
    assert!(!ui.settings.editor.loaded(token, draft_basis()));
    assert!(!ui.settings_active());
    assert!(ui.settings.editor.open().is_some());
    ui.settings.note(Notice::Unknown, ui.clock.now());
    assert_eq!(
        ui.settings
            .notice(ui.clock.now() + Duration::from_secs(3600)),
        Some(Notice::Unknown)
    );
    ui.clear_settings_notice();
    assert!(ui.settings.notice(ui.clock.now()).is_none());
}
