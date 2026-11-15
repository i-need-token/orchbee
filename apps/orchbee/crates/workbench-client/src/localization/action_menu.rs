use super::{Locale, PanelDirection, TabMenuScope};

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
    Find,
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
/// Textual action identity without an executable target, profile or captured command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuLabel {
    SplitPanel(PanelDirection),
    Zoom,
    Copy,
    CopySelection,
    Takeover,
    ClosePane,
    MovePanel(PanelDirection),
    NewTab,
    NewTabWithProfile,
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
    JoinGroup,
    RemoveFromGroup,
    Ungroup,
    RenameTab,
    RenamePanel,
    FindInTerminal,
    ClearTerminal,
    LastPanel,
    CloseSelectedTabs,
    CloseOtherTabs,
    CloseTrailingTabs,
    CloseGroup,
    CloseTab,
    QuickWorkspace,
    WorkspaceAtDirectory,
    RenameWorkspace,
    RenameProject,
    WorkspaceProjects,
    WorkspaceCreation,
    WorkspaceWorktrees,
    CreateWorkspaceProject,
    LeaveWorkspaceProject,
    JoinWorkspaceProject,
    WorkspaceInFolder,
    OpenWorktreeAt,
    RemoveWorktree,
    DeleteWorkspace,
    DeleteProject,
    CloseProjectWorkspaces,
    CloseWorkspace,
    NewWorkspace,
    NewWorktree,
    OpenWorktree,
    Settings,
    Update,
    Machines,
    Sidebar,
    Navigator,
    StartAgent,
    StartAgentInTab,
    MoreAgents,
    OpenNotificationTarget,
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
    ConfiguredCommand,
    /// A key row's generic title; its row names the mode and the key's own words.
    Key,
}

/// A labeled group inside a submenu; never a candidate and never an action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuHeading {
    Folders,
    Worktrees,
    /// The new-Tab menu's Agents, after its shells.
    Agents,
}

/// Secondary text that ends a row, such as the default launch profile's mark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuRowNote {
    DefaultProfile,
}

/// Why the action owner has withheld a menu action; never inferred from translated text. Only a
/// few reasons have words: see [`Locale::menu_unavailable`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuUnavailable {
    WorkspaceRequired,
    FocusedPanelRequired,
    /// No Panel lies on that side of the focused one.
    NoPanel(PanelDirection),
    PanelMaximized,
    /// One Panel already fills its Tab, so there is nothing to maximize.
    MultiplePanelsRequired,
    /// Last Panel has no earlier Panel that still exists.
    NoEarlierPanel,
    /// A Tab moves only among Tabs pinned as it is; these name why it has none to pass.
    OnlyOneTab,
    NoOtherPinnedTab,
    NoOtherUnpinnedTab,
    /// The Tab already stands at that end of the Tabs pinned as it is, outside any group.
    TabAlreadyFirst,
    TabAlreadyLast,
    /// The focused terminal lost its connection and waits to reattach.
    TerminalDisconnected,
    ProjectWorkspaceRequired,
    OperationBusy,
    PointerReleased,
    NoMatchingTabs,
    WorktreeChecking,
    WorktreeUnavailable,
    NotGitRepository,
    NoOtherWorktrees,
    NoSelection,
    /// A key row's key works only in keyboard copy mode.
    KeyInCopyMode,
    /// A key row's key works only while the sidebar has the keyboard.
    KeyInSidebar,
    /// A key row's key picks among targets the menu cannot choose, so only the key runs it.
    KeyOnly,
}

