//! Shared semantic metadata for native action discovery.
//!
//! Execution owners keep the typed `ClientAction` / `MenuAction` input and exact target. This
//! module only projects the stable identity and discovery metadata that help and menus must agree
//! on. Availability is supplied by the owning UI snapshot; it is never inferred here.

use crate::client_actions::{ClientAction, MenuAction, PrefixAction};
use crate::localization::PanelDirection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionIdentity<'a> {
    BuiltIn(&'a str),
    /// The configured identifier is already the user's durable identity. Keep it opaque.
    ConfiguredCommand(&'a str),
}

#[cfg(test)]
impl<'a> ActionIdentity<'a> {
    pub(crate) const fn stable_id(self) -> &'a str {
        match self {
            Self::BuiltIn(id) => id,
            Self::ConfiguredCommand(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionCategory {
    Application,
    Client,
    Workspace,
    Project,
    Tab,
    Pane,
    Navigation,
    Agent,
    Input,
}

pub(crate) use crate::localization::ActionSearchAlias;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActionMetadata<'a> {
    pub(crate) identity: ActionIdentity<'a>,
    pub(crate) category: ActionCategory,
    pub(crate) search_aliases: &'static [ActionSearchAlias],
}

/// Intent tone is independent of reading hierarchy, availability and pointer state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionTone {
    Ordinary,
    Destructive,
}

impl ActionTone {
    pub(crate) const fn content_role(
        self,
        ordinary: crate::tui_theme::Role,
    ) -> crate::tui_theme::Role {
        match self {
            Self::Ordinary => ordinary,
            Self::Destructive => crate::tui_theme::Role::AccentDestructive,
        }
    }
}

impl<'a> ActionMetadata<'a> {
    pub(crate) fn same_discovery_entry(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.category == other.category
            && self.search_aliases == other.search_aliases
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionAvailability<R> {
    Enabled,
    Disabled(R),
    Hidden,
}

impl<R> ActionAvailability<R> {
    pub(crate) fn from_disabled_reason(reason: Option<R>) -> Self {
        reason.map_or(Self::Enabled, Self::Disabled)
    }

    pub(crate) const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub(crate) const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }
}

const ACTIONS: &[ActionSearchAlias] = &[ActionSearchAlias::Actions, ActionSearchAlias::Commands];
const PREFERENCES: &[ActionSearchAlias] = &[ActionSearchAlias::Preferences];
const WORKSPACE: &[ActionSearchAlias] = &[ActionSearchAlias::Workspace];
const WORKTREE: &[ActionSearchAlias] = &[ActionSearchAlias::Worktree];
const PROJECT: &[ActionSearchAlias] = &[ActionSearchAlias::Project];
const TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab];
const PANE: &[ActionSearchAlias] = &[ActionSearchAlias::Pane];
const SPLIT: &[ActionSearchAlias] = &[ActionSearchAlias::Pane, ActionSearchAlias::Split];
const RESIZE: &[ActionSearchAlias] = &[ActionSearchAlias::Pane, ActionSearchAlias::Resize];
const COPY: &[ActionSearchAlias] = &[ActionSearchAlias::Copy];
const CLOSE_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Close];
const RENAME_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Rename];
const MOVE_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Move];
const SELECT_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Select];
const SIDEBAR: &[ActionSearchAlias] = &[ActionSearchAlias::Sidebar];
const NAVIGATOR: &[ActionSearchAlias] = &[ActionSearchAlias::Navigator];
const AGENT: &[ActionSearchAlias] = &[ActionSearchAlias::Agent];
const HELP: &[ActionSearchAlias] = &[ActionSearchAlias::Help];
const MOUSE: &[ActionSearchAlias] = &[ActionSearchAlias::Mouse];
const DETACH: &[ActionSearchAlias] = &[ActionSearchAlias::Detach];
const EARLIER_LAYOUTS: &[ActionSearchAlias] = &[
    ActionSearchAlias::History,
    ActionSearchAlias::Workspace,
    ActionSearchAlias::Tab,
];
const NONE: &[ActionSearchAlias] = &[];

const fn built_in(
    id: &'static str,
    category: ActionCategory,
    search_aliases: &'static [ActionSearchAlias],
) -> ActionMetadata<'static> {
    ActionMetadata {
        identity: ActionIdentity::BuiltIn(id),
        category,
        search_aliases,
    }
}

