//! The same common preferences, with a literal-argument launch-profile editor.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Row {
    Language,
    Theme,
    LightTheme,
    DarkTheme,
    ColorSource,
    TerminalTheme,
    Foreground,
    Background,
    CopyOnSelect,
    TrimAgents,
    ResumeAgents,
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
    pub(crate) fn content_rows(self) -> &'static [Row] {
        let rows = self.rows();
        &rows[..rows.len() - self.footer_rows().len()]
    }

    pub(crate) const fn footer_rows(self) -> &'static [Row] {
        match self {
            Self::General => &[Row::ResetAll, Row::Reload, Row::Save],
            Self::Profiles => &[Row::ResetProfiles, Row::Back],
        }
    }
    pub(crate) const fn rows(self) -> &'static [Row] {
        match self {
            Self::General => &[
                Row::Language,
                Row::Theme,
                Row::LightTheme,
                Row::DarkTheme,
                Row::ColorSource,
                Row::TerminalTheme,
                Row::Foreground,
                Row::Background,
                Row::CopyOnSelect,
                Row::TrimAgents,
                Row::ResumeAgents,
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
    pub(super) const fn mask(self) -> u64 {
        const fn bit(row: Row) -> u64 {
            let index = row as u32;
            assert!(
                index < u64::BITS,
                "SettingsDraw painted mask capacity exceeded"
            );
            1_u64 << index
        }
        // Exhaustiveness requires new variants to join this mapping. Every bit is evaluated at
        // compile time, so extending the enum past the receipt capacity cannot defer a bad shift.
        match self {
            Self::Language => const { bit(Row::Language) },
            Self::Theme => const { bit(Row::Theme) },
            Self::LightTheme => const { bit(Row::LightTheme) },
            Self::DarkTheme => const { bit(Row::DarkTheme) },
            Self::ColorSource => const { bit(Row::ColorSource) },
            Self::TerminalTheme => const { bit(Row::TerminalTheme) },
            Self::Foreground => const { bit(Row::Foreground) },
            Self::Background => const { bit(Row::Background) },
            Self::CopyOnSelect => const { bit(Row::CopyOnSelect) },
            Self::TrimAgents => const { bit(Row::TrimAgents) },
            Self::ResumeAgents => const { bit(Row::ResumeAgents) },
            Self::Reminders => const { bit(Row::Reminders) },
            Self::CompletionReminders => const { bit(Row::CompletionReminders) },
            Self::CompletionThreshold => const { bit(Row::CompletionThreshold) },
            Self::Sound => const { bit(Row::Sound) },
            Self::Scrollback => const { bit(Row::Scrollback) },
            Self::LaunchProfiles => const { bit(Row::LaunchProfiles) },
            Self::ResetAll => const { bit(Row::ResetAll) },
            Self::Reload => const { bit(Row::Reload) },
            Self::Save => const { bit(Row::Save) },
            Self::Profile => const { bit(Row::Profile) },
            Self::ProfileName => const { bit(Row::ProfileName) },
            Self::CommandType => const { bit(Row::CommandType) },
            Self::Executable => const { bit(Row::Executable) },
            Self::Login => const { bit(Row::Login) },
            Self::Argument => const { bit(Row::Argument) },
            Self::ArgumentValue => const { bit(Row::ArgumentValue) },
            Self::AddArgument => const { bit(Row::AddArgument) },
            Self::DeleteArgument => const { bit(Row::DeleteArgument) },
            Self::SetDefault => const { bit(Row::SetDefault) },
            Self::AddProfile => const { bit(Row::AddProfile) },
            Self::DeleteProfile => const { bit(Row::DeleteProfile) },
            Self::ResetProfiles => const { bit(Row::ResetProfiles) },
            Self::Back => const { bit(Row::Back) },
        }
    }
}
