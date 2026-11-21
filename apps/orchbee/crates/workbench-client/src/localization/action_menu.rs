use super::{Locale, PanelDirection};

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
    /// The row holds the value in force; it shows as the row's check, not as words.
    Applied,
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

    /// The words a note ends its row with; a note shown another way has none.
    pub(crate) const fn menu_row_note(self, note: MenuRowNote) -> Option<&'static str> {
        match (self, note) {
            (Self::EnUs, MenuRowNote::DefaultProfile) => Some("Default"),
            (Self::ZhCn, MenuRowNote::DefaultProfile) => Some("默认"),
            (_, MenuRowNote::Applied) => None,
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
}
