//! Chooses what the TUI paints from the appearance preferences and host facts: the selected theme
//! slot on a truecolor host whose appearance is known, otherwise the host terminal's own colors.
use super::{ColorLevel, Polarity, Theme};
use orchbee_workbench_protocol::{
    ApplicationAppearance, TerminalDisplayAppearance, TuiColorSource, UserPreferencesValues,
};

/// The appearance leaves of one confirmed record or reversible Settings draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ColorPreferences<'a> {
    pub(crate) application_theme: ApplicationAppearance,
    pub(crate) light_theme: &'a str,
    pub(crate) dark_theme: &'a str,
    pub(crate) tui_color_source: TuiColorSource,
    pub(crate) terminal_display_theme: TerminalDisplayAppearance,
    pub(crate) custom_foreground: &'a str,
    pub(crate) custom_background: &'a str,
}

impl<'a> From<&'a UserPreferencesValues> for ColorPreferences<'a> {
    fn from(values: &'a UserPreferencesValues) -> Self {
        Self {
            application_theme: values.application_theme,
            light_theme: &values.light_theme,
            dark_theme: &values.dark_theme,
            tui_color_source: values.tui_color_source,
            terminal_display_theme: values.terminal_display_theme,
            custom_foreground: &values.custom_foreground,
            custom_background: &values.custom_background,
        }
    }
}

/// Why the TUI shows the host terminal's own colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalReason {
    /// The user chose Terminal colors.
    Chosen,
    /// The host shows fewer than 24-bit colors, which collapse a theme's canvas tint and its
    /// interaction mixes.
    ReducedColor,
    /// System appearance before the host appearance settles, or on a platform without an
    /// appearance adapter whose terminal does not report its background.
    UnknownAppearance,
}

/// What the TUI paints: one shipped theme, or Terminal colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Resolution {
    Theme(&'static orchbee_color_themes::Theme),
    Terminal(TerminalReason),
}

impl Resolution {
    /// `host` is the settled host polarity: the outer terminal's reported default background, or
    /// the operating system's appearance after an unanswered probe.
    pub(crate) fn of(
        preferences: ColorPreferences<'_>,
        host: Option<Polarity>,
        level: ColorLevel,
    ) -> Self {
        if !named_themes_available(level) {
            return Self::Terminal(TerminalReason::ReducedColor);
        }
        if preferences.tui_color_source == TuiColorSource::Terminal {
            return Self::Terminal(TerminalReason::Chosen);
        }
        let polarity = match preferences.application_theme {
            ApplicationAppearance::Light => Polarity::Light,
            ApplicationAppearance::Dark => Polarity::Dark,
            ApplicationAppearance::System => match host {
                Some(polarity) => polarity,
                None => return Self::Terminal(TerminalReason::UnknownAppearance),
            },
        };
        Self::Theme(match polarity {
            Polarity::Light => orchbee_color_themes::select(
                orchbee_color_themes::Polarity::Light,
                preferences.light_theme,
            ),
            Polarity::Dark => orchbee_color_themes::select(
                orchbee_color_themes::Polarity::Dark,
                preferences.dark_theme,
            ),
        })
    }
}

/// Whether this host can show a named theme at all, independently of the stored preferences.
pub(crate) const fn named_themes_available(level: ColorLevel) -> bool {
    matches!(level, ColorLevel::Truecolor)
}

impl Theme {
    /// The complete chrome and terminal-default resolution for one set of preferences. The
    /// caller applies the outer host's observed colors afterwards.
    pub(crate) fn resolve(
        preferences: ColorPreferences<'_>,
        host: Option<Polarity>,
        level: ColorLevel,
    ) -> Self {
        let resolution = Resolution::of(preferences, host, level);
        match resolution {
            Resolution::Theme(theme) => Self::named(theme),
            Resolution::Terminal(_) => Self::terminal_native(),
        }
        .quantize(level)
        .with_terminal_appearance(preferences)
    }
}
