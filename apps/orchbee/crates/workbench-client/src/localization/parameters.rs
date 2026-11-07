//! Text parameters shared by action owners and localized presentation.
//! They carry no captured command, binding selection, capability or effect.

/// Geometric direction named by panel action labels and their owning interaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelDirection {
    Down,
    Left,
    Right,
    Up,
}

/// Semantic operation category carried by Client refusal notices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendAction {
    ReloadConfiguration,
    CreateWorkspace,
    QuickCreateWorkspace,
    CreateWorktree,
    OpenWorktree,
    CloseWorkspace,
    ConfigureWorkspaceDirectory,
    RenameWorkspace,
    RenameProject,
    RenameTab,
    CloseTab,
    MovePreviousTab,
    MoveNextTab,
    CreateTab,
    SelectNextTab,
    SelectPreviousTab,
    SelectTab(u8),
    SelectPreviousContext,
    SelectNextContext,
    SplitPanel,
    ClosePanel,
    MovePanel,
}

/// Client chrome feedback; action selection and delivery remain with the feature owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Notice {
    OnboardingSaveFailed,
    WorkspaceNavigationUnavailable,
    /// Open notification target found no current Agent card; nothing moved.
    NoNotificationTarget,
    NavigatorChanged,
    NavigatorTooSmall,
    ResizeBusy,
    ResizeFailed,
    ResizeUnknown,
    ResizeContended,
    ResizeNoSplit,
    ResizeLimit,
    CheckpointRecovered,
    CheckpointEmpty,
    ConfigurationPublished,
    ConfigurationWarning,
    PreferencesUnreadable,
    ConfigurationFailed,
    ConfigurationUnknown,
    WindowTitleUnavailable,
    WindowTitleUnsupported,
    NotificationUnavailable,
    NotificationUnsupported,
    BackendUnavailable(BackendAction),
    TerminalInputUnavailable,
    /// Terminal input arrived while a requested navigation had not landed and was not sent.
    NavigationInputHeld,
    UnknownPrefix,
    /// A pointer gesture's queued input was dropped: the pending-focus
    /// queue overflowed or its bounded wait expired.
    PointerInputDropped,
}
