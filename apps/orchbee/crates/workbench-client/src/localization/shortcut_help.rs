//! Copy-only shortcut labels; discovery, filtering and identity belong to tui_shortcut_help.

use super::{ActionSearchAlias, Locale, PanelDirection};

/// Stable help category selected by discovery and named by each locale.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ShortcutHelpGroup {
    Common,
    Workspace,
    Tab,
    Panel,
    Agent,
    Client,
    Other,
}

/// Semantic titles selected by shortcut discovery before text rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShortcutLabel {
    Settings,
    ReloadSettings,
    ActionMenu,
    ResizePanel,
    CopyText,
    ConfiguredCommand,
    CloseWorkspace,
    WorkspaceAtDirectory,
    WorkspaceDirectory,
    NewWorktree,
    OpenWorktree,
    RenameWorkspace,
    RenameProject,
    RenameTab,
    NewTab,
    CloseTab,
    MoveTabLeft,
    MoveTabRight,
    NextTab,
    PreviousTab,
    PreviousWorkspace,
    NextWorkspace,
    PreviousTerminal,
    NextTerminal,
    LiteralPrefix,
    CancelPrefix,
    PointerCapture,
    Exit,
    ControlTakeover,
    Machines,
    PreviousAgent,
    NextAgent,
    ZoomPanel,
    ClosePanel,
    Sidebar,
    Navigator,
    AgentBoard,
    StartAgent,
    Help,
    TabNumber,
    AgentNumber,
    Other,
    FocusPanel(PanelDirection),
    SplitPanel(PanelDirection),
    MovePanel(PanelDirection),
}

/// Active feature scope used only for localized search aliases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShortcutContext {
    Sidebar,
    Copy,
    Resize,
}

