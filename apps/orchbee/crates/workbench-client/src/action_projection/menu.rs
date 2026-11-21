//! One declaration per menu action: its title in every shipped locale, its stable identity, its
//! category, the words a search finds it by, and its tone.
//!
//! A new action is its `MenuAction` variant, its arm here, what it runs
//! (`MenuAction::client_action` or its menu's owner) and its place in a menu's list.

use orchbee_workbench_protocol::TabGroupColor;

use super::{ActionCategory, ActionIdentity, ActionMetadata, ActionTone};
use crate::client_actions::{KeyRow, MenuAction};
use crate::localization::{ConfirmationAnswer, Locale, PanelDirection, PrefixDesc, Words, words};

/// The words a search finds an action by, beyond its title and its key.
mod alias {
    use crate::localization::{Words, words};

    pub(super) const ACTIONS: Words = words("actions", "动作");
    pub(super) const COMMANDS: Words = words("commands", "命令");
    pub(super) const PREFERENCES: Words = words("preferences", "偏好设置");
    pub(super) const WORKSPACE: Words = words("workspace", "工作区");
    pub(super) const WORKTREE: Words = words("worktree", "工作树");
    pub(super) const PROJECT: Words = words("project", "项目");
    pub(super) const TAB: Words = words("tab", "标签页");
    pub(super) const PANE: Words = words("panel", "面板");
    pub(super) const SPLIT: Words = words("split", "拆分 分屏");
    pub(super) const RESIZE: Words = words("resize", "调整大小");
    pub(super) const COPY: Words = words("copy", "复制");
    pub(super) const FIND: Words = words("find search", "查找 搜索");
    pub(super) const CLOSE: Words = words("close", "关闭");
    pub(super) const RENAME: Words = words("rename", "重命名");
    pub(super) const MOVE: Words = words("move", "移动");
    pub(super) const SELECT: Words = words("select", "选择");
    pub(super) const SIDEBAR: Words = words("sidebar", "边栏");
    pub(super) const NAVIGATOR: Words = words("navigator", "导航");
    pub(super) const AGENT: Words = words("agent", "智能体");
    pub(super) const HELP: Words = words("help", "帮助");
    pub(super) const MOUSE: Words = words("mouse", "鼠标");
    // Leaving is also what people call quitting or exiting; the title stays Leave view.
    pub(super) const DETACH: Words = words("detach quit exit", "分离 退出 关闭");
}

const ACTIONS: &[Words] = &[alias::ACTIONS, alias::COMMANDS];
const PREFERENCES: &[Words] = &[alias::PREFERENCES];
const WORKSPACE: &[Words] = &[alias::WORKSPACE];
const WORKTREE: &[Words] = &[alias::WORKTREE];
const PROJECT: &[Words] = &[alias::PROJECT];
const TAB: &[Words] = &[alias::TAB];
const PANE: &[Words] = &[alias::PANE];
const SPLIT: &[Words] = &[alias::PANE, alias::SPLIT];
const RESIZE: &[Words] = &[alias::PANE, alias::RESIZE];
const COPY: &[Words] = &[alias::COPY];
const FIND: &[Words] = &[alias::COPY, alias::FIND];
const CLOSE_TAB: &[Words] = &[alias::TAB, alias::CLOSE];
const RENAME_TAB: &[Words] = &[alias::TAB, alias::RENAME];
const MOVE_TAB: &[Words] = &[alias::TAB, alias::MOVE];
const SELECT_TAB: &[Words] = &[alias::TAB, alias::SELECT];
const SIDEBAR: &[Words] = &[alias::SIDEBAR];
const NAVIGATOR: &[Words] = &[alias::NAVIGATOR];
const AGENT: &[Words] = &[alias::AGENT];
const MOUSE: &[Words] = &[alias::MOUSE];
const DETACH: &[Words] = &[alias::DETACH];
const NONE: &[Words] = &[];
const COMMANDS: &[Words] = &[alias::COMMANDS];
/// Key rows answer a search for help, which is where their keys were once listed.
const KEYS: &[Words] = &[alias::HELP];
const COPY_KEYS: &[Words] = &[alias::COPY, alias::HELP];
const SIDEBAR_KEYS: &[Words] = &[alias::SIDEBAR, alias::HELP];

