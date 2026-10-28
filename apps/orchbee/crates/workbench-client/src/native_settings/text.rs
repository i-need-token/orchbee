//! Selects text descriptions from admitted Settings drafts and metadata.
//! The draft/token/enablement and saved profile owners remain here; text gets only selected facts.

use super::{
    Choice, Confirmation, DraftView, Page, Reconciliation, Row, SettingCategory, setting_for_row,
};
use crate::localization::{
    SettingsCategory, SettingsChoice, SettingsDetail, SettingsDetailKind, SettingsDraftState,
    SettingsFieldRequirement, SettingsProfileLabel, SettingsRow, SettingsValue,
};
use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchField};

impl From<Row> for SettingsRow {
    fn from(row: Row) -> Self {
        match row {
            Row::Language => Self::Language,
            Row::Theme => Self::Theme,
            Row::TerminalTheme => Self::TerminalTheme,
            Row::Foreground => Self::Foreground,
            Row::Background => Self::Background,
            Row::CopyOnSelect => Self::CopyOnSelect,
            Row::TrimAgents => Self::TrimAgents,
            Row::Reminders => Self::Reminders,
            Row::CompletionReminders => Self::CompletionReminders,
            Row::CompletionThreshold => Self::CompletionThreshold,
            Row::Sound => Self::Sound,
            Row::Scrollback => Self::Scrollback,
            Row::LaunchProfiles => Self::LaunchProfiles,
            Row::ResetAll => Self::ResetAll,
            Row::Reload => Self::Reload,
            Row::Save => Self::Save,
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
            Row::ResetProfiles => Self::ResetProfiles,
            Row::Back => Self::Back,
        }
    }
}

impl From<SettingCategory> for SettingsCategory {
    fn from(category: SettingCategory) -> Self {
        match category {
            SettingCategory::Appearance => Self::Appearance,
            SettingCategory::Terminal => Self::Terminal,
            SettingCategory::Notifications => Self::Notifications,
            SettingCategory::LaunchProfiles => Self::LaunchProfiles,
        }
    }
}

impl From<Choice> for SettingsChoice {
    fn from(choice: Choice) -> Self {
        match choice {
            Choice::Locale(value) => Self::Locale(value),
            Choice::ApplicationTheme(value) => Self::ApplicationTheme(value),
            Choice::TerminalTheme(value) => Self::TerminalTheme(value),
            Choice::AgentReminders(value) => Self::AgentReminders(value),
            Choice::CompletionReminders(value) => Self::CompletionReminders(value),
        }
    }
}

impl From<Row> for SettingsCategory {
    fn from(row: Row) -> Self {
        if let Some(metadata) = setting_for_row(row) {
            return metadata.category.into();
        }
        match row {
            Row::ResetAll | Row::Reload | Row::Save => Self::General,
            Row::Profile
            | Row::ProfileName
            | Row::CommandType
            | Row::Executable
            | Row::Login
            | Row::Argument
            | Row::ArgumentValue
            | Row::AddArgument
            | Row::DeleteArgument
            | Row::SetDefault
            | Row::AddProfile
            | Row::DeleteProfile
            | Row::ResetProfiles
            | Row::Back => Self::LaunchProfiles,
            _ => unreachable!("metadata-backed setting row resolved above"),
        }
    }
}

impl From<DraftView<'_>> for SettingsDraftState {
    fn from(draft: DraftView<'_>) -> Self {
        match draft.reconciliation {
            Some(Reconciliation::OutcomeUnknown) => Self::Unknown,
            Some(Reconciliation::SourceChanged) => Self::SourceChanged,
            None if draft.conflict => Self::SourceChanged,
            None if draft.dirty => Self::Dirty,
            None => Self::Clean,
        }
    }
}