impl Locale {
    pub(crate) fn sidebar_marker_label(self, marker: &str) -> &'static str {
        match (self, marker) {
            (Self::EnUs, "unread") => "Unread activity",
            (Self::ZhCn, "unread") => "未读动态",
            (Self::EnUs, "expanded") => "Expanded",
            (Self::ZhCn, "expanded") => "已展开",
            (Self::EnUs, "collapsed") => "Collapsed",
            (Self::ZhCn, "collapsed") => "已折叠",
            (Self::EnUs, _) => "Sidebar symbols",
            (Self::ZhCn, _) => "边栏符号",
        }
    }
    pub(crate) const fn shortcut_group_name(self, group: ShortcutHelpGroup) -> &'static str {
        match (self, group) {
            (Self::EnUs, ShortcutHelpGroup::Common) => "Common tasks",
            (Self::ZhCn, ShortcutHelpGroup::Common) => "常用操作",
            (Self::EnUs, ShortcutHelpGroup::Workspace) => "Workspace",
            (Self::ZhCn, ShortcutHelpGroup::Workspace) => "工作区",
            (Self::EnUs, ShortcutHelpGroup::Tab) => "Tab",
            (Self::ZhCn, ShortcutHelpGroup::Tab) => "标签页",
            (Self::EnUs, ShortcutHelpGroup::Panel) => "Panel",
            (Self::ZhCn, ShortcutHelpGroup::Panel) => "面板",
            (Self::EnUs, ShortcutHelpGroup::Agent) => "Agent",
            (Self::ZhCn, ShortcutHelpGroup::Agent) => "智能体",
            (Self::EnUs, ShortcutHelpGroup::Client) => "Client",
            (Self::ZhCn, ShortcutHelpGroup::Client) => "客户端",
            (Self::EnUs, ShortcutHelpGroup::Other) => "Other",
            (Self::ZhCn, ShortcutHelpGroup::Other) => "其他",
        }
    }

    pub(crate) const fn shortcut_scope_label(self, expanded: bool) -> &'static str {
        match (self, expanded) {
            (Self::EnUs, false) => "[Tab: All + symbols]",
            (Self::ZhCn, false) => "[标签页: 全部及符号]",
            (Self::EnUs, true) => "[Tab: Common]",
            (Self::ZhCn, true) => "[标签页: 常用]",
        }
    }
    pub(crate) const fn shortcut_buttons_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Buttons: </> page · x close",
            Self::ZhCn => "按钮：</> 翻页 · x 关闭",
        }
    }
    pub(crate) const fn shortcut_search_label(self) -> &'static str {
        match self {
            Self::EnUs => "Filter",
            Self::ZhCn => "筛选",
        }
    }

    pub(crate) const fn shortcut_close_action(self) -> &'static str {
        match self {
            Self::EnUs => "close",
            Self::ZhCn => "关闭",
        }
    }

    pub(crate) const fn shortcut_scroll_action(self) -> &'static str {
        match self {
            Self::EnUs => "scroll",
            Self::ZhCn => "滚动",
        }
    }

    pub(crate) const fn shortcut_page_action(self) -> &'static str {
        match self {
            Self::EnUs => "page",
            Self::ZhCn => "翻屏",
        }
    }

    pub(crate) const fn shortcut_erase_action(self) -> &'static str {
        match self {
            Self::EnUs => "erase",
            Self::ZhCn => "删除",
        }
    }

    pub(crate) const fn shortcut_clear_action(self) -> &'static str {
        match self {
            Self::EnUs => "clear",
            Self::ZhCn => "清空",
        }
    }

    pub(crate) const fn shortcut_no_matches(self) -> &'static str {
        match self {
            Self::EnUs => "No matching shortcuts",
            Self::ZhCn => "没有匹配的快捷键",
        }
    }

    pub(crate) const fn shortcut_requires_session(self) -> &'static str {
        match self {
            Self::EnUs => "Connect to an Orchbee session",
            Self::ZhCn => "请先连接 Orchbee 会话",
        }
    }

    pub(crate) const fn shortcut_close_label(self) -> &'static str {
        match self {
            Self::EnUs => "Close help",
            Self::ZhCn => "关闭帮助",
        }
    }

    pub(crate) const fn shortcut_scroll_label(self) -> &'static str {
        match self {
            Self::EnUs => "Scroll Help",
            Self::ZhCn => "滚动帮助",
        }
    }

    pub(crate) const fn shortcut_context(self, context: ShortcutContext) -> &'static str {
        match (self, context) {
            (Self::EnUs, ShortcutContext::Sidebar) => "sidebar navigation",
            (Self::ZhCn, ShortcutContext::Sidebar) => "边栏导航",
            (Self::EnUs, ShortcutContext::Copy) => "copy mode",
            (Self::ZhCn, ShortcutContext::Copy) => "复制模式",
            (Self::EnUs, ShortcutContext::Resize) => "resize mode",
            (Self::ZhCn, ShortcutContext::Resize) => "调整大小模式",
        }
    }

    pub(crate) fn shortcut_label(self, label: ShortcutLabel) -> String {
        match label {
            ShortcutLabel::FocusPanel(direction) => {
                self.panel_direction_title("Focus panel", "聚焦面板", direction)
            }
            ShortcutLabel::SplitPanel(direction) => {
                self.panel_direction_title("Split panel", "分割面板", direction)
            }
            ShortcutLabel::MovePanel(direction) => {
                self.panel_direction_title("Move panel", "移动面板", direction)
            }
            ShortcutLabel::Settings => match self {
                Self::EnUs => "Settings",
                Self::ZhCn => "设置",
            }
            .to_owned(),
            ShortcutLabel::ReloadSettings => match self {
                Self::EnUs => "Reload settings",
                Self::ZhCn => "重新加载设置",
            }
            .to_owned(),
            ShortcutLabel::ActionMenu => match self {
                Self::EnUs => "Open action menu",
                Self::ZhCn => "打开操作菜单",
            }
            .to_owned(),
            ShortcutLabel::ResizePanel => match self {
                Self::EnUs => "Resize panel",
                Self::ZhCn => "调整面板大小",
            }
            .to_owned(),
            ShortcutLabel::CopyText => match self {
                Self::EnUs => "Copy text",
                Self::ZhCn => "复制文本",
            }
            .to_owned(),
            ShortcutLabel::ConfiguredCommand => match self {
                Self::EnUs => "Run configured command",
                Self::ZhCn => "运行已配置的命令",
            }
            .to_owned(),
            ShortcutLabel::CloseWorkspace => match self {
                Self::EnUs => "Close Workspace",
                Self::ZhCn => "关闭工作区",
            }
            .to_owned(),
            ShortcutLabel::WorkspaceAtDirectory => match self {
                Self::EnUs => "New Workspace in directory",
                Self::ZhCn => "在指定目录新建工作区",
            }
            .to_owned(),
            ShortcutLabel::WorkspaceDirectory => match self {
                Self::EnUs => "Set Workspace directory",
                Self::ZhCn => "设置工作区目录",
            }
            .to_owned(),
            ShortcutLabel::NewWorktree => match self {
                Self::EnUs => "New Worktree",
                Self::ZhCn => "新建工作树",
            }
            .to_owned(),
            ShortcutLabel::OpenWorktree => match self {
                Self::EnUs => "Open Worktree",
                Self::ZhCn => "打开工作树",
            }
            .to_owned(),
            ShortcutLabel::RenameWorkspace => match self {
                Self::EnUs => "Rename Workspace",
                Self::ZhCn => "重命名工作区",
            }
            .to_owned(),
            ShortcutLabel::RenameProject => match self {
                Self::EnUs => "Rename Project",
                Self::ZhCn => "重命名项目",
            }
            .to_owned(),
            ShortcutLabel::RenameTab => match self {
                Self::EnUs => "Rename Tab",
                Self::ZhCn => "重命名标签页",
            }
            .to_owned(),
            ShortcutLabel::NewTab => match self {
                Self::EnUs => "New Tab",
                Self::ZhCn => "新建标签页",
            }
            .to_owned(),
            ShortcutLabel::CloseTab => match self {
                Self::EnUs => "Close Tab",
                Self::ZhCn => "关闭标签页",
            }
            .to_owned(),
            ShortcutLabel::MoveTabLeft => match self {
                Self::EnUs => "Move Tab left",
                Self::ZhCn => "向左移动标签页",
            }
            .to_owned(),
            ShortcutLabel::MoveTabRight => match self {
                Self::EnUs => "Move Tab right",
                Self::ZhCn => "向右移动标签页",
            }
            .to_owned(),
            ShortcutLabel::NextTab => match self {
                Self::EnUs => "Select next Tab",
                Self::ZhCn => "选择下一个标签页",
            }
            .to_owned(),
            ShortcutLabel::PreviousTab => match self {
                Self::EnUs => "Select previous Tab",
                Self::ZhCn => "选择上一个标签页",
            }
            .to_owned(),
            ShortcutLabel::PreviousWorkspace => match self {
                Self::EnUs => "Select previous Workspace",
                Self::ZhCn => "选择上一个工作区",
            }
            .to_owned(),
            ShortcutLabel::NextWorkspace => match self {
                Self::EnUs => "Select next Workspace",
                Self::ZhCn => "选择下一个工作区",
            }
            .to_owned(),
            ShortcutLabel::PreviousTerminal => match self {
                Self::EnUs => "Select previous terminal in panel",
                Self::ZhCn => "选择面板内上一个终端",
            }
            .to_owned(),
            ShortcutLabel::NextTerminal => match self {
                Self::EnUs => "Select next terminal in panel",
                Self::ZhCn => "选择面板内下一个终端",
            }
            .to_owned(),
            ShortcutLabel::LiteralPrefix => match self {
                Self::EnUs => "Send prefix to terminal",
                Self::ZhCn => "向终端发送前缀",
            }
            .to_owned(),
            ShortcutLabel::CancelPrefix => match self {
                Self::EnUs => "Cancel prefix",
                Self::ZhCn => "取消前缀",
            }
            .to_owned(),
            ShortcutLabel::PointerCapture => match self {
                Self::EnUs => "Toggle mouse capture",
                Self::ZhCn => "切换鼠标捕获",
            }
            .to_owned(),
            ShortcutLabel::Exit => match self {
                Self::EnUs => "Leave view (keep terminals running)",
                Self::ZhCn => "离开界面（终端继续运行）",
            }
            .to_owned(),
            ShortcutLabel::ControlTakeover => match self {
                Self::EnUs => "Enable typing in this panel",
                Self::ZhCn => "获取此面板的输入权限",
            }
            .to_owned(),
            ShortcutLabel::Machines => match self {
                Self::EnUs => "Browse machines, Workspaces and Agents",
                Self::ZhCn => "浏览机器、工作区和智能体",
            }
            .to_owned(),
            ShortcutLabel::PreviousAgent => match self {
                Self::EnUs => "Select previous Agent",
                Self::ZhCn => "选择上一个智能体",
            }
            .to_owned(),
            ShortcutLabel::NextAgent => match self {
                Self::EnUs => "Select next Agent",
                Self::ZhCn => "选择下一个智能体",
            }
            .to_owned(),
            ShortcutLabel::ZoomPanel => match self {
                Self::EnUs => "Toggle panel maximization",
                Self::ZhCn => "切换面板最大化",
            }
            .to_owned(),
            ShortcutLabel::ClosePanel => match self {
                Self::EnUs => "Close panel",
                Self::ZhCn => "关闭面板",
            }
            .to_owned(),
            ShortcutLabel::Sidebar => match self {
                Self::EnUs => "Toggle sidebar",
                Self::ZhCn => "切换边栏",
            }
            .to_owned(),
            ShortcutLabel::Navigator => match self {
                Self::EnUs => "Workspace navigator",
                Self::ZhCn => "工作区导航器",
            }
            .to_owned(),
            ShortcutLabel::AgentBoard => match self {
                Self::EnUs => "Agent board",
                Self::ZhCn => "智能体看板",
            }
            .to_owned(),
            ShortcutLabel::StartAgent => match self {
                Self::EnUs => "Start Agent",
                Self::ZhCn => "启动智能体",
            }
            .to_owned(),
            ShortcutLabel::Help => match self {
                Self::EnUs => "Shortcut help",
                Self::ZhCn => "快捷键帮助",
            }
            .to_owned(),
            ShortcutLabel::TabNumber => match self {
                Self::EnUs => "Select Tab by number",
                Self::ZhCn => "按编号选择标签页",
            }
            .to_owned(),
            ShortcutLabel::AgentNumber => match self {
                Self::EnUs => "Select Agent by number",
                Self::ZhCn => "按编号选择智能体",
            }
            .to_owned(),
            ShortcutLabel::Other => match self {
                Self::EnUs => "Shortcut",
                Self::ZhCn => "快捷键",
            }
            .to_owned(),
        }
    }

    fn panel_direction_title(
        self,
        english: &'static str,
        chinese: &'static str,
        direction: PanelDirection,
    ) -> String {
        let (verb, direction) = match (self, direction) {
            (Self::EnUs, PanelDirection::Left) => (english, "left"),
            (Self::EnUs, PanelDirection::Down) => (english, "down"),
            (Self::EnUs, PanelDirection::Up) => (english, "up"),
            (Self::EnUs, PanelDirection::Right) => (english, "right"),
            (Self::ZhCn, PanelDirection::Left) => (chinese, "向左"),
            (Self::ZhCn, PanelDirection::Down) => (chinese, "向下"),
            (Self::ZhCn, PanelDirection::Up) => (chinese, "向上"),
            (Self::ZhCn, PanelDirection::Right) => (chinese, "向右"),
        };
        match self {
            Self::EnUs => format!("{verb} {direction}"),
            Self::ZhCn => format!("{direction}{verb}"),
        }
    }

    pub(crate) fn shortcut_search_alias(self, alias: ActionSearchAlias) -> &'static str {
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
            (Self::EnUs, A::Pane) => "pane panel",
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
            (Self::EnUs, A::Navigator) => "navigate",
            (Self::ZhCn, A::Navigator) => "导航",
            (Self::EnUs, A::Agent) => "agent",
            (Self::ZhCn, A::Agent) => "智能体",
            (Self::EnUs, A::Help) => "help",
            (Self::ZhCn, A::Help) => "帮助",
            (Self::EnUs, A::Mouse) => "mouse pointer",
            (Self::ZhCn, A::Mouse) => "鼠标指针",
            // Leaving is also what people call quitting or exiting; the title stays Leave view.
            (Self::EnUs, A::Detach) => "detach quit exit",
            (Self::ZhCn, A::Detach) => "分离 退出 关闭",
            (Self::EnUs, A::History) => "history restore",
            (Self::ZhCn, A::History) => "历史 恢复",
        }
    }
}
