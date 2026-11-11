//! A reversible appearance projection cannot carry clipboard, notification or launch policy.
use orchbee_workbench_protocol::{
    ApplicationAppearance, SettingsLocale, TerminalDisplayAppearance, TuiColorSource,
    UserSettingsValues,
};

/// Validated presentation-only leaves, borrowed from the draft or latest confirmed snapshot.
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
    pub(super) fn project(
        basis: &'a UserSettingsValues,
        draft: &'a UserSettingsValues,
        effective: &'a UserSettingsValues,
        reset: bool,
    ) -> Self {
        fn leaf<'a, T: PartialEq + ?Sized>(
            basis: &T,
            draft: &'a T,
            effective: &'a T,
            reset: bool,
        ) -> &'a T {
            if reset || basis != draft {
                draft
            } else {
                effective
            }
        }
        Self {
            locale: *leaf(&basis.locale, &draft.locale, &effective.locale, reset),
            application_theme: *leaf(
                &basis.application_theme,
                &draft.application_theme,
                &effective.application_theme,
                reset,
            ),
            light_theme: leaf(
                &basis.light_theme,
                &draft.light_theme,
                &effective.light_theme,
                reset,
            ),
            dark_theme: leaf(
                &basis.dark_theme,
                &draft.dark_theme,
                &effective.dark_theme,
                reset,
            ),
            tui_color_source: *leaf(
                &basis.tui_color_source,
                &draft.tui_color_source,
                &effective.tui_color_source,
                reset,
            ),
            terminal_display_theme: *leaf(
                &basis.terminal_display_theme,
                &draft.terminal_display_theme,
                &effective.terminal_display_theme,
                reset,
            ),
            custom_foreground: leaf(
                &basis.custom_foreground,
                &draft.custom_foreground,
                &effective.custom_foreground,
                reset,
            ),
            custom_background: leaf(
                &basis.custom_background,
                &draft.custom_background,
                &effective.custom_background,
                reset,
            ),
        }
    }

    #[cfg(test)]
    /// Projects a complete validated appearance for independent expected-value assertions.
    pub(crate) fn from_values(values: &'a UserSettingsValues) -> Self {
        Self::project(values, values, values, false)
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
