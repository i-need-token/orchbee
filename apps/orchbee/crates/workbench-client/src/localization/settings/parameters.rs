//! Copy inputs selected by the Settings feature. No draft, token, picker, profile or persistence authority.
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, PreferencesLocale,
    TerminalDisplayAppearance,
};

/// Semantic row description; drawing masks, enablement and row navigation remain feature-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsRow {
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

/// The selected category label, including the general action footer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsCategory {
    General,
    Appearance,
    Terminal,
    Notifications,
    LaunchProfiles,
}

/// One already-selected canonical finite value, without its mutable choice picker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsChoice {
    Locale(PreferencesLocale),
    ApplicationTheme(ApplicationAppearance),
    TerminalTheme(TerminalDisplayAppearance),
    AgentReminders(AgentReminderMode),
    CompletionReminders(CompletionReminderMode),
}

/// Shared save-feedback priority selected from the draft owner's reconciliation facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsDraftState {
    Unknown,
    SourceChanged,
    Dirty,
    Clean,
}

/// A selected row value; only the displayed value crosses the text boundary.
pub(crate) enum SettingsValue<'a> {
    Choice(SettingsChoice),
    Text(&'a str),
    Toggle(bool),
    Integer(u64),
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

/// A field rule or one feature-selected explanation; text does not inspect the draft.
#[derive(Clone, Copy)]
pub(crate) enum SettingsDetail<'a> {
    Field {
        name: SettingsProfileLabel<'a>,
        requirement: SettingsFieldRequirement,
    },
    Message(SettingsDetailKind),
}

/// Details selected in confirmation/validation/availability/preview order by the draft owner.
#[derive(Clone, Copy)]
pub(crate) enum SettingsDetailKind {
    ConfirmResetAll,
    ConfirmResetProfiles,
    ConfirmReload,
    ConfirmDeleteProfile,
    InvalidValue,
    SourceChangedBeforeSave,
    UnknownSave,
    ConflictBeforeSave,
    NoChanges,
    CustomColorsRequired,
    LoginUnsupported,
    CustomExecutableRequired,
    MinimumProfile,
    MaximumProfiles,
    AlreadyDefault,
    Unavailable,
    LiteralArguments,
    UnknownPreview,
    LocalPreview,
    CopyAfterSave,
    TrimAgentsAfterSave,
    AgentNotificationsAfterSave,
    CompletionAfterSave,
    SoundAfterSave,
    ScrollbackAfterSave,
    LaunchProfilesAfterSave,
    DeviceSettings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingUnit {
    Seconds,
    Bytes,
}

/// Semantic label paired with one locally effective Settings binding group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsHintLabel {
    Search,
    Save,
    Close,
    Back,
    Cancel,
    Confirm,
    Accept,
    Clear,
    ActivateEdit,
    Select,
    Change,
}
