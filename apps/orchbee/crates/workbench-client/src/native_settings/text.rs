//! Selects the facts the localized text needs from the page; the text sees no page state.
use super::{Category, Choice, FileState, Group, Page, PageView, Row};
use crate::localization::{
    SettingsCategory, SettingsChoice, SettingsDetail, SettingsDetailKind, SettingsFieldRequirement,
    SettingsFileFact, SettingsGroup, SettingsProfileLabel, SettingsRow, SettingsValue,
};
use orchbee_color_themes::Polarity;
use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchField};

impl From<Row> for SettingsRow {
    fn from(row: Row) -> Self {
        match row {
            Row::Language => Self::Language,
            Row::Theme => Self::Theme,
            Row::LightTheme => Self::LightTheme,
            Row::DarkTheme => Self::DarkTheme,
            Row::ThemeOption(theme) => match theme.theme().polarity() {
                Polarity::Light => Self::LightTheme,
                Polarity::Dark => Self::DarkTheme,
            },
            Row::ColorSource => Self::ColorSource,
            Row::TerminalTheme => Self::TerminalTheme,
            Row::Foreground => Self::Foreground,
            Row::Background => Self::Background,
            Row::CopyOnSelect => Self::CopyOnSelect,
            Row::TrimAgents => Self::TrimAgents,
            Row::ResumeAgents => Self::ResumeAgents,
            Row::RestoreOutput => Self::RestoreOutput,
            Row::Reminders => Self::Reminders,
            Row::CompletionReminders => Self::CompletionReminders,
            Row::CompletionThreshold => Self::CompletionThreshold,
            Row::Sound => Self::Sound,
            Row::Scrollback => Self::Scrollback,
            Row::LaunchProfiles => Self::LaunchProfiles,
            Row::NewTerminalFolder => Self::NewTerminalFolder,
            Row::PrefixKey => Self::PrefixKey,
            Row::Profile => Self::Profile,
            Row::ProfileName => Self::ProfileName,
            Row::CommandType => Self::CommandType,
            Row::Executable => Self::Executable,
            Row::Login => Self::Login,
            Row::Argument => Self::Argument,
            Row::ArgumentValue => Self::ArgumentValue,
            Row::AddArgument => Self::AddArgument,
            Row::DeleteArgument => Self::DeleteArgument,
            Row::SetDefault => Self::SetDefault,
            Row::AddProfile => Self::AddProfile,
            Row::DeleteProfile => Self::DeleteProfile,
            Row::Back => Self::Back,
            Row::AgentIntegrations => Self::AgentIntegrations,
            Row::Agent(id) => Self::Agent(id),
            Row::RemoteHosts => Self::RemoteHosts,
            Row::RefreshIntegrations => Self::RefreshIntegrations,
        }
    }
}

impl From<Category> for SettingsCategory {
    fn from(category: Category) -> Self {
        match category {
            Category::General => Self::General,
            Category::Appearance => Self::Appearance,
            Category::Themes => Self::Themes,
            Category::Terminal => Self::Terminal,
            Category::Notifications => Self::Notifications,
            Category::Keyboard => Self::Keyboard,
            Category::Agents => Self::Agents,
        }
    }
}

impl From<Group> for SettingsGroup {
    fn from(group: Group) -> Self {
        match group {
            Group::Language => Self::Language,
            Group::AfterRestart | Group::AgentRestore => Self::AfterRestart,
            Group::AppAppearance => Self::AppAppearance,
            Group::TerminalColors => Self::TerminalColors,
            Group::LightThemes => Self::LightThemes,
            Group::DarkThemes => Self::DarkThemes,
            Group::SelectionKeys => Self::SelectionKeys,
            Group::Output => Self::Output,
            Group::LaunchProfiles => Self::LaunchProfiles,
            Group::WhenToNotify => Self::WhenToNotify,
            Group::SoundDelivery => Self::SoundDelivery,
            Group::Prefix => Self::Prefix,
            Group::AgentIntegrations => Self::AgentIntegrations,
            Group::LocalAgents => Self::LocalAgents,
            Group::RemoteHosts => Self::RemoteHosts,
        }
    }
}

