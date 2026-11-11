//! The page's categories and groups in the Desktop page's order, and the entries a category, a
//! search, or a child page lists. Values only decide which dependent rows show.
use super::{Choice, Page, Row};
use crate::localization::Locale;
use orchbee_agent_integration::IntegrationId;
use orchbee_workbench_protocol::{CompletionReminderMode, TerminalDisplayAppearance};

/// One entry of the page's left column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    General,
    Appearance,
    Themes,
    Terminal,
    Notifications,
    Keyboard,
    Agents,
}

impl Category {
    pub(crate) const ALL: [Self; 7] = [
        Self::General,
        Self::Appearance,
        Self::Themes,
        Self::Terminal,
        Self::Notifications,
        Self::Keyboard,
        Self::Agents,
    ];

    pub(crate) const fn groups(self) -> &'static [Group] {
        match self {
            Self::General => &[Group::Language, Group::AfterRestart],
            Self::Appearance => &[Group::AppAppearance, Group::TerminalColors],
            Self::Themes => &[Group::LightThemes, Group::DarkThemes],
            Self::Terminal => &[Group::SelectionKeys, Group::Output, Group::LaunchProfiles],
            Self::Notifications => &[Group::WhenToNotify, Group::SoundDelivery],
            Self::Keyboard => &[Group::Prefix],
            Self::Agents => &[Group::AgentRestore, Group::AgentIntegrations],
        }
    }

    /// Every category but Themes ends with the line that points to the settings file.
    pub(crate) const fn config_file(self) -> bool {
        !matches!(self, Self::Themes)
    }
}

/// One framed group of rows under its heading; a few groups have no heading of their own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    Language,
    AfterRestart,
    AppAppearance,
    TerminalColors,
    LightThemes,
    DarkThemes,
    SelectionKeys,
    Output,
    LaunchProfiles,
    WhenToNotify,
    SoundDelivery,
    Prefix,
    AgentRestore,
    AgentIntegrations,
    /// Sections of the Agent integrations page.
    LocalAgents,
    RemoteHosts,
}

impl Group {
    /// The rows this group can show, in order; theme lists come from the shipped catalog.
    pub(crate) fn rows(self) -> Vec<Row> {
        match self {
            Self::Language => vec![Row::Language],
            Self::AfterRestart => vec![Row::RestoreOutput],
            Self::AppAppearance => vec![Row::Theme, Row::ColorSource],
            Self::TerminalColors => vec![Row::TerminalTheme, Row::Foreground, Row::Background],
            Self::LightThemes => theme_rows(Row::LightTheme),
            Self::DarkThemes => theme_rows(Row::DarkTheme),
            Self::SelectionKeys => vec![Row::CopyOnSelect],
            Self::Output => vec![Row::NewTerminalFolder, Row::Scrollback, Row::TrimAgents],
            Self::LaunchProfiles => vec![Row::LaunchProfiles],
            Self::WhenToNotify => vec![
                Row::Reminders,
                Row::CompletionReminders,
                Row::CompletionThreshold,
            ],
            Self::SoundDelivery => vec![Row::Sound],
            Self::Prefix => vec![Row::PrefixKey],
            Self::AgentRestore => vec![Row::ResumeAgents],
            Self::AgentIntegrations => vec![Row::AgentIntegrations],
            Self::LocalAgents => IntegrationId::ALL.map(Row::Agent).to_vec(),
            Self::RemoteHosts => vec![Row::RemoteHosts],
        }
    }

    pub(crate) const fn is_theme_list(self) -> bool {
        matches!(self, Self::LightThemes | Self::DarkThemes)
    }
}

fn theme_rows(slot: Row) -> Vec<Row> {
    Choice::options(slot)
        .iter()
        .filter_map(|choice| match choice {
            Choice::ColorTheme(theme) => Some(Row::ThemeOption(*theme)),
            _ => None,
        })
        .collect()
}

/// One line group of the right column. Headings and frame edges never take focus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    /// A group's heading: the top edge of its frame, with its title and description.
    Heading(Group),
    Setting(Row),
    /// One option of the open choice list, under its row.
    Option(Choice),
    /// The bottom edge of a group's frame.
    End,
    /// The closing line that points to the settings file.
    ConfigFile,
    /// The search found nothing.
    NoResults,
}

