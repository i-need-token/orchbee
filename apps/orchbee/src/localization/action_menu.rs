use super::Locale;
use crate::action_menu::{MenuAction, MenuUnavailable};

impl Locale {
    pub(crate) const fn menu_unavailable(
        self,
        reason: MenuUnavailable,
        compact: bool,
    ) -> &'static str {
        match (self, reason, compact) {
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
                "Select a Workspace to configure its directory."
            }
            (Self::ZhCn, MenuUnavailable::WorkspaceRequired, false) => "选择工作区后配置目录。",
            (Self::EnUs, MenuUnavailable::WorkspaceRequired, true) => "Workspace",
            (Self::ZhCn, MenuUnavailable::WorkspaceRequired, true) => "需工作区",
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
        }
    }
    pub(crate) fn menu_disabled_hint(self, reason: MenuUnavailable, width: usize) -> String {
        let owner = match self {
            Self::EnUs => "Menu",
            Self::ZhCn => "菜单",
        };
        [
            format!("{owner} · {} · Esc", self.menu_unavailable(reason, false)),
            format!("{owner} · {} · Esc", self.menu_unavailable(reason, true)),
            format!("{owner} · Esc"),
            "Esc".to_owned(),
        ]
        .into_iter()
        .find(|hint| crate::terminal_text::display_width(hint) <= width)
        .unwrap_or_default()
    }
    pub(crate) const fn menu_button(self) -> &'static str {
        match self {
            Self::EnUs => "[Menu]",
            Self::ZhCn => "[菜单]",
        }
    }
    pub(crate) const fn menu_title(self) -> &'static str {
        match self {
            Self::EnUs => "Actions",
            Self::ZhCn => "动作菜单",
        }
    }
    pub(crate) const fn menu_hint(self, pending: bool) -> &'static str {
        match (self, pending) {
            (Self::EnUs, false) => "Menu · ↑/↓ select · Enter run · Esc cancel",
            (Self::ZhCn, false) => "菜单 · ↑/↓ 选择 · Enter 执行 · Esc 取消",
            (Self::EnUs, true) => "Menu · selecting target · Esc cancel",
            (Self::ZhCn, true) => "菜单 · 正在定位目标 · Esc 取消",
        }
    }
    pub(crate) fn menu_status_hint(self, pending: bool, width: usize) -> &'static str {
        let full = self.menu_hint(pending);
        let compact = match (self, pending) {
            (Self::EnUs, false) => "Menu · ↑/↓ · Enter · Esc",
            (Self::ZhCn, false) => "菜单 · ↑/↓ · Enter · Esc",
            (Self::EnUs, true) => "Menu · selecting · Esc",
            (Self::ZhCn, true) => "菜单 · 定位中 · Esc",
        };
        let minimal = match self {
            Self::EnUs => "Menu · Esc",
            Self::ZhCn => "菜单 · Esc",
        };
        [full, compact, minimal, "Esc"]
            .into_iter()
            .find(|hint| crate::terminal_text::display_width(hint) <= width)
            .unwrap_or("")
    }
    pub(crate) const fn menu_footer(self, compact: bool) -> &'static str {
        match (self, compact) {
            (Self::EnUs | Self::ZhCn, false) => "Esc · Enter",
            (Self::EnUs | Self::ZhCn, true) => "Esc",
        }
    }
    pub(crate) const fn menu_action(self, action: MenuAction) -> &'static str {
        use MenuAction::*;
        match (self, action) {
            (Self::EnUs, SplitVertical) => "Split side by side",
            (Self::ZhCn, SplitVertical) => "左右分屏",
            (Self::EnUs, SplitHorizontal) => "Split top and bottom",
            (Self::ZhCn, SplitHorizontal) => "上下分屏",
            (Self::EnUs, Zoom) => "Toggle pane zoom",
            (Self::ZhCn, Zoom) => "切换面板缩放",
            (Self::EnUs, Resize) => "Resize panes",
            (Self::ZhCn, Resize) => "调整面板大小",
            (Self::EnUs, Copy) => "Keyboard copy mode",
            (Self::ZhCn, Copy) => "键盘复制模式",
            (Self::EnUs, Takeover) => "Take control",
            (Self::ZhCn, Takeover) => "接管输入与尺寸",
            (Self::EnUs, ClosePane) => "Close pane…",
            (Self::ZhCn, ClosePane) => "关闭面板…",
            (Self::EnUs, NewTab) => "New tab",
            (Self::ZhCn, NewTab) => "新建标签页",
            (Self::EnUs, RenameTab) => "Rename tab…",
            (Self::ZhCn, RenameTab) => "重命名标签页…",
            (Self::EnUs, CloseTab) => "Close tab…",
            (Self::ZhCn, CloseTab) => "关闭标签页…",
            (Self::EnUs, QuickWorkspace) => "Quick new Workspace",
            (Self::ZhCn, QuickWorkspace) => "快速新建工作区",
            (Self::EnUs, WorkspaceDirectory) => "Workspace directory…",
            (Self::ZhCn, WorkspaceDirectory) => "工作区目录…",
            (Self::EnUs, RenameWorkspace) => "Rename Workspace…",
            (Self::ZhCn, RenameWorkspace) => "重命名工作区…",
            (Self::EnUs, RenameProject) => "Rename Project…",
            (Self::ZhCn, RenameProject) => "重命名项目…",
            (Self::EnUs, CloseWorkspace) => "Close Workspace…",
            (Self::ZhCn, CloseWorkspace) => "关闭工作区…",
            (Self::EnUs, NewWorkspace) => "New Workspace (choose Project)…",
            (Self::ZhCn, NewWorkspace) => "新建工作区（选择项目）…",
            (Self::EnUs, NewWorktree) => "New Worktree…",
            (Self::ZhCn, NewWorktree) => "新建 Worktree…",
            (Self::EnUs, OpenWorktree) => "Open Worktree…",
            (Self::ZhCn, OpenWorktree) => "打开 Worktree…",
            (Self::EnUs, Reload) => "Reload configuration",
            (Self::ZhCn, Reload) => "重载配置",
            (Self::EnUs, Settings) => "Settings…",
            (Self::ZhCn, Settings) => "设置…",
            (Self::EnUs, Help) => "Shortcut help",
            (Self::ZhCn, Help) => "快捷键帮助",
            (Self::EnUs, Sidebar) => "Toggle sidebar",
            (Self::ZhCn, Sidebar) => "切换侧栏",
            (Self::EnUs, Navigator) => "Workspace navigator",
            (Self::ZhCn, Navigator) => "工作区导航器",
            (Self::EnUs, AgentBoard) => "Agent board",
            (Self::ZhCn, AgentBoard) => "Agent 看板",
            (Self::EnUs, ReleasePointer) => "Release mouse",
            (Self::ZhCn, ReleasePointer) => "释放鼠标",
            (Self::EnUs, Detach) => "Detach window",
            (Self::ZhCn, Detach) => "分离当前窗口",
        }
    }
}
