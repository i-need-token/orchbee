//! Shipped chrome palette names, independent of rendering and host capabilities.
/// The chrome palette drawn before shared preferences select the application theme.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeName {
    /// Preserve the host terminal's default foreground and background.
    #[default]
    TerminalNative,
    /// Explicit light background with the shipped One Light palette; render evidence only.
    #[cfg(test)]
    OneLight,
    /// Explicit dark background with the shipped One Dark palette; render evidence only.
    #[cfg(test)]
    OneDark,
}
