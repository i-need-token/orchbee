//! The page's rows: the common settings, the launch-profile editor and About.
use super::choices::ColorTheme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Row {
    Language,
    Theme,
    /// The light slot's identity; the Themes category lists its themes as [`Row::ThemeOption`].
    LightTheme,
    DarkTheme,
    TerminalTheme,
    Foreground,
    Background,
    CopyOnSelect,
    ResumeAgents,
    RestoreOutput,
    Reminders,
    CompletionReminders,
    CompletionThreshold,
    Sound,
    Scrollback,
    /// Whether the Server installs new versions in the background (`update.automatic`).
    AutomaticUpdates,
    /// Whether the Server answers Claude Code's routine confirmations
    /// (`agents.auto_allow.claude`).
    AutoAllowClaude,
    LaunchProfiles,
    /// Where a new terminal starts (`terminal.new_cwd`).
    NewTerminalFolder,
    /// The key pressed before a command key (`keys.prefix`).
    PrefixKey,
    /// One shipped theme of a Themes list; activating it puts it in use.
    ThemeOption(ColorTheme),
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
    /// Leaves a child page; leaving the launch-profile editor applies its catalog.
    Back,
    /// About: this Client's version.
    Version,
    /// About: Check for updates, or the Update control while a newer version is offered.
    Updates,
}

/// The categories, or a child page that opens from one of their rows and returns to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Page {
    General,
    Profiles,
}

impl Page {
    /// A child page's rows in order; the categories list theirs through the catalog.
    pub(crate) const fn rows(self) -> &'static [Row] {
        match self {
            Self::General => &[],
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
                Row::Back,
            ],
        }
    }

    pub(crate) const fn is_child(self) -> bool {
        !matches!(self, Self::General)
    }

    /// The row that opened this child page, whose name titles it.
    pub(crate) const fn opener(self) -> Row {
        match self {
            Self::General | Self::Profiles => Row::LaunchProfiles,
        }
    }
}