/// Projects a resolved binding target without replacing that target as dispatch authority.
pub(crate) fn binding_metadata(action: &PrefixAction) -> Option<ActionMetadata<'_>> {
    use ActionCategory as C;
    use ClientAction as A;
    let metadata = match action {
        PrefixAction::Client(A::OpenSettings) => {
            built_in("workbench.settings.open", C::Application, PREFERENCES)
        }
        PrefixAction::Client(A::OpenActionMenu) => {
            built_in("orchbee.native.action-menu.open", C::Application, ACTIONS)
        }
        PrefixAction::Client(A::ConfiguredCommand(id)) => ActionMetadata {
            identity: ActionIdentity::ConfiguredCommand(id),
            category: C::Application,
            search_aliases: COMMANDS,
        },
        PrefixAction::Client(A::EnterResizeMode) => {
            built_in("orchbee.native.panel.resize-mode.enter", C::Pane, RESIZE)
        }
        PrefixAction::Client(A::ResizePane { .. }) => {
            built_in("workbench.panel.active.resize", C::Pane, RESIZE)
        }
        PrefixAction::Client(A::EnterCopyMode) => {
            built_in("orchbee.native.copy-mode.enter", C::Input, COPY)
        }
        PrefixAction::Client(A::FindInTerminal) => built_in("terminal.search.open", C::Pane, COPY),
        PrefixAction::Client(A::ReloadConfiguration) => built_in(
            "orchbee.native.configuration.reload",
            C::Application,
            PREFERENCES,
        ),
        PrefixAction::Client(A::DismissOnboarding) => {
            built_in("orchbee.native.onboarding.dismiss", C::Application, HELP)
        }
        PrefixAction::Client(A::RequestWorkspaceCreate) => {
            built_in("workbench.workspace.create", C::Workspace, WORKSPACE)
        }
        PrefixAction::Client(A::RequestQuickWorkspace) => built_in(
            "orchbee.native.workspace.quick-create",
            C::Workspace,
            WORKSPACE,
        ),
        PrefixAction::Client(A::RequestWorkspaceAtDirectory) => built_in(
            "orchbee.native.workspace.create-in-directory",
            C::Workspace,
            WORKSPACE,
        ),
        PrefixAction::Client(A::RequestWorktreeCreate) => {
            built_in("orchbee.native.worktree.create", C::Workspace, WORKTREE)
        }
        PrefixAction::Client(A::RequestWorktreeOpen) => {
            built_in("orchbee.native.worktree.open", C::Workspace, WORKTREE)
        }
        PrefixAction::Client(A::RequestEarlierLayouts) => earlier_layouts(),
        PrefixAction::Client(A::RequestWorkspaceClose) => built_in(
            "workbench.workspace.delete",
            C::Workspace,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Close],
        ),
        PrefixAction::Client(A::RequestControlTakeover) => {
            built_in("orchbee.native.panel.control.takeover", C::Pane, PANE)
        }
        PrefixAction::Client(A::RequestWorkspaceDirectory) => built_in(
            "orchbee.native.workspace.directory.configure",
            C::Workspace,
            WORKSPACE,
        ),
        PrefixAction::Client(A::RequestWorkspaceRename) => built_in(
            "orchbee.native.workspace.rename",
            C::Workspace,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Rename],
        ),
        PrefixAction::Client(A::RequestProjectRename) => {
            built_in("workbench.project.rename", C::Project, PROJECT)
        }
        PrefixAction::Client(A::RequestRenameTab) => {
            built_in("workbench.tab.title.set", C::Tab, RENAME_TAB)
        }
        PrefixAction::Client(A::RequestRenamePanel) => built_in(
            "workbench.panel.rename",
            C::Pane,
            &[ActionSearchAlias::Pane, ActionSearchAlias::Rename],
        ),
        PrefixAction::Client(A::ClearTerminal) => built_in("terminal.clear", C::Pane, PANE),
        PrefixAction::Client(A::FocusLastPanel) => last_panel(),
        PrefixAction::Client(A::RequestCloseTab) => built_in("tabs.tab.close", C::Tab, CLOSE_TAB),
        PrefixAction::Client(A::MovePreviousTab | A::MoveNextTab) => {
            built_in("tabs.tab.move", C::Tab, MOVE_TAB)
        }
        PrefixAction::Client(A::CreateTab) => built_in("workbench.terminal.create", C::Tab, TAB),
        PrefixAction::Client(A::SelectNextTab) => {
            built_in("workbench.tab.select-next", C::Navigation, SELECT_TAB)
        }
        PrefixAction::Client(A::SelectPreviousTab) => {
            built_in("workbench.tab.select-previous", C::Navigation, SELECT_TAB)
        }
        PrefixAction::Client(A::SelectPreviousContext | A::SelectNextContext) => built_in(
            "orchbee.native.workspace.select",
            C::Navigation,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Select],
        ),
        PrefixAction::Client(A::PreviousPresentation | A::NextPresentation) => built_in(
            "orchbee.native.presentation.select",
            C::Navigation,
            &[ActionSearchAlias::Pane, ActionSearchAlias::Select],
        ),
        PrefixAction::Client(A::MovePanelFocus(direction)) => panel_focus(*direction),
        PrefixAction::Client(A::ToggleZoom) => {
            built_in("workbench.panel.active.maximization.toggle", C::Pane, PANE)
        }
        PrefixAction::Client(A::SplitPanel(direction)) => split_panel(*direction),
        PrefixAction::Client(A::ClosePanel) => built_in(
            "workbench.terminal.active.close",
            C::Pane,
            &[ActionSearchAlias::Pane, ActionSearchAlias::Close],
        ),
        PrefixAction::Client(A::MovePanel(direction)) => move_panel(*direction),
        PrefixAction::Client(A::TogglePointerCapture) => {
            built_in("orchbee.native.pointer-capture.toggle", C::Input, MOUSE)
        }
        PrefixAction::Client(A::ForwardLiteralPrefix) => built_in(
            "orchbee.native.terminal.literal-prefix.forward",
            C::Input,
            NONE,
        ),
        PrefixAction::Client(A::DismissPrefix) => {
            built_in("orchbee.native.prefix.dismiss", C::Input, NONE)
        }
        PrefixAction::Client(A::Exit) => {
            built_in("orchbee.native.window.detach", C::Client, DETACH)
        }
        PrefixAction::Client(A::FocusBusinessSidebar) => {
            built_in("workbench.focus.sidebar", C::Navigation, SIDEBAR)
        }
        PrefixAction::Client(A::OpenMachines) => {
            built_in("orchbee.native.machines.open", C::Navigation, NONE)
        }
        PrefixAction::Client(A::OpenNotificationTarget) => {
            built_in("orchbee.native.notification.target.open", C::Agent, AGENT)
        }
        PrefixAction::Client(A::PreviousAgent | A::NextAgent) => built_in(
            "orchbee.native.agent.select",
            C::Agent,
            &[ActionSearchAlias::Agent, ActionSearchAlias::Select],
        ),
        PrefixAction::SelectTabDigit => built_in(
            "workbench.tab.select-by-position",
            C::Navigation,
            SELECT_TAB,
        ),
        PrefixAction::SelectAgentDigit => built_in(
            "orchbee.native.agent.focus",
            C::Agent,
            &[ActionSearchAlias::Agent, ActionSearchAlias::Select],
        ),
        PrefixAction::SidebarToggle => built_in("workbench.sidebar.toggle", C::Navigation, SIDEBAR),
        PrefixAction::NavigatorToggle => built_in(
            "orchbee.native.workspace-navigator.toggle",
            C::Navigation,
            NAVIGATOR,
        ),
        PrefixAction::BoardToggle => built_in("orchbee.native.agent-board.toggle", C::Agent, AGENT),
        PrefixAction::Client(A::StartAgent) => {
            built_in("orchbee.native.agent.start", C::Agent, AGENT)
        }
        PrefixAction::ShowHelp => {
            built_in("orchbee.native.shortcut-help.toggle", C::Application, HELP)
        }
        PrefixAction::HelpPagePrevious | PrefixAction::HelpPageNext => {
            built_in("orchbee.native.shortcut-help.page", C::Application, HELP)
        }
        _ => return None,
    };
    Some(metadata)
}