/// What decides which entries show: dependent rows hide until their setting applies.
#[derive(Clone, Copy)]
pub(super) struct Filter<'a> {
    pub(super) query: &'a str,
    pub(super) custom_colors: bool,
    pub(super) completion_reminders: bool,
    pub(super) integrations: bool,
}

impl Filter<'_> {
    pub(super) fn of<'a>(
        query: &'a str,
        values: &orchbee_workbench_protocol::UserSettingsValues,
        integrations: bool,
    ) -> Filter<'a> {
        Filter {
            query,
            custom_colors: values.terminal_display_theme == TerminalDisplayAppearance::Custom,
            completion_reminders: values.completion_reminder_mode != CompletionReminderMode::Never,
            integrations,
        }
    }

    fn shows(self, row: Row) -> bool {
        match row {
            Row::Foreground | Row::Background => self.custom_colors,
            Row::CompletionThreshold => self.completion_reminders,
            Row::AgentIntegrations => self.integrations,
            _ => true,
        }
    }
}

/// The entries of a page: a category's framed groups, every matching row across categories
/// while a query is set, or a child page's rows.
pub(super) fn entries(
    page: Page,
    category: Category,
    filter: Filter<'_>,
    picker: Option<(Row, &'static [Choice])>,
) -> Vec<Entry> {
    let mut entries = Vec::new();
    match page {
        Page::Profiles => {
            entries.extend(page.rows().iter().copied().map(Entry::Setting));
            return entries;
        }
        Page::Integrations => {
            entries.push(Entry::Heading(Group::LocalAgents));
            entries.extend(Group::LocalAgents.rows().into_iter().map(Entry::Setting));
            entries.push(Entry::Heading(Group::RemoteHosts));
            entries.extend(
                page.rows()
                    .iter()
                    .copied()
                    .skip(IntegrationId::ALL.len())
                    .map(Entry::Setting),
            );
            return entries;
        }
        Page::Diagnosis(_) => {
            entries.push(Entry::Setting(Row::Back));
            return entries;
        }
        Page::General => {}
    }
    let terms: Vec<String> = filter
        .query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let categories: &[Category] = if terms.is_empty() {
        std::slice::from_ref(&category)
    } else {
        &Category::ALL
    };
    for category in categories {
        for group in category.groups() {
            let rows: Vec<Row> = group
                .rows()
                .into_iter()
                .filter(|row| filter.shows(*row))
                .filter(|row| terms.is_empty() || matches(*row, *category, *group, &terms))
                .collect();
            if rows.is_empty() {
                continue;
            }
            entries.push(Entry::Heading(*group));
            for row in rows {
                entries.push(Entry::Setting(row));
                if let Some((open, options)) = picker
                    && open == row
                {
                    entries.extend(options.iter().copied().map(Entry::Option));
                }
            }
            entries.push(Entry::End);
        }
    }
    if terms.is_empty() {
        if category.config_file() {
            entries.push(Entry::ConfigFile);
        }
    } else if entries.is_empty() {
        entries.push(Entry::NoResults);
    }
    entries
}

/// Every term appears in the row's name, description, group or category, in either locale; a
/// theme line also matches its theme's name and the Agent integrations row its Agents' names.
fn matches(row: Row, category: Category, group: Group, terms: &[String]) -> bool {
    let document = [Locale::EnUs, Locale::ZhCn]
        .map(|locale| {
            let name = match row {
                Row::ThemeOption(theme) => locale.settings_choice(Choice::ColorTheme(theme)),
                _ => locale.settings_row(row),
            };
            let extra = if row == Row::AgentIntegrations {
                IntegrationId::ALL
                    .map(|id| locale.agent_integration_name(id))
                    .join(" ")
            } else {
                String::new()
            };
            format!(
                "{name} {} {} {} {extra}",
                locale.settings_description(row),
                locale.settings_group_name(group).unwrap_or_default(),
                locale.settings_category_name(category),
            )
        })
        .join(" ")
        .to_lowercase();
    terms.iter().all(|term| document.contains(term.as_str()))
}
