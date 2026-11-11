//! The page's rows: the common settings, the launch-profile editor and Agent integrations.
use super::choices::ColorTheme;
use orchbee_agent_integration::IntegrationId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Row {
    Language,
    Theme,
    /// The light slot's identity; the Themes category lists its themes as [`Row::ThemeOption`].
    LightTheme,
    DarkTheme,
    ColorSource,
    TerminalTheme,
    Foreground,
    Background,
    CopyOnSelect,
    TrimAgents,
    ResumeAgents,
    RestoreOutput,
    Reminders,
    CompletionReminders,
    CompletionThreshold,
    Sound,
    Scrollback,
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
    /// Opens Agent integrations; it holds no setting.
    AgentIntegrations,
    /// One Agent's integration switch; changing it runs a Worker job at once.
    Agent(IntegrationId),
    /// The manual-only explanation for remote hosts.
    RemoteHosts,
    RefreshIntegrations,
}

/// The categories, or a child page that opens from one of their rows and returns to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Page {
    General,
    Profiles,
    Integrations,
    /// One Agent's read-only explain result.
    Diagnosis(IntegrationId),
}

const INTEGRATION_ROWS: [Row; IntegrationId::ALL.len() + 3] = {
    let mut rows = [Row::Back; IntegrationId::ALL.len() + 3];
    let mut index = 0;
    while index < IntegrationId::ALL.len() {
        rows[index] = Row::Agent(IntegrationId::ALL[index]);
        index += 1;
    }
    rows[index] = Row::RemoteHosts;
    rows[index + 1] = Row::RefreshIntegrations;
    rows
};

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
            Self::Integrations => &INTEGRATION_ROWS,
            Self::Diagnosis(_) => &[Row::Back],
        }
    }

    pub(crate) const fn is_child(self) -> bool {
        !matches!(self, Self::General)
    }

    pub(crate) const fn is_integrations(self) -> bool {
        matches!(self, Self::Integrations | Self::Diagnosis(_))
    }

    /// The row that opened this child page, whose name titles it.
    pub(crate) const fn opener(self) -> Row {
        match self {
            Self::General | Self::Profiles => Row::LaunchProfiles,
            Self::Integrations | Self::Diagnosis(_) => Row::AgentIntegrations,
        }
    }
}