impl From<Choice> for SettingsChoice {
    fn from(choice: Choice) -> Self {
        match choice {
            Choice::Locale(value) => Self::Locale(value),
            Choice::ApplicationTheme(value) => Self::ApplicationTheme(value),
            Choice::ColorTheme(choice) => Self::ColorTheme(choice.theme().id()),
            Choice::ColorSource(value) => Self::ColorSource(value),
            Choice::TerminalTheme(value) => Self::TerminalTheme(value),
            Choice::AgentReminders(value) => Self::AgentReminders(value),
            Choice::CompletionReminders(value) => Self::CompletionReminders(value),
        }
    }
}

impl<'a> PageView<'a> {
    /// The settings file the page cannot use, which its file line names; `None` while it can.
    pub(crate) fn file_fact(self) -> Option<SettingsFileFact<'a>> {
        match self.file() {
            FileState::Usable => None,
            FileState::SyntaxError { path, line, column } => Some(SettingsFileFact::SyntaxError {
                path: path.as_deref(),
                line: *line,
                column: *column,
            }),
            FileState::Unreadable { path } => Some(SettingsFileFact::Unreadable {
                path: path.as_deref(),
            }),
        }
    }
}

fn detail_kind(page: PageView<'_>) -> SettingsDetailKind {
    if page.confirm_delete() {
        return SettingsDetailKind::ConfirmDeleteProfile;
    }
    if page.invalid() {
        return SettingsDetailKind::InvalidValue;
    }
    let selected = page.selected();
    if !page.enabled(selected) {
        return match selected {
            Row::Foreground | Row::Background => SettingsDetailKind::CustomColorsRequired,
            Row::ColorSource | Row::ThemeOption(_) => SettingsDetailKind::TruecolorRequired,
            Row::Login if !page.login_shell_supported() => SettingsDetailKind::LoginUnsupported,
            Row::Executable
            | Row::Argument
            | Row::ArgumentValue
            | Row::AddArgument
            | Row::DeleteArgument => SettingsDetailKind::CustomExecutableRequired,
            Row::DeleteProfile => SettingsDetailKind::MinimumProfile,
            Row::AddProfile => SettingsDetailKind::MaximumProfiles,
            Row::SetDefault => SettingsDetailKind::AlreadyDefault,
            _ => SettingsDetailKind::Unavailable,
        };
    }
    SettingsDetailKind::LiteralArguments
}

/// The launch-profile editor's line for its focused row: a field's rule, a confirmation, or why
/// the row is unavailable.
impl<'a> From<PageView<'a>> for SettingsDetail<'a> {
    fn from(page: PageView<'a>) -> Self {
        if !page.confirm_delete() {
            let field = page.invalid_field().or_else(|| {
                if page.page() != Page::Profiles || !page.enabled(page.selected()) {
                    return None;
                }
                let field = match page.selected() {
                    Row::ProfileName => UserLaunchField::Name,
                    Row::Executable => UserLaunchField::Executable,
                    Row::ArgumentValue => UserLaunchField::Argument(page.argument()),
                    _ => return None,
                };
                Some((page.profile(), field))
            });
            if let Some((index, field)) = field
                && let Some(profile) = page.values().launch_profiles.profiles.get(index)
            {
                let name = match profile.name.as_deref() {
                    Some(name) => SettingsProfileLabel::Text(name),
                    None if profile.id == "system-default" => SettingsProfileLabel::SystemDefault,
                    None => SettingsProfileLabel::Text(&profile.id),
                };
                let requirement = match field {
                    UserLaunchField::Name if profile.id == "system-default" => {
                        SettingsFieldRequirement::SystemName
                    }
                    UserLaunchField::Name => SettingsFieldRequirement::Name,
                    UserLaunchField::Executable => SettingsFieldRequirement::Executable,
                    UserLaunchField::Argument(_) => SettingsFieldRequirement::Argument,
                };
                return Self::Field { name, requirement };
            }
        }
        Self::Message(detail_kind(page))
    }
}

