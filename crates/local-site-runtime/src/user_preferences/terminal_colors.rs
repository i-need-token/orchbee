//! Resolve device terminal defaults without changing a retained terminal or a Client preview.

use orchbee_workbench_protocol::{
    ApplicationAppearance, PreferencesFailure, TerminalDisplayAppearance,
};

/// A validated default-color pair and an optional explicit terminal color scheme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserTerminalColors {
    foreground: u32,
    background: u32,
    dark: Option<bool>,
}

impl UserTerminalColors {
    /// Shipped light terminal defaults, also used when a new Host has no captured profile.
    pub const LIGHT: Self = Self {
        foreground: 0x141414,
        background: 0xfcfcfc,
        dark: Some(false),
    };
    /// Shipped dark terminal defaults, independent of explicit ANSI palette entries.
    pub const DARK: Self = Self {
        foreground: 0xd4d4d4,
        background: 0x1e1e1e,
        dark: Some(true),
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
            TerminalDisplayAppearance::Custom => Some(Self {
                foreground: rgb(custom_foreground)?,
                background: rgb(custom_background)?,
                dark: None,
            }),
        })
    }

    /// Returns foreground then background, each within the 24-bit RGB range.
    pub const fn rgb(self) -> (u32, u32) {
        (self.foreground, self.background)
    }

    /// Custom colors preserve the Host's scheme; Light/Dark explicitly select it.
    pub const fn dark(self) -> Option<bool> {
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
