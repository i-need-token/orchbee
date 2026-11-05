//! The terminal theme a launch installs and a Host-following recolor applies: the effective default
//! colors with the cursor and ANSI 0-15 of the Orchbee theme of their polarity, followed by the
//! standard xterm color cube and gray ramp. It does not choose the default colors, apply a theme
//! to a Host, or own theme data.

use orchbee_color_themes::{Polarity, orchbee};
use orchbee_local_runtime::UserTerminalColors;
use orchbee_terminal_protocol::{ColorScheme, TerminalTheme};

#[cfg(test)]
mod tests;

const CUBE_LEVELS: [u32; 6] = [0, 95, 135, 175, 215, 255];

/// The complete theme for terminals whose default colors are `colors`; `colors.dark()` picks both
/// the scheme and the theme of the default pair.
pub(crate) fn effective(colors: UserTerminalColors) -> TerminalTheme {
    let (foreground, background) = colors.rgb();
    let (color_scheme, polarity) = if colors.dark() {
        (ColorScheme::Dark, Polarity::Dark)
    } else {
        (ColorScheme::Light, Polarity::Light)
    };
    let theme = orchbee(polarity);
    let mut palette: Vec<u32> = theme
        .ansi()
        .iter()
        .map(|color| u32::from(color.value()))
        .collect();
    for red in CUBE_LEVELS {
        for green in CUBE_LEVELS {
            for blue in CUBE_LEVELS {
                palette.push((red << 16) | (green << 8) | blue);
            }
        }
    }
    for index in 0..24 {
        let value = 8 + index * 10;
        palette.push((value << 16) | (value << 8) | value);
    }
    TerminalTheme {
        color_scheme,
        default_background_rgb: background,
        default_cursor_rgb: Some(u32::from(theme.canvas().cursor.value())),
        default_foreground_rgb: foreground,
        palette,
    }
}
