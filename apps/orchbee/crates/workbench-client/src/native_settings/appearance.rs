//! A reversible appearance projection cannot carry clipboard, notification or launch policy.
use orchbee_workbench_protocol::{
    ApplicationAppearance, SettingsLocale, TerminalDisplayAppearance, TuiColorSource,
    UserSettingsValues,
};

/// Validated presentation-only leaves, borrowed from the values the Settings page shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppearancePreview<'a> {
    pub locale: SettingsLocale,
    pub(crate) application_theme: ApplicationAppearance,
    pub(crate) light_theme: &'a str,
    pub(crate) dark_theme: &'a str,
    pub(crate) tui_color_source: TuiColorSource,
    pub(crate) terminal_display_theme: TerminalDisplayAppearance,
    pub(crate) custom_foreground: &'a str,
    pub(crate) custom_background: &'a str,
}

impl<'a> AppearancePreview<'a> {
    /// The appearance a complete set of values gives.
    pub(crate) fn of(values: &'a UserSettingsValues) -> Self {
        Self {
            locale: values.locale,
            application_theme: values.application_theme,
            light_theme: &values.light_theme,
            dark_theme: &values.dark_theme,
            tui_color_source: values.tui_color_source,
            terminal_display_theme: values.terminal_display_theme,
            custom_foreground: &values.custom_foreground,
            custom_background: &values.custom_background,
        }
    }

    #[cfg(test)]
    /// Projects a complete validated appearance for independent expected-value assertions.
    pub(crate) fn from_values(values: &'a UserSettingsValues) -> Self {
        Self::of(values)
    }
}

impl<'a> From<AppearancePreview<'a>> for crate::tui_theme::ColorPreferences<'a> {
    fn from(preview: AppearancePreview<'a>) -> Self {
        Self {
            application_theme: preview.application_theme,
            light_theme: preview.light_theme,
            dark_theme: preview.dark_theme,
            tui_color_source: preview.tui_color_source,
            terminal_display_theme: preview.terminal_display_theme,
            custom_foreground: preview.custom_foreground,
            custom_background: preview.custom_background,
        }
    }
}
