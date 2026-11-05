//! The terminal theme a launch installs and a Host-following recolor applies: the effective default
//! colors with the cursor and ANSI 0-15 of the selected theme of their polarity, followed by the
//! standard xterm color cube and gray ramp. It does not choose the default colors, apply a theme
//! to a Host, or own theme data.

use orchbee_color_themes::Polarity;
use orchbee_local_runtime::{UserColorThemes, UserTerminalColors};
use orchbee_terminal_protocol::{ColorScheme, TerminalTheme};

#[cfg(test)]
mod tests;

const CUBE_LEVELS: [u32; 6] = [0, 95, 135, 175, 215, 255];

/// The complete theme for terminals whose default colors are `colors`; their polarity picks both
/// the scheme and the slot of `themes` whose theme supplies the cursor and ANSI colors. A slot id
/// this build does not ship takes the Orchbee theme of that polarity.
pub(crate) fn effective(colors: UserTerminalColors, themes: &UserColorThemes) -> TerminalTheme {
    let (foreground, background) = colors.rgb();
    let polarity = colors.polarity();
    let color_scheme = match polarity {
        Polarity::Light => ColorScheme::Light,
        Polarity::Dark => ColorScheme::Dark,
    };
    let theme = themes.theme(polarity);
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
