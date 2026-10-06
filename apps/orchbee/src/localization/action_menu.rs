use super::Locale;
use crate::action_menu::MenuAction;

impl Locale {
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
            (Self::EnUs, CloseWorkspace) => "Close Workspace…",
            (Self::ZhCn, CloseWorkspace) => "关闭工作区…",
            (Self::EnUs, NewWorkspace) => "New Workspace…",
            (Self::ZhCn, NewWorkspace) => "新建工作区…",
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
