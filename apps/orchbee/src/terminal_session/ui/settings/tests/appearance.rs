//! Whole-frame preview, restoration and confirmed publication across the native painters.
use super::*;
use crate::{
    localization::Locale,
    tui_shell::tests::settings_frame,
    tui_theme::{ColorLevel, Role},
};
use orchbee_workbench_protocol::{
    ApplicationAppearance, PreferencesLocale, TerminalDisplayAppearance, UserPreferencesValues,
};

fn paint(
    ui: &mut NativeUi,
    shell: &mut ClientView,
    size: (u16, u16),
    locale: Locale,
) -> ratatui::buffer::Buffer {
    let (buffer, receipt) = settings_frame(size, shell, locale, || {
        TerminalSessionView::Native(ui.view())
    });
    ui.after_close_draw(receipt);
    buffer
}

#[test]
fn draft_appearance_reaches_background_chrome_and_program_defaults_without_runtime_effects() {
    for size in [(120, 40), (80, 24)] {
        for fallback in [Locale::EnUs, Locale::ZhCn] {
            for level in [
                ColorLevel::Truecolor,
                ColorLevel::Ansi256,
                ColorLevel::Ansi16,
                ColorLevel::Monochrome,
            ] {
                let mut ui = ready();
                let mut shell = ClientView::new(size.0);
                shell.configure_color_level(level);
                let values = UserPreferencesValues::default();
                shell.apply_user_preferences(values.clone());
                ui.apply_user_preferences(&values);
                let area = ratatui::layout::Rect::new(0, 0, size.0, size.1);
                let areas = crate::tui_layout::project_shell_areas(area, shell.sidebar_surface());
                let pane = crate::tui_shell::active_pane_area(
                    &areas,
                    Some(TerminalSessionView::Native(ui.view())),
                )
                .unwrap();
                let key =
                    super::super::super::tests::viewport::install_projection(&mut ui, None, 0);
                let projection = ui.projections.iter_mut().find(|p| p.key == key).unwrap();
                let mut frame = super::super::super::tests::viewport::frame(
                    &projection.value,
                    0,
                    false,
                    pane.height,
                );
                frame.geometry.columns = pane.width;
                frame.foreground = 0x102030;
                frame.background = 0xe0d0c0;
                frame.cursor.visible = false;
                let row = &mut frame.rows[0];
                row.cells[0].text = "d".into();
                row.cells[0].style.foreground = 0x1000000;
                row.cells[0].style.background = 0x1000000;
                row.cells
                    .resize(usize::from(pane.width), row.cells[0].clone());
                let mut explicit = row.cells[0].clone();
                explicit.text = "e".into();
                explicit.style.foreground = 0xabcdef;
                explicit.style.background = 0x123456;
                row.cells[usize::from(pane.width) - 1] = explicit;
                projection.value.install_frame_for_test(frame);

                let original = paint(&mut ui, &mut shell, size, fallback);
                let confirmed_shell = shell.clone();
                ui.settings.editor.edit(Edit::Select(Row::Theme));
                ui.settings.editor.edit(Edit::Adjust(true));
                ui.settings.editor.edit(Edit::Select(Row::Language));
                ui.settings
                    .editor
                    .edit(Edit::Adjust(fallback == Locale::ZhCn));
                ui.settings.editor.edit(Edit::Select(Row::CopyOnSelect));
                ui.settings.editor.edit(Edit::Adjust(true));
                ui.settings.editor.edit(Edit::Select(Row::Sound));
                ui.settings.editor.edit(Edit::Adjust(true));
                ui.settings.editor.edit(Edit::Select(Row::TerminalTheme));
                for appearance in [
                    TerminalDisplayAppearance::Light,
                    TerminalDisplayAppearance::Dark,
                    TerminalDisplayAppearance::Custom,
                    TerminalDisplayAppearance::Host,
                ] {
                    ui.settings.editor.edit(Edit::Adjust(true));
                    let preview = ui.view().settings_appearance_preview().unwrap();
                    assert_eq!(preview.terminal_display_theme, appearance);
                    let expected = shell
                        .preview_theme(preview)
                        .with_host_terminal_colors(0x102030, 0xe0d0c0);
                    let buffer = paint(&mut ui, &mut shell, size, fallback);
                    assert_eq!(buffer[(0, 0)].bg, expected.color(Role::SurfaceChrome));
                    let defaults = expected.terminal_defaults();
                    assert_eq!(buffer[(pane.right() - 2, pane.y)].symbol(), "d");
                    assert_eq!(buffer[(pane.right() - 2, pane.y)].fg, defaults.0);
                    assert_eq!(buffer[(pane.right() - 2, pane.y)].bg, defaults.1);
                    assert_eq!(buffer[(pane.right() - 1, pane.y)].symbol(), "e");
                    assert_eq!(
                        buffer[(pane.right() - 1, pane.y)].fg,
                        ratatui::style::Color::Rgb(171, 205, 239)
                    );
                    assert_eq!(
                        buffer[(pane.right() - 1, pane.y)].bg,
                        ratatui::style::Color::Rgb(18, 52, 86)
                    );
                    let preview_locale = if fallback == Locale::EnUs {
                        Locale::ZhCn
                    } else {
                        Locale::EnUs
                    };
                    assert!(
                        buffer
                            .content
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>()
                            .replace(' ', "")
                            .contains(preview_locale.settings_title())
                    );
                    assert_eq!(shell, confirmed_shell);
                    assert_eq!(ui.preferences, values);
                    assert!(ui.take_settings_snapshot().is_none());
                    assert_eq!(
                        crate::tui_layout::project_shell_areas(area, shell.sidebar_surface()),
                        areas
                    );
                }
                ui.settings.editor.edit(Edit::Cancel);
                let restored = paint(&mut ui, &mut shell, size, fallback);
                assert_eq!(restored[(0, 0)], original[(0, 0)]);
                assert_eq!(
                    restored[(pane.right() - 2, pane.y)],
                    original[(pane.right() - 2, pane.y)]
                );
                assert_eq!(shell, confirmed_shell);
            }
        }
    }
}

