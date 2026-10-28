//! Localized machine and sidebar descriptions; navigation/filter/drop owners select the facts.

use super::Locale;
/// Description of one sidebar command selected by its navigation owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarActionLabel {
    MoveUp,
    MoveDown,
    NewWorkspace,
    Search,
    Previous,
    Next,
    First,
    Last,
    Expand,
    Collapse,
    Activate,
    OpenMenu,
    Narrower,
    Wider,
    ResetWidth,
    Help,
    Exit,
}

/// Geometric drop description selected after the owning sidebar validates its candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarPlacementLabel {
    Before,
    After,
    Into,
    Ungrouped,
    End,
}

/// Already-selected Agent filter/control label; filter choices remain in machine_picker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MachineControlLabel {
    Done,
    Clear,
    AllStates,
    Blocked,
    DoneState,
    Error,
    Working,
    Idle,
    Unknown,
    AllScope,
    MachineScope,
    ContextScope,
    PlacementOrder,
    PriorityOrder,
    ProviderOrder,
    TitleOrder,
    SequenceOrder,
    Search,
}

impl Locale {
    pub(crate) const fn sidebar_drop_zone(self, workspace: bool) -> &'static str {
        match (self, workspace) {
            (Self::EnUs, true) => "No project",
            (Self::ZhCn, true) => "不属于项目",
            (Self::EnUs, false) => "End of projects",
            (Self::ZhCn, false) => "项目列表末尾",
        }
    }
    pub(crate) fn sidebar_reorder_hint(
        self,
        destination: Option<(SidebarPlacementLabel, &str)>,
    ) -> String {
        use SidebarPlacementLabel::*;
        let Some((placement, label)) = destination else {
            return match self {
                Self::EnUs => "Drag to a row or the bottom drop area · Esc cancel".to_owned(),
                Self::ZhCn => "拖到目标行或底部放置区 · Esc 取消".to_owned(),
            };
        };
        match (self, placement) {
            (Self::EnUs, Before) => format!("Release to place before {label} · Esc cancel"),
            (Self::EnUs, After) => format!("Release to place after {label} · Esc cancel"),
            (Self::EnUs, Into) => format!("Release to move into {label} · Esc cancel"),
            (Self::EnUs, Ungrouped) => "Release to move out of Project · Esc cancel".to_owned(),
            (Self::EnUs, End) => "Release to place at end of Projects · Esc cancel".to_owned(),
            (Self::ZhCn, Before) => format!("释放后放到「{label}」前面 · Esc 取消"),
            (Self::ZhCn, After) => format!("释放后放到「{label}」后面 · Esc 取消"),
            (Self::ZhCn, Into) => format!("释放后移入「{label}」 · Esc 取消"),
            (Self::ZhCn, Ungrouped) => "释放后移出项目 · Esc 取消".to_owned(),
            (Self::ZhCn, End) => "释放后放到项目列表末尾 · Esc 取消".to_owned(),
        }
    }

    pub(crate) fn sidebar_navigation_action(
        self,
        action: impl Into<SidebarActionLabel>,
    ) -> &'static str {
        use SidebarActionLabel as SidebarCommand;
        match (self, action.into()) {
            (Self::EnUs, SidebarCommand::NewWorkspace) => "New workspace",
            (Self::ZhCn, SidebarCommand::NewWorkspace) => "新建工作区",
            (Self::EnUs, SidebarCommand::Search) => "Search",
            (Self::ZhCn, SidebarCommand::Search) => "搜索",
            (Self::EnUs, SidebarCommand::MoveUp) => "Move row up",
            (Self::ZhCn, SidebarCommand::MoveUp) => "上移当前项",
            (Self::EnUs, SidebarCommand::MoveDown) => "Move row down",
            (Self::ZhCn, SidebarCommand::MoveDown) => "下移当前项",
            (Self::EnUs, SidebarCommand::Previous) => "Previous row",
            (Self::ZhCn, SidebarCommand::Previous) => "上一行",
            (Self::EnUs, SidebarCommand::Next) => "Next row",
            (Self::ZhCn, SidebarCommand::Next) => "下一行",
            (Self::EnUs, SidebarCommand::First) => "First row",
            (Self::ZhCn, SidebarCommand::First) => "首行",
            (Self::EnUs, SidebarCommand::Last) => "Last row",
            (Self::ZhCn, SidebarCommand::Last) => "末行",
            (Self::EnUs, SidebarCommand::Expand) => "Expand / child",
            (Self::ZhCn, SidebarCommand::Expand) => "展开／子项",
            (Self::EnUs, SidebarCommand::Collapse) => "Collapse / parent",
            (Self::ZhCn, SidebarCommand::Collapse) => "收起／父项",
            (Self::EnUs, SidebarCommand::Activate) => "Open / expand",
            (Self::ZhCn, SidebarCommand::Activate) => "打开／展开",
            (Self::EnUs, SidebarCommand::OpenMenu) => "Row menu",
            (Self::ZhCn, SidebarCommand::OpenMenu) => "行菜单",
            (Self::EnUs, SidebarCommand::Narrower) => "Narrower sidebar",
            (Self::ZhCn, SidebarCommand::Narrower) => "缩窄侧栏",
            (Self::EnUs, SidebarCommand::Wider) => "Wider sidebar",
            (Self::ZhCn, SidebarCommand::Wider) => "加宽侧栏",
            (Self::EnUs, SidebarCommand::ResetWidth) => "Reset sidebar width",
            (Self::ZhCn, SidebarCommand::ResetWidth) => "重置侧栏宽度",
            (Self::EnUs, SidebarCommand::Help) => "Help",
            (Self::ZhCn, SidebarCommand::Help) => "帮助",
            (Self::EnUs, SidebarCommand::Exit) => "Return to terminal",
            (Self::ZhCn, SidebarCommand::Exit) => "返回终端",
        }
    }
    pub(crate) const fn sidebar_restore_label(self) -> &'static str {
        match self {
            Self::EnUs => "Sidebar",
            Self::ZhCn => "侧栏",
        }
    }
    pub(crate) fn machine_picker_text(self, key: &str) -> &'static str {
        match (self, key) {
            (Self::EnUs, "copy-id") => "[Ctrl+y Copy ID]",
            (Self::ZhCn, "copy-id") => "[Ctrl+y 复制 ID]",
            (Self::EnUs, "copy-id-unavailable") => "Copy unavailable; connect to a terminal first",
            (Self::ZhCn, "copy-id-unavailable") => "暂时无法复制，请先连接终端",
            (Self::EnUs, "sidebar-navigation") => "Sidebar",
            (Self::ZhCn, "sidebar-navigation") => "侧栏",
            (Self::EnUs, "local") => "Local",
            (Self::ZhCn, "local") => "本地",
            (Self::EnUs, "disabled") => "Disabled",
            (Self::ZhCn, "disabled") => "已禁用",
            (Self::EnUs, "current") => "Current",
            (Self::ZhCn, "current") => "当前",
            (Self::EnUs, "waiting") => "Waiting",
            (Self::ZhCn, "waiting") => "等待连接",
            (Self::EnUs, "opening") => "Connecting",
            (Self::ZhCn, "opening") => "连接中",
            (Self::EnUs, "online") => "Online",
            (Self::ZhCn, "online") => "在线",
            (Self::EnUs, "reconnecting") => "Reconnecting",
            (Self::ZhCn, "reconnecting") => "正在重连",
            (Self::EnUs, "attention") => "Needs attention",
            (Self::ZhCn, "attention") => "需要处理",
            (Self::EnUs, "stale") => "Stale",
            (Self::ZhCn, "stale") => "状态已过期",
            (Self::EnUs, "capture-hint") => "Esc Close · ↑/↓ Choose · Enter Open · Tab Mode",
            (Self::ZhCn, "capture-hint") => "Esc 关闭 · ↑/↓ 选择 · Enter 打开 · 标签页切换",
            (Self::EnUs, "capture-short-hint") => "Esc Close",
            (Self::ZhCn, "capture-short-hint") => "Esc 关闭",
            (Self::EnUs, "picker-close-hint") => "Close",
            (Self::ZhCn, "picker-close-hint") => "关闭",
            (Self::EnUs, "picker-choose-hint") => "Choose",
            (Self::ZhCn, "picker-choose-hint") => "选择",
            (Self::EnUs, "picker-open-hint") => "Open",
            (Self::ZhCn, "picker-open-hint") => "打开",
            (Self::EnUs, "picker-mode-hint") => "Mode",
            (Self::ZhCn, "picker-mode-hint") => "模式",
            (Self::EnUs, "picker-finish-hint") => "Finish",
            (Self::ZhCn, "picker-finish-hint") => "完成",
            (Self::EnUs, "picker-clear-hint") => "Clear",
            (Self::ZhCn, "picker-clear-hint") => "清空",
            (Self::EnUs, "machines-mode") => "Machines",
            (Self::ZhCn, "machines-mode") => "机器",
            (Self::EnUs, "workspaces-mode") => "Workspaces",
            (Self::ZhCn, "workspaces-mode") => "工作区",
            (Self::EnUs, "agents-mode") => "Agents",
            (Self::ZhCn, "agents-mode") => "智能体",
            (Self::EnUs, "workspace-sidebar-open") => "Browse",
            (Self::ZhCn, "workspace-sidebar-open") => "浏览",
            (Self::EnUs, "sidebar-create") => "Create",
            (Self::ZhCn, "sidebar-create") => "新建",
            (Self::EnUs, "sidebar-ungrouped") => "Ungrouped",
            (Self::ZhCn, "sidebar-ungrouped") => "未分组",
            (Self::EnUs, "agent-sidebar-open") => "Filter",
            (Self::ZhCn, "agent-sidebar-open") => "筛选",
            (Self::EnUs, "no-agents") => "No agents",
            (Self::ZhCn, "no-agents") => "暂无智能体",
            (Self::EnUs, "agents-no-matches") => "No matching agents",
            (Self::ZhCn, "agents-no-matches") => "暂无匹配的智能体",
            (Self::EnUs, "agent-done") => "Done",
            (Self::ZhCn, "agent-done") => "已完成",
            (Self::EnUs, "agent-error") => "Error",
            (Self::ZhCn, "agent-error") => "错误",
            (Self::EnUs, "agent-source-sort") => "Sort from view",
            (Self::ZhCn, "agent-source-sort") => "排序由视图控制",
            (Self::EnUs, "agent-source") => "View",
            (Self::ZhCn, "agent-source") => "视图",
            (Self::EnUs, "agent-search") => "Search",
            (Self::ZhCn, "agent-search") => "搜索",
            (Self::EnUs, "agent-search-prompt") => "to search agents",
            (Self::ZhCn, "agent-search-prompt") => "搜索智能体",
            (Self::EnUs, "workspace-search") => "Search Workspaces",
            (Self::ZhCn, "workspace-search") => "搜索工作区",
            (Self::EnUs, "workspaces-no-matches") => "No matching Workspaces.",
            (Self::ZhCn, "workspaces-no-matches") => "暂无匹配的工作区。",
            (Self::EnUs, "workspace-query-limit") => "Search is limited to 128 characters.",
            (Self::ZhCn, "workspace-query-limit") => "搜索词最多 128 个字符。",
            (Self::EnUs, "workspace-query-invalid") => {
                "Search accepts single-line text without controls."
            }
            (Self::ZhCn, "workspace-query-invalid") => "搜索词须为不含控制字符的单行文本。",
            (Self::EnUs, "agent-all-states") => "All states",
            (Self::ZhCn, "agent-all-states") => "全部状态",
            (Self::EnUs, "agent-scope-all") => "All machines",
            (Self::ZhCn, "agent-scope-all") => "全部机器",
            (Self::EnUs, "agent-scope-machine") => "Current machine",
            (Self::ZhCn, "agent-scope-machine") => "当前机器",
            (Self::EnUs, "agent-scope-context") => "Current location",
            (Self::ZhCn, "agent-scope-context") => "当前位置",
            (Self::EnUs, "agent-order-placement") => "Placement",
            (Self::ZhCn, "agent-order-placement") => "布局顺序",
            (Self::EnUs, "agent-order-priority") => "Status priority",
            (Self::ZhCn, "agent-order-priority") => "状态优先",
            (Self::EnUs, "agent-order-provider") => "Agent name",
            (Self::ZhCn, "agent-order-provider") => "智能体名称",
            (Self::EnUs, "agent-order-title") => "Title",
            (Self::ZhCn, "agent-order-title") => "标题",
            (Self::EnUs, "agent-order-sequence") => "Update version",
            (Self::ZhCn, "agent-order-sequence") => "更新序号",
            (Self::EnUs, "agents-loading") => "Reading agents",
            (Self::ZhCn, "agents-loading") => "读取智能体中",
            (Self::EnUs, "agents-unavailable") => "Agents unavailable",
            (Self::ZhCn, "agents-unavailable") => "智能体不可用",
            (Self::EnUs, "agent-idle") => "Idle",
            (Self::ZhCn, "agent-idle") => "空闲",
            (Self::EnUs, "agent-working") => "Working",
            (Self::ZhCn, "agent-working") => "工作中",
            (Self::EnUs, "agent-blocked") => "Blocked",
            (Self::ZhCn, "agent-blocked") => "受阻",
            (Self::EnUs, "agent-unknown") => "Unknown",
            (Self::ZhCn, "agent-unknown") => "未知",
            (Self::EnUs, "no-workspace") => "No workspace",
            (Self::ZhCn, "no-workspace") => "未打开工作区",
            (Self::EnUs, "ungrouped") => "Ungrouped",
            (Self::ZhCn, "ungrouped") => "未分组",
            (Self::EnUs, "no-workspaces") => "No available workspaces.",
            (Self::ZhCn, "no-workspaces") => "暂无可用工作区。",
            (Self::EnUs, "hint") => {
                "↑/↓ Choose · Enter/Click Open\nTab Machines/Workspaces/Agents · Esc Close"
            }
            (Self::ZhCn, "hint") => {
                "↑/↓ 选择 · Enter/点击打开\nTab 切换机器/工作区/智能体 · Esc 关闭"
            }
            (Self::EnUs, "connecting") => {
                "Switching… Esc cancels selection; background discovery continues."
            }
            (Self::ZhCn, "connecting") => "正在切换… Esc 取消选机，后台发现继续。",
            (Self::EnUs, "catalog") => "Machine catalog unavailable. Local remains available.",
            (Self::ZhCn, "catalog") => "机器目录不可用，仍可选择本地。",
            (Self::EnUs, "failed") => {
                "Connection failed. Check SSH readiness, then choose to retry."
            }
            (Self::ZhCn, "failed") => "连接失败。请检查 SSH 就绪状态，然后重新选择以重试。",
            (Self::EnUs, "removed") => {
                "Machine removed or disabled. Choose Local or another machine."
            }
            (Self::ZhCn, "removed") => "机器已删除或禁用，请选择本地或其他机器。",
            _ => "",
        }
    }

    pub(crate) fn machine_agent_control(self, control: MachineControlLabel) -> String {
        let label = match control {
            MachineControlLabel::Done => {
                return match self {
                    Self::EnUs => "Done",
                    Self::ZhCn => "完成",
                }
                .into();
            }
            MachineControlLabel::Clear => {
                return match self {
                    Self::EnUs => "Clear",
                    Self::ZhCn => "清空",
                }
                .into();
            }
            MachineControlLabel::AllStates => "agent-all-states",
            MachineControlLabel::Blocked => "agent-blocked",
            MachineControlLabel::DoneState => "agent-done",
            MachineControlLabel::Error => "agent-error",
            MachineControlLabel::Working => "agent-working",
            MachineControlLabel::Idle => "agent-idle",
            MachineControlLabel::Unknown => "agent-unknown",
            MachineControlLabel::AllScope => "agent-scope-all",
            MachineControlLabel::MachineScope => "agent-scope-machine",
            MachineControlLabel::ContextScope => "agent-scope-context",
            MachineControlLabel::PlacementOrder => "agent-order-placement",
            MachineControlLabel::PriorityOrder => "agent-order-priority",
            MachineControlLabel::ProviderOrder => "agent-order-provider",
            MachineControlLabel::TitleOrder => "agent-order-title",
            MachineControlLabel::SequenceOrder => "agent-order-sequence",
            MachineControlLabel::Search => "agent-search",
        };
        self.machine_picker_text(label).to_owned()
    }

    pub(crate) fn machine_overview(
        self,
        machines: usize,
        workspaces: usize,
        tabs: usize,
    ) -> String {
        match self {
            Self::EnUs => {
                format!(
                    "Remote online: {machines} machines · {workspaces} workspaces · {tabs} tabs"
                )
            }
            Self::ZhCn => {
                format!("远端在线：{machines} 台机器 · {workspaces} 个工作区 · {tabs} 个标签页")
            }
        }
    }
    pub(crate) fn machine_details(self, projects: usize, workspaces: usize, tabs: usize) -> String {
        match self {
            Self::EnUs => {
                format!("Selected: {projects} projects · {workspaces} workspaces · {tabs} tabs")
            }
            Self::ZhCn => {
                format!("所选：{projects} 个项目 · {workspaces} 个工作区 · {tabs} 个标签页")
            }
        }
    }
}
