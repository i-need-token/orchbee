//! Localized machine and sidebar descriptions; navigation/filter/drop owners select the facts.

use super::Locale;
use crate::machine_discovery::{Blocked, HubStatus};
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
    /// The bottom drop target of a Project drag; a Workspace drag has none.
    pub(crate) const fn sidebar_drop_zone(self) -> &'static str {
        match self {
            Self::EnUs => "End of projects",
            Self::ZhCn => "项目列表末尾",
        }
    }
    pub(crate) fn sidebar_reorder_hint(
        self,
        workspace: bool,
        destination: Option<(SidebarPlacementLabel, &str)>,
    ) -> String {
        use SidebarPlacementLabel::*;
        let Some((placement, label)) = destination else {
            return match (self, workspace) {
                (Self::EnUs, true) => "Drag to a row · Esc cancel".to_owned(),
                (Self::ZhCn, true) => "拖到目标行 · Esc 取消".to_owned(),
                (Self::EnUs, false) => {
                    "Drag to a row or the bottom drop area · Esc cancel".to_owned()
                }
                (Self::ZhCn, false) => "拖到目标行或底部放置区 · Esc 取消".to_owned(),
            };
        };
        match (self, placement) {
            (Self::EnUs, Before) => format!("Release to place before {label} · Esc cancel"),
            (Self::EnUs, After) => format!("Release to place after {label} · Esc cancel"),
            (Self::EnUs, Into) => format!("Release to move into {label} · Esc cancel"),
            (Self::EnUs, End) => "Release to place at end of Projects · Esc cancel".to_owned(),
            (Self::ZhCn, Before) => format!("释放后放到「{label}」前面 · Esc 取消"),
            (Self::ZhCn, After) => format!("释放后放到「{label}」后面 · Esc 取消"),
            (Self::ZhCn, Into) => format!("释放后移入「{label}」 · Esc 取消"),
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
            (Self::ZhCn, SidebarCommand::Narrower) => "缩窄边栏",
            (Self::EnUs, SidebarCommand::Wider) => "Wider sidebar",
            (Self::ZhCn, SidebarCommand::Wider) => "加宽边栏",
            (Self::EnUs, SidebarCommand::ResetWidth) => "Reset sidebar width",
            (Self::ZhCn, SidebarCommand::ResetWidth) => "重置边栏宽度",
            (Self::EnUs, SidebarCommand::Help) => "Help",
            (Self::ZhCn, SidebarCommand::Help) => "帮助",
            (Self::EnUs, SidebarCommand::Exit) => "Return to terminal",
            (Self::ZhCn, SidebarCommand::Exit) => "返回终端",
        }
    }
    pub(crate) fn machine_picker_text(self, key: &str) -> &'static str {
        match (self, key) {
            (Self::EnUs, "copy-id") => "Copy ID",
            (Self::ZhCn, "copy-id") => "复制 ID",
            (Self::EnUs, "copy-id-unavailable") => "Copy unavailable; connect to a terminal first",
            (Self::ZhCn, "copy-id-unavailable") => "暂时无法复制，请先连接终端",
            (Self::EnUs, "sidebar-navigation") => "Sidebar",
            (Self::ZhCn, "sidebar-navigation") => "边栏",
            (Self::EnUs, "local") => "Local",
            (Self::ZhCn, "local") => "本地",
            (Self::EnUs, "current") => "Current",
            (Self::ZhCn, "current") => "当前",
            // A remote row's one word: the hub's state of the Machine, the same in every interface.
            (Self::EnUs, "disabled") => "Disabled",
            (Self::ZhCn, "disabled") => "已停用",
            (Self::EnUs, "unsupported") => "Unsupported",
            (Self::ZhCn, "unsupported") => "不支持",
            (Self::EnUs, "opening") => "Connecting",
            (Self::ZhCn, "opening") => "连接中",
            (Self::EnUs, "online") => "Online",
            (Self::ZhCn, "online") => "在线",
            (Self::EnUs, "reconnecting") => "Reconnecting",
            (Self::ZhCn, "reconnecting") => "重连中",
            (Self::EnUs, "offline") => "Offline",
            (Self::ZhCn, "offline") => "离线",
            (Self::EnUs, "needs-login") => "Needs login",
            (Self::ZhCn, "needs-login") => "需要登录",
            (Self::EnUs, "unresponsive") => "Unresponsive",
            (Self::ZhCn, "unresponsive") => "无响应",
            (Self::EnUs, "untrusted") => "Untrusted",
            (Self::ZhCn, "untrusted") => "未信任",
            (Self::EnUs, "not-installed") => "Not installed",
            (Self::ZhCn, "not-installed") => "未安装",
            (Self::EnUs, "needs-update") => "Needs update",
            (Self::ZhCn, "needs-update") => "需更新",
            (Self::EnUs, "local-needs-update") => "Local outdated",
            (Self::ZhCn, "local-needs-update") => "本机需更新",
            (Self::EnUs, "login-failed") => "Login failed",
            (Self::ZhCn, "login-failed") => "登录失败",
            (Self::EnUs, "key-changed") => "Key changed",
            (Self::ZhCn, "key-changed") => "密钥已变",
            (Self::EnUs, "incompatible") => "Incompatible",
            (Self::ZhCn, "incompatible") => "不兼容",
            (Self::EnUs, "unknown") => "Unknown",
            (Self::ZhCn, "unknown") => "未知",
            (Self::EnUs, "hub-unavailable") => "Unavailable",
            (Self::ZhCn, "hub-unavailable") => "不可用",
            (Self::EnUs, "hub-stopped") => "Stopped",
            (Self::ZhCn, "hub-stopped") => "已停止",
            // A Workspace row whose last checkpoint outlived its source.
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
            (Self::EnUs, "sidebar-create") => "Create",
            (Self::ZhCn, "sidebar-create") => "新建",
            (Self::EnUs, "sidebar-recents") => "Recents",
            (Self::ZhCn, "sidebar-recents") => "最近",
            (Self::EnUs, "agent-sidebar-open") => "Filter",
            (Self::ZhCn, "agent-sidebar-open") => "筛选",
            (Self::EnUs, "no-agents") => "No agents",
            (Self::ZhCn, "no-agents") => "暂无智能体",
            (Self::EnUs, "agents-no-matches") => "No matching agents",
            (Self::ZhCn, "agents-no-matches") => "暂无匹配的智能体",
            (Self::EnUs, "agent-done") => "Done",
            (Self::ZhCn, "agent-done") => "完成",
            (Self::EnUs, "agent-error") => "Failed",
            (Self::ZhCn, "agent-error") => "失败",
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
            (Self::EnUs, "agent-idle") => "Ready",
            (Self::ZhCn, "agent-idle") => "就绪",
            (Self::EnUs, "agent-working") => "Working",
            (Self::ZhCn, "agent-working") => "运行中",
            (Self::EnUs, "agent-starting") => "Starting",
            (Self::ZhCn, "agent-starting") => "启动中",
            (Self::EnUs, "agent-blocked") => "Needs attention",
            (Self::ZhCn, "agent-blocked") => "需要处理",
            (Self::EnUs, "agent-unknown") => "Limited status",
            (Self::ZhCn, "agent-unknown") => "状态受限",
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
            (Self::EnUs, "hub") => "Machine connections are unavailable. Local remains available.",
            (Self::ZhCn, "hub") => "机器连接不可用，仍可使用本地。",
            (Self::EnUs, "hub-message-stopped") => {
                "Machine connections stopped. Choose a machine to start them."
            }
            (Self::ZhCn, "hub-message-stopped") => "机器连接已停止，选择机器即可重新开始。",
            (Self::EnUs, "hub-message-blocked") => {
                "Machine connections cannot start; the machine's details say why."
            }
            (Self::ZhCn, "hub-message-blocked") => "机器连接无法开始，原因见机器详情。",
            (Self::EnUs, "failed") => {
                "Connection failed. Check SSH readiness, then choose to retry."
            }
            (Self::ZhCn, "failed") => "连接失败。请检查 SSH 就绪状态，然后重新选择以重试。",
            (Self::EnUs, "removed") => {
                "Machine removed or disabled. Choose Local or another machine."
            }
            (Self::ZhCn, "removed") => "机器已删除或停用，请选择本地或其他机器。",
            // Refusals name the side that must change; each line fits the 70-column picker.
            (Self::EnUs, "refused-environment") => {
                "The remote Orchbee belongs to another environment.\nUse release on both, or builds from the same checkout path."
            }
            (Self::ZhCn, "refused-environment") => {
                "远端 Orchbee 属于另一个环境。\n请两端都用正式版，或都用同一检出路径的构建。"
            }
            (Self::EnUs, "refused-client-old") => {
                "This Orchbee is too old for the remote Session.\nUpdate Orchbee on this computer, then choose to retry."
            }
            (Self::ZhCn, "refused-client-old") => {
                "本机 Orchbee 版本过旧，远端会话不接受。\n请更新本机的 Orchbee，然后重新选择以重试。"
            }
            (Self::EnUs, "refused-session-old") => {
                "The remote Session is too old for this Orchbee.\nUpdate Orchbee there, then run `orchbee server stop` there."
            }
            (Self::ZhCn, "refused-session-old") => {
                "远端会话版本过旧，无法与本机 Orchbee 配合。\n请在远端更新 Orchbee，再运行 `orchbee server stop`。"
            }
            (Self::EnUs, "refused-build") => {
                "The remote Session runs a different development build.\nStop it there with `orchbee server stop`, then retry."
            }
            (Self::ZhCn, "refused-build") => {
                "远端会话运行的是另一个开发构建。\n请在远端运行 `orchbee server stop` 停止它，然后重试。"
            }
            _ => "",
        }
    }

    /// The selected row's explanation while no hub state holds: what happened and what the person
    /// can do. Each line fits the 70-column picker; the row itself shows one word.
    pub(crate) fn machine_hub_detail(self, status: &HubStatus) -> Option<String> {
        let text = match (self, status) {
            (_, HubStatus::Unknown) => return None,
            (Self::EnUs, HubStatus::Stopped) => {
                "Machine connections were stopped.\nChoosing this machine starts them again.".into()
            }
            (Self::ZhCn, HubStatus::Stopped) => "机器连接已被停止。\n选择这台机器会重新开始。".into(),
            (Self::EnUs, HubStatus::Unavailable) => {
                "Machine connections could not start.\nOrchbee tries again by itself.".into()
            }
            (Self::ZhCn, HubStatus::Unavailable) => {
                "机器连接未能开始。\nOrchbee 会自动重试。".into()
            }
            (Self::EnUs, HubStatus::Blocked(Blocked::Held { version })) => format!(
                "Orchbee {version} still holds machine connections.\nClose its windows and stop its Session; connections then resume."
            ),
            (Self::ZhCn, HubStatus::Blocked(Blocked::Held { version })) => format!(
                "Orchbee {version} 仍占用着机器连接。\n关闭它的窗口并停止它的会话后，连接会自动恢复。"
            ),
            (Self::EnUs, HubStatus::Blocked(Blocked::Environment { environment })) => format!(
                "Orchbee of another environment holds machine connections:\n{environment}. Close it; connections then resume."
            ),
            (Self::ZhCn, HubStatus::Blocked(Blocked::Environment { environment })) => format!(
                "另一个环境的 Orchbee 占用着机器连接：\n{environment}。关闭它后，连接会自动恢复。"
            ),
            (Self::EnUs, HubStatus::Blocked(Blocked::Denied)) => {
                "Another user's Orchbee holds machine connections here.\nThey resume once it closes."
                    .into()
            }
            (Self::ZhCn, HubStatus::Blocked(Blocked::Denied)) => {
                "另一个用户的 Orchbee 占用着这里的机器连接。\n它关闭后，连接会自动恢复。".into()
            }
            (Self::EnUs, HubStatus::Blocked(Blocked::Protocol)) => {
                "Machine connections answered outside the protocol.\nChoose this machine to try again."
                    .into()
            }
            (Self::ZhCn, HubStatus::Blocked(Blocked::Protocol)) => {
                "机器连接的应答不符合协议。\n选择这台机器可再试一次。".into()
            }
            (Self::EnUs, HubStatus::Blocked(Blocked::DataRoot)) => {
                "The Orchbee data folder is unavailable.\nCheck it, then choose this machine again."
                    .into()
            }
            (Self::ZhCn, HubStatus::Blocked(Blocked::DataRoot)) => {
                "Orchbee 数据目录不可用。\n检查后，再选择这台机器。".into()
            }
        };
        Some(text)
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
