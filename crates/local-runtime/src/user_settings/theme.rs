//! Exact named-theme admission, independent of rendering and host capabilities.
/// A shipped chrome theme selected from shared user preferences.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeName {
    /// Preserve the host terminal's default foreground and background.
    #[default]
    TerminalNative,
    /// Explicit light background with the shipped One Light palette.
    OneLight,
    /// Explicit dark background with the shipped One Dark palette.
    OneDark,
}
impl ThemeName {
    /// The exact theme spelling; no aliases or normalization are admitted.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TerminalNative => "terminal-native",
            Self::OneLight => "one-light",
            Self::OneDark => "one-dark",
        }
    }
    /// Admit one of the three shipped names without consulting host appearance.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "terminal-native" => Some(Self::TerminalNative),
            "one-light" => Some(Self::OneLight),
            "one-dark" => Some(Self::OneDark),
            _ => None,
        }
    }
}
