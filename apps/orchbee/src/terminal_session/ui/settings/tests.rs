//! The shell keeps draft effects local and preserves committed/unknown receipts.
use super::*;
use crate::{
    client_view::{ClientView, KeyCode, Modifiers},
    terminal_session::{NativeDrawReceipt, TerminalSessionView},
};
use orchbee_workbench_protocol::{ConfigurationThemeName, NativeSettingsSource, WorkbenchRevision};
use ratatui::{Terminal, backend::TestBackend};
fn ready() -> NativeUi {
    let (mut ui, _, _, _) = super::super::workspace_setup_tests::ready(true);
    let token = ui.settings.editor.open().unwrap();
    assert!(ui.settings.editor.loaded(
        token,
        Basis {
            revision: WorkbenchRevision::new(1),
            source: NativeSettingsSource::parse("missing").unwrap(),
            values: Values {
                theme: ConfigurationThemeName::TerminalNative,
                reminders: true,
                sound: false
            }
        }
    ));
    ui
}
fn draft_basis() -> Basis {
    Basis {
        revision: WorkbenchRevision::new(1),
        source: NativeSettingsSource::parse("missing").unwrap(),
        values: Values {
            theme: ConfigurationThemeName::TerminalNative,
            reminders: true,
            sound: false,
        },
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
            super::super::viewport_tests::install_projection(&mut ui, Some((9, false, 24)), 0);
            let mut shell = ClientView::new(width);
            shell.machines.available = true;
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
    assert!(matches!(
        ui.settings.editor.view(),
        Some(View::Draft {
            values: Values {
                theme: ConfigurationThemeName::OneLight,
                ..
            },
            ..
        })
    ));
    ui.capture_settings_key(KeyEvent::press(KeyCode::Char('i'), Modifiers::CTRL));
    assert!(matches!(
        ui.settings.editor.view(),
        Some(View::Draft {
            selected: Row::Reminders,
            ..
        })
    ));
    ui.capture_settings_key(KeyEvent::press(
        KeyCode::Char('i'),
        Modifiers::CTRL.with_shift(),
    ));
    assert!(matches!(
        ui.settings.editor.view(),
        Some(View::Draft {
            selected: Row::Theme,
            ..
        })
    ));
    let mut release = KeyEvent::press(KeyCode::Esc, Modifiers::NONE);
    release.kind = crate::client_view::KeyKind::Release;
    ui.capture_settings_key(release);
    assert!(ui.settings_active());
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
        assert!(!ui.settings_active());
        assert_eq!(ui.settings.notice(ui.clock.now()), Some(Notice::TooSmall));
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