impl<'a> From<(PageView<'a>, Row)> for SettingsValue<'a> {
    fn from((page, row): (PageView<'a>, Row)) -> Self {
        let values = page.values();
        let profiles = &values.launch_profiles.profiles;
        let profile = &profiles[page.profile().min(profiles.len() - 1)];
        match row {
            Row::Language => Self::Choice(SettingsChoice::Locale(values.locale)),
            Row::Theme => Self::Choice(SettingsChoice::ApplicationTheme(values.application_theme)),
            Row::LightTheme | Row::DarkTheme | Row::ColorSource => {
                match Choice::from_values(row, values) {
                    Some(choice) => Self::Choice(choice.into()),
                    None => Self::Empty,
                }
            }
            Row::ThemeOption(theme) => Self::Choice(Choice::ColorTheme(theme).into()),
            Row::TerminalTheme => {
                Self::Choice(SettingsChoice::TerminalTheme(values.terminal_display_theme))
            }
            Row::Foreground => Self::Text(&values.custom_foreground),
            Row::Background => Self::Text(&values.custom_background),
            Row::CopyOnSelect => Self::Toggle(values.copy_on_select),
            Row::TrimAgents => Self::Toggle(values.trim_trailing_blank_rows_for_agents),
            Row::ResumeAgents => Self::Toggle(values.resume_agents_on_restore),
            Row::RestoreOutput => Self::Toggle(values.restore_terminal_output),
            Row::Reminders => Self::Choice(SettingsChoice::AgentReminders(
                values.agent_notification_mode,
            )),
            Row::CompletionReminders => Self::Choice(SettingsChoice::CompletionReminders(
                values.completion_reminder_mode,
            )),
            Row::CompletionThreshold => {
                Self::Seconds(u64::from(values.completion_reminder_minimum_seconds))
            }
            Row::Sound => Self::Toggle(values.sound_enabled),
            Row::Scrollback => {
                Self::ScrollbackMegabytes(super::scrollback_megabytes(values.scrollback_bytes))
            }
            Row::NewTerminalFolder | Row::PrefixKey => match page.text_value(row) {
                Some(text) => Self::Text(text),
                None => Self::Default,
            },
            Row::LaunchProfiles => match profiles
                .iter()
                .find(|p| p.id == values.launch_profiles.default_profile_id)
            {
                Some(profile) => Self::ProfileName(profile.name.as_deref()),
                None => Self::Empty,
            },
            Row::Profile | Row::ProfileName => Self::ProfileName(profile.name.as_deref()),
            Row::CommandType => match &profile.command {
                UserLaunchCommand::SystemShell { .. } => Self::SystemShell,
                UserLaunchCommand::CustomExecutable { .. } => Self::CustomExecutable,
            },
            Row::Executable => match &profile.command {
                UserLaunchCommand::CustomExecutable { executable, .. } if executable.is_empty() => {
                    Self::Required
                }
                UserLaunchCommand::CustomExecutable { executable, .. } => Self::Text(executable),
                UserLaunchCommand::SystemShell { .. } => Self::Dash,
            },
            Row::Login => match &profile.command {
                UserLaunchCommand::SystemShell { login } if page.login_shell_supported() => {
                    Self::Toggle(*login)
                }
                UserLaunchCommand::SystemShell { .. }
                | UserLaunchCommand::CustomExecutable { .. } => Self::Unavailable,
            },
            Row::Argument => match &profile.command {
                UserLaunchCommand::CustomExecutable { arguments, .. } if !arguments.is_empty() => {
                    Self::ArgumentPosition {
                        position: page.argument() + 1,
                        total: arguments.len(),
                    }
                }
                UserLaunchCommand::SystemShell { .. }
                | UserLaunchCommand::CustomExecutable { .. } => Self::Dash,
            },
            Row::ArgumentValue => match &profile.command {
                UserLaunchCommand::CustomExecutable { arguments, .. } => arguments
                    .get(page.argument())
                    .map_or(Self::Dash, |value| Self::Text(value)),
                UserLaunchCommand::SystemShell { .. } => Self::Dash,
            },
            Row::SetDefault if profile.id == values.launch_profiles.default_profile_id => {
                Self::Default
            }
            Row::Agent(id) => Self::Integration(page.integrations().status(id)),
            Row::RemoteHosts => {
                Self::Integration(crate::localization::IntegrationStatus::ManualOnly)
            }
            _ => Self::Empty,
        }
    }
}
