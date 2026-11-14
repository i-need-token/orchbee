//! Action definitions shared by bindings, menus, capability policy, and discovery.
//!
//! These are plain typed intents. Execution owners keep exact targets, availability, and effects;
//! [`crate::action_projection`] projects their stable identity and discovery metadata.

use crate::localization::{BackendAction, PanelDirection};

/// Plain Client view intent; executable menu captures and their proofs remain with Workbench.
// Not `Copy`: `FocusPanel` carries a `WorkbenchPanelId` (heap identity).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientAction {
    TabMenu {
        action: MenuAction,
        tab_id: Option<orchbee_workbench_protocol::WorkbenchTabId>,
    },
    ToggleTabGroup(orchbee_workbench_protocol::WorkbenchTabGroupId),
    OpenActionMenu,
    OpenSettings,
    ConfiguredCommand(String),
    EnterResizeMode,
    EnterCopyMode,
    /// Opens the Find bar over the focused Panel.
    FindInTerminal,
    /// Copies the focused pane's settled Host selection without entering copy mode.
    CopySelection,
    ExitResizeMode,
    ResizePane {
        direction: PanelDirection,
        fine: bool,
    },
    ReloadConfiguration,
    RequestWorkspaceCreate,
    RequestQuickWorkspace,
    RequestWorkspaceAtDirectory,
    RequestProjectQuickWorkspace,
    RequestWorktreeCreate,
    RequestWorktreeOpen,
    RequestWorkspaceClose,
    RequestWorkspaceCloseAt(orchbee_workbench_protocol::WorkbenchWorkspaceId),
    RequestSidebarWorkspaceDelete,
    RequestControlTakeover,
    RequestWorkspaceRename,
    RequestProjectRename,
    RequestTargetedProjectRename,
    RequestWorkspaceOrganization,
    RequestProjectCloseAll(Option<orchbee_workbench_protocol::WorkbenchProjectId>),
    RequestRenameTab,
    RequestRenamePanel,
    ClearTerminal,
    /// Focuses the most recent previously focused Panel that still exists, in any Tab or
    /// Workspace of the Session Workbench.
    FocusLastPanel,
    /// Lists the Session's earlier layouts and opens the picker over them.
    RequestEarlierLayouts,
    /// Reopens what the earlier layout saved at this Unix millisecond has and the Workbench lacks.
    ReopenEarlierLayout(u64),
    RequestCloseTab,
    ConfirmClose,
    CancelClose,
    MovePreviousTab,
    MoveNextTab,
    ToggleBusinessSidebar,
    FocusBusinessSidebar,
    OpenShortcutHelp,
    CloseShortcutHelp,
    PageShortcutHelp,
    OpenWorkbenchNavigator,
    OpenMachines,
    PreviousAgent,
    NextAgent,
    FocusAgent(u8),
    /// Opens the Agent the visible notification card announced.
    OpenNotificationTarget,
    CloseWorkbenchNavigator,
    /// `Ctrl+B g` — toggles the Client-owned Agent board overlay.
    ToggleAgentBoard,
    /// `Ctrl+B G` — opens the Start Agent picker.
    StartAgent,
    CreateTab,
    SelectNextTab,
    SelectPreviousTab,
    SelectTab(u8),
    SelectPreviousContext,
    SelectNextContext,
    PreviousPresentation,
    NextPresentation,
    MovePanelFocus(PanelDirection),
    /// Pointer gesture: focus the panel with this stable identity in the
    /// active tab.
    FocusPanel(orchbee_workbench_protocol::WorkbenchPanelId),
    /// Client-local zoom on the active tab: draw the focused leaf across
    /// the whole pane area while hidden leaves stay attached. Volatile
    /// session view state — never a backend action.
    ToggleZoom,
    SplitPanel(PanelDirection),
    ClosePanel,
    MovePanel(PanelDirection),
    TogglePointerCapture,
    /// `Ctrl+B PageUp` while help is closed: enter retained-history mode on
    /// the focus pane. The session owns admission; an unenterable pane
    /// simply stays Live.
    EnterHistory,
    ForwardLiteralPrefix,
    DismissPrefix,
    UnknownPrefix,
    Exit,
}

