use super::Locale;
use crate::action_menu::{MenuAction, MenuUnavailable};

impl Locale {
    /// Uses the same saved name or localized built-in label for every launch row.
    pub(crate) fn launch_profile_name(
        self,
        profile: &orchbee_workbench_protocol::UserLaunchProfile,
    ) -> String {
        self.launch_profile_label(profile.name.as_deref())
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
            (_, MenuUnavailable::LegacyWorkspace, false) => {
                self.legacy_workspace_action_unavailable()
            }
            (Self::EnUs, MenuUnavailable::LegacyWorkspace, true) => "New workspace",
            (Self::ZhCn, MenuUnavailable::LegacyWorkspace, true) => "需新工作区",
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
    pub(crate) const fn menu_title(self) -> &'static str {
        match self {
            Self::EnUs => "Actions",
            Self::ZhCn => "动作菜单",
        }
    }
    pub(crate) const fn menu_action(self, action: &MenuAction) -> &'static str {
        use MenuAction::*;
        match (self, action) {
            (Self::EnUs, TabCreation) => "New Tab options",
            (Self::ZhCn, TabCreation) => "新建标签页选项",
            (Self::EnUs, NewTabInRecentGroup) => "New Tab in recent group",
            (Self::ZhCn, NewTabInRecentGroup) => "在最近的分组中新建标签页",
            (Self::EnUs, TabTools) => "Selection and movement",
            (Self::ZhCn, TabTools) => "选择与移动",
            (Self::EnUs, GroupMenu) => "Group actions",
            (Self::ZhCn, GroupMenu) => "分组操作",
            (Self::EnUs, MoveToNewWindow) => "Move to new window",
            (Self::ZhCn, MoveToNewWindow) => "移至新窗口",
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
            (Self::EnUs, NewTabToRight) => "New Tab to right",
            (Self::ZhCn, NewTabToRight) => "在此标签页右侧新建",
            (Self::EnUs, NewTabInGroup) => "New Tab in group",
            (Self::ZhCn, NewTabInGroup) => "在分组中新建标签页",
            (Self::EnUs, NewGroupWithTab) => "New group with Tab",
            (Self::ZhCn, NewGroupWithTab) => "新建分组及标签页",
            (Self::EnUs, DuplicateTab) => "Duplicate Tab",
            (Self::ZhCn, DuplicateTab) => "复制标签页",
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
            (Self::EnUs, DeselectTab) => "Remove this Tab from selection",
            (Self::ZhCn, DeselectTab) => "将此标签页移出选择",
            (Self::EnUs, SelectTabRange) => "Extend selection to this Tab",
            (Self::ZhCn, SelectTabRange) => "将选择扩展至此标签页",
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
            (Self::EnUs, CreateGroup) => "New group for Tab…",
            (Self::ZhCn, CreateGroup) => "为标签页新建分组…",
            (Self::EnUs, RenameGroup) => "Rename group…",
            (Self::ZhCn, RenameGroup) => "重命名分组…",
            (Self::EnUs, ChooseGroupColor) => "Group color…",
            (Self::ZhCn, ChooseGroupColor) => "分组颜色…",
            (Self::EnUs, JoinGroup(_)) => "Move Tab to group",
            (Self::ZhCn, JoinGroup(_)) => "将标签页移入分组",
            (Self::EnUs, RemoveFromGroup) => "Remove Tab from group",
            (Self::ZhCn, RemoveFromGroup) => "将标签页移出分组",
            (Self::EnUs, Ungroup) => "Ungroup, keep Tabs",
            (Self::ZhCn, Ungroup) => "解散分组，保留标签页",
            (Self::EnUs, CollapseGroup) => "Collapse group",
            (Self::ZhCn, CollapseGroup) => "折叠分组",
            (Self::EnUs, ExpandGroup) => "Expand group",
            (Self::ZhCn, ExpandGroup) => "展开分组",
            (Self::EnUs, MoveGroupPrevious) => "Move group left",
            (Self::ZhCn, MoveGroupPrevious) => "向左移动分组",
            (Self::EnUs, MoveGroupNext) => "Move group right",
            (Self::ZhCn, MoveGroupNext) => "向右移动分组",
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
            (Self::EnUs, NewTabWithProfile(_)) => "New Tab",
            (Self::ZhCn, NewTabWithProfile(_)) => "新建标签页",
            (Self::EnUs, RenameTab) => "Rename this Tab…",
            (Self::ZhCn, RenameTab) => "重命名此标签页…",
            (Self::EnUs, QuickWorkspace) => "New Workspace",
            (Self::ZhCn, QuickWorkspace) => "新建工作区",
            (Self::EnUs, WorkspaceAtDirectory) => "New Workspace in directory…",
            (Self::ZhCn, WorkspaceAtDirectory) => "在指定目录新建工作区…",
            (Self::EnUs, MoveWorkspace) => "Move to Project…",
            (Self::ZhCn, MoveWorkspace) => "移入项目…",
            (Self::EnUs, WorkspaceProjects) => "Project",
            (Self::ZhCn, WorkspaceProjects) => "项目",
            (Self::EnUs, WorkspaceWorktrees) => "Worktree",
            (Self::ZhCn, WorkspaceWorktrees) => "Worktree",
            (Self::EnUs, CreateWorkspaceProject) => "New Project…",
            (Self::ZhCn, CreateWorkspaceProject) => "新建项目…",
            (Self::EnUs, LeaveWorkspaceProject) => "Leave Project",
            (Self::ZhCn, LeaveWorkspaceProject) => "移出项目",
            (Self::EnUs, JoinWorkspaceProject(_)) => "Move to Project",
            (Self::ZhCn, JoinWorkspaceProject(_)) => "移入项目",
            (Self::EnUs, OpenWorktreeAt(_)) => "Open Worktree",
            (Self::ZhCn, OpenWorktreeAt(_)) => "打开 Worktree",
            (Self::EnUs, RemoveWorktree) => "Remove Worktree…",
            (Self::ZhCn, RemoveWorktree) => "移除 Worktree…",
            (Self::EnUs, DeleteWorkspace | DeleteProject) => "Delete",
            (Self::ZhCn, DeleteWorkspace | DeleteProject) => "删除",
            (Self::EnUs, CloseProjectWorkspaces) => "Close all Workspaces…",
            (Self::ZhCn, CloseProjectWorkspaces) => "关闭项目内全部工作区…",
            (Self::EnUs, WorkspaceDirectory) => "Workspace directory…",
            (Self::ZhCn, WorkspaceDirectory) => "工作区目录…",
            (Self::EnUs, RenameWorkspace) => "Rename Workspace…",
            (Self::ZhCn, RenameWorkspace) => "重命名工作区…",
            (Self::EnUs, RenameProject) => "Rename Project…",
            (Self::ZhCn, RenameProject) => "重命名项目…",
            (Self::EnUs, CloseWorkspace) => "Close Workspace…",
            (Self::ZhCn, CloseWorkspace) => "关闭工作区…",
            (Self::EnUs, NewWorkspace) => "New Workspace",
            (Self::ZhCn, NewWorkspace) => "新建工作区",
            (Self::EnUs, NewWorktree) => "New Worktree",
            (Self::ZhCn, NewWorktree) => "新建 Worktree",
            (Self::EnUs, OpenWorktree) => "Open Worktree…",
            (Self::ZhCn, OpenWorktree) => "打开 Worktree…",
            (Self::EnUs, Reload) => "Reload configuration",
            (Self::ZhCn, Reload) => "重载配置",
            (Self::EnUs, Settings) => "Settings…",
            (Self::ZhCn, Settings) => "设置…",
            (Self::EnUs, Machines) => "Search machines, Workspaces and Agents",
            (Self::ZhCn, Machines) => "搜索机器、工作区和 Agent",
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
            (_, Detach) => self.client_detach_label(),
        }
    }
}
