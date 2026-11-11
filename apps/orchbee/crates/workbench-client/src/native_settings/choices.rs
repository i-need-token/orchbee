//! Finite choices separate a provisional preview from the accepted parent draft.
use super::{AppearancePreview, Row};
use orchbee_color_themes::Polarity;
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, SettingsLocale,
    TerminalDisplayAppearance, TuiColorSource, UserSettingsValues,
};
use std::sync::LazyLock;

/// One canonical preference value; a choice cannot carry an unrelated setting's payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Choice {
    Locale(SettingsLocale),
    ApplicationTheme(ApplicationAppearance),
    /// A shipped theme for the slot of its polarity.
    ColorTheme(ColorTheme),
    ColorSource(TuiColorSource),
    TerminalTheme(TerminalDisplayAppearance),
    AgentReminders(AgentReminderMode),
    CompletionReminders(CompletionReminderMode),
}

/// One shipped theme by its position in the build's fixed catalog, so a picker episode stays a
/// few bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorTheme(u8);

impl ColorTheme {
    /// The slot's effective theme: an id this build does not ship, or one of the other
    /// polarity, resolves to the slot's Orchbee theme.
    fn effective(polarity: Polarity, id: &str) -> Self {
        let theme = orchbee_color_themes::select(polarity, id);
        let position = orchbee_color_themes::shipped()
            .iter()
            .position(|shipped| std::ptr::eq(shipped, theme))
            .expect("select returns a shipped theme");
        Self(u8::try_from(position).expect("the shipped catalog stays small"))
    }

    pub(crate) fn theme(self) -> &'static orchbee_color_themes::Theme {
        &orchbee_color_themes::shipped()[usize::from(self.0)]
    }
}

/// The shipped themes of one polarity, in catalog order: the Orchbee theme first.
fn color_themes(polarity: Polarity) -> Vec<Choice> {
    orchbee_color_themes::shipped()
        .iter()
        .filter(|theme| theme.polarity() == polarity)
        .map(|theme| Choice::ColorTheme(ColorTheme::effective(polarity, theme.id())))
        .collect()
}

static LIGHT_THEMES: LazyLock<Vec<Choice>> = LazyLock::new(|| color_themes(Polarity::Light));
static DARK_THEMES: LazyLock<Vec<Choice>> = LazyLock::new(|| color_themes(Polarity::Dark));

