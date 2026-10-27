//! Resolve device terminal defaults without changing a retained terminal or a Client preview.

use orchbee_workbench_protocol::{
    ApplicationAppearance, PreferencesFailure, TerminalDisplayAppearance,
};

/// A validated default-color pair and the terminal color scheme it reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserTerminalColors {
    foreground: u32,
    background: u32,
    dark: bool,
}

impl UserTerminalColors {
    /// Shipped light terminal defaults, also used when a new Host has no captured profile.
    pub const LIGHT: Self = Self {
        foreground: 0x141414,
        background: 0xfcfcfc,
        dark: false,
    };
    /// Shipped dark terminal defaults, independent of explicit ANSI palette entries.
    pub const DARK: Self = Self {
        foreground: 0xd4d4d4,
        background: 0x1e1e1e,
        dark: true,
    };

    /// Host follows explicit application Light/Dark; System leaves captured Host defaults owned
    /// by the terminal. Explicit terminal choices take precedence over application appearance.
    ///
    /// # Errors
    /// Returns `InvalidSource` when an effective Custom color is not an RGB hex value.
    pub fn resolve(
        application: ApplicationAppearance,
        terminal: TerminalDisplayAppearance,
        custom_foreground: &str,
        custom_background: &str,
    ) -> Result<Option<Self>, PreferencesFailure> {
        let terminal = match (terminal, application) {
            (TerminalDisplayAppearance::Host, ApplicationAppearance::Light) => {
                TerminalDisplayAppearance::Light
            }
            (TerminalDisplayAppearance::Host, ApplicationAppearance::Dark) => {
                TerminalDisplayAppearance::Dark
            }
            (terminal, _) => terminal,
        };
        Ok(match terminal {
            TerminalDisplayAppearance::Host => None,
            TerminalDisplayAppearance::Light => Some(Self::LIGHT),
            TerminalDisplayAppearance::Dark => Some(Self::DARK),
            TerminalDisplayAppearance::Custom => Some(Self::from_rgb(
                rgb(custom_foreground)?,
                rgb(custom_background)?,
            )),
        })
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

    /// Light/Dark select the scheme explicitly; other pairs derive it from the background.
    pub const fn dark(self) -> bool {
        self.dark
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
