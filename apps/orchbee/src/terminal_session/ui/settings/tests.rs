//! The shell keeps draft effects local and preserves committed/unknown receipts.
use super::*;
use crate::native_settings::Basis;
use crate::{
    client_view::{ClientView, KeyCode, Modifiers},
    terminal_session::{NativeDrawReceipt, TerminalSessionView},
};
use ratatui::{Terminal, backend::TestBackend};
fn ready() -> NativeUi {
    let (mut ui, _, _, _) = super::super::tests::workspace_setup::ready(true);
    let token = ui.settings.editor.open().unwrap();
    assert!(ui.settings.editor.loaded(
        token,
        Basis {
            source: "missing".into(),
            values: orchbee_workbench_protocol::UserPreferencesValues::default(),
            claimed: Default::default(),
            desktop_imported: false,
            desktop_main_imported: false
        }
    ));
    ui
}
fn draft_basis() -> Basis {
    Basis {
        source: "missing".into(),
        values: orchbee_workbench_protocol::UserPreferencesValues::default(),
        claimed: Default::default(),
        desktop_imported: false,
        desktop_main_imported: false,
    }
}
#[test]
fn ux_status_settings_replaces_old_notice_and_machine_cells() {
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
            #[cfg(windows)]
            {
                shell.machines.available = true;
            }
            shell.startup_notice(crate::client_view::Notice::UnknownPrefix);
            let theme = crate::tui_theme::Theme::one_light();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    let mut hits = crate::tui_pointer::HitMapBuilder::new();
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &shell,
                        locale,
                        Some(TerminalSessionView::Native(ui.view())),
                        &theme,
                        &mut hits,
                    );
                    #[cfg(windows)]
                    shell.machines.render(frame, locale, &theme);
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
            assert_eq!(
                row.replace(' ', ""),
                locale
                    .settings_hint(ui.view().settings().unwrap(), ui.view().settings_pending())
                    .replace(' ', "")
            );
            assert_eq!(
                terminal.backend().buffer()[(width - 1, height - 1)].bg,
                theme.color(crate::tui_theme::Role::SurfaceChrome)
            );
            assert!(!terminal.backend().cursor_visible());
        }
    }
}
#[test]
fn settings_enter_tab_and_release_follow_canonical_host_keys() {
    let mut ui = ready();
    ui.capture_settings_key(KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL));
    assert!(
        matches!(ui.settings.editor.view(), Some(View::Draft(draft)) if draft.values.locale == orchbee_workbench_protocol::PreferencesLocale::EnUs)
    );
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
    let mut release = KeyEvent::press(KeyCode::Esc, Modifiers::NONE);
    release.kind = crate::client_view::KeyKind::Release;
    ui.capture_settings_key(release);
    assert!(ui.settings_active());
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
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &shell,
                        locale,
                        Some(TerminalSessionView::Native(ui.view())),
                        &crate::tui_theme::Theme::one_light(),
                        &mut crate::tui_pointer::HitMapBuilder::new(),
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
                locale.settings_notice(notice).replace(' ', "")
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
        let mut latest = draft_basis();
        latest.source = "c".repeat(64);
        latest.values.locale = orchbee_workbench_protocol::PreferencesLocale::ZhCn;
        assert!(ui.settings.observe_snapshot(latest.clone(), ui.clock.now()));
        assert_eq!(
            ui.settings.notice(ui.clock.now()),
            Some(notice.unwrap_or(Notice::SourceChanged))
        );
        assert!(
            matches!(ui.settings.editor.view(), Some(View::Draft(draft)) if draft.values.locale == orchbee_workbench_protocol::PreferencesLocale::EnUs)
        );
        assert_eq!(ui.take_settings_snapshot(), Some(latest.clone()));
        assert!(!ui.settings.observe_snapshot(latest, ui.clock.now()));
    }
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
        let mut receipt = NativeDrawReceipt::default();
        terminal
            .draw(|frame| {
                receipt = crate::tui_render::render_terminal_shell(
                    frame,
                    &shell,
                    locale,
                    Some(TerminalSessionView::Native(ui.view())),
                    &crate::tui_theme::Theme::terminal_native(),
                    &mut crate::tui_pointer::HitMapBuilder::new(),
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
        assert_eq!(ui.settings.notice(ui.clock.now()), Some(Notice::TooSmall));
        assert!(ui.settings.editor.open().is_none());
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