impl Locale {
    pub(crate) const fn menu_search_label(self) -> &'static str {
        match self {
            Self::EnUs => "Search",
            Self::ZhCn => "搜索",
        }
    }

    pub(crate) fn menu_search_count(self, count: usize) -> String {
        match self {
            Self::EnUs => format!("{count} matches"),
            Self::ZhCn => format!("{count} 项匹配"),
        }
    }

    pub(crate) const fn menu_search_no_results(self) -> &'static str {
        match self {
            Self::EnUs => "No actions match this query.",
            Self::ZhCn => "没有符合此搜索的动作。",
        }
    }

    pub(crate) const fn menu_search_alias(self, alias: ActionSearchAlias) -> &'static str {
        use ActionSearchAlias as A;
        match (self, alias) {
            (Self::EnUs, A::Actions) => "actions",
            (Self::ZhCn, A::Actions) => "动作",
            (Self::EnUs, A::Commands) => "commands",
            (Self::ZhCn, A::Commands) => "命令",
            (Self::EnUs, A::Preferences) => "preferences",
            (Self::ZhCn, A::Preferences) => "偏好设置",
            (Self::EnUs, A::Workspace) => "workspace",
            (Self::ZhCn, A::Workspace) => "工作区",
            (Self::EnUs, A::Worktree) => "worktree",
            (Self::ZhCn, A::Worktree) => "工作树",
            (Self::EnUs, A::Project) => "project",
            (Self::ZhCn, A::Project) => "项目",
            (Self::EnUs, A::Tab) => "tab",
            (Self::ZhCn, A::Tab) => "标签页",
            (Self::EnUs, A::Pane) => "panel",
            (Self::ZhCn, A::Pane) => "面板",
            (Self::EnUs, A::Split) => "split",
            (Self::ZhCn, A::Split) => "拆分 分屏",
            (Self::EnUs, A::Resize) => "resize",
            (Self::ZhCn, A::Resize) => "调整大小",
            (Self::EnUs, A::Copy) => "copy",
            (Self::ZhCn, A::Copy) => "复制",
            (Self::EnUs, A::Find) => "find search",
            (Self::ZhCn, A::Find) => "查找 搜索",
            (Self::EnUs, A::Close) => "close",
            (Self::ZhCn, A::Close) => "关闭",
            (Self::EnUs, A::Rename) => "rename",
            (Self::ZhCn, A::Rename) => "重命名",
            (Self::EnUs, A::Move) => "move",
            (Self::ZhCn, A::Move) => "移动",
            (Self::EnUs, A::Select) => "select",
            (Self::ZhCn, A::Select) => "选择",
            (Self::EnUs, A::Sidebar) => "sidebar",
            (Self::ZhCn, A::Sidebar) => "边栏",
            (Self::EnUs, A::Navigator) => "navigator",
            (Self::ZhCn, A::Navigator) => "导航",
            (Self::EnUs, A::Agent) => "agent",
            (Self::ZhCn, A::Agent) => "智能体",
            (Self::EnUs, A::Help) => "help",
            (Self::ZhCn, A::Help) => "帮助",
            (Self::EnUs, A::Mouse) => "mouse",
            (Self::ZhCn, A::Mouse) => "鼠标",
            // Leaving is also what people call quitting or exiting; the title stays Leave view.
            (Self::EnUs, A::Detach) => "detach quit exit",
            (Self::ZhCn, A::Detach) => "分离 退出 关闭",
        }
    }

    pub(crate) const fn menu_row_note(self, note: MenuRowNote) -> &'static str {
        match (self, note) {
            (Self::EnUs, MenuRowNote::DefaultProfile) => "Default",
            (Self::ZhCn, MenuRowNote::DefaultProfile) => "默认",
        }
    }

    /// Saved names are user data; the missing built-in name is localized here.
    pub(crate) fn launch_profile_label(self, name: Option<&str>) -> String {
        crate::terminal_text::sanitize_terminal_text(name.unwrap_or(match self {
            Self::EnUs => "System default",
            Self::ZhCn => "系统默认",
        }))
        .into_owned()
    }
    pub(crate) const fn menu_back_button(self) -> &'static str {
        match self {
            Self::EnUs => "Back",
            Self::ZhCn => "返回",
        }
    }
    pub(crate) const fn menu_close_button(self) -> &'static str {
        match self {
            Self::EnUs => "Close",
            Self::ZhCn => "关闭",
        }
    }
    pub(crate) const fn menu_cancel_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn menu_run_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Run",
            Self::ZhCn => "执行",
        }
    }
    pub(crate) const fn menu_select_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Select",
            Self::ZhCn => "选择",
        }
    }
    pub(crate) fn tab_drag_hint(self, count: usize, valid: bool) -> String {
        let noun = if count == 1 { "Tab" } else { "Tabs" };
        match (self, valid) {
            (Self::EnUs, true) => format!("Move {count} {noun} · release to place · Esc cancel"),
            (Self::ZhCn, true) => format!("移动 {count} 个标签页 · 松手放置 · Esc 取消"),
            (Self::EnUs, false) => {
                format!("Move {count} {noun} · release here to cancel · Esc cancel")
            }
            (Self::ZhCn, false) => format!("移动 {count} 个标签页 · 此处松手取消 · Esc 取消"),
        }
    }
    /// The words a grey row shows when it is the candidate. Only a reason whose fix is not obvious
    /// has words; every other unavailable row is simply grey.
    pub(crate) const fn menu_unavailable(self, reason: MenuUnavailable) -> Option<&'static str> {
        Some(match (self, reason) {
            (Self::EnUs, MenuUnavailable::TerminalDisconnected) => "Terminal disconnected",
            (Self::ZhCn, MenuUnavailable::TerminalDisconnected) => "终端已断开",
            (Self::EnUs, MenuUnavailable::NotGitRepository) => "Not in a Git repository",
            (Self::ZhCn, MenuUnavailable::NotGitRepository) => "不在 Git 仓库里",
            (Self::EnUs, MenuUnavailable::NoSelection) => "Select text first",
            (Self::ZhCn, MenuUnavailable::NoSelection) => "请先选中文本",
            _ => return None,
        })
    }
    /// The mode a key row's key works in, which leads the row: `Copy mode · Search …`.
    pub(crate) const fn menu_key_mode(self, sidebar: bool) -> &'static str {
        match (self, sidebar) {
            (Self::EnUs, false) => "Copy mode",
            (Self::ZhCn, false) => "复制模式",
            (Self::EnUs, true) => "Sidebar",
            (Self::ZhCn, true) => "边栏",
        }
    }

    pub(crate) const fn menu_heading(self, heading: MenuHeading) -> &'static str {
        match (self, heading) {
            (Self::EnUs, MenuHeading::Folders) => "Folders",
            (Self::ZhCn, MenuHeading::Folders) => "文件夹",
            (Self::EnUs, MenuHeading::Worktrees) => "Git Worktrees",
            (Self::ZhCn, MenuHeading::Worktrees) => "Git 工作树",
            (Self::EnUs, MenuHeading::Agents) => "Agents",
            (Self::ZhCn, MenuHeading::Agents) => "智能体",
        }
    }
    pub(crate) const fn menu_title(self) -> &'static str {
        match self {
            Self::EnUs => "Actions",
            Self::ZhCn => "操作",
        }
    }
    pub(crate) fn menu_action(self, action: impl Into<MenuLabel>) -> &'static str {
        use MenuLabel::*;
        match (self, action.into()) {
            (Self::EnUs, TabCreation) => "New Tab options",
            (Self::ZhCn, TabCreation) => "新建标签页选项",
            (Self::EnUs, CloseSelectedTabs) => "Close Tab…",
            (Self::ZhCn, CloseSelectedTabs) => "关闭标签页…",
            (Self::EnUs, CloseOtherTabs) => "Close other Tabs…",
            (Self::ZhCn, CloseOtherTabs) => "关闭其他标签页…",
            (Self::EnUs, CloseTrailingTabs) => "Close Tabs to the right…",
            (Self::ZhCn, CloseTrailingTabs) => "关闭右侧标签页…",
            (Self::EnUs, CloseGroup) => "Close group…",
            (Self::ZhCn, CloseGroup) => "关闭分组…",
            (Self::EnUs, CloseTab) => "Close Tab…",
            (Self::ZhCn, CloseTab) => "关闭标签页…",
            (Self::EnUs, NewTabToRight) => "New Tab to right",
            (Self::ZhCn, NewTabToRight) => "在此标签页右侧新建",
            (Self::EnUs, NewTabInGroup) => "New Tab in group",
            (Self::ZhCn, NewTabInGroup) => "在分组中新建标签页",
            (Self::EnUs, DuplicateTab) => "Duplicate Tab",
            (Self::ZhCn, DuplicateTab) => "复制标签页",
            (Self::EnUs, SplitPanel(PanelDirection::Left)) => "Split left",
            (Self::ZhCn, SplitPanel(PanelDirection::Left)) => "向左拆分",
            (Self::EnUs, SplitPanel(PanelDirection::Down)) => "Split down",
            (Self::ZhCn, SplitPanel(PanelDirection::Down)) => "向下拆分",
            (Self::EnUs, SplitPanel(PanelDirection::Up)) => "Split up",
            (Self::ZhCn, SplitPanel(PanelDirection::Up)) => "向上拆分",
            (Self::EnUs, SplitPanel(PanelDirection::Right)) => "Split right",
            (Self::ZhCn, SplitPanel(PanelDirection::Right)) => "向右拆分",
            (Self::EnUs, Zoom) => "Maximize / Restore",
            (Self::ZhCn, Zoom) => "最大化 / 还原",
            (Self::EnUs, Copy) => "Keyboard copy mode",
            (Self::ZhCn, Copy) => "键盘复制模式",
            (Self::EnUs, CopySelection) => "Copy",
            (Self::ZhCn, CopySelection) => "复制",
            (Self::EnUs, Takeover) => "Type here",
            (Self::ZhCn, Takeover) => "在这里输入",
            (Self::EnUs, ClosePane) => "Close panel…",
            (Self::ZhCn, ClosePane) => "关闭面板…",
            (Self::EnUs, MovePanel(PanelDirection::Left)) => "Move panel left",
            (Self::ZhCn, MovePanel(PanelDirection::Left)) => "向左移动面板",
            (Self::EnUs, MovePanel(PanelDirection::Down)) => "Move panel down",
            (Self::ZhCn, MovePanel(PanelDirection::Down)) => "向下移动面板",
            (Self::EnUs, MovePanel(PanelDirection::Up)) => "Move panel up",
            (Self::ZhCn, MovePanel(PanelDirection::Up)) => "向上移动面板",
            (Self::EnUs, MovePanel(PanelDirection::Right)) => "Move panel right",
            (Self::ZhCn, MovePanel(PanelDirection::Right)) => "向右移动面板",
            (Self::EnUs, MoveTabPrevious) => "Move this Tab left",
            (Self::ZhCn, MoveTabPrevious) => "向左移动此标签页",
            (Self::EnUs, MoveTabNext) => "Move this Tab right",
            (Self::ZhCn, MoveTabNext) => "向右移动此标签页",
            (Self::EnUs, MoveTabFirst) => "Move this Tab to start",
            (Self::ZhCn, MoveTabFirst) => "将此标签页移至开头",
            (Self::EnUs, MoveTabLast) => "Move this Tab to end",
            (Self::ZhCn, MoveTabLast) => "将此标签页移至末尾",
            (Self::EnUs, SelectTab) => "Add this Tab to selection",
            (Self::ZhCn, SelectTab) => "将此标签页加入选择",
            (Self::EnUs, SelectAllTabs) => "Select all Tabs",
            (Self::ZhCn, SelectAllTabs) => "选择所有标签页",
            (Self::EnUs, ClearTabSelection) => "Keep only current Tab selected",
            (Self::ZhCn, ClearTabSelection) => "仅保留当前标签页的选择",
            (Self::EnUs, PinTab) => "Pin Tab",
            (Self::ZhCn, PinTab) => "固定标签页",
            (Self::EnUs, UnpinTab) => "Unpin Tab",
            (Self::ZhCn, UnpinTab) => "取消固定标签页",
            (Self::EnUs, ManageGroups) => "Add Tab to group",
            (Self::ZhCn, ManageGroups) => "将标签页加入分组",
            (Self::EnUs, CreateGroup) => "New group…",
            (Self::ZhCn, CreateGroup) => "新建分组…",
            (Self::EnUs, RenameTab) => "Rename Tab",
            (Self::EnUs, RenamePanel) => "Rename Panel",
            (Self::EnUs, FindInTerminal) => "Find in terminal",
            (Self::ZhCn, FindInTerminal) => "在终端中查找",
            (Self::EnUs, ClearTerminal) => "Clear terminal",
            (Self::ZhCn, ClearTerminal) => "清除终端",
            (Self::EnUs, LastPanel) => "Back to last focused panel",
            (Self::ZhCn, LastPanel) => "回到上次聚焦的面板",
            (Self::ZhCn, RenamePanel) => "重命名面板",
            (Self::EnUs, RenameWorkspace) => "Rename Workspace",
            (Self::EnUs, RenameProject) => "Rename Project",
            (Self::EnUs, RenameGroup) => "Rename group",
            (Self::ZhCn, RenameTab) => "重命名标签页",
            (Self::ZhCn, RenameWorkspace) => "重命名工作区",
            (Self::ZhCn, RenameProject) => "重命名项目",
            (Self::ZhCn, RenameGroup) => "重命名分组",
            (Self::EnUs, ChooseGroupColor) => "Group color",
            (Self::ZhCn, ChooseGroupColor) => "分组颜色",
            (Self::EnUs, JoinGroup) => "Move Tab to group",
            (Self::ZhCn, JoinGroup) => "将标签页移入分组",
            (Self::EnUs, RemoveFromGroup) => "Remove Tab from group",
            (Self::ZhCn, RemoveFromGroup) => "将标签页移出分组",
            (Self::EnUs, Ungroup) => "Ungroup, keep Tabs",
            (Self::ZhCn, Ungroup) => "取消分组，保留标签页",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Blue)) => "Blue",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Blue)) => "蓝色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Cyan)) => "Cyan",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Cyan)) => "青色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Green)) => "Green",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Green)) => "绿色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Grey)) => "Grey",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Grey)) => "灰色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Orange)) => "Orange",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Orange)) => "橙色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Pink)) => "Pink",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Pink)) => "粉色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Purple)) => "Purple",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Purple)) => "紫色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Red)) => "Red",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Red)) => "红色",
            (Self::EnUs, GroupColor(orchbee_workbench_protocol::TabGroupColor::Yellow)) => "Yellow",
            (Self::ZhCn, GroupColor(orchbee_workbench_protocol::TabGroupColor::Yellow)) => "黄色",
            (Self::EnUs, NewTab) => "New Tab",
            (Self::ZhCn, NewTab) => "新建标签页",
            (Self::EnUs, NewTabWithProfile) => "New Tab",
            (Self::ZhCn, NewTabWithProfile) => "新建标签页",
            (Self::EnUs, QuickWorkspace) => "New Workspace",
            (Self::ZhCn, QuickWorkspace) => "新建工作区",
            (Self::EnUs, WorkspaceAtDirectory) => "New Workspace in directory…",
            (Self::ZhCn, WorkspaceAtDirectory) => "在指定目录新建工作区…",
            (Self::EnUs, WorkspaceProjects) => "Move to Project",
            (Self::ZhCn, WorkspaceProjects) => "移动到项目",
            (Self::EnUs, WorkspaceCreation) => "New Workspace",
            (Self::ZhCn, WorkspaceCreation) => "新建工作区",
            (Self::EnUs, WorkspaceWorktrees) => "Open Worktree",
            (Self::ZhCn, WorkspaceWorktrees) => "打开工作树",
            (Self::EnUs, WorkspaceInFolder) => "New Workspace in folder",
            (Self::ZhCn, WorkspaceInFolder) => "在文件夹中新建工作区",
            (Self::EnUs, CreateWorkspaceProject) => "New Project…",
            (Self::ZhCn, CreateWorkspaceProject) => "新建项目…",
            (Self::EnUs, LeaveWorkspaceProject) => "Remove from Project",
            (Self::ZhCn, LeaveWorkspaceProject) => "移出项目",
            (Self::EnUs, JoinWorkspaceProject) => "Move to Project",
            (Self::ZhCn, JoinWorkspaceProject) => "移入项目",
            (Self::EnUs, OpenWorktreeAt) => "Open Worktree",
            (Self::ZhCn, OpenWorktreeAt) => "打开工作树",
            (Self::EnUs, RemoveWorktree) => "Remove Worktree…",
            (Self::ZhCn, RemoveWorktree) => "移除工作树…",
            (Self::EnUs, DeleteWorkspace) => "Close Workspace…",
            (Self::ZhCn, DeleteWorkspace) => "关闭工作区…",
            (Self::EnUs, DeleteProject) => "Ungroup",
            (Self::ZhCn, DeleteProject) => "取消分组",
            (Self::EnUs, CloseProjectWorkspaces) => "Close all Workspaces…",
            (Self::ZhCn, CloseProjectWorkspaces) => "关闭项目内全部工作区…",
            (Self::EnUs, CloseWorkspace) => "Close Workspace…",
            (Self::ZhCn, CloseWorkspace) => "关闭工作区…",
            (Self::EnUs, NewWorkspace) => "New Workspace",
            (Self::ZhCn, NewWorkspace) => "新建工作区",
            (Self::EnUs, NewWorktree) => "New Worktree…",
            (Self::ZhCn, NewWorktree) => "新建工作树…",
            (Self::EnUs, OpenWorktree) => "Open Worktree…",
            (Self::ZhCn, OpenWorktree) => "打开工作树…",
            (Self::EnUs, Settings) => "Settings…",
            (Self::ZhCn, Settings) => "设置…",
            (Self::EnUs, Update) => "Update",
            (Self::ZhCn, Update) => "更新",
            (Self::EnUs, Machines) => "Browse machines, Workspaces and Agents",
            (Self::ZhCn, Machines) => "浏览机器、工作区和智能体",
            (Self::EnUs, Sidebar) => "Toggle sidebar",
            (Self::ZhCn, Sidebar) => "切换边栏",
            (Self::EnUs, Navigator) => "Workspace navigator",
            (Self::ZhCn, Navigator) => "工作区导航器",
            (Self::EnUs, StartAgent) => "Start Agent…",
            (Self::ZhCn, StartAgent) => "启动智能体…",
            // A row shows its provider's product name; this names the action itself.
            (Self::EnUs, StartAgentInTab) => "Start Agent in new Tab",
            (Self::ZhCn, StartAgentInTab) => "在新标签页中启动智能体",
            (Self::EnUs, MoreAgents) => "More Agents…",
            (Self::ZhCn, MoreAgents) => "更多智能体…",
            (Self::EnUs, OpenNotificationTarget) => "Go to the Agent that notified",
            (Self::ZhCn, OpenNotificationTarget) => "跳到发通知的智能体",
            (Self::EnUs, ReleasePointer) => "Release mouse",
            (Self::ZhCn, ReleasePointer) => "释放鼠标",
            (_, Detach) => self.client_detach_label(),
            (Self::EnUs, NextTab) => "Select next Tab",
            (Self::ZhCn, NextTab) => "选择下一个标签页",
            (Self::EnUs, PreviousTab) => "Select previous Tab",
            (Self::ZhCn, PreviousTab) => "选择上一个标签页",
            (Self::EnUs, NextWorkspace) => "Select next Workspace",
            (Self::ZhCn, NextWorkspace) => "选择下一个工作区",
            (Self::EnUs, PreviousWorkspace) => "Select previous Workspace",
            (Self::ZhCn, PreviousWorkspace) => "选择上一个工作区",
            (Self::EnUs, FocusPanel(PanelDirection::Left)) => "Focus panel left",
            (Self::ZhCn, FocusPanel(PanelDirection::Left)) => "向左聚焦面板",
            (Self::EnUs, FocusPanel(PanelDirection::Down)) => "Focus panel down",
            (Self::ZhCn, FocusPanel(PanelDirection::Down)) => "向下聚焦面板",
            (Self::EnUs, FocusPanel(PanelDirection::Up)) => "Focus panel up",
            (Self::ZhCn, FocusPanel(PanelDirection::Up)) => "向上聚焦面板",
            (Self::EnUs, FocusPanel(PanelDirection::Right)) => "Focus panel right",
            (Self::ZhCn, FocusPanel(PanelDirection::Right)) => "向右聚焦面板",
            (Self::EnUs, ResizePanel(PanelDirection::Left)) => "Resize panel left",
            (Self::ZhCn, ResizePanel(PanelDirection::Left)) => "向左调整面板大小",
            (Self::EnUs, ResizePanel(PanelDirection::Down)) => "Resize panel down",
            (Self::ZhCn, ResizePanel(PanelDirection::Down)) => "向下调整面板大小",
            (Self::EnUs, ResizePanel(PanelDirection::Up)) => "Resize panel up",
            (Self::ZhCn, ResizePanel(PanelDirection::Up)) => "向上调整面板大小",
            (Self::EnUs, ResizePanel(PanelDirection::Right)) => "Resize panel right",
            (Self::ZhCn, ResizePanel(PanelDirection::Right)) => "向右调整面板大小",
            (Self::EnUs, NextAgent) => "Select next Agent",
            (Self::ZhCn, NextAgent) => "选择下一个智能体",
            (Self::EnUs, PreviousAgent) => "Select previous Agent",
            (Self::ZhCn, PreviousAgent) => "选择上一个智能体",
            (Self::EnUs, FocusSidebar) => "Focus sidebar",
            (Self::ZhCn, FocusSidebar) => "聚焦边栏",
            (Self::EnUs, ConfiguredCommand) => "Run configured command",
            (Self::ZhCn, ConfiguredCommand) => "运行已配置的命令",
            (Self::EnUs, Key) => "Shortcut",
            (Self::ZhCn, Key) => "快捷键",
        }
    }

    /// A context menu's frame title already names the object it opened on, so a row acting on that
    /// object alone keeps only its verb. Rows naming another object or a counted set keep their
    /// full labels, as do the global menu, dialog titles and shortcut help.
    pub(crate) fn menu_object_action(
        self,
        action: impl Into<MenuLabel>,
        scope: Option<TabMenuScope>,
    ) -> String {
        use MenuLabel::*;
        let action = action.into();
        let own_object = matches!(
            scope,
            None | Some(TabMenuScope::Addressed | TabMenuScope::Group(_))
        );
        let verb = match (self, action) {
            _ if !own_object => None,
            (
                Self::EnUs,
                RenameTab | RenamePanel | RenameWorkspace | RenameProject | RenameGroup,
            ) => Some("Rename"),
            (
                Self::ZhCn,
                RenameTab | RenamePanel | RenameWorkspace | RenameProject | RenameGroup,
            ) => Some("重命名"),
            (
                Self::EnUs,
                CloseSelectedTabs | ClosePane | CloseGroup | CloseWorkspace | DeleteWorkspace,
            ) => Some("Close"),
            (
                Self::ZhCn,
                CloseSelectedTabs | ClosePane | CloseGroup | CloseWorkspace | DeleteWorkspace,
            ) => Some("关闭"),
            (Self::EnUs, DeleteProject) => Some("Ungroup"),
            (Self::ZhCn, DeleteProject) => Some("取消分组"),
            (Self::EnUs, PinTab) => Some("Pin"),
            (Self::ZhCn, PinTab) => Some("固定"),
            (Self::EnUs, UnpinTab) => Some("Unpin"),
            (Self::ZhCn, UnpinTab) => Some("取消固定"),
            (Self::EnUs, ManageGroups) => Some("Add to group"),
            (Self::ZhCn, ManageGroups) => Some("加入分组"),
            (Self::EnUs, RemoveFromGroup) => Some("Remove from group"),
            (Self::ZhCn, RemoveFromGroup) => Some("移出分组"),
            (Self::EnUs, Zoom) => Some("Maximize / Restore"),
            (Self::ZhCn, Zoom) => Some("最大化 / 还原"),
            _ => None,
        };
        match (verb, scope) {
            (Some(verb), _) => verb.to_owned(),
            (None, Some(scope)) => self.menu_tab_action(action, scope),
            (None, None) => self.menu_action(action).to_owned(),
        }
    }
}
