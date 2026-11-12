//! The narrow tier, page admission and the inline text field's projection of raw text.
use crate::{
    localization::Locale,
    native_settings::{Category, Control, Edit, Editor, Pane, Row, View},
    tui_hit_map::HitMapBuilder,
    tui_presentation::ModalPresentation,
    tui_theme::Theme,
};
use orchbee_workbench_protocol::{
    SettingsFile, SettingsPlatform, SettingsSnapshot, UserSettingsValues, WorkbenchRevision,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn snapshot() -> SettingsSnapshot {
    let values = UserSettingsValues::default();
    SettingsSnapshot {
        revision: WorkbenchRevision::new(1),
        file_path: None,
        file: SettingsFile::Ok {},
        platform: SettingsPlatform::Linux,
        values: Default::default(),
        set: Vec::new(),
        launch_profiles: values.launch_profiles,
        diagnostics: Default::default(),
    }
}

/// Paints the page and reports the frame to the editor, as the shell does.
fn paint(editor: &mut Editor, size: (u16, u16), locale: Locale) -> String {
    text(present(editor, size, locale).backend().buffer())
}

fn present(editor: &mut Editor, size: (u16, u16), locale: Locale) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
    let theme = Theme::orchbee_light();
    let mut hits = HitMapBuilder::default();
    let mut presented = None;
    terminal
        .draw(|frame| {
            let area = frame.area();
            presented = super::super::render_settings_for_test(
                frame,
                area,
                editor.view().unwrap(),
                false,
                locale,
                &theme,
                &mut hits,
            )
            .into_presented();
        })
        .unwrap();
    editor.presented(presented.expect("the page fits"));
    terminal
}

/// The painted text without spaces, so a wide character's trailing cell does not split it.
fn text(buffer: &Buffer) -> String {
    buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
        .replace(' ', "")
}

fn has(text: &str, needle: &str) -> bool {
    text.contains(&needle.replace(' ', ""))
}

#[test]
fn below_the_narrow_width_the_page_shows_one_column_at_a_time() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (size, narrow) in [((120, 36), false), ((50, 20), true)] {
            let mut editor = Editor::default();
            let token = editor.open().unwrap();
            assert!(editor.loaded(token, &snapshot()));
            let first = paint(&mut editor, size, locale);
            let row = locale.settings_row(crate::native_settings::Row::Language);
            let other = locale.settings_category_name(Category::Notifications);
            assert!(has(&first, other), "{size:?}: categories show first:");
            assert_eq!(has(&first, row), !narrow, "{size:?}, {locale:?}");

            let View::Page(page) = editor.view().unwrap() else {
                panic!("the page is open");
            };
            let token = page.token();
            editor.control(token, Control::Category(Category::General));
            let rows = paint(&mut editor, size, locale);
            assert!(has(&rows, row), "{size:?}, {locale:?}");
            assert!(
                has(
                    &rows,
                    locale.settings_description(crate::native_settings::Row::Language)
                ),
                "descriptions stay visible: {size:?}, {locale:?}"
            );
            assert_eq!(has(&rows, other), !narrow, "{size:?}, {locale:?}");
            let View::Page(page) = editor.view().unwrap() else {
                panic!("the page is open");
            };
            assert_eq!(page.narrow(), narrow);
            assert_eq!(
                page.pane(),
                if narrow { Pane::Rows } else { Pane::Categories }
            );
        }
    }
}

#[test]
fn a_viewport_below_the_page_minimum_is_not_presented() {
    let mut editor = Editor::default();
    let token = editor.open().unwrap();
    assert!(editor.loaded(token, &snapshot()));
    let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();
    let theme = Theme::orchbee_light();
    let mut hits = HitMapBuilder::default();
    terminal
        .draw(|frame| {
            let area = frame.area();
            assert!(matches!(
                super::super::render_settings_for_test(
                    frame,
                    area,
                    editor.view().unwrap(),
                    false,
                    Locale::EnUs,
                    &theme,
                    &mut hits,
                ),
                ModalPresentation::NotPresented
            ));
        })
        .unwrap();
}

