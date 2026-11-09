//! Immutable presentation facts shared by Workbench projection and text.
//! No executable capture, mutable feature owner, transport or view is retained here.

/// Selected Workbench display phase, independent of its mutable connection and command owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayPhase {
    Starting,
    Empty,
    Ready,
    Creating,
    CreateCommittedNeedDetach,
    ProjectionUnavailable,
    MoveProjectionUnavailable,
    PanelProjectionUnavailable,
    RenameProjectionUnavailable,
    Switching,
    SelectedWithoutAttachment,
    RetryableAttachUnavailable,
    Retired,
}

/// Observed input-control role for the selected terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DisplayController {
    Controlling,
    ReadOnly,
}

/// Observed launch or binding provenance; no value authorizes another launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DisplayProvenance {
    Started,
    Pending,
    Unavailable,
    Failed,
    OutcomeUnknown,
    BindingOnly,
}

/// Selected Client feedback; its semantic values retain no executable operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchClientNotice {
    Unsupported(crate::host_capabilities::Capability),
    RecordUnusable(
        orchbee_local_runtime::VersionedRecord,
        orchbee_local_runtime::RecordProblem,
    ),
    Settings(super::super::SettingsNotice),
    MenuUnavailable,
    LocalBusy,
    NoTabAtPosition(u8),
    CreateNotCreated,
    CreateOrderConflict,
    LaunchRequestTooLarge,
    DuplicatePending,
    DuplicateNotStarted,
    DuplicateOutcomeUnknown,
    MoveNotMoved,
    MoveOrderConflict,
    PanelNotApplied,
    RenameNotRenamed,
    TerminalClearing,
    TerminalCleared,
    TerminalClearUnconfirmed,
    TerminalClearRefused,
    TerminalClearUnavailable,
    TerminalClearUnknown,
    ScrollbackEditorUnavailable,
    ScrollbackEditorTooSmall,
    CloseRequestNotApplied,
    WorkspaceCloseUnavailable,
    WorkspaceChanged,
    AgentUnavailable,
    NoWorkspace,
    WorkspaceNotReady,
    ConfirmationCancelled,
    DraftCancelled,
    DialogRefused,
    ModalOwnsInput,
    AgentStart(super::super::AgentStartStatus),
    HistoryRecovering,
    HistoryUnavailable,
    HistoryPositionLost,
    CopyModeUnavailable,
    CopyModeBusy,
    CopyModeInvalidated,
    CopyModeQueryFailed,
    CopyModeNoMatch,
    CopyModeSearchLimit,
    CopyModeNoSelection,
    SelectionEvicted,
    CopyFailed,
    CopyUnknown,
    BellFailed,
    BellUnknown,
    PositionChanged,
    PositionChangedEmpty,
    PositionLocationChanged,
    PositionLocationChangedEmpty,
    PositionUnavailable,
    PopupUnavailable,
    CommandAccepted,
    CommandFailed,
    CommandUnknown,
    CommandRefused(orchbee_workbench_protocol::ConfiguredCommandFailure),
    LaunchProfileSelected,
    LaunchProfileUnavailable,
    /// A held restore's Retry found the saved directory still unavailable.
    RestoreDirectoryUnavailable,
    /// A held restore started in the home folder, so its Agent conversation was not resumed.
    RestoreAgentNotResumed,
    /// A held restore's terminal did not start for another reason.
    RestoreNotStarted,
    /// The Server gave up resuming an Agent conversation after a restart and typed nothing. The
    /// Agent is `None` when this Client does not know the provider.
    AgentNotResumed(
        Option<crate::agent_provider::IntegrationId>,
        orchbee_workbench_protocol::AgentNotResumedReason,
    ),
    /// What an earlier-layout list or reopen did.
    EarlierLayout(super::super::EarlierLayoutNotice),
    LinkOpened,
    LinkHandled,
    LinkNoLink,
    LinkBusy,
    LinkFailed,
    LinkUnknown,
}

/// Snapshot refresh feedback with the last known revision when one exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RefreshStatus {
    Refreshing,
    Success(u64),
    Failure(u64),
}

/// Current availability of retained terminal display data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientProjectionStatus {
    Empty,
    FrameAvailable,
    WaitingForFullFrame,
    Blocked,
}

/// Why display freshness was retired; unknown never authorizes replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayRetirement {
    LaunchRequestTooLarge,
    Unavailable,
    OutcomeUnknown,
}

/// Display context for one local notice; it retains no executable text or launch authority.
pub(crate) enum NoticeContext {
    /// A missing name is the localized built-in System default label.
    LaunchProfile(Option<String>),
    ConfiguredCommand(String),
}

/// Bounded launch facts for a failed member; no command arguments or environment.
#[derive(Clone, Copy)]
pub(crate) struct LaunchFailure<'a> {
    pub(crate) profile: LaunchProfile<'a>,
    pub(crate) executable: Option<&'a str>,
}

/// Display-only profile description for a failed terminal launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LaunchProfile<'a> {
    Named(&'a str),
    SystemDefault,
    Unknown,
}

/// Terminal connection feedback selected by the attachment owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStatus {
    Connecting,
    Controlling,
    ReadOnly,
    Blocked,
    Unavailable,
    OutcomeUnknown,
}

/// Why a retained Tab duplication stopped, as the status line names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DuplicateStop {
    Changed,
    Deadline,
    NotCreated,
    TerminalNotStarted,
    Recovery,
}

/// Confirmed duplication counts and termination facts for one status line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DuplicateProgress {
    pub(crate) total: usize,
    pub(crate) submitted: usize,
    pub(crate) committed: usize,
    pub(crate) started: usize,
    pub(crate) stop: Option<DuplicateStop>,
    pub(crate) complete: bool,
}
