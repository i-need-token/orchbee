//! Shipped chrome palette names, independent of rendering and host capabilities.
/// The chrome palette drawn before shared preferences select the application theme.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeName {
    /// Preserve the host terminal's default foreground and background.
    #[default]
    TerminalNative,
    /// The shipped Orchbee Light theme; render evidence only.
    #[cfg(any(test, feature = "test-support"))]
    OrchbeeLight,
    /// The shipped Orchbee Dark theme; render evidence only.
    #[cfg(any(test, feature = "test-support"))]
    OrchbeeDark,
}
