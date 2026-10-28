//! Contrast decisions for displayed cells, including independent terminal and chrome themes.

use super::{Color, ColorLevel, Modifier, Theme, contrast_ratio};
use crate::tui_theme::cursor_style;
use orchbee_workbench_protocol::{TerminalDisplayAppearance as Appearance, UserPreferencesValues};

#[test]
fn every_terminal_appearance_resolves_cursor_contrast_from_the_actual_cell_background() {
    for chrome in [
        Theme::one_light(),
        Theme::one_dark(),
        Theme::terminal_native(),
    ] {
        for appearance in [
            Appearance::Light,
            Appearance::Dark,
            Appearance::Custom,
            Appearance::Host,
        ] {
            for custom_background in ["#fcfcfc", "#101010", "#777777", "#00bb99"] {
                let values = UserPreferencesValues {
                    terminal_display_theme: appearance,
                    custom_foreground: custom_background.into(),
                    custom_background: custom_background.into(),
                    ..UserPreferencesValues::default()
                };
                for (host_foreground, host_background) in
                    [(0x141414, 0xfcfcfc), (0xeeeeee, 0x121212)]
                {
                    let theme = chrome
                        .with_terminal_preferences(&values)
                        .with_host_terminal_colors(host_foreground, host_background);
                    let (foreground, background) = theme.terminal_defaults();
                    let cursor = cursor_style(foreground, background, Modifier::empty());
                    let fill = cursor.bg.unwrap();
                    assert!(
                        contrast_ratio(fill, background) >= 4.5,
                        "{appearance:?} {background:?}"
                    );
                    assert!(contrast_ratio(cursor.fg.unwrap(), fill) >= 4.5);
                    assert!(cursor.add_modifier.is_empty());
                }
            }
        }
    }
}

#[test]
fn reversed_and_low_contrast_program_cells_keep_a_readable_glyph_inside_the_cursor() {
    for (foreground, background, modifiers, displayed_background) in [
        (
            Color::Rgb(252, 252, 252),
            Color::Rgb(20, 20, 20),
            Modifier::REVERSED,
            Color::Rgb(252, 252, 252),
        ),
        (
            Color::Rgb(110, 110, 110),
            Color::Rgb(110, 110, 110),
            Modifier::DIM | Modifier::HIDDEN | Modifier::SLOW_BLINK,
            Color::Rgb(110, 110, 110),
        ),
    ] {
        let cursor = cursor_style(foreground, background, modifiers);
        assert_eq!(cursor.fg, Some(displayed_background));
        assert!(contrast_ratio(cursor.bg.unwrap(), displayed_background) >= 4.5);
        assert!(cursor.add_modifier.is_empty());
    }
}

#[test]
fn cursor_uses_the_cells_color_encoding_and_unknown_defaults_use_reverse_video() {
    for level in [
        ColorLevel::Monochrome,
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::Truecolor,
    ] {
        let theme = Theme::one_light().quantize(level);
        let (foreground, background) = theme.terminal_defaults();
        let cursor = cursor_style(foreground, background, Modifier::empty());
        let fill = cursor.bg.unwrap();
        match level {
            ColorLevel::Monochrome => {
                assert_eq!(
                    (cursor.fg, cursor.bg),
                    (Some(Color::Reset), Some(Color::Reset))
                );
                assert_eq!(cursor.add_modifier, Modifier::REVERSED);
            }
            ColorLevel::Ansi16 => assert!(!matches!(fill, Color::Rgb(..) | Color::Indexed(_))),
            ColorLevel::Ansi256 => assert!(matches!(fill, Color::Indexed(16 | 231))),
            ColorLevel::Truecolor => assert!(matches!(fill, Color::Rgb(..))),
        }
    }
    for foreground in [Color::Reset, Color::Black, Color::Rgb(255, 255, 255)] {
        let cursor = cursor_style(foreground, Color::Reset, Modifier::empty());
        assert_eq!(
            (cursor.fg, cursor.bg),
            (Some(Color::Reset), Some(Color::Reset))
        );
        assert_eq!(cursor.add_modifier, Modifier::REVERSED);
    }
    for background in [Color::Reset, Color::Black, Color::Rgb(255, 255, 255)] {
        let cursor = cursor_style(Color::Reset, background, Modifier::REVERSED);
        assert_eq!(
            (cursor.fg, cursor.bg),
            (Some(Color::Reset), Some(Color::Reset))
        );
        assert!(
            cursor.add_modifier.is_empty(),
            "reversing again would match the displayed default foreground"
        );
    }
}
