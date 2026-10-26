//! Shared semantic metadata for native action discovery.
//!
//! Execution owners keep the typed `ClientAction` / `MenuAction` input and exact target. This
//! module only projects the stable identity and discovery metadata that help and menus must agree
//! on. Availability is supplied by the owning UI snapshot; it is never inferred here.

use crate::action_menu::MenuAction;
use crate::client_view::{ClientAction, PanelDirection, PrefixAction};

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

/// Reviewed semantic aliases. Localization supplies the actual searchable words.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionSearchAlias {
    Actions,
    Commands,
    Preferences,
    Workspace,
    Worktree,
    Project,
    Tab,
    Pane,
    Split,
    Resize,
    Copy,
    Close,
    Rename,
    Move,
    Select,
    Sidebar,
    Navigator,
    Agent,
    Help,
    Mouse,
    Detach,
}

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
            built_in("orchbee.native.pane.resize-mode.enter", C::Pane, RESIZE)
        }
        PrefixAction::Client(A::ResizePane { .. }) => {
            built_in("workbench.panel.active.resize", C::Pane, RESIZE)
        }
        PrefixAction::Client(A::EnterCopyMode) => {
            built_in("orchbee.native.copy-mode.enter", C::Input, COPY)
        }
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
        PrefixAction::Client(A::RequestWorkspaceAtDirectory(_)) => built_in(
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
        PrefixAction::Client(A::RequestWorkspaceClose) => built_in(
            "workbench.workspace.delete",
            C::Workspace,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Close],
        ),
        PrefixAction::Client(A::RequestControlTakeover) => {
            built_in("orchbee.native.pane.control.takeover", C::Pane, PANE)
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

const fn split_panel(direction: PanelDirection) -> ActionMetadata<'static> {
    let id = match direction {
        PanelDirection::Down => "workbench.panel.active.split-below",
        PanelDirection::Right => "workbench.panel.active.split-right",
        PanelDirection::Left => "orchbee.native.pane.split-left",
        PanelDirection::Up => "orchbee.native.pane.split-above",
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

/// Resolves only bindings whose dispatch target has the same meaning as this menu entry.
///
/// Stable metadata identity is deliberately insufficient here: a prefix action aimed at the
/// current Tab, Workspace, or Pane must not be advertised for an exact background menu target.
pub(crate) fn menu_binding_action(
    target: &crate::action_menu::MenuTarget,
    action: crate::action_menu::MenuAction,
) -> Option<PrefixAction> {
    use crate::action_menu::{MenuAction as M, MenuTarget as T};
    use ClientAction as A;

    if let T::Drawn { target, .. } = target {
        return menu_binding_action(target, action);
    }

    let action = match (target, action) {
        (T::Global | T::Create(None), M::NewWorkspace) => {
            PrefixAction::Client(A::RequestWorkspaceCreate)
        }
        (T::Global | T::Create(None), M::NewWorktree) => {
            PrefixAction::Client(A::RequestWorktreeCreate)
        }
        (T::Global | T::Create(None), M::OpenWorktree) => {
            PrefixAction::Client(A::RequestWorktreeOpen)
        }
        (T::Global, M::SplitPanel(direction)) => PrefixAction::Client(A::SplitPanel(direction)),
        (T::Global, M::ClosePane) => PrefixAction::Client(A::ClosePanel),
        (T::Global, M::Zoom) => PrefixAction::Client(A::ToggleZoom),
        (T::Global, M::MovePanel(direction)) => PrefixAction::Client(A::MovePanel(direction)),
        (T::Global, M::RenameTab) => PrefixAction::Client(A::RequestRenameTab),
        (T::Global, M::Copy) => PrefixAction::Client(A::EnterCopyMode),
        (T::Global, M::Resize) => PrefixAction::Client(A::EnterResizeMode),
        (T::Global, M::Reload) => PrefixAction::Client(A::ReloadConfiguration),
        (T::Global, M::Settings) => PrefixAction::Client(A::OpenSettings),
        (T::Global, M::Machines) => PrefixAction::Client(A::OpenMachines),
        (T::Global, M::Help) => PrefixAction::ShowHelp,
        (T::Global, M::Sidebar) => PrefixAction::SidebarToggle,
        (T::Global, M::Navigator) => PrefixAction::NavigatorToggle,
        (T::Global, M::AgentBoard) => PrefixAction::BoardToggle,
        (T::Global, M::ReleasePointer) => PrefixAction::Client(A::TogglePointerCapture),
        (T::Global, M::Detach) => PrefixAction::Client(A::Exit),
        (T::TabCreate(_), M::NewTab) => PrefixAction::Client(A::CreateTab),
        _ => return None,
    };
    Some(action)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuActionMetadata {
    pub(crate) action: ActionMetadata<'static>,
    pub(crate) tone: ActionTone,
}

impl MenuActionMetadata {
    pub(crate) fn starts_section(self, input: MenuAction) -> bool {
        use MenuAction as A;
        match self.action.identity {
            ActionIdentity::BuiltIn(
                "tabs.tab.duplicate"
                | "tabs.group.ungroup"
                | "tabs.group.collapsed.set"
                | "orchbee.native.tab-tools.open"
                | "orchbee.native.tab-group.open"
                | "tabs.group.create-with-tab",
            ) => true,
            ActionIdentity::BuiltIn("tabs.tab.close") => input == A::CloseSelectedTabs,
            ActionIdentity::BuiltIn("workbench.terminal.create") => input == A::NewTabInGroup,
            ActionIdentity::BuiltIn("tabs.tab.move") => input == A::MoveTabPrevious,
            ActionIdentity::BuiltIn(_) | ActionIdentity::ConfiguredCommand(_) => false,
        }
    }
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
            | A::DeleteProject
            | A::CloseSelectedTabs
            | A::CloseOtherTabs
            | A::CloseTrailingTabs
            | A::CloseGroup
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
        A::Resize => built_in("orchbee.native.pane.resize-mode.enter", C::Pane, RESIZE),
        A::Copy => built_in("orchbee.native.copy-mode.enter", C::Input, COPY),
        A::Takeover => built_in("orchbee.native.pane.control.takeover", C::Pane, PANE),
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
        A::SelectTab | A::DeselectTab | A::SelectTabRange => {
            built_in("tabs.tab.select", C::Tab, SELECT_TAB)
        }
        A::SelectAllTabs => built_in("orchbee.native.tab-selection.all", C::Tab, SELECT_TAB),
        A::ClearTabSelection => built_in("orchbee.native.tab-selection.clear", C::Tab, SELECT_TAB),
        A::PinTab | A::UnpinTab => built_in("tabs.tab.pinned.set", C::Tab, TAB),
        A::ManageGroups => built_in("orchbee.native.tab-groups.open", C::Tab, ACTIONS),
        A::TabTools => built_in("orchbee.native.tab-tools.open", C::Tab, ACTIONS),
        A::GroupMenu => built_in("orchbee.native.tab-group.open", C::Tab, ACTIONS),
        A::ChooseGroupColor => built_in(
            "orchbee.native.tab-group.color-choices.open",
            C::Tab,
            ACTIONS,
        ),
        A::MoveToNewWindow => built_in("tabs.tab.move-to-window", C::Tab, MOVE_TAB),
        A::CreateGroup => built_in("tabs.group.create", C::Tab, TAB),
        A::RenameGroup | A::GroupColor(_) => built_in("tabs.group.update", C::Tab, TAB),
        A::JoinGroup(_) | A::RemoveFromGroup => {
            built_in("tabs.tab.move-to-group", C::Tab, MOVE_TAB)
        }
        A::Ungroup => built_in("tabs.group.ungroup", C::Tab, TAB),
        A::CollapseGroup | A::ExpandGroup => built_in("tabs.group.collapsed.set", C::Tab, TAB),
        A::MoveGroupPrevious | A::MoveGroupNext => built_in("tabs.group.move", C::Tab, MOVE_TAB),
        A::RenameTab => built_in("workbench.tab.title.set", C::Tab, RENAME_TAB),
        A::ResumeTabClose => built_in("orchbee.native.tab-close.resume", C::Tab, CLOSE_TAB),
        A::CloseSelectedTabs | A::CloseOtherTabs | A::CloseTrailingTabs | A::CloseGroup => {
            built_in("tabs.tab.close", C::Tab, CLOSE_TAB)
        }
        A::QuickWorkspace => built_in(
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
        A::WorkspaceWorktrees => {
            built_in("orchbee.native.worktree-menu.open", C::Workspace, WORKTREE)
        }
        A::DeleteProject => built_in("workbench.project.dissolve", C::Project, PROJECT),
        A::CloseProjectWorkspaces => built_in("workbench.project.close-all", C::Project, PROJECT),
        A::CloseWorkspace | A::DeleteWorkspace => built_in(
            "workbench.workspace.delete",
            C::Workspace,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Close],
        ),
        A::NewWorkspace => built_in("workbench.workspace.create", C::Workspace, WORKSPACE),
        A::NewWorktree => built_in("orchbee.native.worktree.create", C::Workspace, WORKTREE),
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
