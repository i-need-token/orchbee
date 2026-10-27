//! Text parameters shared by action owners and localized presentation.
//! They carry no captured command, binding selection, capability or effect.

/// Geometric direction named by panel action labels and their owning interaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PanelDirection {
    Down,
    Left,
    Right,
    Up,
}

/// Semantic operation category carried by Client refusal notices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackendAction {
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
pub(crate) enum Notice {
    OnboardingSaveFailed,
    WorkspaceNavigationUnavailable,
    NavigatorChanged,
    NavigatorTooSmall,
    ResizeBusy,
    ResizeFailed,
    ResizeUnknown,
    ResizeNoSplit,
    ResizeLimit,
    CheckpointRecovered,
    CheckpointEmpty,
    ConfigurationPublished,
    ConfigurationWarning,
    ConfigurationFailed,
    ConfigurationUnknown,
    WindowTitleUnavailable,
    WindowTitleUnsupported,
    NotificationUnavailable,
    NotificationUnsupported,
    BackendUnavailable(BackendAction),
    TerminalInputUnavailable,
    UnknownPrefix,
    /// A pointer gesture's queued input was dropped: the pending-focus
    /// queue overflowed or its bounded wait expired.
    PointerInputDropped,
}
