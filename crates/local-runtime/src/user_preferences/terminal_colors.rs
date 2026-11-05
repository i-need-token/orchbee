//! Resolve device terminal defaults without changing a retained terminal or a Client preview.

use orchbee_color_themes::{Polarity, Theme, select};
use orchbee_workbench_protocol::{
    ApplicationAppearance, PreferencesFailure, TerminalDisplayAppearance, UserPreferencesValues,
};

/// The device's selected color-theme ids, one per polarity slot, as stored.
///
/// An id stays as stored so a later build that ships it can use it; [`Self::theme`] resolves an id
/// this build does not ship to the Orchbee theme of the slot's polarity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserColorThemes {
    light: String,
    dark: String,
}

impl Default for UserColorThemes {
    /// The Orchbee pair, the preference defaults.
    fn default() -> Self {
        Self::from_values(&UserPreferencesValues::default())
    }
}

impl UserColorThemes {
    #[must_use]
    pub const fn new(light: String, dark: String) -> Self {
        Self { light, dark }
    }

    #[must_use]
    pub fn from_values(values: &UserPreferencesValues) -> Self {
        Self::new(values.light_theme.clone(), values.dark_theme.clone())
    }

    /// The stored id of the light slot.
    #[must_use]
    pub fn light(&self) -> &str {
        &self.light
    }

    /// The stored id of the dark slot.
    #[must_use]
    pub fn dark(&self) -> &str {
        &self.dark
    }

    /// The shipped theme of the `polarity` slot, with the fallback of
    /// [`orchbee_color_themes::select`].
    #[must_use]
    pub fn theme(&self, polarity: Polarity) -> &'static Theme {
        let id = match polarity {
            Polarity::Light => &self.light,
            Polarity::Dark => &self.dark,
        };
        select(polarity, id)
    }
}

/// A validated default-color pair and the terminal color scheme it reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserTerminalColors {
    foreground: u32,
    background: u32,
    dark: bool,
}

impl UserTerminalColors {
    /// The Orchbee Light canvas, used when a new Host has no captured profile.
    pub const LIGHT: Self = Self {
        foreground: 0x141414,
        background: 0xfcfcfc,
        dark: false,
    };
    /// The Orchbee Dark canvas.
    pub const DARK: Self = Self {
        foreground: 0xd4d4d4,
        background: 0x1e1e1e,
        dark: true,
    };

    /// Theme and Host take the canvas of the active slot's theme when the application appearance
    /// is explicitly Light or Dark; with System they leave captured Host defaults owned by the
    /// terminal. Custom takes the custom pair under every appearance.
    ///
    /// # Errors
    /// Returns `InvalidSource` when an effective Custom color is not an RGB hex value.
    pub fn resolve(
        application: ApplicationAppearance,
        terminal: TerminalDisplayAppearance,
        themes: &UserColorThemes,
        custom_foreground: &str,
        custom_background: &str,
    ) -> Result<Option<Self>, PreferencesFailure> {
        let polarity = match (terminal, application) {
            (TerminalDisplayAppearance::Custom, _) => {
                return Ok(Some(Self::from_rgb(
                    rgb(custom_foreground)?,
                    rgb(custom_background)?,
                )));
            }
            (_, ApplicationAppearance::System) => return Ok(None),
            (_, ApplicationAppearance::Light) => Polarity::Light,
            (_, ApplicationAppearance::Dark) => Polarity::Dark,
        };
        let canvas = themes.theme(polarity).canvas();
        Ok(Some(Self::with_scheme(
            u32::from(canvas.foreground),
            u32::from(canvas.background),
            polarity == Polarity::Dark,
        )))
    }

    /// Defaults whose scheme follows the background luminance, as Custom colors and an outer
    /// terminal's observed OSC 10/11 colors do. Callers supply 24-bit RGB values.
    #[must_use]
    pub const fn from_rgb(foreground: u32, background: u32) -> Self {
        let red = (background >> 16) & 0xff;
        let green = (background >> 8) & 0xff;
        let blue = background & 0xff;
        Self {
            foreground,
            background,
            dark: 299 * red + 587 * green + 114 * blue < 128_000,
        }
    }

    /// A pair whose scheme an earlier capture already resolved. Callers supply 24-bit RGB values.
    #[must_use]
    pub const fn with_scheme(foreground: u32, background: u32, dark: bool) -> Self {
        Self {
            foreground,
            background,
            dark,
        }
    }

    /// Returns foreground then background, each within the 24-bit RGB range.
    pub const fn rgb(self) -> (u32, u32) {
        (self.foreground, self.background)
    }

    /// A theme's canvas carries its declared polarity; other pairs derive it from the background.
    pub const fn dark(self) -> bool {
        self.dark
    }

    /// The polarity whose theme slot supplies this pair's cursor and ANSI colors.
    #[must_use]
    pub const fn polarity(self) -> Polarity {
        if self.dark {
            Polarity::Dark
        } else {
            Polarity::Light
        }
    }
}

fn rgb(value: &str) -> Result<u32, PreferencesFailure> {
    value
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or(PreferencesFailure::InvalidSource)
}

#[cfg(test)]
mod tests;