impl ClientAction {
    pub(crate) fn backend_action(&self) -> Option<BackendAction> {
        match self {
            Self::OpenActionMenu | Self::FocusBusinessSidebar => None,
            Self::OpenSettings => None,
            Self::EnterResizeMode
            | Self::EnterCopyMode
            | Self::FindInTerminal
            | Self::CopySelection
            | Self::ExitResizeMode
            | Self::ResizePane { .. } => None,
            Self::ConfiguredCommand(_) => None,
            Self::OpenMachines
            | Self::PreviousAgent
            | Self::NextAgent
            | Self::FocusAgent(_)
            | Self::OpenNotificationTarget => None,
            Self::TabMenu { .. } | Self::ToggleTabGroup(_) => None,
            Self::ReloadConfiguration => Some(BackendAction::ReloadConfiguration),
            Self::RequestWorkspaceCreate => Some(BackendAction::CreateWorkspace),
            Self::RequestQuickWorkspace
            | Self::RequestWorkspaceAtDirectory
            | Self::RequestProjectQuickWorkspace => Some(BackendAction::QuickCreateWorkspace),
            Self::RequestWorktreeCreate => Some(BackendAction::CreateWorktree),
            Self::RequestWorktreeOpen => Some(BackendAction::OpenWorktree),
            Self::RequestWorkspaceClose => Some(BackendAction::CloseWorkspace),
            Self::RequestControlTakeover => None,
            Self::RequestWorkspaceRename => Some(BackendAction::RenameWorkspace),
            Self::RequestProjectRename | Self::RequestTargetedProjectRename => {
                Some(BackendAction::RenameProject)
            }
            Self::RequestWorkspaceCloseAt(_)
            | Self::RequestSidebarWorkspaceDelete
            | Self::RequestWorkspaceOrganization
            | Self::RequestProjectCloseAll(_) => None,
            Self::RequestRenameTab => Some(BackendAction::RenameTab),
            Self::RequestRenamePanel => None,
            Self::ClearTerminal | Self::FocusLastPanel => None,
            Self::RequestEarlierLayouts | Self::ReopenEarlierLayout(_) => None,
            Self::RequestCloseTab => Some(BackendAction::CloseTab),
            Self::CreateTab => Some(BackendAction::CreateTab),
            Self::MovePreviousTab => Some(BackendAction::MovePreviousTab),
            Self::MoveNextTab => Some(BackendAction::MoveNextTab),
            Self::SelectNextTab => Some(BackendAction::SelectNextTab),
            Self::SelectPreviousTab => Some(BackendAction::SelectPreviousTab),
            Self::SelectTab(position) => Some(BackendAction::SelectTab(*position)),
            Self::SelectPreviousContext => Some(BackendAction::SelectPreviousContext),
            Self::SelectNextContext => Some(BackendAction::SelectNextContext),
            Self::PreviousPresentation
            | Self::NextPresentation
            | Self::MovePanelFocus(_)
            | Self::FocusPanel(_)
            | Self::ToggleZoom => None,
            Self::SplitPanel(_) => Some(BackendAction::SplitPanel),
            Self::ClosePanel => Some(BackendAction::ClosePanel),
            Self::MovePanel(_) => Some(BackendAction::MovePanel),
            Self::ToggleBusinessSidebar
            | Self::ConfirmClose
            | Self::CancelClose
            | Self::OpenShortcutHelp
            | Self::CloseShortcutHelp
            | Self::PageShortcutHelp
            | Self::OpenWorkbenchNavigator
            | Self::CloseWorkbenchNavigator
            | Self::ToggleAgentBoard
            | Self::StartAgent
            | Self::TogglePointerCapture
            | Self::EnterHistory
            | Self::ForwardLiteralPrefix
            | Self::DismissPrefix
            | Self::UnknownPrefix
            | Self::Exit => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuAction {
    SplitPanel(PanelDirection),
    Zoom,
    Resize,
    Copy,
    /// Copies the focused pane's settled Host selection; `Copy` enters keyboard copy mode.
    CopySelection,
    Takeover,
    ClosePane,
    MovePanel(PanelDirection),
    NewTab,
    /// A frozen saved choice; never changes the shared default profile.
    NewTabWithProfile(Box<orchbee_workbench_protocol::UserLaunchProfile>),
    TabCreation,
    NewTabToRight,
    NewTabInGroup,
    DuplicateTab,
    MoveTabPrevious,
    MoveTabNext,
    MoveTabFirst,
    MoveTabLast,
    SelectTab,
    SelectAllTabs,
    ClearTabSelection,
    PinTab,
    UnpinTab,
    ManageGroups,
    CreateGroup,
    RenameGroup,
    ChooseGroupColor,
    GroupColor(orchbee_workbench_protocol::TabGroupColor),
    JoinGroup(u16),
    RemoveFromGroup,
    Ungroup,
    RenameTab,
    RenamePanel,
    /// Opens the Find bar on the Panel the menu addresses.
    FindInTerminal,
    ClearTerminal,
    LastPanel,
    ResumeTabClose,
    CloseSelectedTabs,
    CloseOtherTabs,
    CloseTrailingTabs,
    CloseGroup,
    /// The global menu's Close Tab: the current Tab through the same close request as its key.
    CloseTab,
    QuickWorkspace,
    WorkspaceAtDirectory,
    RenameWorkspace,
    RenameProject,
    WorkspaceProjects,
    WorkspaceCreation,
    /// A Workspace's Open Worktree submenu of its other Git Worktrees.
    WorkspaceWorktrees,
    CreateWorkspaceProject,
    LeaveWorkspaceProject,
    JoinWorkspaceProject(orchbee_workbench_protocol::WorkbenchProjectId),
    /// A new Workspace of the menu's Project in one of its recorded folders.
    WorkspaceInFolder(orchbee_workbench_protocol::WorkbenchDirectoryPath),
    OpenWorktreeAt(orchbee_workbench_protocol::WorkbenchDirectoryPath),
    RemoveWorktree,
    DeleteWorkspace,
    DeleteProject,
    CloseProjectWorkspaces,
    CloseWorkspace,
    NewWorkspace,
    NewWorktree,
    OpenWorktree,
    /// Opens the earlier-layout picker.
    EarlierLayouts,
    /// One picker row: the listed earlier layout it reopens.
    ReopenEarlierLayout(orchbee_workbench_protocol::LayoutHistoryEntry),
    Reload,
    Settings,
    Machines,
    Help,
    Sidebar,
    Navigator,
    AgentBoard,
    StartAgent,
    /// A new-Tab menu row: starts this provider in a new Tab of the current Workspace.
    StartAgentInTab(&'static str),
    /// The new-Tab menu's last Agent row: the picker, opening in a new Tab.
    MoreAgents,
    /// Shown only while an Agent notification card is current.
    OpenNotificationTarget,
    ReleasePointer,
    Detach,
}

impl From<&MenuAction> for crate::localization::MenuLabel {
    fn from(action: &MenuAction) -> Self {
        match action {
            MenuAction::SplitPanel(value) => Self::SplitPanel(*value),
            MenuAction::Zoom => Self::Zoom,
            MenuAction::Resize => Self::Resize,
            MenuAction::Copy => Self::Copy,
            MenuAction::CopySelection => Self::CopySelection,
            MenuAction::Takeover => Self::Takeover,
            MenuAction::ClosePane => Self::ClosePane,
            MenuAction::MovePanel(value) => Self::MovePanel(*value),
            MenuAction::NewTab => Self::NewTab,
            MenuAction::NewTabWithProfile(_) => Self::NewTabWithProfile,
            MenuAction::TabCreation => Self::TabCreation,
            MenuAction::NewTabToRight => Self::NewTabToRight,
            MenuAction::NewTabInGroup => Self::NewTabInGroup,
            MenuAction::DuplicateTab => Self::DuplicateTab,
            MenuAction::MoveTabPrevious => Self::MoveTabPrevious,
            MenuAction::MoveTabNext => Self::MoveTabNext,
            MenuAction::MoveTabFirst => Self::MoveTabFirst,
            MenuAction::MoveTabLast => Self::MoveTabLast,
            MenuAction::SelectTab => Self::SelectTab,
            MenuAction::SelectAllTabs => Self::SelectAllTabs,
            MenuAction::ClearTabSelection => Self::ClearTabSelection,
            MenuAction::PinTab => Self::PinTab,
            MenuAction::UnpinTab => Self::UnpinTab,
            MenuAction::ManageGroups => Self::ManageGroups,
            MenuAction::CreateGroup => Self::CreateGroup,
            MenuAction::RenameGroup => Self::RenameGroup,
            MenuAction::ChooseGroupColor => Self::ChooseGroupColor,
            MenuAction::GroupColor(value) => Self::GroupColor(*value),
            MenuAction::JoinGroup(_) => Self::JoinGroup,
            MenuAction::RemoveFromGroup => Self::RemoveFromGroup,
            MenuAction::Ungroup => Self::Ungroup,
            MenuAction::RenameTab => Self::RenameTab,
            MenuAction::RenamePanel => Self::RenamePanel,
            MenuAction::FindInTerminal => Self::FindInTerminal,
            MenuAction::ClearTerminal => Self::ClearTerminal,
            MenuAction::LastPanel => Self::LastPanel,
            MenuAction::ResumeTabClose => Self::ResumeTabClose,
            MenuAction::CloseSelectedTabs => Self::CloseSelectedTabs,
            MenuAction::CloseOtherTabs => Self::CloseOtherTabs,
            MenuAction::CloseTrailingTabs => Self::CloseTrailingTabs,
            MenuAction::CloseGroup => Self::CloseGroup,
            MenuAction::CloseTab => Self::CloseTab,
            MenuAction::QuickWorkspace => Self::QuickWorkspace,
            MenuAction::WorkspaceAtDirectory => Self::WorkspaceAtDirectory,
            MenuAction::RenameWorkspace => Self::RenameWorkspace,
            MenuAction::RenameProject => Self::RenameProject,
            MenuAction::WorkspaceProjects => Self::WorkspaceProjects,
            MenuAction::WorkspaceCreation => Self::WorkspaceCreation,
            MenuAction::WorkspaceWorktrees => Self::WorkspaceWorktrees,
            MenuAction::WorkspaceInFolder(_) => Self::WorkspaceInFolder,
            MenuAction::CreateWorkspaceProject => Self::CreateWorkspaceProject,
            MenuAction::LeaveWorkspaceProject => Self::LeaveWorkspaceProject,
            MenuAction::JoinWorkspaceProject(_) => Self::JoinWorkspaceProject,
            MenuAction::OpenWorktreeAt(_) => Self::OpenWorktreeAt,
            MenuAction::RemoveWorktree => Self::RemoveWorktree,
            MenuAction::DeleteWorkspace => Self::DeleteWorkspace,
            MenuAction::DeleteProject => Self::DeleteProject,
            MenuAction::CloseProjectWorkspaces => Self::CloseProjectWorkspaces,
            MenuAction::CloseWorkspace => Self::CloseWorkspace,
            MenuAction::NewWorkspace => Self::NewWorkspace,
            MenuAction::NewWorktree => Self::NewWorktree,
            MenuAction::OpenWorktree => Self::OpenWorktree,
            MenuAction::EarlierLayouts => Self::EarlierLayouts,
            MenuAction::ReopenEarlierLayout(_) => Self::ReopenEarlierLayout,
            MenuAction::Reload => Self::Reload,
            MenuAction::Settings => Self::Settings,
            MenuAction::Machines => Self::Machines,
            MenuAction::Help => Self::Help,
            MenuAction::Sidebar => Self::Sidebar,
            MenuAction::Navigator => Self::Navigator,
            MenuAction::AgentBoard => Self::AgentBoard,
            MenuAction::StartAgent => Self::StartAgent,
            MenuAction::StartAgentInTab(_) => Self::StartAgentInTab,
            MenuAction::MoreAgents => Self::MoreAgents,
            MenuAction::OpenNotificationTarget => Self::OpenNotificationTarget,
            MenuAction::ReleasePointer => Self::ReleasePointer,
            MenuAction::Detach => Self::Detach,
        }
    }
}

impl MenuAction {
    pub(crate) const fn submenu(&self) -> bool {
        matches!(
            self,
            Self::ManageGroups
                | Self::ChooseGroupColor
                | Self::TabCreation
                | Self::WorkspaceProjects
                | Self::WorkspaceCreation
                | Self::WorkspaceWorktrees
        )
    }
    /// Addressed sidebar choices require their menu's frozen source owner.
    /// They cannot fall back to an action on the current Workspace.
    pub(crate) fn client_action(&self) -> Option<ClientAction> {
        Some(match self {
            Self::SplitPanel(direction) => ClientAction::SplitPanel(*direction),
            Self::Zoom => ClientAction::ToggleZoom,
            Self::Resize => ClientAction::EnterResizeMode,
            Self::Copy => ClientAction::EnterCopyMode,
            Self::CopySelection => ClientAction::CopySelection,
            Self::Takeover => ClientAction::RequestControlTakeover,
            Self::ClosePane => ClientAction::ClosePanel,
            Self::MovePanel(direction) => ClientAction::MovePanel(*direction),
            Self::MoveTabPrevious
            | Self::ResumeTabClose
            | Self::CloseSelectedTabs
            | Self::CloseOtherTabs
            | Self::CloseTrailingTabs
            | Self::CloseGroup
            | Self::MoveTabNext
            | Self::MoveTabFirst
            | Self::MoveTabLast
            | Self::SelectTab
            | Self::SelectAllTabs
            | Self::ClearTabSelection
            | Self::PinTab
            | Self::NewTabInGroup
            | Self::NewTabWithProfile(_)
            | Self::TabCreation
            | Self::NewTabToRight
            | Self::DuplicateTab
            | Self::UnpinTab
            | Self::ManageGroups
            | Self::CreateGroup
            | Self::RenameGroup
            | Self::ChooseGroupColor
            | Self::GroupColor(_)
            | Self::JoinGroup(_)
            | Self::RemoveFromGroup
            | Self::Ungroup => ClientAction::TabMenu {
                action: self.clone(),
                tab_id: None,
            },
            Self::NewTab => ClientAction::CreateTab,
            Self::CloseTab => ClientAction::RequestCloseTab,
            Self::RenameTab => ClientAction::RequestRenameTab,
            Self::RenamePanel => ClientAction::RequestRenamePanel,
            Self::FindInTerminal => ClientAction::FindInTerminal,
            Self::ClearTerminal => ClientAction::ClearTerminal,
            Self::LastPanel => ClientAction::FocusLastPanel,
            Self::QuickWorkspace => ClientAction::RequestQuickWorkspace,
            Self::WorkspaceAtDirectory => ClientAction::RequestWorkspaceAtDirectory,
            Self::RenameWorkspace => ClientAction::RequestWorkspaceRename,
            Self::RenameProject => ClientAction::RequestProjectRename,
            Self::CloseProjectWorkspaces => ClientAction::RequestProjectCloseAll(None),
            Self::CloseWorkspace => ClientAction::RequestWorkspaceClose,
            Self::NewWorkspace => ClientAction::RequestWorkspaceCreate,
            Self::NewWorktree => ClientAction::RequestWorktreeCreate,
            Self::OpenWorktree => ClientAction::RequestWorktreeOpen,
            Self::EarlierLayouts => ClientAction::RequestEarlierLayouts,
            Self::ReopenEarlierLayout(entry) => ClientAction::ReopenEarlierLayout(entry.saved_at),
            Self::Reload => ClientAction::ReloadConfiguration,
            Self::Settings => ClientAction::OpenSettings,
            Self::Machines => ClientAction::OpenMachines,
            Self::Help => ClientAction::OpenShortcutHelp,
            Self::Sidebar => ClientAction::ToggleBusinessSidebar,
            Self::Navigator => ClientAction::OpenWorkbenchNavigator,
            Self::AgentBoard => ClientAction::ToggleAgentBoard,
            Self::StartAgent => ClientAction::StartAgent,
            Self::OpenNotificationTarget => ClientAction::OpenNotificationTarget,
            Self::ReleasePointer => ClientAction::TogglePointerCapture,
            Self::Detach => ClientAction::Exit,
            Self::WorkspaceProjects
            | Self::WorkspaceCreation
            | Self::WorkspaceWorktrees
            | Self::WorkspaceInFolder(_)
            | Self::CreateWorkspaceProject
            | Self::LeaveWorkspaceProject
            | Self::JoinWorkspaceProject(_)
            | Self::OpenWorktreeAt(_)
            | Self::RemoveWorktree
            | Self::DeleteWorkspace
            | Self::DeleteProject
            // The menu's owner runs these through the picker's own new-Tab start.
            | Self::StartAgentInTab(_)
            | Self::MoreAgents => return None,
        })
    }
}

/// What one member key resolves to when matched under an armed prefix.
/// `Client` actions map one-to-one to `ClientAction`; the toggle intents own
/// their local view effects inside `ClientView::decide_prefix`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PrefixAction {
    Client(ClientAction),
    SelectTabDigit,
    SelectAgentDigit,
    SidebarToggle,
    NavigatorToggle,
    BoardToggle,
    ShowHelp,
    HelpPagePrevious,
    HelpPageNext,
}