const COMMANDS: &[ActionSearchAlias] = &[ActionSearchAlias::Commands];

const fn panel_focus(direction: PanelDirection) -> ActionMetadata<'static> {
    let id = match direction {
        PanelDirection::Down => "workbench.panel.active.focus-down",
        PanelDirection::Left => "workbench.panel.active.focus-left",
        PanelDirection::Right => "workbench.panel.active.focus-right",
        PanelDirection::Up => "workbench.panel.active.focus-up",
    };
    built_in(
        id,
        ActionCategory::Navigation,
        &[ActionSearchAlias::Pane, ActionSearchAlias::Select],
    )
}

/// Shares the GUI Workbench identity: the same Server listing and reopen.
const fn earlier_layouts() -> ActionMetadata<'static> {
    built_in(
        "workbench.layout.reopen-earlier",
        ActionCategory::Workspace,
        EARLIER_LAYOUTS,
    )
}

/// Shares the GUI Workbench identity: the same Session-wide focus history step.
const fn last_panel() -> ActionMetadata<'static> {
    built_in(
        "workbench.panel.focus-last",
        ActionCategory::Navigation,
        &[ActionSearchAlias::Pane, ActionSearchAlias::Select],
    )
}

const fn split_panel(direction: PanelDirection) -> ActionMetadata<'static> {
    let id = match direction {
        PanelDirection::Down => "workbench.panel.active.split-below",
        PanelDirection::Right => "workbench.panel.active.split-right",
        PanelDirection::Left => "orchbee.native.panel.split-left",
        PanelDirection::Up => "orchbee.native.panel.split-above",
    };
    built_in(id, ActionCategory::Pane, SPLIT)
}