const fn built_in(
    id: &'static str,
    category: ActionCategory,
    search_aliases: &'static [Words],
) -> ActionMetadata<'static> {
    ActionMetadata {
        identity: ActionIdentity::BuiltIn(id),
        category,
        search_aliases,
    }
}

/// What one menu action declares about itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Declaration {
    title: Words,
    pub(crate) action: ActionMetadata<'static>,
    pub(crate) tone: ActionTone,
}

const fn declared(title: Words, action: ActionMetadata<'static>) -> Declaration {
    Declaration {
        title,
        action,
        tone: ActionTone::Ordinary,
    }
}

impl Declaration {
    /// The action ends something the user may want back.
    const fn destructive(mut self) -> Self {
        self.tone = ActionTone::Destructive;
        self
    }
}

/// Operations several rows reach. Those rows differ in title or target and share the identity,
/// the category and the search words.
mod operation {
    use super::super::{ActionCategory as C, ActionMetadata};
    use super::{
        AGENT, CLOSE_TAB, MOVE_TAB as MOVE_TAB_WORDS, PROJECT, TAB, WORKSPACE, WORKTREE, built_in,
    };

    pub(super) const CREATE_TAB: ActionMetadata<'static> =
        built_in("workbench.terminal.create", C::Tab, TAB);
    pub(super) const MOVE_TAB: ActionMetadata<'static> =
        built_in("tabs.tab.move", C::Tab, MOVE_TAB_WORDS);
    pub(super) const PIN_TAB: ActionMetadata<'static> =
        built_in("tabs.tab.pinned.set", C::Tab, TAB);
    pub(super) const UPDATE_GROUP: ActionMetadata<'static> =
        built_in("tabs.group.update", C::Tab, TAB);
    pub(super) const MOVE_TO_GROUP: ActionMetadata<'static> =
        built_in("tabs.tab.move-to-group", C::Tab, MOVE_TAB_WORDS);
    pub(super) const CLOSE_TABS: ActionMetadata<'static> =
        built_in("tabs.tab.close", C::Tab, CLOSE_TAB);
    pub(super) const QUICK_WORKSPACE: ActionMetadata<'static> = built_in(
        "orchbee.native.workspace.quick-create",
        C::Workspace,
        WORKSPACE,
    );
    pub(super) const SET_PROJECT: ActionMetadata<'static> =
        built_in("workbench.workspace.set-project", C::Workspace, PROJECT);
    pub(super) const OPEN_WORKTREE: ActionMetadata<'static> =
        built_in("orchbee.native.worktree.open", C::Workspace, WORKTREE);
    pub(super) const CLOSE_WORKSPACE: ActionMetadata<'static> = built_in(
        "workbench.workspace.delete",
        C::Workspace,
        &[super::alias::WORKSPACE, super::alias::CLOSE],
    );
    pub(super) const START_AGENT: ActionMetadata<'static> =
        built_in("orchbee.native.agent.start", C::Agent, AGENT);
    pub(super) const CONFIRMATIONS: ActionMetadata<'static> =
        built_in("orchbee.native.agent.confirmations.choose", C::Agent, AGENT);
    pub(super) const SELECT_WORKSPACE: ActionMetadata<'static> = built_in(
        "orchbee.native.workspace.select",
        C::Navigation,
        &[super::alias::WORKSPACE, super::alias::SELECT],
    );
    pub(super) const SELECT_AGENT: ActionMetadata<'static> = built_in(
        "orchbee.native.agent.select",
        C::Agent,
        &[super::alias::AGENT, super::alias::SELECT],
    );
}

