use super::Locale;
impl Locale {
    pub(crate) fn machine_picker_text(self, key: &str) -> &'static str {
        match (self, key) {
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
            (Self::EnUs, "query-capture-hint") => "Enter/Esc Finish · Type Search · Ctrl+U Clear",
            (Self::ZhCn, "query-capture-hint") => "Enter/Esc 完成输入 · 输入搜索 · Ctrl+U 清空",
            (Self::EnUs, "query-capture-short-hint") => "Esc Finish",
            (Self::ZhCn, "query-capture-short-hint") => "Esc 完成输入",
            (Self::EnUs | Self::ZhCn, "capture-key") => "Esc",
            (Self::EnUs, "machines-mode") => "Machines",
            (Self::ZhCn, "machines-mode") => "机器",
            (Self::EnUs, "workspaces-mode") => "Workspaces",
            (Self::ZhCn, "workspaces-mode") => "工作区",
            (Self::EnUs, "agents-mode") => "Agents",
            (Self::ZhCn, "agents-mode") => "Agent",
            (Self::EnUs, "workspace-sidebar-open") => "Browse workspaces",
            (Self::ZhCn, "workspace-sidebar-open") => "浏览工作区",
            (Self::EnUs, "agent-sidebar-open") => "Browse / filters",
            (Self::ZhCn, "agent-sidebar-open") => "浏览／筛选",
            (Self::EnUs, "no-agents") => "No agents",
            (Self::ZhCn, "no-agents") => "暂无 Agent",
            (Self::EnUs, "agents-no-matches") => "No matching agents",
            (Self::ZhCn, "agents-no-matches") => "暂无匹配的 Agent",
            (Self::EnUs, "agent-done") => "Done",
            (Self::ZhCn, "agent-done") => "已完成",
            (Self::EnUs, "agent-source-sort") => "Sort from view",
            (Self::ZhCn, "agent-source-sort") => "排序由视图控制",
            (Self::EnUs, "agent-source") => "View",
            (Self::ZhCn, "agent-source") => "视图",
            (Self::EnUs, "agent-search") => "Search",
            (Self::ZhCn, "agent-search") => "搜索",
            (Self::EnUs, "agent-query-hint") => {
                "Type search · Backspace Erase · Ctrl+U Clear\nEnter/Esc Finish"
            }
            (Self::ZhCn, "agent-query-hint") => {
                "输入搜索 · Backspace 删除 · Ctrl+U 清空\nEnter/Esc 完成输入"
            }
            (Self::EnUs, "agent-controls-hint") => {
                "↑/↓ Choose · Enter Open · f State · s Sort\nc Scope · / Search · Tab Mode · Esc Close"
            }
            (Self::ZhCn, "agent-controls-hint") => {
                "↑/↓ 选择 · Enter 打开 · f 状态 · s 排序\nc 范围 · / 搜索 · Tab 切换 · Esc 关闭"
            }
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
            (Self::EnUs, "standalone") => "Standalone",
            (Self::ZhCn, "standalone") => "独立终端",
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
        let (shortcut, label) = match control {
            Control::State => (
                "f",
                match view.state {
                    None => "agent-all-states",
                    Some(State::Blocked) => "agent-blocked",
                    Some(State::Done) => "agent-done",
                    Some(State::Working) => "agent-working",
                    Some(State::Idle) => "agent-idle",
                    Some(State::Unknown) => "agent-unknown",
                },
            ),
            Control::Scope => (
                "c",
                match view.scope {
                    Scope::All => "agent-scope-all",
                    Scope::Machine => "agent-scope-machine",
                    Scope::Context => "agent-scope-context",
                },
            ),
            Control::Order => (
                "s",
                match view.order {
                    Order::Placement => "agent-order-placement",
                    Order::Priority => "agent-order-priority",
                    Order::Provider => "agent-order-provider",
                    Order::Title => "agent-order-title",
                    Order::Sequence => "agent-order-sequence",
                },
            ),
            Control::Search => ("/", "agent-search"),
        };
        format!("{shortcut} {}", self.machine_picker_text(label))
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

    pub(crate) fn cli_machine_help(self, verb: Option<&str>) -> &'static str {
        match (self, verb) {
            (Self::EnUs, None) => {
                "Usage: orchbee machine <list|add|rename|remove|enable|disable>\n\nManage saved Windows SSH machines. Add verifies an installed remote Client, starts the explicit Site, and verifies readiness before saving. SSH authentication and host trust must already work in batch mode. Removing or disabling a profile leaves remote sessions running. Use orchbee --machine <label-or-id> <command> for remote commands. Help is local; duplicate labels and disabled profiles are refused. Terminal --file/--stdin are read locally; paths such as --cwd belong to the remote machine. TUI and terminal streams are not routed. In the Windows TUI, use Ctrl+B then Shift+M or click the machine indicator to connect and switch. Enabled machines connect automatically for read-only status and workspace discovery; selection opens the terminal surface. Press Tab in the picker for combined Local/remote workspace or Agent navigation.\n"
            }
            (Self::ZhCn, None) => {
                "用法：orchbee machine <list|add|rename|remove|enable|disable>\n\n管理已保存的 Windows SSH 机器。Add 检查已安装的远端 Client，启动指定 Site，并在保存前确认就绪。SSH 认证和主机信任须已能以批处理模式使用。删除或禁用配置不会停止远端会话。使用 orchbee --machine <标签或ID> <命令> 执行远端命令。帮助本地显示；拒绝重名标签及已禁用配置。终端 --file/--stdin 在本地读取，--cwd 等路径属于远端。不路由 TUI 和交互式终端流。在 Windows TUI 中按 Ctrl+B 后 Shift+M，或点击机器指示器连接和切换。已启用机器自动连接以读取状态和工作区概览；选中后打开终端界面。在选择器中按 Tab 可合并浏览本地和远端工作区或 Agent。\n"
            }
            (Self::EnUs, Some("list")) => {
                "Usage: orchbee machine list [--json | --format text|json]\n\nRead the saved machine catalog without contacting SSH or creating directories. Enabled is a saved preference, not a live connection observation.\n"
            }
            (Self::ZhCn, Some("list")) => {
                "用法：orchbee machine list [--json | --format text|json]\n\n读取已保存的机器目录，不连接 SSH 或创建目录。启用状态是已保存的偏好，不代表实时连接。\n"
            }
            (Self::EnUs, Some("add")) => {
                "Usage: orchbee machine add <ssh-target> --label <label> [--remote-session <name>] [--format text|json]\n\nWindows remote with an installed compatible Orchbee cohort is required. Session defaults to default. Preparation has a 60-second total deadline. Failed preparation does not save a profile; an already started remote Site is retained. Automatic remote installation and interactive authentication are not yet supported.\n"
            }
            (Self::ZhCn, Some("add")) => {
                "用法：orchbee machine add <ssh-target> --label <label> [--remote-session <name>] [--format text|json]\n\n远端须为 Windows 且已安装兼容的 Orchbee cohort。会话默认 default。准备过程总期限为 60 秒。准备失败不保存配置；已启动的远端 Site 会保留。暂不支持自动安装远端程序及交互认证。\n"
            }
            (Self::EnUs, Some("rename")) => {
                "Usage: orchbee machine rename <profile-id> --label <label> [--format text|json]\n\nRename the exact saved profile. Labels cannot be blank or contain control characters and are limited to 128 UTF-8 bytes.\n"
            }
            (Self::ZhCn, Some("rename")) => {
                "用法：orchbee machine rename <profile-id> --label <label> [--format text|json]\n\n重命名精确 ID 对应的配置。标签不能为空白或含控制字符，最多 128 UTF-8 字节。\n"
            }
            (Self::EnUs, Some("remove")) => {
                "Usage: orchbee machine remove <profile-id> [--format text|json]\n\nRemove only the saved profile. Remote sessions and OpenSSH data remain owned by their existing users.\n"
            }
            (Self::ZhCn, Some("remove")) => {
                "用法：orchbee machine remove <profile-id> [--format text|json]\n\n仅删除已保存的配置。远端会话和 OpenSSH 数据仍由原有用户管理。\n"
            }
            (Self::EnUs, Some("enable")) => {
                "Usage: orchbee machine enable <profile-id> [--format text|json]\n\nEnable the saved profile. This command does not establish a connection or start a remote Site.\n"
            }
            (Self::ZhCn, Some("enable")) => {
                "用法：orchbee machine enable <profile-id> [--format text|json]\n\n启用已保存的配置。本命令不会建立连接或启动远端 Site。\n"
            }
            (Self::EnUs, Some("disable")) => {
                "Usage: orchbee machine disable <profile-id> [--format text|json]\n\nDisable the saved profile without stopping its remote Site.\n"
            }
            (Self::ZhCn, Some("disable")) => {
                "用法：orchbee machine disable <profile-id> [--format text|json]\n\n禁用已保存的配置，不停止远端 Site。\n"
            }
            _ => unreachable!("machine help vocabulary"),
        }
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
                "The remote Site may be running; this command does not roll it back."
            }
            (Self::ZhCn, "remote-retained") => "远端 Site 可能仍在运行；本命令不会回滚它。",
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
                "The remote Site could not be confirmed ready through a new SSH connection."
            }
            (Self::ZhCn, "remote-not-ready") => "无法通过新的 SSH 连接确认远端 Site 已就绪。",
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
