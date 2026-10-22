use super::Locale;
impl Locale {
    #[cfg(windows)]
    pub(crate) const fn sidebar_drop_zone(self, workspace: bool) -> &'static str {
        match (self, workspace) {
            (Self::EnUs, true) => "No project",
            (Self::ZhCn, true) => "不属于项目",
            (Self::EnUs, false) => "End of projects",
            (Self::ZhCn, false) => "项目列表末尾",
        }
    }
    #[cfg(windows)]
    pub(crate) fn sidebar_reorder_hint(
        self,
        destination: Option<(crate::tui_pointer::SidebarDropPlacement, &str)>,
    ) -> String {
        use crate::tui_pointer::SidebarDropPlacement::*;
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

    pub(crate) const fn sidebar_back_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Back",
            Self::ZhCn => "返回",
        }
    }
    pub(crate) fn sidebar_navigation_action(
        self,
        action: crate::machine_picker::SidebarCommand,
    ) -> &'static str {
        use crate::machine_picker::SidebarCommand;
        match (self, action) {
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
            (Self::ZhCn, "capture-hint") => "Esc 关闭 · ↑/↓ 选择 · Enter 打开 · Tab 切换",
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
            (Self::ZhCn, "agents-mode") => "Agent",
            (Self::EnUs, "workspace-sidebar-open") => "Browse",
            (Self::ZhCn, "workspace-sidebar-open") => "浏览",
            (Self::EnUs, "sidebar-create") => "Create",
            (Self::ZhCn, "sidebar-create") => "新建",
            (Self::EnUs, "sidebar-ungrouped") => "Ungrouped",
            (Self::ZhCn, "sidebar-ungrouped") => "未分组",
            (Self::EnUs, "agent-sidebar-open") => "Filter",
            (Self::ZhCn, "agent-sidebar-open") => "筛选",
            (Self::EnUs, "no-agents") => "No agents",
            (Self::ZhCn, "no-agents") => "暂无 Agent",
            (Self::EnUs, "agents-no-matches") => "No matching agents",
            (Self::ZhCn, "agents-no-matches") => "暂无匹配的 Agent",
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
            (Self::ZhCn, "agent-search-prompt") => "搜索 Agent",
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
            (Self::ZhCn, "agent-order-provider") => "Agent 名称",
            (Self::EnUs, "agent-order-title") => "Title",
            (Self::ZhCn, "agent-order-title") => "标题",
            (Self::EnUs, "agent-order-sequence") => "Update version",
            (Self::ZhCn, "agent-order-sequence") => "更新序号",
            (Self::EnUs, "agents-loading") => "Reading agents",
            (Self::ZhCn, "agents-loading") => "读取 Agent 中",
            (Self::EnUs, "agents-unavailable") => "Agents unavailable",
            (Self::ZhCn, "agents-unavailable") => "Agent 不可用",
            (Self::EnUs, "agent-idle") => "Idle",
            (Self::ZhCn, "agent-idle") => "空闲",
            (Self::EnUs, "agent-working") => "Working",
            (Self::ZhCn, "agent-working") => "工作中",
            (Self::EnUs, "agent-blocked") => "Blocked",
            (Self::ZhCn, "agent-blocked") => "受阻",
            (Self::EnUs, "agent-unknown") => "Unknown",
            (Self::ZhCn, "agent-unknown") => "未知",
            (Self::EnUs, "standalone") => "Legacy workspace",
            (Self::ZhCn, "standalone") => "原有工作区",
            (Self::EnUs, "no-workspaces") => "No available workspaces.",
            (Self::ZhCn, "no-workspaces") => "暂无可用工作区。",
            (Self::EnUs, "hint") => {
                "↑/↓ Choose · Enter/Click Open\nTab Machines/Workspaces/Agents · Esc Close"
            }
            (Self::ZhCn, "hint") => {
                "↑/↓ 选择 · Enter/点击打开\nTab 切换机器/工作区/Agent · Esc 关闭"
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

    pub(crate) fn machine_agent_control(
        self,
        control: crate::machine_picker::Control,
        view: &crate::machine_agents::view::View,
    ) -> String {
        use crate::machine_agents::view::{Order, Scope};
        use crate::machine_picker::Control;
        use orchbee_workbench_protocol::MetadataState as State;
        let label = match control {
            Control::Done => {
                return match self {
                    Self::EnUs => "Done",
                    Self::ZhCn => "完成",
                }
                .into();
            }
            Control::Clear => {
                return match self {
                    Self::EnUs => "Clear",
                    Self::ZhCn => "清空",
                }
                .into();
            }
            Control::State => match view.state {
                None => "agent-all-states",
                Some(State::Blocked) => "agent-blocked",
                Some(State::Done) => "agent-done",
                Some(State::Error) => "agent-error",
                Some(State::Working) => "agent-working",
                Some(State::Idle) => "agent-idle",
                Some(State::Unknown) => "agent-unknown",
            },
            Control::Scope => match view.scope {
                Scope::All => "agent-scope-all",
                Scope::Machine => "agent-scope-machine",
                Scope::Context => "agent-scope-context",
            },
            Control::Order => match view.order {
                Order::Placement => "agent-order-placement",
                Order::Priority => "agent-order-priority",
                Order::Provider => "agent-order-provider",
                Order::Title => "agent-order-title",
                Order::Sequence => "agent-order-sequence",
            },
            Control::Search => "agent-search",
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

    pub(crate) fn cli_machine_route_failure(self, unknown: bool) -> &'static str {
        match (self, unknown) {
            (Self::EnUs, false) => "Remote command was not submitted.",
            (Self::ZhCn, false) => "远端命令未提交。",
            (Self::EnUs, true) => {
                "Remote command outcome is unknown. Check remote state before retrying."
            }
            (Self::ZhCn, true) => "远端命令结果未知，请检查远端状态后再决定是否重试。",
        }
    }

    pub(crate) fn cli_machine_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::MachineList => (
                "orchbee machine list [--json | --format text|json]",
                "Read the saved machine catalog without contacting SSH or creating directories. Enabled is a saved preference, not a live connection observation.",
                "读取已保存的机器目录，不连接 SSH 或创建目录。启用状态是已保存的偏好，不代表实时连接。",
            ),
            C::MachineAdd => (
                "orchbee machine add <ssh-target> --label <label> [--remote-session <name>] [--format text|json]",
                "Windows remote with an installed compatible Orchbee cohort is required. Session defaults to default. Preparation has a 60-second total deadline. Failed preparation does not save a profile; an already started remote session is retained. Automatic remote installation and interactive authentication are not yet supported.",
                "远端须为 Windows 且已安装兼容的 Orchbee cohort。会话默认 default。准备过程总期限为 60 秒。准备失败不保存配置；已启动的远端会话会保留。暂不支持自动安装远端程序及交互认证。",
            ),
            C::MachineRename => (
                "orchbee machine rename <profile-id> --label <label> [--format text|json]",
                "Rename the exact saved profile. Labels cannot be blank or contain control characters and are limited to 128 UTF-8 bytes.",
                "重命名精确 ID 对应的配置。标签不能为空白或含控制字符，最多 128 UTF-8 字节。",
            ),
            C::MachineRemove => (
                "orchbee machine remove <profile-id> [--format text|json]",
                "Remove only the saved profile. Remote sessions and OpenSSH data remain owned by their existing users.",
                "仅删除已保存的配置。远端会话和 OpenSSH 数据仍由原有用户管理。",
            ),
            C::MachineEnable => (
                "orchbee machine enable <profile-id> [--format text|json]",
                "Enable the saved profile. This command does not establish a connection or start a remote session.",
                "启用已保存的配置。本命令不会建立连接或启动远端会话。",
            ),
            C::MachineDisable => (
                "orchbee machine disable <profile-id> [--format text|json]",
                "Disable the saved profile without stopping its remote session.",
                "禁用已保存的配置，不停止远端会话。",
            ),
            _ => unreachable!("command help ownership"),
        };
        crate::localization::HelpDetails::new(
            usage,
            match self {
                Self::EnUs => english,
                Self::ZhCn => chinese,
            },
        )
    }
    pub(crate) fn cli_machine_message(self, code: &str) -> &'static str {
        match (self, code) {
            (Self::EnUs, "empty") => "No saved SSH machines.",
            (Self::ZhCn, "empty") => "尚无已保存的 SSH 机器。",
            (Self::EnUs, "enabled") => "enabled",
            (Self::ZhCn, "enabled") => "已启用",
            (Self::EnUs, "disabled") => "disabled",
            (Self::ZhCn, "disabled") => "已禁用",
            (Self::EnUs, "saved") => "Saved machine catalog change:",
            (Self::ZhCn, "saved") => "已保存机器目录变更：",
            (Self::EnUs, "unchanged") => "Machine profile unchanged:",
            (Self::ZhCn, "unchanged") => "机器配置未变化：",
            (Self::EnUs, "already-committed") => {
                "The catalog change was committed before this interruption or failure."
            }
            (Self::ZhCn, "already-committed") => "目录变更已在本次中断或失败前提交。",
            (Self::EnUs, "remote-retained") => {
                "The remote session may be running; this command does not roll it back."
            }
            (Self::ZhCn, "remote-retained") => "远端会话可能仍在运行；本命令不会回滚它。",
            (Self::EnUs, "configuration") => "The Windows configuration location is unavailable.",
            (Self::ZhCn, "configuration") => "Windows 配置位置不可用。",
            (Self::EnUs, "invalid-catalog") => {
                "The machine catalog is invalid; existing data was preserved."
            }
            (Self::ZhCn, "invalid-catalog") => "机器目录无效；已有数据保持不变。",
            (Self::EnUs, "storage") => "The private machine catalog could not be read or saved.",
            (Self::ZhCn, "storage") => "无法读取或保存私有机器目录。",
            (Self::EnUs, "busy") => {
                "Another machine catalog writer is active. Retry after it finishes."
            }
            (Self::ZhCn, "busy") => "另一个机器目录写入者正在运行，请在其结束后重试。",
            (Self::EnUs, "not-found") => {
                "The exact machine profile ID was not found. Run 'orchbee machine list'."
            }
            (Self::ZhCn, "not-found") => "未找到精确机器配置 ID，请运行 ‘orchbee machine list’。",
            (Self::EnUs, "capacity") => {
                "The machine catalog has reached its capacity of 64 profiles."
            }
            (Self::ZhCn, "capacity") => "机器目录已达到 64 个配置的容量上限。",
            (Self::EnUs, "revision-exhausted") => "The machine catalog revision cannot advance.",
            (Self::ZhCn, "revision-exhausted") => "机器目录修订号无法继续增加。",
            (Self::EnUs, "ssh-unavailable") => {
                "Windows OpenSSH Client is unavailable in the system directory."
            }
            (Self::ZhCn, "ssh-unavailable") => "系统目录中的 Windows OpenSSH Client 不可用。",
            (Self::EnUs, "ssh-failed") => {
                "SSH preparation failed. Check OpenSSH batch authentication, host trust, and the installed remote orchbee.exe."
            }
            (Self::ZhCn, "ssh-failed") => {
                "SSH 准备失败，请检查 OpenSSH 批处理认证、主机信任及已安装的远端 orchbee.exe。"
            }
            (Self::EnUs, "remote-protocol") => {
                "The remote Client returned an invalid or unsupported status document."
            }
            (Self::ZhCn, "remote-protocol") => "远端 Client 返回了无效或不支持的状态文档。",
            (Self::EnUs, "remote-incompatible") => {
                "The remote Windows Orchbee version, protocols, or cohort are incompatible. Install the matching cohort first."
            }
            (Self::ZhCn, "remote-incompatible") => {
                "远端 Windows Orchbee 版本、协议或 cohort 不兼容，请先安装匹配的 cohort。"
            }
            (Self::EnUs, "remote-not-ready") => {
                "The remote session could not be confirmed ready through a new SSH connection."
            }
            (Self::ZhCn, "remote-not-ready") => "无法通过新的 SSH 连接确认远端会话已就绪。",
            (Self::EnUs, "cancelled") => "Machine operation interrupted.",
            (Self::ZhCn, "cancelled") => "机器操作已中断。",
            (Self::EnUs, "timeout") => "The machine operation deadline expired.",
            (Self::ZhCn, "timeout") => "机器操作已超时。",
            (Self::EnUs, "physical-unknown") => {
                "SSH process cleanup could not be confirmed. Remote state may have changed."
            }
            (Self::ZhCn, "physical-unknown") => "无法确认 SSH 进程清理完成，远端状态可能已经变化。",
            (Self::EnUs, _) => "Machine operation failed before completion.",
            (Self::ZhCn, _) => "机器操作未能完成。",
        }
    }
}