const fn panel_focus(direction: PanelDirection) -> Declaration {
    let (title, id) = match direction {
        PanelDirection::Left => (
            words("Focus panel left", "向左聚焦面板"),
            "workbench.panel.active.focus-left",
        ),
        PanelDirection::Down => (
            words("Focus panel down", "向下聚焦面板"),
            "workbench.panel.active.focus-down",
        ),
        PanelDirection::Up => (
            words("Focus panel up", "向上聚焦面板"),
            "workbench.panel.active.focus-up",
        ),
        PanelDirection::Right => (
            words("Focus panel right", "向右聚焦面板"),
            "workbench.panel.active.focus-right",
        ),
    };
    declared(
        title,
        built_in(
            id,
            ActionCategory::Navigation,
            &[alias::PANE, alias::SELECT],
        ),
    )
}

const fn split_panel(direction: PanelDirection) -> Declaration {
    let (title, id) = match direction {
        PanelDirection::Left => (
            words("Split left", "向左拆分"),
            "orchbee.native.panel.split-left",
        ),
        PanelDirection::Down => (
            words("Split down", "向下拆分"),
            "workbench.panel.active.split-below",
        ),
        PanelDirection::Up => (
            words("Split up", "向上拆分"),
            "orchbee.native.panel.split-above",
        ),
        PanelDirection::Right => (
            words("Split right", "向右拆分"),
            "workbench.panel.active.split-right",
        ),
    };
    declared(title, built_in(id, ActionCategory::Pane, SPLIT))
}

const fn resize_panel(direction: PanelDirection) -> Declaration {
    let (title, id) = match direction {
        PanelDirection::Left => (
            words("Resize panel left", "向左调整面板大小"),
            "workbench.panel.active.resize-left",
        ),
        PanelDirection::Down => (
            words("Resize panel down", "向下调整面板大小"),
            "workbench.panel.active.resize-down",
        ),
        PanelDirection::Up => (
            words("Resize panel up", "向上调整面板大小"),
            "workbench.panel.active.resize-up",
        ),
        PanelDirection::Right => (
            words("Resize panel right", "向右调整面板大小"),
            "workbench.panel.active.resize-right",
        ),
    };
    declared(title, built_in(id, ActionCategory::Pane, RESIZE))
}

const fn move_panel(direction: PanelDirection) -> Declaration {
    let (title, id) = match direction {
        PanelDirection::Left => (
            words("Move panel left", "向左移动面板"),
            "workbench.panel.active.move-left",
        ),
        PanelDirection::Down => (
            words("Move panel down", "向下移动面板"),
            "workbench.panel.active.move-down",
        ),
        PanelDirection::Up => (
            words("Move panel up", "向上移动面板"),
            "workbench.panel.active.move-up",
        ),
        PanelDirection::Right => (
            words("Move panel right", "向右移动面板"),
            "workbench.panel.active.move-right",
        ),
    };
    declared(title, built_in(id, ActionCategory::Pane, PANE))
}

const fn group_color(color: TabGroupColor) -> Words {
    match color {
        TabGroupColor::Blue => words("Blue", "蓝色"),
        TabGroupColor::Cyan => words("Cyan", "青色"),
        TabGroupColor::Green => words("Green", "绿色"),
        TabGroupColor::Grey => words("Grey", "灰色"),
        TabGroupColor::Orange => words("Orange", "橙色"),
        TabGroupColor::Pink => words("Pink", "粉色"),
        TabGroupColor::Purple => words("Purple", "紫色"),
        TabGroupColor::Red => words("Red", "红色"),
        TabGroupColor::Yellow => words("Yellow", "黄色"),
    }
}

const fn confirmation_answer(answer: ConfirmationAnswer) -> Words {
    match answer {
        ConfirmationAnswer::Off => words("Off", "关闭"),
        ConfirmationAnswer::Run => words("This Agent only", "仅此智能体"),
        // `{agent}` is the product name; the row fills it.
        ConfirmationAnswer::AgentType => words("Always for {agent}", "{agent} 始终允许"),
        ConfirmationAnswer::No => words("No", "不用"),
    }
}

