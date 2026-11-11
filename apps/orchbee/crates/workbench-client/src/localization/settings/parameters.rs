//! Copy inputs selected by the Settings page. No page state, token or persistence authority.
use super::IntegrationStatus;
use orchbee_agent_integration::IntegrationId;
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, SettingsLocale,
    TerminalDisplayAppearance, TuiColorSource,
};

/// Semantic row description; focus, availability and navigation remain the page's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsRow {
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
    NewTerminalFolder,
    PrefixKey,
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
    Back,
    AgentIntegrations,
    Agent(IntegrationId),
    RemoteHosts,
    RefreshIntegrations,
}

/// One entry of the page's left column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsCategory {
    General,
    Appearance,
    Themes,
    Terminal,
    Notifications,
    Keyboard,
    Agents,
}

/// One group heading; some groups have none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsGroup {
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
    AgentIntegrations,
    LocalAgents,
    RemoteHosts,
}

/// One already-selected canonical finite value, without its open list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsChoice {
    Locale(SettingsLocale),
    ApplicationTheme(ApplicationAppearance),
    /// A shipped theme id; names stay untranslated except the Orchbee pair.
    ColorTheme(&'static str),
    ColorSource(TuiColorSource),
    TerminalTheme(TerminalDisplayAppearance),
    AgentReminders(AgentReminderMode),
    CompletionReminders(CompletionReminderMode),
}

/// A settings file the page cannot use, for its file line; the path is the Server's spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsFileFact<'a> {
    SyntaxError {
        path: Option<&'a str>,
        line: u32,
        column: u32,
    },
    Unreadable {
        path: Option<&'a str>,
    },
}

/// A selected row value; only the displayed value crosses the text boundary.
pub(crate) enum SettingsValue<'a> {
    Choice(SettingsChoice),
    Text(&'a str),
    Toggle(bool),
    Seconds(u64),
    ScrollbackMegabytes(String),
    ProfileName(Option<&'a str>),
    SystemShell,
    CustomExecutable,
    Required,
    Dash,
    Unavailable,
    ArgumentPosition { position: usize, total: usize },
    Default,
    Empty,
    Integration(IntegrationStatus),
}

/// Selected profile name/fallback only; no launch configuration or profile identity is retained.
#[derive(Clone, Copy)]
pub(crate) enum SettingsProfileLabel<'a> {
    Text(&'a str),
    SystemDefault,
}

/// The exact field rule selected by validation and current profile context.
#[derive(Clone, Copy)]
pub(crate) enum SettingsFieldRequirement {
    SystemName,
    Name,
    Executable,
    Argument,
}

/// A field rule or one page-selected explanation; text does not inspect the page.
#[derive(Clone, Copy)]
pub(crate) enum SettingsDetail<'a> {
    Field {
        name: SettingsProfileLabel<'a>,
        requirement: SettingsFieldRequirement,
    },
    Message(SettingsDetailKind),
}

/// The launch-profile editor's line, in confirmation, validation and availability order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsDetailKind {
    ConfirmDeleteProfile,
    InvalidValue,
    CustomColorsRequired,
    TruecolorRequired,
    LoginUnsupported,
    CustomExecutableRequired,
    MinimumProfile,
    MaximumProfiles,
    AlreadyDefault,
    Unavailable,
    LiteralArguments,
}

/// What a row editor's typed value must be, named when the page refuses one before sending it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SettingsTextRequirement {
    Color,
    WholeNumber { minimum: u64, maximum: u64 },
    Number { minimum: String, maximum: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingUnit {
    Seconds,
    Bytes,
}

/// Semantic label paired with one group of keys the page answers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsHintLabel {
    Close,
    Back,
    Cancel,
    Confirm,
    Accept,
    Move,
    Switch,
    Open,
    Change,
    Toggle,
    Select,
    Search,
    GoTo,
    Restore,
    Explain,
}
