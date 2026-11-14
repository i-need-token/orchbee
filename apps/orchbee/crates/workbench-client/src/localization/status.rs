//! Raw status-line copy. Binding owners provide separately formatted effective key labels.

use super::Locale;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StatusText {
    pub(crate) prefix: &'static str,
    pub(crate) pointer_released: &'static str,
    pub(crate) zoomed: &'static str,
    pub(crate) navigator: &'static str,
    pub(crate) sidebar: &'static str,
    pub(crate) focus_sidebar: &'static str,
    pub(crate) directory_missing: &'static str,
    pub(crate) disconnected: &'static str,
    pub(crate) terminal_exited: &'static str,
    pub(crate) resize_pending: &'static str,
    pub(crate) divider: &'static str,
    pub(crate) release_to_apply: &'static str,
    pub(crate) shortcuts: &'static str,
    pub(crate) help: &'static str,
    pub(crate) actions: &'static str,
    pub(crate) exit: &'static str,
    pub(crate) detach: &'static str,
    pub(crate) cancel: &'static str,
    pub(crate) restore: &'static str,
    pub(crate) create_tab: &'static str,
    pub(crate) start_agent: &'static str,
    pub(crate) split: &'static str,
    pub(crate) create_workspace: &'static str,
    pub(crate) refresh: &'static str,
    pub(crate) check_again: &'static str,
    pub(crate) take_control: &'static str,
    pub(crate) retry_rename: &'static str,
    pub(crate) switch_panel: &'static str,
}

const EN_US: StatusText = StatusText {
    prefix: "PREFIX",
    pointer_released: "Mouse released",
    zoomed: "Maximized",
    navigator: "Navigator",
    sidebar: "Sidebar",
    focus_sidebar: "sidebar",
    directory_missing: "Directory missing",
    disconnected: "Disconnected",
    terminal_exited: "Terminal exited",
    resize_pending: "Resizing",
    divider: "Divider",
    release_to_apply: "release to apply",
    shortcuts: "shortcuts",
    help: "help",
    actions: "actions",
    exit: "exit",
    detach: "detach",
    cancel: "cancel",
    restore: "restore",
    create_tab: "create Tab",
    start_agent: "start Agent",
    split: "split",
    create_workspace: "new Workspace",
    refresh: "refresh",
    check_again: "check again",
    take_control: "take control",
    retry_rename: "rename again",
    switch_panel: "switch",
};

const ZH_CN: StatusText = StatusText {
    prefix: "前缀",
    pointer_released: "鼠标已释放",
    zoomed: "已最大化",
    navigator: "导航器",
    sidebar: "边栏",
    focus_sidebar: "边栏",
    directory_missing: "目录不存在",
    disconnected: "已断开",
    terminal_exited: "终端已退出",
    resize_pending: "正在调整",
    divider: "分隔线",
    release_to_apply: "松手应用",
    shortcuts: "快捷键",
    help: "帮助",
    actions: "操作",
    exit: "退出",
    detach: "分离",
    cancel: "取消",
    restore: "恢复",
    create_tab: "创建标签页",
    start_agent: "启动智能体",
    split: "拆分",
    create_workspace: "新建工作区",
    refresh: "刷新",
    check_again: "再次查询",
    take_control: "接管",
    retry_rename: "再次重命名",
    switch_panel: "切换",
};

impl Locale {
    pub(crate) const fn status(self) -> StatusText {
        match self {
            Self::EnUs => EN_US,
            Self::ZhCn => ZH_CN,
        }
    }

    /// Names the shown Panel's position while a split Tab shows it alone.
    pub(crate) fn panel_position(self, position: usize, total: usize) -> String {
        match self {
            Self::EnUs => format!("Panel {position}/{total}"),
            Self::ZhCn => format!("面板 {position}/{total}"),
        }
    }
}

/// A lasting state the floating status names in a word or two beside the action that fixes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PersistentStatus {
    DirectoryMissing,
    /// The focused Panel is held by a restore whose saved directory was unavailable; its
    /// placeholder offers the actions.
    DirectoryUnavailable,
    Disconnected,
    TerminalEnded(super::TerminalEnd),
}
