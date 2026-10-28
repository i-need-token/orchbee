//! Finite choices separate a provisional preview from the accepted parent draft.
use super::{AppearancePreview, Row};
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, PreferencesLocale,
    TerminalDisplayAppearance, UserPreferencesValues,
};

/// One canonical preference value; a choice cannot carry an unrelated setting's payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Choice {
    Locale(PreferencesLocale),
    ApplicationTheme(ApplicationAppearance),
    TerminalTheme(TerminalDisplayAppearance),
    AgentReminders(AgentReminderMode),
    CompletionReminders(CompletionReminderMode),
}

impl Choice {
    /// Returns the closed choice catalog, or an empty slice for other control families.
    pub(crate) fn options(row: Row) -> &'static [Self] {
        match row {
            Row::Language => &[
                Self::Locale(PreferencesLocale::System),
                Self::Locale(PreferencesLocale::EnUs),
                Self::Locale(PreferencesLocale::ZhCn),
            ],
            Row::Theme => &[
                Self::ApplicationTheme(ApplicationAppearance::System),
                Self::ApplicationTheme(ApplicationAppearance::Light),
                Self::ApplicationTheme(ApplicationAppearance::Dark),
            ],
            Row::TerminalTheme => &[
                Self::TerminalTheme(TerminalDisplayAppearance::Host),
                Self::TerminalTheme(TerminalDisplayAppearance::Light),
                Self::TerminalTheme(TerminalDisplayAppearance::Dark),
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

    pub(super) fn from_values(row: Row, values: &UserPreferencesValues) -> Option<Self> {
        match row {
            Row::Language => Some(Self::Locale(values.locale)),
            Row::Theme => Some(Self::ApplicationTheme(values.application_theme)),
            Row::TerminalTheme => Some(Self::TerminalTheme(values.terminal_display_theme)),
            Row::Reminders => Some(Self::AgentReminders(values.agent_notification_mode)),
            Row::CompletionReminders => {
                Some(Self::CompletionReminders(values.completion_reminder_mode))
            }
            _ => None,
        }
    }

    pub(super) fn apply(self, values: &mut UserPreferencesValues) {
        match self {
            Self::Locale(value) => values.locale = value,
            Self::ApplicationTheme(value) => values.application_theme = value,
            Self::TerminalTheme(value) => values.terminal_display_theme = value,
            Self::AgentReminders(value) => values.agent_notification_mode = value,
            Self::CompletionReminders(value) => values.completion_reminder_mode = value,
        }
    }

    pub(super) fn preview(self, preview: &mut AppearancePreview<'_>) {
        match self {
            Self::Locale(value) => preview.locale = value,
            Self::ApplicationTheme(value) => preview.application_theme = value,
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
    pub(super) fn open(row: Row, values: &UserPreferencesValues) -> Option<Self> {
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
