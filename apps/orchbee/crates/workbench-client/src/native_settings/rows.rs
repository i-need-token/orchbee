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
const AGENT_BASE: u32 = 37;
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
            Self::Reminders => 11,
            Self::CompletionReminders => 12,
            Self::CompletionThreshold => 13,
            Self::Sound => 14,
            Self::Scrollback => 15,
            Self::LaunchProfiles => 16,
            Self::ResetAll => 17,
            Self::Reload => 18,
            Self::Save => 19,
            Self::Profile => 20,
            Self::ProfileName => 21,
            Self::CommandType => 22,
            Self::Executable => 23,
            Self::Login => 24,
            Self::Argument => 25,
            Self::ArgumentValue => 26,
            Self::AddArgument => 27,
            Self::DeleteArgument => 28,
            Self::SetDefault => 29,
            Self::AddProfile => 30,
            Self::DeleteProfile => 31,
            Self::ResetProfiles => 32,
            Self::Back => 33,
            Self::AgentIntegrations => 34,
            Self::RemoteHosts => 35,
            Self::RefreshIntegrations => 36,
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
