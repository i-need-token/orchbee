//! Finite choices: a list's provisional candidate previews; accepting it applies the value.
use super::{AppearancePreview, Row};
use orchbee_color_themes::Polarity;
use orchbee_local_runtime::InstalledShell;
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, SettingsLocale,
    TerminalDisplayAppearance, UserSettingsValues,
};
use std::sync::LazyLock;

/// One canonical preference value; a choice cannot carry an unrelated setting's payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Choice {
    Locale(SettingsLocale),
    ApplicationTheme(ApplicationAppearance),
    /// A shipped theme for the slot of its polarity.
    ColorTheme(ColorTheme),
    TerminalTheme(TerminalDisplayAppearance),
    AgentReminders(AgentReminderMode),
    CompletionReminders(CompletionReminderMode),
    /// An entry of Add profile's list, which adds a profile rather than changing a setting.
    NewProfile(NewProfile),
}

/// What Add profile adds: a shell installed here, or a system-shell profile to fill in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NewProfile {
    /// A shell by its place in the list detected when Settings first listed them.
    Shell(u8),
    Custom,
}

/// The shells installed here, detected once; a profile catalog holds at most 64 profiles.
static NEW_PROFILE_SHELLS: LazyLock<Vec<InstalledShell>> = LazyLock::new(|| {
    let mut shells = orchbee_local_runtime::installed_shells();
    shells.truncate(64);
    shells
});

/// Add profile's list: each installed shell, then Custom…; empty when no shell was found.
static NEW_PROFILE_CHOICES: LazyLock<Vec<Choice>> = LazyLock::new(|| {
    if NEW_PROFILE_SHELLS.is_empty() {
        return Vec::new();
    }
    (0..NEW_PROFILE_SHELLS.len())
        .filter_map(|index| u8::try_from(index).ok())
        .map(|index| Choice::NewProfile(NewProfile::Shell(index)))
        .chain([Choice::NewProfile(NewProfile::Custom)])
        .collect()
});

impl NewProfile {
    /// The installed shell this entry adds; none for Custom….
    pub(crate) fn shell(self) -> Option<&'static InstalledShell> {
        match self {
            Self::Shell(index) => NEW_PROFILE_SHELLS.get(usize::from(index)),
            Self::Custom => None,
        }
    }
}

/// One shipped theme by its position in the build's fixed catalog, so a picker episode stays a
/// few bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorTheme(u8);

impl ColorTheme {
    /// Whether this theme is the one its slot's stored id resolves to.
    pub(crate) fn current(self, values: &UserSettingsValues) -> bool {
        let polarity = self.theme().polarity();
        Self::effective(
            polarity,
            match polarity {
                Polarity::Light => &values.light_theme,
                Polarity::Dark => &values.dark_theme,
            },
        ) == self
    }
}

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
            Row::AddProfile => &NEW_PROFILE_CHOICES,
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

    /// The value of a finite row, which the common values carry.
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
            Row::TerminalTheme => Some(Self::TerminalTheme(values.terminal_display_theme)),
            Row::Reminders => Some(Self::AgentReminders(values.agent_notification_mode)),
            Row::CompletionReminders => {
                Some(Self::CompletionReminders(values.completion_reminder_mode))
            }
            _ => None,
        }
    }

    /// Picking a theme stores it in the slot of its polarity and leaves Appearance alone, so the
    /// window shows what Appearance selects. Under System it shows when the host matches.
    /// Re-picking the slot's effective theme leaves an unshipped stored id untouched.
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
            }
            Self::TerminalTheme(value) => values.terminal_display_theme = value,
            Self::AgentReminders(value) => values.agent_notification_mode = value,
            Self::CompletionReminders(value) => values.completion_reminder_mode = value,
            Self::NewProfile(_) => {}
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
            }
            Self::TerminalTheme(value) => preview.terminal_display_theme = value,
            Self::AgentReminders(_) | Self::CompletionReminders(_) | Self::NewProfile(_) => {}
        }
    }
}

/// An open choice list whose cursor always belongs to its row's closed catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChoicePicker {
    pub(super) row: Row,
    pub(super) original: Choice,
    pub(super) cursor: usize,
}

impl ChoicePicker {
    /// Identifies the parent setting without exposing cursor mutation to rendering.
    pub(crate) const fn row(self) -> Row {
        self.row
    }
    /// Whether the list marks `choice` as the row's value; Add profile's list has none.
    pub(crate) fn marks(self, choice: Choice) -> bool {
        self.original == choice && !matches!(choice, Choice::NewProfile(_))
    }
    /// Opens the row's list on `original`, the value the row shows.
    pub(super) fn open(row: Row, original: Choice) -> Option<Self> {
        let cursor = Choice::options(row)
            .iter()
            .position(|choice| *choice == original)?;
        Some(Self {
            row,
            original,
            cursor,
        })
    }
    pub(crate) fn candidate(self) -> Choice {
        Choice::options(self.row)[self.cursor]
    }
    pub(super) fn navigate(&mut self, forward: bool) {
        let count = Choice::options(self.row).len();
        self.cursor = (self.cursor + if forward { 1 } else { count - 1 }) % count;
    }
}
