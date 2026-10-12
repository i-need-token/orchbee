//! The same common preferences, with a literal-argument launch-profile editor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Row {
    Language,
    Theme,
    TerminalTheme,
    Foreground,
    Background,
    CopyOnSelect,
    TrimAgents,
    Reminders,
    CompletionReminders,
    CompletionThreshold,
    Sound,
    Scrollback,
    LaunchProfiles,
    ResetAll,
    Reload,
    Save,
    Profile,
    ProfileName,
    CommandType,
    Executable,
    Login,
    Argument,
    ArgumentValue,
    AddArgument,
    DeleteArgument,
    SetDefault,
    AddProfile,
    DeleteProfile,
    ResetProfiles,
    Back,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Page {
    General,
    Profiles,
}
impl Page {
    pub(crate) const fn rows(self) -> &'static [Row] {
        match self {
            Self::General => &[
                Row::Language,
                Row::Theme,
                Row::TerminalTheme,
                Row::Foreground,
                Row::Background,
                Row::CopyOnSelect,
                Row::TrimAgents,
                Row::Reminders,
                Row::CompletionReminders,
                Row::CompletionThreshold,
                Row::Sound,
                Row::Scrollback,
                Row::LaunchProfiles,
                Row::ResetAll,
                Row::Reload,
                Row::Save,
            ],
            Self::Profiles => &[
                Row::Profile,
                Row::ProfileName,
                Row::CommandType,
                Row::Executable,
                Row::Login,
                Row::Argument,
                Row::ArgumentValue,
                Row::AddArgument,
                Row::DeleteArgument,
                Row::SetDefault,
                Row::AddProfile,
                Row::DeleteProfile,
                Row::ResetProfiles,
                Row::Back,
            ],
        }
    }
}
impl Row {
    pub(crate) const fn text_limit(self) -> Option<usize> {
        match self {
            Self::Foreground | Self::Background => Some(7),
            Self::CompletionThreshold | Self::Scrollback => Some(10),
            Self::ProfileName => Some(512),
            Self::Executable | Self::ArgumentValue => Some(32 * 1024),
            _ => None,
        }
    }
}
