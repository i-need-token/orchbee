//! Copy inputs selected by the Settings page. No page state, token or persistence authority.
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, SettingsLocale,
    TerminalDisplayAppearance,
};

/// One already-selected canonical finite value, without its open list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsChoice {
    Locale(SettingsLocale),
    ApplicationTheme(ApplicationAppearance),
    /// A shipped theme id; names stay untranslated except the Orchbee pair.
    ColorTheme(&'static str),
    TerminalTheme(TerminalDisplayAppearance),
    AgentReminders(AgentReminderMode),
    CompletionReminders(CompletionReminderMode),
    NewTerminalFolder(StartFolder),
}

/// The entries of the new terminal folder choice (`terminal.new_cwd`), in their order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartFolder {
    /// `workspace`.
    Workspace,
    /// `home`.
    Home,
    /// `follow`, which unset and empty mean too.
    Follow,
    /// A folder the person types; any other stored text shows here.
    Custom,
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
    /// About's Check for updates.
    Check,
    /// About's Update control.
    Update,
}
