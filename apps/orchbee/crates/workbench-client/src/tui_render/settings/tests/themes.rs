//! Theme slot pickers: one row per theme of the slot's polarity, a swatch where the host can show
//! one, and the System notice beside the slot description.
use super::*;
use crate::native_settings::View;
use orchbee_workbench_protocol::ApplicationAppearance;

fn open_dark_picker(width: u16, height: u16, theme: &Theme) -> (Editor, Terminal<TestBackend>) {
    let mut editor = editor_with(UserPreferencesValues::default());
    select_and_present(&mut editor, Row::DarkTheme, Locale::EnUs, theme);
    editor.edit(Edit::Activate);
    let (terminal, _, _) = present_once(&mut editor, width, height, Locale::EnUs, theme);
    (editor, terminal)
}

fn rgb(color: orchbee_color_themes::Rgb) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(color.red, color.green, color.blue)
}

#[test]
fn each_theme_row_paints_its_canvas_sample_and_ansi_fills_on_a_truecolor_host() {
    let theme = Theme::orchbee_dark();
    let (_, terminal) = open_dark_picker(120, 40, &theme);
    let buffer = terminal.backend().buffer();
    let dark: Vec<_> = orchbee_color_themes::shipped()
        .iter()
        .filter(|named| named.polarity() == orchbee_color_themes::Polarity::Dark)
        .collect();
    assert_eq!(dark.len(), 10);
    for named in dark {
        let y = (0..40)
            .find(|&y| line_text(&terminal, y).contains(&format!(") {}", named.name())))
            .unwrap_or_else(|| panic!("{} has a picker row", named.id()));
        let line = line_text(&terminal, y);
        let sample = u16::try_from(display_width(&line[..line.find(" Aa ").unwrap()])).unwrap();
        let canvas = named.canvas();
        assert_eq!(buffer[(sample + 1, y)].fg, rgb(canvas.foreground));
        assert_eq!(buffer[(sample + 1, y)].bg, rgb(canvas.background));
        for (offset, color) in (0_u16..).zip(&named.ansi()[1..=5]) {
            assert_eq!(
                buffer[(sample + 4 + offset * 2, y)].bg,
                rgb(color.value()),
                "{} ANSI {}",
                named.id(),
                offset + 1
            );
        }
    }
}

#[test]
fn a_host_without_exact_color_shows_theme_names_without_swatches() {
    for theme in [
        Theme::terminal_native().quantize(ColorLevel::Ansi256),
        Theme::terminal_native()
            .with_no_color(true)
            .quantize(ColorLevel::Monochrome),
    ] {
        let (_, terminal) = open_dark_picker(120, 40, &theme);
        let text = screen_text(&terminal);
        assert!(text.contains("Gruvbox Dark"));
        assert!(!text.contains(" Aa "));
    }
}

#[test]
fn a_short_picker_scrolls_to_keep_the_last_theme_in_view() {
    let theme = Theme::orchbee_dark();
    let (mut editor, _) = open_dark_picker(58, 12, &theme);
    editor.edit(Edit::End);
    let (terminal, _, _) = present_once(&mut editor, 58, 12, Locale::EnUs, &theme);
    let text = screen_text(&terminal);
    assert!(text.contains("Kanagawa Wave"));
    assert!(!text.contains("Orchbee Dark"));
}

#[test]
fn slot_rows_say_when_they_apply_only_while_appearance_follows_the_system() {
    let theme = Theme::orchbee_dark();
    for (application, notice) in [
        (ApplicationAppearance::System, true),
        (ApplicationAppearance::Dark, false),
    ] {
        let mut editor = editor_with(UserPreferencesValues {
            application_theme: application,
            ..UserPreferencesValues::default()
        });
        for (row, locale, expected) in [
            (
                Row::LightTheme,
                Locale::EnUs,
                "Colors for the interface and terminals when the appearance is light. \
                 Used when the system is light.",
            ),
            (
                Row::DarkTheme,
                Locale::ZhCn,
                "外观为深色时，界面和终端使用的配色。系统为深色时使用。",
            ),
        ] {
            editor.edit(Edit::Select(row));
            let Some(View::Draft(draft)) = editor.view() else {
                panic!("draft")
            };
            let description = draft_description(draft, locale, &theme);
            if notice {
                assert_eq!(description, expected);
            } else {
                assert_eq!(description, locale.settings_description(row));
            }
        }
    }
}