impl Choice {
    /// Returns the closed choice catalog, or an empty slice for other control families.
    pub(crate) fn options(row: Row) -> &'static [Self] {
        match row {
            Row::LightTheme => &LIGHT_THEMES,
            Row::DarkTheme => &DARK_THEMES,
            Row::ColorSource => &[
                Self::ColorSource(TuiColorSource::Theme),
                Self::ColorSource(TuiColorSource::Terminal),
            ],
            Row::Language => &[
                Self::Locale(SettingsLocale::System),
                Self::Locale(SettingsLocale::EnUs),
                Self::Locale(SettingsLocale::ZhCn),
            ],
            Row::Theme => &[
                Self::ApplicationTheme(ApplicationAppearance::System),
                Self::ApplicationTheme(ApplicationAppearance::Light),
                Self::ApplicationTheme(ApplicationAppearance::Dark),
            ],
            Row::TerminalTheme => &[
                Self::TerminalTheme(TerminalDisplayAppearance::Theme),
                Self::TerminalTheme(TerminalDisplayAppearance::Host),
                Self::TerminalTheme(TerminalDisplayAppearance::Custom),
            ],
            Row::Reminders => &[
                Self::AgentReminders(AgentReminderMode::Never),
                Self::AgentReminders(AgentReminderMode::Hidden),
            ],
            Row::CompletionReminders => &[
                Self::CompletionReminders(CompletionReminderMode::Never),
                Self::CompletionReminders(CompletionReminderMode::Unfocused),
                Self::CompletionReminders(CompletionReminderMode::Always),
            ],
            _ => &[],
        }
    }

    pub(super) fn from_values(row: Row, values: &UserSettingsValues) -> Option<Self> {
        match row {
            Row::Language => Some(Self::Locale(values.locale)),
            Row::Theme => Some(Self::ApplicationTheme(values.application_theme)),
            // An id this build does not ship opens on the Orchbee theme it resolves to.
            Row::LightTheme => Some(Self::ColorTheme(ColorTheme::effective(
                Polarity::Light,
                &values.light_theme,
            ))),
            Row::DarkTheme => Some(Self::ColorTheme(ColorTheme::effective(
                Polarity::Dark,
                &values.dark_theme,
            ))),
            Row::ColorSource => Some(Self::ColorSource(values.tui_color_source)),
            Row::TerminalTheme => Some(Self::TerminalTheme(values.terminal_display_theme)),
            Row::Reminders => Some(Self::AgentReminders(values.agent_notification_mode)),
            Row::CompletionReminders => {
                Some(Self::CompletionReminders(values.completion_reminder_mode))
            }
            _ => None,
        }
    }

    /// Picking a theme sets its slot and shows it: Appearance fixed to the other polarity
    /// switches, and Terminal colors give way to Theme. Under System it shows when the host
    /// matches. Re-picking the slot's effective theme leaves an unshipped stored id untouched.
    pub(super) fn apply(self, values: &mut UserSettingsValues) {
        match self {
            Self::Locale(value) => values.locale = value,
            Self::ApplicationTheme(value) => values.application_theme = value,
            Self::ColorTheme(choice) => {
                let theme = choice.theme();
                let polarity = theme.polarity();
                let slot = match polarity {
                    Polarity::Light => &mut values.light_theme,
                    Polarity::Dark => &mut values.dark_theme,
                };
                if ColorTheme::effective(polarity, slot) != choice {
                    theme.id().clone_into(slot);
                }
                match (values.application_theme, polarity) {
                    (ApplicationAppearance::Light, Polarity::Dark) => {
                        values.application_theme = ApplicationAppearance::Dark;
                    }
                    (ApplicationAppearance::Dark, Polarity::Light) => {
                        values.application_theme = ApplicationAppearance::Light;
                    }
                    _ => {}
                }
                values.tui_color_source = TuiColorSource::Theme;
            }
            Self::ColorSource(value) => values.tui_color_source = value,
            Self::TerminalTheme(value) => values.terminal_display_theme = value,
            Self::AgentReminders(value) => values.agent_notification_mode = value,
            Self::CompletionReminders(value) => values.completion_reminder_mode = value,
        }
    }

    /// A theme candidate shows at its own polarity, as the GUI previews it.
    pub(super) fn preview(self, preview: &mut AppearancePreview<'_>) {
        match self {
            Self::Locale(value) => preview.locale = value,
            Self::ApplicationTheme(value) => preview.application_theme = value,
            Self::ColorTheme(choice) => {
                let theme = choice.theme();
                let id = theme.id();
                match theme.polarity() {
                    Polarity::Light => {
                        preview.light_theme = id;
                        preview.application_theme = ApplicationAppearance::Light;
                    }
                    Polarity::Dark => {
                        preview.dark_theme = id;
                        preview.application_theme = ApplicationAppearance::Dark;
                    }
                }
                preview.tui_color_source = TuiColorSource::Theme;
            }
            Self::ColorSource(value) => preview.tui_color_source = value,
            Self::TerminalTheme(value) => preview.terminal_display_theme = value,
            Self::AgentReminders(_) | Self::CompletionReminders(_) => {}
        }
    }
}

/// A valid finite-choice episode whose cursor always belongs to its row's closed catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChoicePicker {
    pub(super) row: Row,
    pub(super) original: Choice,
    pub(super) cursor: usize,
    pub(super) viewport: usize,
}

impl ChoicePicker {
    /// Identifies the parent setting without exposing cursor mutation to rendering.
    pub(crate) const fn row(self) -> Row {
        self.row
    }
    /// Returns the parent draft's accepted value, independently of navigation.
    pub(crate) const fn original(self) -> Choice {
        self.original
    }
    /// Returns the valid provisional position in this row's catalog.
    pub(crate) const fn cursor(self) -> usize {
        self.cursor
    }
    /// Keeps the provisional choice visible while preserving an admitted wheel viewport.
    pub(crate) fn first_visible(self, visible: usize) -> usize {
        let visible = visible.max(1);
        let first = self
            .viewport
            .min(Choice::options(self.row).len().saturating_sub(visible));
        if self.cursor < first {
            self.cursor
        } else if self.cursor >= first.saturating_add(visible) {
            self.cursor.saturating_add(1).saturating_sub(visible)
        } else {
            first
        }
    }
    pub(super) fn open(row: Row, values: &UserSettingsValues) -> Option<Self> {
        let original = Choice::from_values(row, values)?;
        let cursor = Choice::options(row)
            .iter()
            .position(|choice| *choice == original)?;
        Some(Self {
            row,
            original,
            cursor,
            viewport: 0,
        })
    }
    pub(super) fn candidate(self) -> Choice {
        Choice::options(self.row)[self.cursor]
    }
    pub(super) fn navigate(&mut self, forward: bool) {
        let count = Choice::options(self.row).len();
        self.cursor = (self.cursor + if forward { 1 } else { count - 1 }) % count;
    }
}
