//! A reversible appearance projection cannot carry clipboard, notification or launch policy.
use orchbee_workbench_protocol::{
    ApplicationAppearance, PreferencesLocale, TerminalDisplayAppearance, UserPreferencesValues,
};

/// Validated presentation-only leaves, borrowed from the draft or latest confirmed snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AppearancePreview<'a> {
    pub(crate) locale: PreferencesLocale,
    pub(crate) application_theme: ApplicationAppearance,
    pub(crate) terminal_display_theme: TerminalDisplayAppearance,
    pub(crate) custom_foreground: &'a str,
    pub(crate) custom_background: &'a str,
}

impl<'a> AppearancePreview<'a> {
    pub(super) fn project(
        basis: &'a UserPreferencesValues,
        draft: &'a UserPreferencesValues,
        effective: &'a UserPreferencesValues,
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
    pub(crate) fn from_values(values: &'a UserPreferencesValues) -> Self {
        Self::project(values, values, values, false)
    }
}
