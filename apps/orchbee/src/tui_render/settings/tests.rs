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
fn profile_arrows_and_child_buttons_are_disjoint_complete_controls_at_small_geometry() {
    for (width, height) in [(36, 10), (58, 12), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::one_dark(),
                Theme::one_light(),
                Theme::terminal_native().quantize(crate::tui_theme::ColorLevel::Monochrome),
            ] {
                let mut editor = draft();
                editor.edit(Edit::Select(Row::LaunchProfiles));
                editor.edit(Edit::Activate);
                editor.edit(Edit::Select(Row::AddProfile));
                editor.edit(Edit::Activate);
                for child in [false, true] {
                    editor.edit(Edit::Select(if child {
                        Row::ProfileName
                    } else {
                        Row::Profile
                    }));
                    if child {
                        editor.edit(Edit::Activate);
                    }
                    let view = editor.view().unwrap();
                    let token = view.token();
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            assert_eq!(
                                render(frame, frame.area(), view, false, locale, &theme, &mut hits),
                                ModalPresentation::Presented(token)
                            );
                        })
                        .unwrap();
                    let map = hits.finish();
                    let controls = if child {
                        [Control::Back, Control::Accept]
                    } else {
                        [
                            Control::Adjust {
                                row: Row::Profile,
                                forward: false,
                            },
                            Control::Adjust {
                                row: Row::Profile,
                                forward: true,
                            },
                        ]
                    };
                    for control in controls.into_iter().chain([Control::Close]) {
                        let target = HitTarget::SettingsControl { token, control };
                        let cells: Vec<_> = (0..height)
                            .flat_map(|y| (0..width).map(move |x| (x, y)))
                            .filter(|&(x, y)| map.hit_target_at(x, y) == Some(&target))
                            .collect();
                        assert!(
                            cells.len() >= 3,
                            "{control:?} at {width}x{height} {locale:?}"
                        );
                        let text: String = cells
                            .iter()
                            .map(|&(x, y)| terminal.backend().buffer()[(x, y)].symbol())
                            .collect();
                        assert!(
                            text.starts_with('[') && text.ends_with(']'),
                            "complete button {text:?}"
                        );
                    }
                    assert_eq!(
                        (0..height).any(|y| (0..width).any(|x| matches!(
                            map.hit_target_at(x, y),
                            Some(HitTarget::SettingsRow { .. })
                        ))),
                        !child,
                        "a child editor cannot expose underlying actions"
                    );
                }
            }
        }
    }
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
fn assert_projected_text_cursor(raw: &str, glyph: &str, right_column: u16, end_column: u16) {
    let mut editor = draft();
    editor.edit(Edit::Select(Row::LaunchProfiles));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Select(Row::ProfileName));
    editor.edit(Edit::Activate);
    editor.edit(Edit::Clear);
    editor.paste(raw);
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    for (edit, column) in [
        (Edit::Home, 0),
        (Edit::Adjust(true), right_column),
        (Edit::End, end_column),
    ] {
        editor.edit(edit);
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    editor.view().unwrap(),
                    false,
                    Locale::EnUs,
                    &Theme::one_light(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let index = buffer
            .content
            .iter()
            .position(|cell| cell.symbol() == glyph)
            .expect("the complete projected grapheme is painted");
        let origin = ratatui::layout::Position::new((index % 40) as u16, (index / 40) as u16);
        assert_eq!(
            terminal.get_cursor_position().unwrap(),
            ratatui::layout::Position::new(origin.x + column, origin.y),
            "cursor after {edit:?} for raw {raw:?}"
        );
        assert!(terminal.backend().cursor_visible());
        assert!(
            matches!(editor.view(), Some(View::Draft(view)) if view.text.is_some_and(|(text, _)| text == raw))
        );
    }
}

#[test]
fn settings_text_cursor_snaps_before_a_combining_cluster_joined_by_stripping_controls() {
    assert_projected_text_cursor("a\u{7}\u{301}", "a\u{301}", 0, 1);
}

#[test]
fn settings_text_cursor_snaps_before_an_emoji_cluster_joined_by_stripping_ansi() {
    assert_projected_text_cursor("\u{2764}\x1b[0m\u{fe0f}", "\u{2764}\u{fe0f}", 0, 2);
}

#[test]
fn settings_text_cursor_preserves_normal_wide_grapheme_and_end_positions() {
    assert_projected_text_cursor("界Z", "界", 2, 3);
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