fn detail_kind(draft: DraftView<'_>) -> SettingsDetailKind {
    if let Some(confirmation) = draft.confirmation {
        return match confirmation {
            Confirmation::ResetAll => SettingsDetailKind::ConfirmResetAll,
            Confirmation::ResetProfiles => SettingsDetailKind::ConfirmResetProfiles,
            Confirmation::Reload => SettingsDetailKind::ConfirmReload,
            Confirmation::DeleteProfile => SettingsDetailKind::ConfirmDeleteProfile,
        };
    }
    if draft.invalid {
        return SettingsDetailKind::InvalidValue;
    }
    if !draft.enabled(draft.selected) {
        return match draft.selected {
            Row::Save => match draft.reconciliation {
                Some(Reconciliation::SourceChanged) => SettingsDetailKind::SourceChangedBeforeSave,
                Some(Reconciliation::OutcomeUnknown) => SettingsDetailKind::UnknownSave,
                None if draft.conflict => SettingsDetailKind::ConflictBeforeSave,
                None => SettingsDetailKind::NoChanges,
            },
            Row::Foreground | Row::Background => SettingsDetailKind::CustomColorsRequired,
            Row::Login if !draft.login_shell_supported => SettingsDetailKind::LoginUnsupported,
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
    if draft.page == Page::Profiles {
        return SettingsDetailKind::LiteralArguments;
    }
    if draft.reconciliation == Some(Reconciliation::OutcomeUnknown) {
        return SettingsDetailKind::UnknownPreview;
    }
    match draft.selected {
        Row::Language | Row::Theme | Row::TerminalTheme | Row::Foreground | Row::Background => {
            SettingsDetailKind::LocalPreview
        }
        Row::CopyOnSelect => SettingsDetailKind::CopyAfterSave,
        Row::TrimAgents => SettingsDetailKind::TrimAgentsAfterSave,
        Row::Reminders => SettingsDetailKind::AgentNotificationsAfterSave,
        Row::CompletionReminders | Row::CompletionThreshold => {
            SettingsDetailKind::CompletionAfterSave
        }
        Row::Sound => SettingsDetailKind::SoundAfterSave,
        Row::Scrollback => SettingsDetailKind::ScrollbackAfterSave,
        Row::LaunchProfiles => SettingsDetailKind::LaunchProfilesAfterSave,
        _ => SettingsDetailKind::DeviceSettings,
    }
}

impl<'a> From<DraftView<'a>> for SettingsDetail<'a> {
    fn from(draft: DraftView<'a>) -> Self {
        if draft.confirmation.is_none() {
            let field = draft.invalid_field.or_else(|| {
                if draft.page != Page::Profiles || !draft.enabled(draft.selected) {
                    return None;
                }
                let field = match draft.selected {
                    Row::ProfileName => UserLaunchField::Name,
                    Row::Executable => UserLaunchField::Executable,
                    Row::ArgumentValue => UserLaunchField::Argument(draft.argument),
                    _ => return None,
                };
                Some((draft.profile, field))
            });
            if let Some((index, field)) = field {
                let profile = &draft.values.launch_profiles.profiles[index];
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
        Self::Message(detail_kind(draft))
    }
}

impl<'a> From<(DraftView<'a>, Row)> for SettingsValue<'a> {
    fn from((draft, row): (DraftView<'a>, Row)) -> Self {
        let values = draft.values;
        let profile = &values.launch_profiles.profiles[draft.profile];
        match row {
            Row::Language => Self::Choice(SettingsChoice::Locale(values.locale)),
            Row::Theme => Self::Choice(SettingsChoice::ApplicationTheme(values.application_theme)),
            Row::TerminalTheme => {
                Self::Choice(SettingsChoice::TerminalTheme(values.terminal_display_theme))
            }
            Row::Foreground => Self::Text(&values.custom_foreground),
            Row::Background => Self::Text(&values.custom_background),
            Row::CopyOnSelect => Self::Toggle(values.copy_on_select),
            Row::TrimAgents => Self::Toggle(values.trim_trailing_blank_rows_for_agents),
            Row::Reminders => Self::Choice(SettingsChoice::AgentReminders(
                values.agent_notification_mode,
            )),
            Row::CompletionReminders => Self::Choice(SettingsChoice::CompletionReminders(
                values.completion_reminder_mode,
            )),
            Row::CompletionThreshold => {
                Self::Integer(u64::from(values.completion_reminder_minimum_seconds))
            }
            Row::Sound => Self::Toggle(values.sound_enabled),
            Row::Scrollback => {
                Self::ScrollbackMegabytes(super::scrollback_megabytes(values.scrollback_bytes))
            }
            Row::LaunchProfiles => match values
                .launch_profiles
                .profiles
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
                UserLaunchCommand::SystemShell { login } if draft.login_shell_supported => {
                    Self::Toggle(*login)
                }
                UserLaunchCommand::SystemShell { .. }
                | UserLaunchCommand::CustomExecutable { .. } => Self::Unavailable,
            },
            Row::Argument => match &profile.command {
                UserLaunchCommand::CustomExecutable { arguments, .. } if !arguments.is_empty() => {
                    Self::ArgumentPosition {
                        position: draft.argument + 1,
                        total: arguments.len(),
                    }
                }
                UserLaunchCommand::SystemShell { .. }
                | UserLaunchCommand::CustomExecutable { .. } => Self::Dash,
            },
            Row::ArgumentValue => match &profile.command {
                UserLaunchCommand::CustomExecutable { arguments, .. } => arguments
                    .get(draft.argument)
                    .map_or(Self::Dash, |value| Self::Text(value)),
                UserLaunchCommand::SystemShell { .. } => Self::Dash,
            },
            Row::SetDefault if profile.id == values.launch_profiles.default_profile_id => {
                Self::Default
            }
            _ => Self::Empty,
        }
    }
}
