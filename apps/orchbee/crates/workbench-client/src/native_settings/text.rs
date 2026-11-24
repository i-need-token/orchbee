//! Selects the facts the localized text needs from the page; the text sees no page state.
use super::{Choice, FileState, Page, PageView, Row};
use crate::localization::{
    SettingsChoice, SettingsDetail, SettingsDetailKind, SettingsFieldRequirement, SettingsFileFact,
    SettingsProfileLabel, SettingsValue,
};
use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchField};

impl From<Choice> for SettingsChoice {
    fn from(choice: Choice) -> Self {
        match choice {
            Choice::Locale(value) => Self::Locale(value),
            Choice::ApplicationTheme(value) => Self::ApplicationTheme(value),
            Choice::ColorTheme(choice) => Self::ColorTheme(choice.theme().id()),
            Choice::TerminalTheme(value) => Self::TerminalTheme(value),
            Choice::AgentReminders(value) => Self::AgentReminders(value),
            Choice::CompletionReminders(value) => Self::CompletionReminders(value),
            Choice::NewProfile(entry) => Self::NewProfile(entry.shell()),
            Choice::MachineColor(color) => Self::MachineColor(color),
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
            Row::ThemeOption(_) => SettingsDetailKind::TruecolorRequired,
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
        if let Some(flag) = row.flag() {
            return Self::Toggle((flag.read)(values));
        }
        match row {
            Row::Language => Self::Choice(SettingsChoice::Locale(values.locale)),
            Row::Theme => Self::Choice(SettingsChoice::ApplicationTheme(values.application_theme)),
            Row::LightTheme | Row::DarkTheme => match Choice::from_values(row, values) {
                Some(choice) => Self::Choice(choice.into()),
                None => Self::Empty,
            },
            Row::ThemeOption(theme) => Self::Choice(Choice::ColorTheme(theme).into()),
            Row::TerminalTheme => {
                Self::Choice(SettingsChoice::TerminalTheme(values.terminal_display_theme))
            }
            Row::Foreground => Self::Text(&values.custom_foreground),
            Row::Background => Self::Text(&values.custom_background),
            Row::QuotaResumePrompt => Self::Text(&values.quota_resume_prompt),
            Row::Reminders => Self::Choice(SettingsChoice::AgentReminders(
                values.agent_notification_mode,
            )),
            Row::CompletionReminders => Self::Choice(SettingsChoice::CompletionReminders(
                values.completion_reminder_mode,
            )),
            Row::CompletionThreshold => {
                Self::Seconds(u64::from(values.completion_reminder_minimum_seconds))
            }
            Row::Scrollback => {
                Self::ScrollbackMegabytes(super::scrollback_megabytes(values.scrollback_bytes))
            }
            Row::PrefixKey => match page.text_value(row) {
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
            Row::Version => Self::Text(crate::local_client::CLIENT_VERSION),
            _ => Self::Empty,
        }
    }
}
