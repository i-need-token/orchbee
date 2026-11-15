//! Shared semantic metadata for native action discovery.
//!
//! Execution owners keep the typed `ClientAction` / `MenuAction` input and exact target. This
//! module only projects the stable identity and discovery metadata that menus and bindings must agree
//! on. Availability is supplied by the owning UI snapshot; it is never inferred here.

use crate::client_actions::MenuAction;
use crate::localization::PanelDirection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionIdentity<'a> {
    BuiltIn(&'a str),
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
const FIND: &[ActionSearchAlias] = &[ActionSearchAlias::Copy, ActionSearchAlias::Find];
const CLOSE_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Close];
const RENAME_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Rename];
const MOVE_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Move];
const SELECT_TAB: &[ActionSearchAlias] = &[ActionSearchAlias::Tab, ActionSearchAlias::Select];
const SIDEBAR: &[ActionSearchAlias] = &[ActionSearchAlias::Sidebar];
const NAVIGATOR: &[ActionSearchAlias] = &[ActionSearchAlias::Navigator];
const AGENT: &[ActionSearchAlias] = &[ActionSearchAlias::Agent];
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

const COMMANDS: &[ActionSearchAlias] = &[ActionSearchAlias::Commands];
/// Key rows answer a search for help, which is where their keys were once listed.
const KEYS: &[ActionSearchAlias] = &[ActionSearchAlias::Help];
const COPY_KEYS: &[ActionSearchAlias] = &[ActionSearchAlias::Copy, ActionSearchAlias::Help];
const SIDEBAR_KEYS: &[ActionSearchAlias] = &[ActionSearchAlias::Sidebar, ActionSearchAlias::Help];

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

const fn resize_panel(direction: PanelDirection) -> ActionMetadata<'static> {
    let id = match direction {
        PanelDirection::Down => "workbench.panel.active.resize-down",
        PanelDirection::Left => "workbench.panel.active.resize-left",
        PanelDirection::Right => "workbench.panel.active.resize-right",
        PanelDirection::Up => "workbench.panel.active.resize-up",
    };
    built_in(id, ActionCategory::Pane, RESIZE)
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
    ) {
        ActionTone::Destructive
    } else {
        ActionTone::Ordinary
    };
    let projected = match action {
        A::SplitPanel(direction) => split_panel(direction),
        A::MovePanel(direction) => move_panel(direction),
        A::Zoom => built_in("workbench.panel.active.maximization.toggle", C::Pane, PANE),
        A::Copy => built_in("orchbee.native.copy-mode.enter", C::Input, COPY),
        // Shares the GUI terminal catalog identity: the same copy operation, not a new one.
        A::CopySelection => built_in("terminal.selection.copy", C::Pane, COPY),
        A::Takeover => built_in("orchbee.native.panel.control.takeover", C::Pane, PANE),
        A::ClosePane => built_in(
            "workbench.terminal.active.close",
            C::Pane,
            &[ActionSearchAlias::Pane, ActionSearchAlias::Close],
        ),
        A::NewTab | A::NewTabWithProfile(_) | A::NewTabToRight | A::NewTabInGroup => {
            built_in("workbench.terminal.create", C::Tab, TAB)
        }
        A::TabCreation => built_in("orchbee.native.tab-create.open", C::Tab, ACTIONS),
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
        A::RenameTab => built_in("workbench.tab.title.set", C::Tab, RENAME_TAB),
        A::RenamePanel => built_in(
            "workbench.panel.rename",
            C::Pane,
            &[ActionSearchAlias::Pane, ActionSearchAlias::Rename],
        ),
        A::FindInTerminal => built_in("terminal.search.open", C::Pane, FIND),
        A::ClearTerminal => built_in("terminal.clear", C::Pane, PANE),
        A::LastPanel => last_panel(),
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
        A::OpenWorktree | A::OpenWorktreeAt(_) => {
            built_in("orchbee.native.worktree.open", C::Workspace, WORKTREE)
        }
        A::RemoveWorktree => built_in("orchbee.native.worktree.remove", C::Workspace, WORKTREE),
        A::Settings => built_in("workbench.settings.open", C::Application, PREFERENCES),
        // Shares the window's identity: the same switch to the active version.
        A::Update => built_in("workbench.client.update", C::Client, NONE),
        A::Machines => built_in("orchbee.native.machines.open", C::Navigation, NONE),
        A::Sidebar => built_in("workbench.sidebar.toggle", C::Navigation, SIDEBAR),
        A::Navigator => built_in(
            "orchbee.native.workspace-navigator.toggle",
            C::Navigation,
            NAVIGATOR,
        ),
        A::StartAgent | A::StartAgentInTab(_) | A::MoreAgents => {
            built_in("orchbee.native.agent.start", C::Agent, AGENT)
        }
        A::OpenNotificationTarget => {
            built_in("orchbee.native.notification.target.open", C::Agent, AGENT)
        }
        A::ReleasePointer => built_in("orchbee.native.pointer-capture.toggle", C::Input, MOUSE),
        A::Detach => built_in("orchbee.native.window.detach", C::Client, DETACH),
        A::NextTab => built_in("workbench.tab.select-next", C::Navigation, SELECT_TAB),
        A::PreviousTab => built_in("workbench.tab.select-previous", C::Navigation, SELECT_TAB),
        A::NextWorkspace | A::PreviousWorkspace => built_in(
            "orchbee.native.workspace.select",
            C::Navigation,
            &[ActionSearchAlias::Workspace, ActionSearchAlias::Select],
        ),
        A::FocusPanel(direction) => panel_focus(direction),
        A::ResizePanel(direction) => resize_panel(direction),
        A::NextAgent | A::PreviousAgent => built_in(
            "orchbee.native.agent.select",
            C::Agent,
            &[ActionSearchAlias::Agent, ActionSearchAlias::Select],
        ),
        A::FocusSidebar => built_in("workbench.focus.sidebar", C::Navigation, SIDEBAR),
        // The configured identity stays opaque; every configured row shares one menu identity.
        A::ConfiguredCommand(_) => built_in(
            "orchbee.native.configured-command.run",
            C::Application,
            COMMANDS,
        ),
        // Tab 1..9 is the GUI's select-by-position operation; other key rows only teach a key.
        A::Key(crate::client_actions::KeyRow::Prefix(
            crate::localization::PrefixDesc::SelectTabDigit,
        )) => built_in("workbench.tab.select-by-position", C::Navigation, KEYS),
        A::Key(row) => built_in(
            "orchbee.native.key-reference",
            C::Input,
            match row {
                crate::client_actions::KeyRow::Prefix(_) => KEYS,
                crate::client_actions::KeyRow::Copy(_) => COPY_KEYS,
                crate::client_actions::KeyRow::Sidebar(_) => SIDEBAR_KEYS,
            },
        ),
    };
    MenuActionMetadata {
        action: projected,
        tone,
    }
}

#[cfg(test)]
mod tests;
