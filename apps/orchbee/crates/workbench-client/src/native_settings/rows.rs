//! The same common preferences, a literal-argument launch-profile editor, and Agent integrations.
use orchbee_agent_integration::IntegrationId;

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
    RestoreOutput,
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
    /// The General entry into Agent integrations; it holds no preference.
    AgentIntegrations,
    /// One Agent's integration switch; changing it runs a Worker job at once.
    Agent(IntegrationId),
    /// The manual-only explanation for remote hosts.
    RemoteHosts,
    RefreshIntegrations,
}
/// Immediate integration actions live on their own pages, outside the atomic preference draft.
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
    pub(crate) fn content_rows(self) -> &'static [Row] {
        let rows = self.rows();
        &rows[..rows.len() - self.footer_rows().len()]
    }

    pub(crate) const fn footer_rows(self) -> &'static [Row] {
        match self {
            Self::General => &[Row::ResetAll, Row::Reload, Row::Save],
            Self::Profiles => &[Row::ResetProfiles, Row::Back],
            Self::Integrations => &[Row::RefreshIntegrations, Row::Back],
            Self::Diagnosis(_) => &[Row::Back],
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
                Row::RestoreOutput,
                Row::Reminders,
                Row::CompletionReminders,
                Row::CompletionThreshold,
                Row::Sound,
                Row::Scrollback,
                Row::LaunchProfiles,
                Row::AgentIntegrations,
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
            Self::Integrations => &INTEGRATION_ROWS,
            Self::Diagnosis(_) => &[Row::Back],
        }
    }

    /// Child pages that preserve the General draft and return to it.
    pub(crate) const fn is_child(self) -> bool {
        !matches!(self, Self::General)
    }

    pub(crate) const fn is_integrations(self) -> bool {
        matches!(self, Self::Integrations | Self::Diagnosis(_))
    }
}
/// Fieldless rows take the first bits; each Agent row follows in canonical order.
const AGENT_BASE: u32 = 38;
const _: () = assert!(
    AGENT_BASE + IntegrationId::ALL.len() as u32 <= u64::BITS,
    "SettingsDraw painted mask capacity exceeded"
);
impl Row {
    pub(super) fn mask(self) -> u64 {
        let index = match self {
            Self::Language => 0,
            Self::Theme => 1,
            Self::LightTheme => 2,
            Self::DarkTheme => 3,
            Self::ColorSource => 4,
            Self::TerminalTheme => 5,
            Self::Foreground => 6,
            Self::Background => 7,
            Self::CopyOnSelect => 8,
            Self::TrimAgents => 9,
            Self::ResumeAgents => 10,
            Self::RestoreOutput => 11,
            Self::Reminders => 12,
            Self::CompletionReminders => 13,
            Self::CompletionThreshold => 14,
            Self::Sound => 15,
            Self::Scrollback => 16,
            Self::LaunchProfiles => 17,
            Self::ResetAll => 18,
            Self::Reload => 19,
            Self::Save => 20,
            Self::Profile => 21,
            Self::ProfileName => 22,
            Self::CommandType => 23,
            Self::Executable => 24,
            Self::Login => 25,
            Self::Argument => 26,
            Self::ArgumentValue => 27,
            Self::AddArgument => 28,
            Self::DeleteArgument => 29,
            Self::SetDefault => 30,
            Self::AddProfile => 31,
            Self::DeleteProfile => 32,
            Self::ResetProfiles => 33,
            Self::Back => 34,
            Self::AgentIntegrations => 35,
            Self::RemoteHosts => 36,
            Self::RefreshIntegrations => 37,
            Self::Agent(id) => {
                AGENT_BASE
                    + IntegrationId::ALL
                        .iter()
                        .position(|candidate| *candidate == id)
                        .and_then(|position| u32::try_from(position).ok())
                        .expect("canonical integration identity")
            }
        };
        1_u64 << index
    }
}
