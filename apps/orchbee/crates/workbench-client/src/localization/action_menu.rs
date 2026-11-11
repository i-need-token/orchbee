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
    History,
}
/// Textual action identity without an executable target, profile or captured command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuLabel {
    SplitPanel(PanelDirection),
    Zoom,
    Resize,
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
    NewTabInRecentGroup,
    NewGroupWithTab,
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
    CollapseGroup,
    RenameTab,
    RenamePanel,
    ClearTerminal,
    EditScrollback,
    LastPanel,
    EarlierLayouts,
    ReopenEarlierLayout,
    ResumeTabClose,
    CloseSelectedTabs,
    CloseOtherTabs,
    CloseTrailingTabs,
    CloseGroup,
    CloseTab,
    QuickWorkspace,
    WorkspaceAtDirectory,
    WorkspaceDirectory,
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
    Reload,
    Settings,
    Machines,
    Help,
    Sidebar,
    Navigator,
    AgentBoard,
    OpenNotificationTarget,
    ReleasePointer,
    Detach,
}

/// A labeled group inside a submenu; never a candidate and never an action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuHeading {
    Folders,
    Worktrees,
}

/// Secondary text that ends a row, such as the default launch profile's mark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuRowNote {
    DefaultProfile,
}

/// Why the action owner has withheld a menu action; never inferred from translated text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuUnavailable {
    DirectoryConfigured,
    EmptyWorkspaceRequired,
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
    NoRecentGroup,
    WorktreeChecking,
    WorktreeUnavailable,
    NotGitRepository,
    NoOtherWorktrees,
    NoSelection,
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
            (Self::EnUs, A::Pane) => "pane",
            (Self::ZhCn, A::Pane) => "面板",
            (Self::EnUs, A::Split) => "split",
            (Self::ZhCn, A::Split) => "分屏",
            (Self::EnUs, A::Resize) => "resize",
            (Self::ZhCn, A::Resize) => "调整大小",
            (Self::EnUs, A::Copy) => "copy",
            (Self::ZhCn, A::Copy) => "复制",
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
            (Self::EnUs, A::History) => "history restore",
            (Self::ZhCn, A::History) => "历史 恢复",
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
    pub(crate) const fn menu_unavailable(
        self,
        reason: MenuUnavailable,
        compact: bool,
    ) -> &'static str {
        match (self, reason, compact) {
            (Self::EnUs, MenuUnavailable::WorktreeChecking, false) => "Checking Git source…",
            (Self::ZhCn, MenuUnavailable::WorktreeChecking, false) => "正在识别 Git 来源…",
            (Self::EnUs, MenuUnavailable::WorktreeChecking, true) => "Checking Git…",
            (Self::ZhCn, MenuUnavailable::WorktreeChecking, true) => "识别 Git 中",
            (Self::EnUs, MenuUnavailable::WorktreeUnavailable, false) => "No available Git source",
            (Self::ZhCn, MenuUnavailable::WorktreeUnavailable, false) => "暂无可用的 Git 来源",
            (Self::EnUs, MenuUnavailable::WorktreeUnavailable, true) => "Git unavailable",
            (Self::ZhCn, MenuUnavailable::WorktreeUnavailable, true) => "Git 不可用",
            (Self::EnUs, MenuUnavailable::NotGitRepository, false) => "Not in a Git repository",
            (Self::ZhCn, MenuUnavailable::NotGitRepository, false) => "不在 Git 仓库中",
            (Self::EnUs, MenuUnavailable::NotGitRepository, true) => "Not in Git",
            (Self::ZhCn, MenuUnavailable::NotGitRepository, true) => "非 Git",
            (Self::EnUs, MenuUnavailable::NoOtherWorktrees, false) => "No other worktrees",
            (Self::ZhCn, MenuUnavailable::NoOtherWorktrees, false) => "没有其他工作树",
            (Self::EnUs, MenuUnavailable::NoOtherWorktrees, true) => "No others",
            (Self::ZhCn, MenuUnavailable::NoOtherWorktrees, true) => "无其他",
            (Self::EnUs, MenuUnavailable::NoRecentGroup, _) => "No recent group",
            (Self::ZhCn, MenuUnavailable::NoRecentGroup, _) => "暂无最近使用的分组",
            (Self::EnUs, MenuUnavailable::NoMatchingTabs, false) => {
                "No Tabs match this close action."
            }
            (Self::ZhCn, MenuUnavailable::NoMatchingTabs, false) => "没有符合此关闭操作的标签页。",
            (Self::EnUs, MenuUnavailable::NoMatchingTabs, true) => "No Tabs",
            (Self::ZhCn, MenuUnavailable::NoMatchingTabs, true) => "无匹配标签页",
            (Self::EnUs, MenuUnavailable::DirectoryConfigured, false) => {
                "This Workspace already has a directory."
            }
            (Self::ZhCn, MenuUnavailable::DirectoryConfigured, false) => "此工作区已配置目录。",
            (Self::EnUs, MenuUnavailable::DirectoryConfigured, true) => "Dir set",
            (Self::ZhCn, MenuUnavailable::DirectoryConfigured, true) => "目录已设置",
            (Self::EnUs, MenuUnavailable::EmptyWorkspaceRequired, false) => {
                "Only an empty Workspace can set a directory."
            }
            (Self::ZhCn, MenuUnavailable::EmptyWorkspaceRequired, false) => {
                "只能为空工作区配置目录。"
            }
            (Self::EnUs, MenuUnavailable::EmptyWorkspaceRequired, true) => "Needs empty",
            (Self::ZhCn, MenuUnavailable::EmptyWorkspaceRequired, true) => "需空工作区",
            (Self::EnUs, MenuUnavailable::WorkspaceRequired, false) => {
                "Select or create a workspace before using this action."
            }
            (Self::ZhCn, MenuUnavailable::WorkspaceRequired, false) => {
                "请先选择或新建工作区，再执行此操作。"
            }
            (Self::EnUs, MenuUnavailable::WorkspaceRequired, true) => "Workspace",
            (Self::ZhCn, MenuUnavailable::WorkspaceRequired, true) => "需工作区",
            (Self::EnUs, MenuUnavailable::FocusedPanelRequired, false) => {
                "A focused panel is required for this action."
            }
            (Self::ZhCn, MenuUnavailable::FocusedPanelRequired, false) => "此操作需要焦点面板。",
            (Self::EnUs, MenuUnavailable::FocusedPanelRequired, true) => "Focus panel",
            (Self::ZhCn, MenuUnavailable::FocusedPanelRequired, true) => "需焦点面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Up), false) => "No Panel above",
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Up), false) => "上方没有面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Up), true) => "None above",
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Up), true) => "上方无面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Down), false) => "No Panel below",
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Down), false) => "下方没有面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Down), true) => "None below",
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Down), true) => "下方无面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Left), false) => {
                "No Panel to the left"
            }
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Left), false) => "左侧没有面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Left), true) => "None to left",
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Left), true) => "左侧无面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Right), false) => {
                "No Panel to the right"
            }
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Right), false) => "右侧没有面板",
            (Self::EnUs, MenuUnavailable::NoPanel(PanelDirection::Right), true) => "None to right",
            (Self::ZhCn, MenuUnavailable::NoPanel(PanelDirection::Right), true) => "右侧无面板",
            (Self::EnUs, MenuUnavailable::PanelMaximized, false) => {
                "Unavailable while a Panel is maximized"
            }
            (Self::ZhCn, MenuUnavailable::PanelMaximized, false) => "面板最大化时不可用",
            (Self::EnUs, MenuUnavailable::PanelMaximized, true) => "Maximized",
            (Self::ZhCn, MenuUnavailable::PanelMaximized, true) => "已最大化",
            (Self::EnUs, MenuUnavailable::MultiplePanelsRequired, false) => {
                "Needs at least two Panels"
            }
            (Self::ZhCn, MenuUnavailable::MultiplePanelsRequired, false) => "至少需要两个面板",
            (Self::EnUs, MenuUnavailable::MultiplePanelsRequired, true) => "Needs two Panels",
            (Self::ZhCn, MenuUnavailable::MultiplePanelsRequired, true) => "需两个面板",
            (Self::EnUs, MenuUnavailable::NoEarlierPanel, false) => "No earlier Panel to return to",
            (Self::ZhCn, MenuUnavailable::NoEarlierPanel, false) => "没有可返回的面板",
            (Self::EnUs, MenuUnavailable::NoEarlierPanel, true) => "None earlier",
            (Self::ZhCn, MenuUnavailable::NoEarlierPanel, true) => "无可返回",
            (Self::EnUs, MenuUnavailable::OnlyOneTab, false) => "Only one Tab",
            (Self::ZhCn, MenuUnavailable::OnlyOneTab, false) => "只有一个标签页",
            (Self::EnUs, MenuUnavailable::OnlyOneTab, true) => "One Tab",
            (Self::ZhCn, MenuUnavailable::OnlyOneTab, true) => "仅一个标签页",
            (Self::EnUs, MenuUnavailable::NoOtherPinnedTab, false) => "No other pinned Tab",
            (Self::ZhCn, MenuUnavailable::NoOtherPinnedTab, false) => "没有其他固定标签页",
            (Self::EnUs, MenuUnavailable::NoOtherPinnedTab, true) => "None pinned",
            (Self::ZhCn, MenuUnavailable::NoOtherPinnedTab, true) => "无其他固定",
            (Self::EnUs, MenuUnavailable::NoOtherUnpinnedTab, false) => "No other unpinned Tab",
            (Self::ZhCn, MenuUnavailable::NoOtherUnpinnedTab, false) => "没有其他未固定的标签页",
            (Self::EnUs, MenuUnavailable::NoOtherUnpinnedTab, true) => "None unpinned",
            (Self::ZhCn, MenuUnavailable::NoOtherUnpinnedTab, true) => "无其他未固定",
            (Self::EnUs, MenuUnavailable::TabAlreadyFirst, false) => "Already first",
            (Self::ZhCn, MenuUnavailable::TabAlreadyFirst, false) => "已在最前",
            (Self::EnUs, MenuUnavailable::TabAlreadyFirst, true) => "First",
            (Self::ZhCn, MenuUnavailable::TabAlreadyFirst, true) => "已最前",
            (Self::EnUs, MenuUnavailable::TabAlreadyLast, false) => "Already last",
            (Self::ZhCn, MenuUnavailable::TabAlreadyLast, false) => "已在最后",
            (Self::EnUs, MenuUnavailable::TabAlreadyLast, true) => "Last",
            (Self::ZhCn, MenuUnavailable::TabAlreadyLast, true) => "已最后",
            (Self::EnUs, MenuUnavailable::TerminalDisconnected, false) => "Terminal disconnected",
            (Self::ZhCn, MenuUnavailable::TerminalDisconnected, false) => "终端已断连",
            (Self::EnUs, MenuUnavailable::TerminalDisconnected, true) => "Disconnected",
            (Self::ZhCn, MenuUnavailable::TerminalDisconnected, true) => "已断连",
            (Self::EnUs, MenuUnavailable::ProjectWorkspaceRequired, false) => {
                "Select a Workspace in this Project first."
            }
            (Self::ZhCn, MenuUnavailable::ProjectWorkspaceRequired, false) => {
                "请先选择此项目中的工作区。"
            }
            (Self::EnUs, MenuUnavailable::ProjectWorkspaceRequired, true) => "Workspace",
            (Self::ZhCn, MenuUnavailable::ProjectWorkspaceRequired, true) => "需工作区",
            (Self::EnUs, MenuUnavailable::OperationBusy, false) => {
                "Another operation is in progress."
            }
            (Self::ZhCn, MenuUnavailable::OperationBusy, false) => "另一项操作正在进行。",
            (Self::EnUs, MenuUnavailable::OperationBusy, true) => "Busy",
            (Self::ZhCn, MenuUnavailable::OperationBusy, true) => "操作进行中",
            (Self::EnUs, MenuUnavailable::PointerReleased, false) => {
                "Mouse capture is already released."
            }
            (Self::ZhCn, MenuUnavailable::PointerReleased, false) => "鼠标捕获已释放。",
            (Self::EnUs, MenuUnavailable::PointerReleased, true) => "Released",
            (Self::ZhCn, MenuUnavailable::PointerReleased, true) => "已释放",
            (Self::EnUs, MenuUnavailable::NoSelection, false) => "Select text in this panel first.",
            (Self::ZhCn, MenuUnavailable::NoSelection, false) => "请先在此面板中选中文本。",
            (Self::EnUs, MenuUnavailable::NoSelection, true) => "No selection",
            (Self::ZhCn, MenuUnavailable::NoSelection, true) => "无选区",
        }
    }
    pub(crate) const fn menu_heading(self, heading: MenuHeading) -> &'static str {
        match (self, heading) {
            (Self::EnUs, MenuHeading::Folders) => "Folders",
            (Self::ZhCn, MenuHeading::Folders) => "文件夹",
            (Self::EnUs, MenuHeading::Worktrees) => "Git Worktrees",
            (Self::ZhCn, MenuHeading::Worktrees) => "Git 工作树",
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
            (Self::EnUs, NewTabInRecentGroup) => "New Tab in recent group",
            (Self::ZhCn, NewTabInRecentGroup) => "在最近的分组中新建标签页",
            (Self::EnUs, ResumeTabClose) => "Resume closing this Tab…",
            (Self::ZhCn, ResumeTabClose) => "继续关闭此标签页…",
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
            (Self::EnUs, NewGroupWithTab) => "New group with Tab",
            (Self::ZhCn, NewGroupWithTab) => "新建分组及标签页",
            (Self::EnUs, DuplicateTab) => "Duplicate Tab",
            (Self::ZhCn, DuplicateTab) => "复制标签页",
            (Self::EnUs, SplitPanel(PanelDirection::Left)) => "Split left",
            (Self::ZhCn, SplitPanel(PanelDirection::Left)) => "向左分屏",
            (Self::EnUs, SplitPanel(PanelDirection::Down)) => "Split down",
            (Self::ZhCn, SplitPanel(PanelDirection::Down)) => "向下分屏",
            (Self::EnUs, SplitPanel(PanelDirection::Up)) => "Split up",
            (Self::ZhCn, SplitPanel(PanelDirection::Up)) => "向上分屏",
            (Self::EnUs, SplitPanel(PanelDirection::Right)) => "Split right",
            (Self::ZhCn, SplitPanel(PanelDirection::Right)) => "向右分屏",
            (Self::EnUs, Zoom) => "Toggle panel maximization",
            (Self::ZhCn, Zoom) => "切换面板最大化",
            (Self::EnUs, Resize) => "Resize panes",
            (Self::ZhCn, Resize) => "调整面板大小",
            (Self::EnUs, Copy) => "Keyboard copy mode",
            (Self::ZhCn, Copy) => "键盘复制模式",
            (Self::EnUs, CopySelection) => "Copy selection",
            (Self::ZhCn, CopySelection) => "复制所选内容",
            (Self::EnUs, Takeover) => "Take control",
            (Self::ZhCn, Takeover) => "接管输入与尺寸",
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
            (Self::EnUs, ClearTerminal) => "Clear terminal",
            (Self::ZhCn, ClearTerminal) => "清除终端",
            (Self::EnUs, EditScrollback) => "Edit scrollback",
            (Self::ZhCn, EditScrollback) => "编辑回滚缓冲区",
            (Self::EnUs, LastPanel) => "Back to last focused panel",
            (Self::ZhCn, LastPanel) => "回到上次聚焦的面板",
            (Self::EnUs, EarlierLayouts) => "Earlier layouts…",
            (Self::ZhCn, EarlierLayouts) => "早先的布局…",
            (Self::EnUs, ReopenEarlierLayout) => "Reopen",
            (Self::ZhCn, ReopenEarlierLayout) => "重新打开",
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
            (Self::ZhCn, Ungroup) => "解散分组，保留标签页",
            (Self::EnUs, CollapseGroup) => "Collapse group",
            (Self::ZhCn, CollapseGroup) => "折叠分组",
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
            (Self::EnUs, DeleteProject) => "Dissolve Project",
            (Self::ZhCn, DeleteProject) => "解散项目",
            (Self::EnUs, CloseProjectWorkspaces) => "Close all Workspaces…",
            (Self::ZhCn, CloseProjectWorkspaces) => "关闭项目内全部工作区…",
            (Self::EnUs, WorkspaceDirectory) => "Set Directory…",
            (Self::ZhCn, WorkspaceDirectory) => "设置目录…",
            (Self::EnUs, CloseWorkspace) => "Close Workspace…",
            (Self::ZhCn, CloseWorkspace) => "关闭工作区…",
            (Self::EnUs, NewWorkspace) => "New Workspace",
            (Self::ZhCn, NewWorkspace) => "新建工作区",
            (Self::EnUs, NewWorktree) => "New Worktree…",
            (Self::ZhCn, NewWorktree) => "新建工作树…",
            (Self::EnUs, OpenWorktree) => "Open Worktree…",
            (Self::ZhCn, OpenWorktree) => "打开工作树…",
            (Self::EnUs, Reload) => "Reload configuration",
            (Self::ZhCn, Reload) => "重载配置",
            (Self::EnUs, Settings) => "Settings…",
            (Self::ZhCn, Settings) => "设置…",
            (Self::EnUs, Machines) => "Browse machines, Workspaces and Agents",
            (Self::ZhCn, Machines) => "浏览机器、工作区和智能体",
            (Self::EnUs, Help) => "Shortcut help",
            (Self::ZhCn, Help) => "快捷键帮助",
            (Self::EnUs, Sidebar) => "Toggle sidebar",
            (Self::ZhCn, Sidebar) => "切换边栏",
            (Self::EnUs, Navigator) => "Workspace navigator",
            (Self::ZhCn, Navigator) => "工作区导航器",
            (Self::EnUs, AgentBoard) => "Agent board",
            (Self::ZhCn, AgentBoard) => "智能体看板",
            (Self::EnUs, OpenNotificationTarget) => "Open notification target",
            (Self::ZhCn, OpenNotificationTarget) => "打开通知目标",
            (Self::EnUs, ReleasePointer) => "Release mouse",
            (Self::ZhCn, ReleasePointer) => "释放鼠标",
            (_, Detach) => self.client_detach_label(),
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
            (Self::EnUs, ResumeTabClose) => Some("Resume closing…"),
            (Self::ZhCn, ResumeTabClose) => Some("继续关闭…"),
            (Self::EnUs, DeleteProject) => Some("Dissolve"),
            (Self::ZhCn, DeleteProject) => Some("解散"),
            (Self::EnUs, PinTab) => Some("Pin"),
            (Self::ZhCn, PinTab) => Some("固定"),
            (Self::EnUs, UnpinTab) => Some("Unpin"),
            (Self::ZhCn, UnpinTab) => Some("取消固定"),
            (Self::EnUs, ManageGroups) => Some("Add to group"),
            (Self::ZhCn, ManageGroups) => Some("加入分组"),
            (Self::EnUs, RemoveFromGroup) => Some("Remove from group"),
            (Self::ZhCn, RemoveFromGroup) => Some("移出分组"),
            (Self::EnUs, Zoom) => Some("Maximize"),
            (Self::ZhCn, Zoom) => Some("切换最大化"),
            _ => None,
        };
        match (verb, scope) {
            (Some(verb), _) => verb.to_owned(),
            (None, Some(scope)) => self.menu_tab_action(action, scope),
            (None, None) => self.menu_action(action).to_owned(),
        }
    }
}