/// Pasted controls and escape sequences never paint, and the cursor lands on a whole grapheme of
/// the projected text while the raw text stays exact.
#[test]
fn the_text_field_cursor_lands_on_whole_projected_graphemes() {
    for (raw, glyph, right_column, end_column) in [
        ("a\u{7}\u{301}", "a\u{301}", 0, 1),
        ("\u{2764}\x1b[0m\u{fe0f}", "\u{2764}\u{fe0f}", 0, 2),
        ("界Z", "界", 2, 3),
    ] {
        let mut editor = Editor::default();
        let token = editor.open().unwrap();
        assert!(editor.loaded(token, &snapshot()));
        present(&mut editor, (120, 36), Locale::EnUs);
        editor.edit(Edit::Select(Row::NewTerminalFolder));
        present(&mut editor, (120, 36), Locale::EnUs);
        editor.edit(Edit::Activate);
        editor.paste(raw);
        for (edit, column) in [
            (Edit::Home, 0),
            (Edit::Adjust(true), right_column),
            (Edit::End, end_column),
        ] {
            editor.edit(edit);
            let mut terminal = present(&mut editor, (120, 36), Locale::EnUs);
            let buffer = terminal.backend().buffer();
            assert!(
                !buffer
                    .content
                    .iter()
                    .any(|cell| cell.symbol().contains('\u{1b}'))
            );
            let index = buffer
                .content
                .iter()
                .position(|cell| cell.symbol() == glyph)
                .expect("the whole projected grapheme is painted");
            let stride = usize::from(buffer.area.width);
            let origin = ratatui::layout::Position::new(
                u16::try_from(index % stride).unwrap(),
                u16::try_from(index / stride).unwrap(),
            );
            assert_eq!(
                terminal.get_cursor_position().unwrap(),
                ratatui::layout::Position::new(origin.x + column, origin.y),
                "cursor after {edit:?} for raw {raw:?}"
            );
            let View::Page(page) = editor.view().unwrap() else {
                panic!("the page is open");
            };
            assert_eq!(page.text().map(|(text, _)| text), Some(raw));
        }
    }
}

/// Every always-visible description fits its row at the reference width of 100 columns in both
/// locales, so the page shortens none of them there.
#[test]
fn every_description_fits_its_row_at_the_reference_width_in_both_locales() {
    let budget = super::description_width(100);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for category in Category::ALL {
            for group in category.groups() {
                if let Some(description) = locale.settings_group_description(*group) {
                    assert!(
                        crate::terminal_text::display_width(description) <= budget + 2,
                        "{locale:?} {group:?}: {description}"
                    );
                }
                for row in group.rows() {
                    let description = locale.settings_description(row);
                    assert!(
                        crate::terminal_text::display_width(description) <= budget,
                        "{locale:?} {row:?} needs {} of {budget} cells: {description}",
                        crate::terminal_text::display_width(description)
                    );
                }
            }
        }
    }
}

/// A focused changed row offers its restore action without taking cells from its description: at
/// the reference width both are painted whole, in both locales.
#[test]
fn the_restore_action_leaves_a_changed_rows_description_whole() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (row, key) in [
            (Row::NewTerminalFolder, "terminal.new_cwd"),
            (Row::PrefixKey, "keys.prefix"),
        ] {
            let mut editor = Editor::default();
            let token = editor.open().unwrap();
            let mut snapshot = snapshot();
            snapshot.set = vec![orchbee_workbench_protocol::SettingPath::parse(key).unwrap()];
            assert!(editor.loaded(token, &snapshot));
            present(&mut editor, (100, 36), locale);
            editor.edit(Edit::Select(row));
            let page = paint(&mut editor, (100, 36), locale);
            assert!(
                has(&page, locale.settings_description(row)),
                "{locale:?} {row:?}: {page}"
            );
            assert!(
                has(&page, &format!("[{}]", locale.settings_restore())),
                "{locale:?} {row:?}: {page}"
            );
        }
    }
}
