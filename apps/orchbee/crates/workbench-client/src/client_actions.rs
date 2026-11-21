//! Action definitions shared by bindings, menus, capability policy, and discovery.
//!
//! These are plain typed intents. Execution owners keep exact targets, availability, and effects;
//! [`crate::action_projection`] declares each menu action's title, stable identity and discovery
//! metadata.

// module-layer: values

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
    /// Switches this TUI to the newer version the release store names active, keeping every
    /// terminal; only offered while the Update control is Ready.
    Update,
    ConfiguredCommand(String),
    EnterCopyMode,
    /// Opens the Find bar over the focused Panel.
    FindInTerminal,
    /// Copies the focused pane's settled Host selection without entering copy mode.
    CopySelection,
    /// Moves the focused Panel's nearest divider one keyboard step in this direction.
    ResizePane {
        direction: PanelDirection,
    },
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
    RequestCloseTab,
    ConfirmClose,
    CancelClose,
    MovePreviousTab,
    MoveNextTab,
    ToggleBusinessSidebar,
    FocusBusinessSidebar,
    OpenWorkbenchNavigator,
    OpenMachines,
    PreviousAgent,
    NextAgent,
    FocusAgent(u8),
    /// Opens the Agent the visible notification card announced.
    OpenNotificationTarget,
    /// Opens the answers about Orchbee answering an Agent's routine confirmations: for an Agent
    /// in the active Tab with an open offer, else for the focused Panel's.
    AgentConfirmations,
    CloseWorkbenchNavigator,
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
            Self::OpenSettings | Self::Update => None,
            Self::EnterCopyMode
            | Self::FindInTerminal
            | Self::CopySelection
            | Self::ResizePane { .. } => None,
            Self::ConfiguredCommand(_) => None,
            Self::OpenMachines
            | Self::PreviousAgent
            | Self::NextAgent
            | Self::FocusAgent(_)
            | Self::OpenNotificationTarget => None,
            Self::TabMenu { .. } | Self::ToggleTabGroup(_) => None,
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
            | Self::OpenWorkbenchNavigator
            | Self::CloseWorkbenchNavigator
            | Self::StartAgent
            | Self::AgentConfirmations
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
    Settings,
    /// Shown only while the Update control is Ready; its row carries the control's tooltip.
    Update,
    Machines,
    Sidebar,
    Navigator,
    StartAgent,
    /// A new-Tab menu row: starts this provider in a new Tab of the current Workspace.
    StartAgentInTab(&'static str),
    /// The new-Tab menu's last Agent row: the picker, opening in a new Tab.
    MoreAgents,
    /// Shown only while an Agent notification card is current.
    OpenNotificationTarget,
    /// Opens the answers about Orchbee answering one Agent's routine confirmations. Shown only
    /// for an Agent the Server can answer for.
    AgentConfirmations,
    /// One of those answers.
    AgentConfirmation(crate::localization::ConfirmationAnswer),
    ReleasePointer,
    Detach,
    NextTab,
    PreviousTab,
    NextWorkspace,
    PreviousWorkspace,
    FocusPanel(PanelDirection),
    ResizePanel(PanelDirection),
    NextAgent,
    PreviousAgent,
    FocusSidebar,
    /// An installed configured command, by the identity its key runs.
    ConfiguredCommand(String),
    /// A key the global menu lists only for search; the menu cannot run it.
    Key(KeyRow),
}

/// A key without a runnable menu row: it works only inside its mode, or it picks one of many
/// targets. Search finds it by its mode, its words and its key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyRow {
    /// A prefix chord with no single target, such as a Tab by its number.
    Prefix(crate::localization::PrefixDesc),
    /// A key of keyboard copy mode.
    Copy(crate::localization::PrefixDesc),
    /// A key of the focused sidebar.
    Sidebar(crate::machine_picker::SidebarCommand),
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
                | Self::AgentConfirmations
        )
    }
    /// Addressed sidebar choices require their menu's frozen source owner.
    /// They cannot fall back to an action on the current Workspace.
    pub(crate) fn client_action(&self) -> Option<ClientAction> {
        Some(match self {
            Self::SplitPanel(direction) => ClientAction::SplitPanel(*direction),
            Self::Zoom => ClientAction::ToggleZoom,
            Self::Copy => ClientAction::EnterCopyMode,
            Self::CopySelection => ClientAction::CopySelection,
            Self::Takeover => ClientAction::RequestControlTakeover,
            Self::ClosePane => ClientAction::ClosePanel,
            Self::MovePanel(direction) => ClientAction::MovePanel(*direction),
            Self::MoveTabPrevious
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
            Self::Settings => ClientAction::OpenSettings,
            Self::Update => ClientAction::Update,
            Self::Machines => ClientAction::OpenMachines,
            Self::Sidebar => ClientAction::ToggleBusinessSidebar,
            Self::Navigator => ClientAction::OpenWorkbenchNavigator,
            Self::StartAgent => ClientAction::StartAgent,
            Self::OpenNotificationTarget => ClientAction::OpenNotificationTarget,
            Self::ReleasePointer => ClientAction::TogglePointerCapture,
            Self::Detach => ClientAction::Exit,
            Self::NextTab => ClientAction::SelectNextTab,
            Self::PreviousTab => ClientAction::SelectPreviousTab,
            Self::NextWorkspace => ClientAction::SelectNextContext,
            Self::PreviousWorkspace => ClientAction::SelectPreviousContext,
            Self::FocusPanel(direction) => ClientAction::MovePanelFocus(*direction),
            Self::ResizePanel(direction) => ClientAction::ResizePane {
                direction: *direction,
            },
            Self::NextAgent => ClientAction::NextAgent,
            Self::PreviousAgent => ClientAction::PreviousAgent,
            Self::FocusSidebar => ClientAction::FocusBusinessSidebar,
            Self::ConfiguredCommand(id) => ClientAction::ConfiguredCommand(id.clone()),
            Self::Key(_) => return None,
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
            | Self::MoreAgents
            // The menu's owner sends these for the Agent its entry names.
            | Self::AgentConfirmations
            | Self::AgentConfirmation(_) => return None,
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
}
