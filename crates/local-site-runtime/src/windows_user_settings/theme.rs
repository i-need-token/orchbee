//! Exact named-theme admission, independent of rendering and host capabilities.
use super::{
    WindowsSettingsDiagnosticDisposition as Disposition, diagnostics::DiagnosticsCollector,
};

/// A shipped chrome theme admitted from the Windows user configuration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowsThemeName {
    /// Preserve the host terminal's default foreground and background.
    #[default]
    TerminalNative,
    /// Explicit light background with the shipped One Light palette.
    OneLight,
    /// Explicit dark background with the shipped One Dark palette.
    OneDark,
}
impl WindowsThemeName {
    /// The exact configuration spelling; no aliases or normalization are admitted.
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

pub(super) fn admit(
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) -> WindowsThemeName {
    let Some(table) = value.as_table() else {
        diagnostics.push("theme", "wrong-type", Disposition::Defaulted);
        return WindowsThemeName::default();
    };
    for key in table.keys().filter(|key| key.as_str() != "name") {
        diagnostics.push(
            super::diagnostics::unknown_field("theme", key),
            "unsupported-herdr",
            Disposition::Unsupported,
        );
    }
    let Some(value) = table.get("name") else {
        return WindowsThemeName::default();
    };
    if let Some(name) = value.as_str().and_then(WindowsThemeName::parse) {
        return name;
    }
    diagnostics.push(
        "theme.name",
        if value.is_str() {
            "invalid-theme"
        } else {
            "wrong-type"
        },
        Disposition::Defaulted,
    );
    WindowsThemeName::default()
}