/// Tab 1..9 is the GUI's select-by-position operation; other key rows only teach a key.
const fn key_row(row: KeyRow) -> ActionMetadata<'static> {
    if matches!(row, KeyRow::Prefix(PrefixDesc::SelectTabDigit)) {
        return built_in(
            "workbench.tab.select-by-position",
            ActionCategory::Navigation,
            KEYS,
        );
    }
    built_in(
        "orchbee.native.key-reference",
        ActionCategory::Input,
        match row {
            KeyRow::Prefix(_) => KEYS,
            KeyRow::Copy(_) => COPY_KEYS,
            KeyRow::Sidebar(_) => SIDEBAR_KEYS,
        },
    )
}

impl MenuAction {
    /// Dynamic color and group indices remain typed `MenuAction` input and therefore do not mint
    /// per-row action identities.
    pub(crate) fn declaration(&self) -> Declaration {
        use ActionCategory as C;
        match self {
            Self::SplitPanel(direction) => split_panel(*direction),
            Self::Zoom => declared(
                words("Maximize / Restore", "最大化 / 还原"),
                built_in("workbench.panel.active.maximization.toggle", C::Pane, PANE),
            ),
            Self::Copy => declared(
                words("Keyboard copy mode", "键盘复制模式"),
                built_in("orchbee.native.copy-mode.enter", C::Input, COPY),
            ),
            // Shares the GUI terminal catalog identity: the same copy operation, not a new one.
            Self::CopySelection => declared(
                words("Copy", "复制"),
                built_in("terminal.selection.copy", C::Pane, COPY),
            ),
            Self::Takeover => declared(
                words("Type here", "在这里输入"),
                built_in("orchbee.native.panel.control.takeover", C::Pane, PANE),
            ),
            Self::ClosePane => declared(
                words("Close panel…", "关闭面板…"),
                built_in(
                    "workbench.terminal.active.close",
                    C::Pane,
                    &[alias::PANE, alias::CLOSE],
                ),
            )
            .destructive(),
            Self::MovePanel(direction) => move_panel(*direction),
            Self::NewTab => declared(words("New Tab", "新建标签页"), operation::CREATE_TAB),
            Self::NewTabWithProfile(_) => {
                declared(words("New Tab", "新建标签页"), operation::CREATE_TAB)
            }
            Self::TabCreation => declared(
                words("New Tab options", "新建标签页选项"),
                built_in("orchbee.native.tab-create.open", C::Tab, ACTIONS),
            ),
            Self::NewTabToRight => declared(
                words("New Tab to right", "在此标签页右侧新建"),
                operation::CREATE_TAB,
            ),
            Self::NewTabInGroup => declared(
                words("New Tab in group", "在分组中新建标签页"),
                operation::CREATE_TAB,
            ),
            Self::DuplicateTab => declared(
                words("Duplicate Tab", "复制标签页"),
                built_in("tabs.tab.duplicate", C::Tab, TAB),
            ),
            Self::MoveTabPrevious => declared(
                words("Move this Tab left", "向左移动此标签页"),
                operation::MOVE_TAB,
            ),
            Self::MoveTabNext => declared(
                words("Move this Tab right", "向右移动此标签页"),
                operation::MOVE_TAB,
            ),
            Self::MoveTabFirst => declared(
                words("Move this Tab to start", "将此标签页移至开头"),
                operation::MOVE_TAB,
            ),
            Self::MoveTabLast => declared(
                words("Move this Tab to end", "将此标签页移至末尾"),
                operation::MOVE_TAB,
            ),
            Self::SelectTab => declared(
                words("Add this Tab to selection", "将此标签页加入选择"),
                built_in("tabs.tab.select", C::Tab, SELECT_TAB),
            ),
            Self::SelectAllTabs => declared(
                words("Select all Tabs", "选择所有标签页"),
                built_in("orchbee.native.tab-selection.all", C::Tab, SELECT_TAB),
            ),
            Self::ClearTabSelection => declared(
                words("Keep only current Tab selected", "仅保留当前标签页的选择"),
                built_in("orchbee.native.tab-selection.clear", C::Tab, SELECT_TAB),
            ),
            Self::PinTab => declared(words("Pin Tab", "固定标签页"), operation::PIN_TAB),
            Self::UnpinTab => declared(words("Unpin Tab", "取消固定标签页"), operation::PIN_TAB),
            Self::ManageGroups => declared(
                words("Add Tab to group", "将标签页加入分组"),
                built_in("orchbee.native.tab-groups.open", C::Tab, ACTIONS),
            ),
            Self::CreateGroup => declared(
                words("New group…", "新建分组…"),
                built_in("tabs.group.create", C::Tab, TAB),
            ),
            Self::RenameGroup => {
                declared(words("Rename group", "重命名分组"), operation::UPDATE_GROUP)
            }
            Self::ChooseGroupColor => declared(
                words("Group color", "分组颜色"),
                built_in(
                    "orchbee.native.tab-group.color-choices.open",
                    C::Tab,
                    ACTIONS,
                ),
            ),
            Self::GroupColor(color) => declared(group_color(*color), operation::UPDATE_GROUP),
            Self::JoinGroup(_) => declared(
                words("Move Tab to group", "将标签页移入分组"),
                operation::MOVE_TO_GROUP,
            ),
            Self::RemoveFromGroup => declared(
                words("Remove Tab from group", "将标签页移出分组"),
                operation::MOVE_TO_GROUP,
            ),
            Self::Ungroup => declared(
                words("Ungroup, keep Tabs", "取消分组，保留标签页"),
                built_in("tabs.group.ungroup", C::Tab, TAB),
            ),
            Self::RenameTab => declared(
                words("Rename Tab", "重命名标签页"),
                built_in("workbench.tab.title.set", C::Tab, RENAME_TAB),
            ),
            Self::RenamePanel => declared(
                words("Rename Panel", "重命名面板"),
                built_in(
                    "workbench.panel.rename",
                    C::Pane,
                    &[alias::PANE, alias::RENAME],
                ),
            ),
            Self::FindInTerminal => declared(
                words("Find in terminal", "在终端中查找"),
                built_in("terminal.search.open", C::Pane, FIND),
            ),
            Self::ClearTerminal => declared(
                words("Clear terminal", "清除终端"),
                built_in("terminal.clear", C::Pane, PANE),
            ),
            // Shares the GUI Workbench identity: the same Session-wide focus history step.
            Self::LastPanel => declared(
                words("Back to last focused panel", "回到上次聚焦的面板"),
                built_in(
                    "workbench.panel.focus-last",
                    C::Navigation,
                    &[alias::PANE, alias::SELECT],
                ),
            ),
            Self::CloseSelectedTabs => {
                declared(words("Close Tab…", "关闭标签页…"), operation::CLOSE_TABS).destructive()
            }
            Self::CloseOtherTabs => declared(
                words("Close other Tabs…", "关闭其他标签页…"),
                operation::CLOSE_TABS,
            )
            .destructive(),
            Self::CloseTrailingTabs => declared(
                words("Close Tabs to the right…", "关闭右侧标签页…"),
                operation::CLOSE_TABS,
            )
            .destructive(),
            Self::CloseGroup => {
                declared(words("Close group…", "关闭分组…"), operation::CLOSE_TABS).destructive()
            }
            Self::CloseTab => {
                declared(words("Close Tab…", "关闭标签页…"), operation::CLOSE_TABS).destructive()
            }
            Self::QuickWorkspace => declared(
                words("New Workspace", "新建工作区"),
                operation::QUICK_WORKSPACE,
            ),
            Self::WorkspaceAtDirectory => declared(
                words(
                    "Choose directory for new Workspace…",
                    "选择目录并新建工作区…",
                ),
                built_in(
                    "orchbee.native.workspace.create-in-directory",
                    C::Workspace,
                    WORKSPACE,
                ),
            ),
            Self::RenameWorkspace => declared(
                words("Rename Workspace", "重命名工作区"),
                built_in(
                    "orchbee.native.workspace.rename",
                    C::Workspace,
                    &[alias::WORKSPACE, alias::RENAME],
                ),
            ),
            Self::RenameProject => declared(
                words("Rename Project", "重命名项目"),
                built_in("workbench.project.rename", C::Project, PROJECT),
            ),
            Self::WorkspaceProjects => declared(
                words("Move to Project", "移动到项目"),
                built_in(
                    "orchbee.native.workspace.project-menu.open",
                    C::Workspace,
                    PROJECT,
                ),
            ),
            Self::WorkspaceCreation => declared(
                words("New Workspace", "新建工作区"),
                built_in(
                    "orchbee.native.workspace.creation-menu.open",
                    C::Workspace,
                    WORKSPACE,
                ),
            ),
            Self::WorkspaceWorktrees => declared(
                words("Open Worktree", "打开工作树"),
                built_in(
                    "orchbee.native.workspace.worktree-menu.open",
                    C::Workspace,
                    WORKTREE,
                ),
            ),
            Self::CreateWorkspaceProject => declared(
                words("New Project…", "新建项目…"),
                built_in("workbench.workspace.create-project", C::Workspace, PROJECT),
            ),
            Self::LeaveWorkspaceProject => declared(
                words("Remove from Project", "移出项目"),
                operation::SET_PROJECT,
            ),
            Self::JoinWorkspaceProject(_) => {
                declared(words("Move to Project", "移入项目"), operation::SET_PROJECT)
            }
            Self::WorkspaceInFolder(_) => declared(
                words("New Workspace in folder", "在文件夹中新建工作区"),
                operation::QUICK_WORKSPACE,
            ),
            Self::OpenWorktreeAt(_) => declared(
                words("Open Worktree", "打开工作树"),
                operation::OPEN_WORKTREE,
            ),
            Self::RemoveWorktree => declared(
                words("Remove Worktree…", "移除工作树…"),
                built_in("orchbee.native.worktree.remove", C::Workspace, WORKTREE),
            )
            .destructive(),
            Self::DeleteWorkspace => declared(
                words("Close Workspace…", "关闭工作区…"),
                operation::CLOSE_WORKSPACE,
            )
            .destructive(),
            Self::DeleteProject => declared(
                words("Ungroup", "取消分组"),
                built_in("workbench.project.dissolve", C::Project, PROJECT),
            ),
            Self::CloseProjectWorkspaces => declared(
                words("Close all Workspaces…", "关闭项目内全部工作区…"),
                built_in("workbench.project.close-all", C::Project, PROJECT),
            ),
            Self::CloseWorkspace => declared(
                words("Close Workspace…", "关闭工作区…"),
                operation::CLOSE_WORKSPACE,
            ),
            Self::NewWorkspace => declared(
                words("New Workspace", "新建工作区"),
                built_in("workbench.workspace.create", C::Workspace, WORKSPACE),
            ),
            Self::NewWorktree => declared(
                words("New Worktree…", "新建工作树…"),
                built_in("orchbee.native.worktree.create", C::Workspace, WORKTREE),
            ),
            Self::OpenWorktree => declared(
                words("Open Worktree…", "打开工作树…"),
                operation::OPEN_WORKTREE,
            ),
            Self::Settings => declared(
                words("Settings…", "设置…"),
                built_in("workbench.settings.open", C::Application, PREFERENCES),
            ),
            // Shares the window's identity: the same switch to the active version.
            Self::Update => declared(
                words("Update", "更新"),
                built_in("workbench.client.update", C::Client, NONE),
            ),
            Self::Machines => declared(
                words(
                    "Browse machines, Workspaces and Agents",
                    "浏览机器、工作区和智能体",
                ),
                built_in("orchbee.native.machines.open", C::Navigation, NONE),
            ),
            Self::Sidebar => declared(
                words("Toggle sidebar", "切换边栏"),
                built_in("workbench.sidebar.toggle", C::Navigation, SIDEBAR),
            ),
            Self::Navigator => declared(
                words("Workspace navigator", "工作区导航器"),
                built_in(
                    "orchbee.native.workspace-navigator.toggle",
                    C::Navigation,
                    NAVIGATOR,
                ),
            ),
            Self::StartAgent => {
                declared(words("Start Agent…", "启动智能体…"), operation::START_AGENT)
            }
            // A row shows its provider's product name; this names the action itself.
            Self::StartAgentInTab(_) => declared(
                words("Start Agent in new Tab", "在新标签页中启动智能体"),
                operation::START_AGENT,
            ),
            Self::MoreAgents => {
                declared(words("More Agents…", "更多智能体…"), operation::START_AGENT)
            }
            Self::OpenNotificationTarget => declared(
                words("Go to the Agent that notified", "跳到发通知的智能体"),
                built_in("orchbee.native.notification.target.open", C::Agent, AGENT),
            ),
            Self::AgentConfirmations => declared(
                words("Allow confirmations", "自动允许确认"),
                operation::CONFIRMATIONS,
            ),
            Self::AgentConfirmation(answer) => {
                declared(confirmation_answer(*answer), operation::CONFIRMATIONS)
            }
            Self::ReleasePointer => declared(
                words("Release mouse", "释放鼠标"),
                built_in("orchbee.native.pointer-capture.toggle", C::Input, MOUSE),
            ),
            Self::Detach => declared(
                words(
                    "Leave view; terminals keep running",
                    "离开界面；终端继续运行",
                ),
                built_in("orchbee.native.window.detach", C::Client, DETACH),
            ),
            Self::NextTab => declared(
                words("Select next Tab", "选择下一个标签页"),
                built_in("workbench.tab.select-next", C::Navigation, SELECT_TAB),
            ),
            Self::PreviousTab => declared(
                words("Select previous Tab", "选择上一个标签页"),
                built_in("workbench.tab.select-previous", C::Navigation, SELECT_TAB),
            ),
            Self::NextWorkspace => declared(
                words("Select next Workspace", "选择下一个工作区"),
                operation::SELECT_WORKSPACE,
            ),
            Self::PreviousWorkspace => declared(
                words("Select previous Workspace", "选择上一个工作区"),
                operation::SELECT_WORKSPACE,
            ),
            Self::FocusPanel(direction) => panel_focus(*direction),
            Self::ResizePanel(direction) => resize_panel(*direction),
            Self::NextAgent => declared(
                words("Select next Agent", "选择下一个智能体"),
                operation::SELECT_AGENT,
            ),
            Self::PreviousAgent => declared(
                words("Select previous Agent", "选择上一个智能体"),
                operation::SELECT_AGENT,
            ),
            Self::FocusSidebar => declared(
                words("Focus sidebar", "聚焦边栏"),
                built_in("workbench.focus.sidebar", C::Navigation, SIDEBAR),
            ),
            // The configured identity stays opaque; every configured row shares one menu identity.
            Self::ConfiguredCommand(_) => declared(
                words("Run configured command", "运行已配置的命令"),
                built_in(
                    "orchbee.native.configured-command.run",
                    C::Application,
                    COMMANDS,
                ),
            ),
            Self::Key(row) => declared(words("Shortcut", "快捷键"), key_row(*row)),
        }
    }

    /// The action's full title; a row that carries a name of its own shows that instead.
    pub(crate) fn title(&self, locale: Locale) -> &'static str {
        locale.words(self.declaration().title)
    }
}
