//! Every common row remains reachable and clickable at the smallest admitted geometry.
use super::*;
use crate::native_settings::{Basis, Edit, Editor, Page};
use orchbee_workbench_protocol::UserPreferencesValues;
use ratatui::{Terminal, backend::TestBackend};
fn draft() -> Editor {
    let mut editor = Editor::default();
    let token = editor.open().unwrap();
    assert!(editor.loaded(
        token,
        Basis {
            source: "missing".into(),
            values: UserPreferencesValues::default(),
            claimed: Default::default(),
            desktop_imported: false,
            desktop_main_imported: false,
        }
    ));
    editor.edit(Edit::Adjust(true));
    editor
}
#[test]
fn settings_scrolls_every_common_row_into_a_frame_bound_target_in_both_locales_and_themes() {
    for (width, height) in [(36, 10), (58, 10), (80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
                Theme::one_light().quantize(crate::tui_theme::ColorLevel::Monochrome),
            ] {
                let mut editor = draft();
                for &row in Page::General.rows() {
                    editor.edit(Edit::Select(row));
                    let view = editor.view().unwrap();
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let mut receipt = ModalPresentation::NotPresented;
                    terminal
                        .draw(|frame| {
                            receipt =
                                render(frame, frame.area(), view, false, locale, &theme, &mut hits);
                        })
                        .unwrap();
                    assert_eq!(receipt, ModalPresentation::Presented(view.token()));
                    let map = hits.finish();
                    let target = HitTarget::SettingsRow {
                        token: view.token(),
                        row,
                    };
                    assert_eq!(
                        (0..height)
                            .any(|y| (0..width).any(|x| map.hit_target_at(x, y) == Some(&target))),
                        matches!(view, View::Draft(draft) if draft.enabled(row)),
                        "selected {row:?} is reachable at {width}x{height}"
                    );
                    let text: String = terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect();
                    assert!(text.contains("Esc"));
                }
            }
        }
    }
}
#[test]
fn text_cursor_stays_inside_the_overlay_and_editor_text_is_sanitized() {
    let mut editor = draft();
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::ProfileName));
    editor.edit(Edit::Activate);
    editor.paste(&format!("{}\x1b[31m", "长名".repeat(50)));
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| {
            render(
                frame,
                frame.area(),
                editor.view().unwrap(),
                false,
                Locale::ZhCn,
                &Theme::one_light(),
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(!text.contains('\u{1b}'));
    assert!(terminal.backend().cursor_visible());
}
#[test]
fn settings_refuses_incomplete_geometry_and_delays_pending_copy() {
    let editor = draft();
    for (width, height) in [(35, 10), (36, 9), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut result = ModalPresentation::Presented(editor.view().unwrap().token());
        terminal
            .draw(|frame| {
                result = render(
                    frame,
                    frame.area(),
                    editor.view().unwrap(),
                    false,
                    Locale::EnUs,
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        assert_eq!(result, ModalPresentation::NotPresented);
    }
    let mut editor = Editor::default();
    editor.open().unwrap();
    for pending in [false, true] {
        let mut terminal = Terminal::new(TestBackend::new(58, 10)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    editor.view().unwrap(),
                    pending,
                    Locale::EnUs,
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert_eq!(text.contains("[..] Opening"), pending);
    }
}