#[test]
fn external_confirmed_values_update_unedited_presentation_and_cancel_reveals_the_latest_snapshot() {
    let mut ui = ready();
    let mut shell = ClientView::new(120);
    shell.apply_user_preferences(UserPreferencesValues::default());
    ui.settings.editor.edit(Edit::Select(Row::Theme));
    ui.settings.editor.edit(Edit::Adjust(true));
    let mut latest = draft_basis();
    latest.source = "f".repeat(64);
    latest.values.application_theme = ApplicationAppearance::System;
    latest.values.locale = PreferencesLocale::ZhCn;
    assert!(ui.settings.observe_snapshot(latest.clone(), ui.clock.now()));
    let confirmed = ui.take_settings_snapshot().unwrap();
    ui.apply_user_preferences(&confirmed.values);
    shell.apply_user_preferences(confirmed.values);
    let preview = ui.view().settings_appearance_preview().unwrap();
    assert_eq!(preview.locale, PreferencesLocale::ZhCn);
    assert_eq!(preview.application_theme, ApplicationAppearance::Dark);
    let expected = shell.preview_theme(preview);
    let buffer = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs);
    assert_eq!(buffer[(0, 0)].bg, expected.color(Role::SurfaceChrome));
    assert!(
        buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .replace(' ', "")
            .contains(Locale::ZhCn.settings_title())
    );
    assert!(matches!(ui.settings.editor.view(), Some(View::Draft(draft))
        if draft.conflict && draft.values.application_theme == ApplicationAppearance::Dark
            && draft.basis_values == &UserPreferencesValues::default()));
    ui.settings.editor.edit(Edit::Cancel);
    assert_eq!(shell.user_preferences.as_ref(), Some(&latest.values));
    let restored = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs);
    assert_eq!(
        restored[(0, 0)].bg,
        shell.configured_theme().unwrap().color(Role::SurfaceChrome)
    );
}

#[cfg(windows)]
#[test]
fn covering_client_owner_uses_confirmed_appearance_until_settings_returns() {
    let mut ui = ready();
    let mut shell = ClientView::new(120);
    shell.apply_user_preferences(UserPreferencesValues::default());
    ui.settings.editor.edit(Edit::Select(Row::Theme));
    ui.settings.editor.edit(Edit::Adjust(true));
    let preview = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs);
    shell.machines.open = true;
    let covered = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs);
    assert_eq!(
        covered[(0, 0)].bg,
        shell.configured_theme().unwrap().color(Role::SurfaceChrome)
    );
    assert_ne!(preview[(0, 0)].bg, covered[(0, 0)].bg);
    shell.machines.open = false;
    let resumed = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs);
    assert_eq!(resumed[(0, 0)].bg, preview[(0, 0)].bg);
}