const fn move_panel(direction: PanelDirection) -> ActionMetadata<'static> {
    let id = match direction {
        PanelDirection::Down => "workbench.panel.active.move-down",
        PanelDirection::Left => "workbench.panel.active.move-left",
        PanelDirection::Right => "workbench.panel.active.move-right",
        PanelDirection::Up => "workbench.panel.active.move-up",
    };
    built_in(id, ActionCategory::Pane, PANE)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuActionMetadata {
    pub(crate) action: ActionMetadata<'static>,
    pub(crate) tone: ActionTone,
}

/// Projects menu discovery metadata. Dynamic color and group indices remain typed `MenuAction`
/// input and therefore do not mint per-row action identities.
pub(crate) fn menu_metadata(action: MenuAction) -> MenuActionMetadata {
    use ActionCategory as C;
    use MenuAction as A;
    let tone = if matches!(
        action,
        A::DeleteWorkspace
            | A::RemoveWorktree
            | A::ClosePane
            | A::CloseSelectedTabs
            | A::CloseOtherTabs
            | A::CloseTrailingTabs
            | A::CloseGroup
            | A::CloseTab
            | A::ResumeTabClose
    ) {
        ActionTone::Destructive
    } else {
        ActionTone::Ordinary
    };
    let projected = match action {
        A::SplitPanel(direction) => split_panel(direction),
        A::MovePanel(direction) => move_panel(direction),
        A::Zoom => built_in("workbench.panel.active.maximization.toggle", C::Pane, PANE),
        A::Resize => built_in("orchbee.native.panel.resize-mode.enter", C::Pane, RESIZE),
        A::Copy => built_in("orchbee.native.copy-mode.enter", C::Input, COPY),
        // Shares the GUI terminal catalog identity: the same copy operation, not a new one.
        A::CopySelection => built_in("terminal.selection.copy", C::Pane, COPY),
        A::Takeover => built_in("orchbee.native.panel.control.takeover", C::Pane, PANE),
        A::ClosePane => built_in(
            "workbench.terminal.active.close",
            C::Pane,
            &[ActionSearchAlias::Pane, ActionSearchAlias::Close],
        ),
        A::NewTab
        | A::NewTabWithProfile(_)
        | A::NewTabToRight
        | A::NewTabInGroup
        | A::NewTabInRecentGroup => built_in("workbench.terminal.create", C::Tab, TAB),
        A::TabCreation => built_in("orchbee.native.tab-create.open", C::Tab, ACTIONS),
        A::NewGroupWithTab => built_in("tabs.group.create-with-tab", C::Tab, TAB),
        A::DuplicateTab => built_in("tabs.tab.duplicate", C::Tab, TAB),
        A::MoveTabPrevious | A::MoveTabNext | A::MoveTabFirst | A::MoveTabLast => {
            built_in("tabs.tab.move", C::Tab, MOVE_TAB)
        }
        A::SelectTab => built_in("tabs.tab.select", C::Tab, SELECT_TAB),
        A::SelectAllTabs => built_in("orchbee.native.tab-selection.all", C::Tab, SELECT_TAB),
        A::ClearTabSelection => built_in("orchbee.native.tab-selection.clear", C::Tab, SELECT_TAB),
        A::PinTab | A::UnpinTab => built_in("tabs.tab.pinned.set", C::Tab, TAB),
        A::ManageGroups => built_in("orchbee.native.tab-groups.open", C::Tab, ACTIONS),
        A::ChooseGroupColor => built_in(
            "orchbee.native.tab-group.color-choices.open",
            C::Tab,
            ACTIONS,
        ),
        A::CreateGroup => built_in("tabs.group.create", C::Tab, TAB),
        A::RenameGroup | A::GroupColor(_) => built_in("tabs.group.update", C::Tab, TAB),
        A::JoinGroup(_) | A::RemoveFromGroup => {
            built_in("tabs.tab.move-to-group", C::Tab, MOVE_TAB)
        }
        A::Ungroup => built_in("tabs.group.ungroup", C::Tab, TAB),
        A::CollapseGroup => built_in("orchbee.native.tab-group.collapse", C::Tab, TAB),
        A::RenameTab => built_in("workbench.tab.title.set", C::Tab, RENAME_TAB),
        A::RenamePanel => built_in(
            "workbench.panel.rename",
            C::Pane,
            &[ActionSearchAlias::Pane, ActionSearchAlias::Rename],
        ),
        A::ClearTerminal => built_in("terminal.clear", C::Pane, PANE),
        A::LastPanel => last_panel(),
        A::ResumeTabClose => built_in("orchbee.native.tab-close.resume", C::Tab, CLOSE_TAB),
        A::CloseSelectedTabs
        | A::CloseOtherTabs
        | A::CloseTrailingTabs
        | A::CloseGroup
        | A::CloseTab => built_in("tabs.tab.close", C::Tab, CLOSE_TAB),
        A::QuickWorkspace | A::WorkspaceInFolder(_) => built_in(
            "orchbee.native.workspace.quick-create",
            C::Workspace,
            WORKSPACE,
        ),
        A::WorkspaceAtDirectory => built_in(
            "orchbee.native.workspace.create-in-directory",
            C::Workspace,
            WORKSPACE,
        ),
        A::WorkspaceDirectory => built_in(
            "orchbee.native.workspace.directory.configure",
            C::Workspace,
            WORKSPACE,
        ),
        A::RenameWorkspace => built_in(
            "orchbee.native.workspace.rename",
            C::Workspace,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Rename],
        ),
        A::RenameProject => built_in("workbench.project.rename", C::Project, PROJECT),
        A::LeaveWorkspaceProject | A::JoinWorkspaceProject(_) => {
            built_in("workbench.workspace.set-project", C::Workspace, PROJECT)
        }
        A::CreateWorkspaceProject => {
            built_in("workbench.workspace.create-project", C::Workspace, PROJECT)
        }
        A::WorkspaceProjects => built_in(
            "orchbee.native.workspace.project-menu.open",
            C::Workspace,
            PROJECT,
        ),
        A::WorkspaceCreation => built_in(
            "orchbee.native.workspace.creation-menu.open",
            C::Workspace,
            WORKSPACE,
        ),
        A::WorkspaceWorktrees => built_in(
            "orchbee.native.workspace.worktree-menu.open",
            C::Workspace,
            WORKTREE,
        ),
        A::DeleteProject => built_in("workbench.project.dissolve", C::Project, PROJECT),
        A::CloseProjectWorkspaces => built_in("workbench.project.close-all", C::Project, PROJECT),
        A::CloseWorkspace | A::DeleteWorkspace => built_in(
            "workbench.workspace.delete",
            C::Workspace,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Close],
        ),
        A::NewWorkspace => built_in("workbench.workspace.create", C::Workspace, WORKSPACE),
        A::NewWorktree => built_in("orchbee.native.worktree.create", C::Workspace, WORKTREE),
        A::EarlierLayouts | A::ReopenEarlierLayout(_) => earlier_layouts(),
        A::OpenWorktree | A::OpenWorktreeAt(_) => {
            built_in("orchbee.native.worktree.open", C::Workspace, WORKTREE)
        }
        A::RemoveWorktree => built_in("orchbee.native.worktree.remove", C::Workspace, WORKTREE),
        A::Reload => built_in(
            "orchbee.native.configuration.reload",
            C::Application,
            PREFERENCES,
        ),
        A::Settings => built_in("workbench.settings.open", C::Application, PREFERENCES),
        A::Machines => built_in("orchbee.native.machines.open", C::Navigation, NONE),
        A::Help => built_in("orchbee.native.shortcut-help.toggle", C::Application, HELP),
        A::Sidebar => built_in("workbench.sidebar.toggle", C::Navigation, SIDEBAR),
        A::Navigator => built_in(
            "orchbee.native.workspace-navigator.toggle",
            C::Navigation,
            NAVIGATOR,
        ),
        A::AgentBoard => built_in("orchbee.native.agent-board.toggle", C::Agent, AGENT),
        A::StartAgent | A::StartAgentInTab(_) | A::MoreAgents => {
            built_in("orchbee.native.agent.start", C::Agent, AGENT)
        }
        A::OpenNotificationTarget => {
            built_in("orchbee.native.notification.target.open", C::Agent, AGENT)
        }
        A::ReleasePointer => built_in("orchbee.native.pointer-capture.toggle", C::Input, MOUSE),
        A::Detach => built_in("orchbee.native.window.detach", C::Client, DETACH),
    };
    MenuActionMetadata {
        action: projected,
        tone,
    }
}

#[cfg(test)]
mod tests;
