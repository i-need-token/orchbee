//! Localized purposes for the build-admitted help topics. Command words remain
//! the invocation grammar's responsibility; this file owns only user-facing copy.

use super::Locale;
use crate::invocation::{CommandPath, Family, HelpTopic};

/// Authored command syntax and localized constraints, independent of layout.
/// The syntax is retained as a single field so mutually exclusive operand forms
/// can be shown without reconstructing them from a list of flags.
#[derive(Clone, Copy)]
pub(crate) struct HelpDetails {
    pub(crate) usage: &'static str,
    pub(crate) notes: &'static str,
}

impl HelpDetails {
    pub(crate) const fn new(usage: &'static str, notes: &'static str) -> Self {
        Self { usage, notes }
    }
}

impl super::CliText {
    pub(crate) const fn cli_grammar_availability(
        self,
        availability: crate::invocation::grammar::Availability,
    ) -> &'static str {
        match (self.locale, availability) {
            (Locale::EnUs, crate::invocation::grammar::Availability::Windows) => {
                "Execution currently requires Windows. Syntax, help and completion are available on every platform; other hosts return unsupported-platform (exit 4)."
            }
            (Locale::ZhCn, crate::invocation::grammar::Availability::Windows) => {
                "执行目前需要 Windows。所有平台均公开相同语法、帮助与补全；其他宿主返回 unsupported-platform（退出码 4）。"
            }
            (Locale::EnUs, crate::invocation::grammar::Availability::Portable) => {
                "Syntax, help and completion are identical on every platform. The selected execution adapter retains its documented capabilities and defaults; unavailable operations return unsupported-platform (exit 4)."
            }
            (Locale::ZhCn, crate::invocation::grammar::Availability::Portable) => {
                "所有平台的语法、帮助与补全相同。所选执行适配保留文档规定的能力和默认值；未实现的操作返回 unsupported-platform（退出码 4）。"
            }
        }
    }

    pub(crate) fn cli_label_target_hint(self, label: &str) -> Option<&'static str> {
        crate::invocation::label_requires_id(label).then_some(match self.locale {
            Locale::EnUs => "This display name uses target syntax. Address this resource with its actual ID shown above.\n",
            Locale::ZhCn => "此显示名称与目标语法相同，请使用上方返回的资源实际 ID 作为目标。\n",
        })
    }

    /// Explain the purpose of one help topic without exposing its implementation.
    pub(crate) fn cli_help_summary(self, topic: HelpTopic) -> &'static str {
        let (english, chinese) = match topic {
            HelpTopic::Command(CommandPath::WorkspaceStatus) => (
                "Read one retained operation by request ID.",
                "按请求 ID 读取一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceResume) => (
                "Resume one retained operation with an exact generation fence.",
                "按精确代次围栏恢复一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::TabStatus) => (
                "Read one retained operation by request ID.",
                "按请求 ID 读取一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::TabResume) => (
                "Resume one retained operation with an exact generation fence.",
                "按精确代次围栏恢复一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::PanelStatus) => (
                "Read one retained operation by request ID.",
                "按请求 ID 读取一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::PanelResume) => (
                "Resume one retained operation with an exact generation fence.",
                "按精确代次围栏恢复一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::ProjectStatus) => (
                "Read one retained operation by request ID.",
                "按请求 ID 读取一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::ProjectResume) => (
                "Resume one retained operation with an exact generation fence.",
                "按精确代次围栏恢复一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::ClientStatus) => (
                "Read one retained operation by request ID.",
                "按请求 ID 读取一项保留操作。",
            ),
            HelpTopic::Command(CommandPath::ServerStatus) => (
                "Read one retained operation by request ID.",
                "按请求 ID 读取一项保留操作。",
            ),

            HelpTopic::Family(Family::PanelGraphics) => (
                "Display and inspect graphics in a panel.",
                "在面板中显示和查看图像。",
            ),
            HelpTopic::Command(CommandPath::PanelGraphicsSet) => {
                ("Display graphics in a panel.", "在面板中显示图像。")
            }
            HelpTopic::Command(CommandPath::PanelGraphicsClear) => {
                ("Clear a panel's graphics.", "清除面板中的图像。")
            }
            HelpTopic::Command(CommandPath::PanelGraphicsInfo) => {
                ("Show a panel's graphics state.", "查看面板图像状态。")
            }
            HelpTopic::Command(CommandPath::PanelGraphicsStream) => (
                "Stream graphics updates to a panel.",
                "向面板持续发送图像更新。",
            ),
            HelpTopic::Family(Family::Client) => (
                "Inspect connected terminal clients.",
                "查看已连接的终端客户端。",
            ),
            HelpTopic::Command(CommandPath::ClientList) => {
                ("List connected clients.", "列出已连接的客户端。")
            }
            HelpTopic::Command(CommandPath::ClientGet) => {
                ("Show one connected client.", "查看一个已连接的客户端。")
            }
            HelpTopic::Command(CommandPath::ClientControlStatus) => (
                "Check a client navigation request.",
                "查看客户端导航请求的状态。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceFocus) => (
                "Switch a client to a workspace.",
                "让客户端切换到指定工作区。",
            ),
            HelpTopic::Command(CommandPath::TabFocus) => {
                ("Switch a client to a tab.", "让客户端切换到指定标签页。")
            }
            HelpTopic::Command(CommandPath::PanelFocus) => {
                ("Focus a terminal panel.", "聚焦指定终端面板。")
            }
            HelpTopic::Command(CommandPath::PanelResize) => {
                ("Adjust a panel's size.", "调整面板大小。")
            }
            HelpTopic::Family(Family::Layout) => (
                "Save, restore and adjust terminal layouts.",
                "保存、恢复和调整终端布局。",
            ),
            HelpTopic::Command(CommandPath::LayoutSetSplitRatio) => {
                ("Set the size ratio of a split.", "设置分屏比例。")
            }
            HelpTopic::Command(CommandPath::LayoutExport) => (
                "Export a reusable terminal layout.",
                "导出可复用的终端布局。",
            ),
            HelpTopic::Command(CommandPath::LayoutHistory) => (
                "List earlier layouts you can reopen.",
                "列出可重新打开的早先布局。",
            ),
            HelpTopic::Command(CommandPath::LayoutReopen) => (
                "Reopen the closed part of an earlier layout.",
                "重新打开早先布局中已关闭的部分。",
            ),
            HelpTopic::Command(CommandPath::LayoutApply) => {
                ("Apply a saved terminal layout.", "应用已保存的终端布局。")
            }
            HelpTopic::Command(CommandPath::PanelZoom) => (
                "Toggle or set a panel's maximized state.",
                "切换或设置面板的最大化状态。",
            ),
            HelpTopic::Command(CommandPath::PanelLayout) => {
                ("Show the panel layout tree.", "显示面板布局树。")
            }
            HelpTopic::Command(CommandPath::PanelNeighbor) => (
                "Find the neighboring panel in a direction.",
                "查找指定方向的相邻面板。",
            ),
            HelpTopic::Command(CommandPath::PanelEdges) => {
                ("Show a panel's split boundaries.", "显示面板的分屏边界。")
            }
            HelpTopic::Command(CommandPath::Tui) => {
                ("Open the terminal interface.", "打开终端界面。")
            }
            HelpTopic::Command(CommandPath::Status) => (
                "Show the current terminal session status.",
                "显示当前终端会话状态。",
            ),
            HelpTopic::Command(CommandPath::StatusServer) => {
                ("Show session service status.", "显示会话服务状态。")
            }
            HelpTopic::Command(CommandPath::StatusClient) => {
                ("Show terminal client status.", "显示终端客户端状态。")
            }
            HelpTopic::Family(Family::Agent) => (
                "Find, run and interact with AI Agents.",
                "查找、启动并操作 AI 智能体。",
            ),
            HelpTopic::Family(Family::AgentView) => (
                "Choose which Agents appear in the sidebar.",
                "选择边栏显示的智能体。",
            ),
            HelpTopic::Command(CommandPath::AgentViewGet) => {
                ("Show the Agent view settings.", "显示智能体视图设置。")
            }
            HelpTopic::Command(CommandPath::AgentViewSet) => (
                "Save Agent filters, grouping and sorting.",
                "保存智能体筛选、分组和排序设置。",
            ),
            HelpTopic::Command(CommandPath::AgentViewClear) => {
                ("Restore the default Agent view.", "恢复默认智能体视图。")
            }
            HelpTopic::Command(CommandPath::AgentList) => {
                ("List observed Agents.", "列出已发现的智能体。")
            }
            HelpTopic::Command(CommandPath::AgentChanges) => (
                "Read a page of Agent state changes.",
                "分页读取智能体状态变化。",
            ),
            HelpTopic::Command(CommandPath::AgentGet) => {
                ("Show one Agent and its state.", "查看一个智能体及其状态。")
            }
            HelpTopic::Command(CommandPath::AgentFocus) => (
                "Focus the terminal containing an Agent.",
                "聚焦智能体所在的终端。",
            ),
            HelpTopic::Command(CommandPath::AgentAttention) => (
                "Show an Agent's seen/unseen state in a client.",
                "查看客户端对智能体的已读／未读状态。",
            ),
            HelpTopic::Command(CommandPath::AgentRead) => {
                ("Read an Agent's terminal output.", "读取智能体的终端输出。")
            }
            HelpTopic::Command(CommandPath::AgentStart) => (
                "Start an Agent in a new Tab, a split, or an idle terminal.",
                "在新标签页、拆分面板或空闲终端中启动智能体。",
            ),
            HelpTopic::Command(CommandPath::AgentPrompt) => {
                ("Submit a prompt to an Agent.", "向智能体提交提示词。")
            }
            HelpTopic::Command(CommandPath::AgentSendKeys) => {
                ("Send key presses to an Agent.", "向智能体发送按键。")
            }
            HelpTopic::Command(CommandPath::AgentExplain) => (
                "Explain how an Agent was identified.",
                "说明智能体的识别依据。",
            ),
            HelpTopic::Command(CommandPath::AgentWait) => (
                "Wait for an Agent to match a condition.",
                "等待智能体满足指定条件。",
            ),
            HelpTopic::Command(CommandPath::AgentDoctor) => {
                ("Check local Agent integrations.", "检查本地智能体集成。")
            }
            HelpTopic::Command(CommandPath::AgentNames) => (
                "List saved Agent session names.",
                "列出已保存的智能体会话名称。",
            ),
            HelpTopic::Command(CommandPath::AgentResolve) => (
                "Resolve an Agent name to its session.",
                "按名称查找智能体会话。",
            ),
            HelpTopic::Command(CommandPath::AgentRename) => {
                ("Give an Agent session a name.", "设置智能体会话名称。")
            }
            HelpTopic::Family(Family::AgentIntegration) => {
                ("Set up local Agent integrations.", "配置本地智能体集成。")
            }
            HelpTopic::Command(CommandPath::AgentIntegrationList) => (
                "List local Agent integration states.",
                "列出本地智能体集成状态。",
            ),
            HelpTopic::Command(CommandPath::AgentIntegrationExplain) => (
                "Explain an Agent integration's setup.",
                "说明智能体集成的配置情况。",
            ),
            HelpTopic::Command(CommandPath::AgentIntegrationEnable) => {
                ("Enable a local Agent integration.", "启用本地智能体集成。")
            }
            HelpTopic::Command(CommandPath::AgentIntegrationDisable) => {
                ("Disable a local Agent integration.", "停用本地智能体集成。")
            }
            HelpTopic::Family(Family::Project) => (
                "Group related workspaces into projects.",
                "用项目组织相关工作区。",
            ),
            HelpTopic::Family(Family::Workspace) => {
                ("Create and organize workspaces.", "创建和整理工作区。")
            }
            HelpTopic::Family(Family::Tab) => (
                "Create, arrange and close tabs.",
                "创建、排列和关闭标签页。",
            ),
            HelpTopic::Family(Family::Panel) => (
                "Split, arrange and control terminal panels.",
                "拆分、排列和操作终端面板。",
            ),
            HelpTopic::Family(Family::Terminal) => (
                "Read terminal output and send input.",
                "读取终端输出并发送输入。",
            ),
            HelpTopic::Command(CommandPath::TerminalAttach) => {
                ("Interact with a running terminal.", "进入正在运行的终端。")
            }
            HelpTopic::Command(CommandPath::AgentAttach) => (
                "Interact with an Agent's terminal.",
                "进入智能体所在的终端。",
            ),
            HelpTopic::Family(Family::WorkbenchClient) => (
                "Stream terminal frames and control messages.",
                "读写终端画面与控制消息流。",
            ),
            HelpTopic::Command(CommandPath::TerminalObserve) => (
                "Read terminal frames as a JSON stream.",
                "以 JSON 流读取终端画面。",
            ),
            HelpTopic::Command(CommandPath::TerminalControl) => (
                "Control a terminal through JSON messages.",
                "通过 JSON 消息控制终端。",
            ),
            HelpTopic::Family(Family::Api) => (
                "Read snapshots and offline API schemas.",
                "读取快照和离线 API 定义。",
            ),
            HelpTopic::Family(Family::Server) => (
                "Manage the session's background service.",
                "管理会话的后台服务。",
            ),
            HelpTopic::Family(Family::Config) => (
                "Check, import and restore settings.",
                "检查、导入和恢复设置。",
            ),
            HelpTopic::Family(Family::Channel) => ("Choose the release channel.", "选择更新频道。"),
            HelpTopic::Family(Family::Machine) => {
                ("Manage saved remote machines.", "管理已保存的远程机器。")
            }
            HelpTopic::Command(CommandPath::MachineList) => {
                ("List saved remote machines.", "列出已保存的远程机器。")
            }
            HelpTopic::Command(CommandPath::MachineAdd) => (
                "Save an SSH machine for remote commands.",
                "保存用于远程命令的 SSH 机器。",
            ),
            HelpTopic::Command(CommandPath::MachineRename) => {
                ("Rename a saved remote machine.", "重命名已保存的远程机器。")
            }
            HelpTopic::Command(CommandPath::MachineRemove) => {
                ("Remove a saved remote machine.", "移除已保存的远程机器。")
            }
            HelpTopic::Command(CommandPath::MachineEnable) => {
                ("Enable a saved remote machine.", "启用已保存的远程机器。")
            }
            HelpTopic::Command(CommandPath::MachineDisable) => {
                ("Disable a saved remote machine.", "停用已保存的远程机器。")
            }
            HelpTopic::Command(CommandPath::MachineConnect) => (
                "Connect to a saved remote machine. Not available yet.",
                "连接已保存的远程机器。暂不可用。",
            ),
            HelpTopic::Command(CommandPath::MachineDisconnect) => (
                "Disconnect from a saved remote machine. Not available yet.",
                "断开与已保存远程机器的连接。暂不可用。",
            ),
            HelpTopic::Command(CommandPath::ChannelShow) => {
                ("Show the configured release channel.", "显示当前更新频道。")
            }
            HelpTopic::Command(CommandPath::ChannelSet) => (
                "Save the release channel and update.",
                "保存更新频道并执行更新。",
            ),
            HelpTopic::Command(CommandPath::Update) => {
                ("Install the configured release.", "安装所选频道的版本。")
            }
            HelpTopic::Command(CommandPath::Install) => (
                "Install this release and make it active.",
                "安装此版本并设为当前版本。",
            ),
            HelpTopic::Command(CommandPath::Uninstall) => (
                "Remove the installed Orchbee; data stays unless --delete-data.",
                "移除已安装的 Orchbee；除非加 --delete-data，否则保留数据。",
            ),
            HelpTopic::Family(Family::Completion) => (
                "Generate shell completion scripts.",
                "生成 Shell 命令补全脚本。",
            ),
            HelpTopic::Command(CommandPath::ProjectList) => {
                ("List project groups.", "列出项目分组。")
            }
            HelpTopic::Command(CommandPath::ProjectGet) => {
                ("Show a project and its workspaces.", "查看项目及其工作区。")
            }
            HelpTopic::Command(CommandPath::ProjectDissolve) => (
                "Remove a project group while keeping its workspaces.",
                "解散项目分组并保留工作区。",
            ),
            HelpTopic::Command(CommandPath::ProjectCloseAll) => (
                "Close every workspace in a project.",
                "关闭项目中的全部工作区。",
            ),
            HelpTopic::Command(CommandPath::ProjectCloseStatus) => (
                "Check a project's workspace closure.",
                "查看项目工作区的关闭状态。",
            ),
            HelpTopic::Command(CommandPath::ProjectCloseResume) => (
                "Resume an unfinished project closure.",
                "继续尚未完成的项目关闭操作。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceSetProject) => (
                "Move a workspace into a project group.",
                "将工作区归入指定项目。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceCreateProject) => (
                "Create a project and group this workspace in it.",
                "创建项目并将当前工作区归入其中。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceUngroup) => (
                "Keep a workspace outside project groups.",
                "将工作区移出项目分组。",
            ),
            HelpTopic::Command(CommandPath::ProjectCreate) => {
                ("Create a project group.", "创建项目分组。")
            }
            HelpTopic::Command(CommandPath::ProjectRename) => {
                ("Rename a project group.", "重命名项目分组。")
            }
            HelpTopic::Command(CommandPath::WorkspaceList) => (
                "List workspaces, optionally within a project.",
                "列出工作区，可按项目筛选。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceGet) => {
                ("Show one workspace.", "查看一个工作区。")
            }
            HelpTopic::Command(CommandPath::WorkspaceCreate) => (
                "Create a workspace and its first shell.",
                "创建工作区及其首个 Shell。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceRename) => {
                ("Rename a workspace.", "重命名工作区。")
            }
            HelpTopic::Command(CommandPath::PanelReportMetadata) => {
                ("Update a panel's display details.", "更新面板显示信息。")
            }
            HelpTopic::Command(CommandPath::PanelReportAgent) => (
                "Report an Agent associated with a panel.",
                "报告面板关联的智能体。",
            ),
            HelpTopic::Command(CommandPath::PanelClearAgentAuthority) => (
                "Clear an Agent reporting assignment.",
                "清除智能体状态报告关联。",
            ),
            HelpTopic::Command(CommandPath::PanelReleaseAgent) => (
                "Release a panel's Agent association.",
                "解除面板的智能体关联。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceReportMetadata) => (
                "Update temporary workspace display details.",
                "更新临时工作区显示信息。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceMove) => (
                "Change a workspace's navigation order.",
                "调整工作区的导航顺序。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceMoveBlock) => {
                ("Move several workspaces together.", "一起移动多个工作区。")
            }
            HelpTopic::Command(CommandPath::WorkspaceAcceptDirectory) => (
                "Accept a workspace's working directory.",
                "确认工作区的工作目录。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceClose) => (
                "Close a workspace and its terminals.",
                "关闭工作区及其终端。",
            ),
            HelpTopic::Command(CommandPath::WorkspaceCloseStatus) => {
                ("Check a workspace closure.", "查看工作区的关闭状态。")
            }
            HelpTopic::Command(CommandPath::WorkspaceCloseResume) => (
                "Resume an unfinished workspace closure.",
                "继续尚未完成的工作区关闭操作。",
            ),
            HelpTopic::Command(CommandPath::TabList) => ("List terminal tabs.", "列出终端标签页。"),
            HelpTopic::Command(CommandPath::TabGet) => {
                ("Show one terminal tab.", "查看一个终端标签页。")
            }
            HelpTopic::Command(CommandPath::TabCreate) => {
                ("Open a terminal in a new tab.", "在新标签页中打开终端。")
            }
            HelpTopic::Command(CommandPath::TabRename) => ("Rename a tab.", "重命名标签页。"),
            HelpTopic::Command(CommandPath::PanelRename) => ("Rename a panel.", "重命名面板。"),
            HelpTopic::Command(CommandPath::PanelInput) => (
                "Choose where right clicks in a panel go.",
                "选择面板中右键点击的去向。",
            ),
            HelpTopic::Command(CommandPath::PanelSwap) => {
                ("Exchange two panels' positions.", "交换两个面板的位置。")
            }
            HelpTopic::Command(CommandPath::TabMove) => (
                "Move a tab within the navigation order.",
                "调整标签页的导航顺序。",
            ),
            HelpTopic::Command(CommandPath::TabClose) => {
                ("Close a tab and its terminals.", "关闭标签页及其终端。")
            }
            HelpTopic::Command(CommandPath::PanelList) => {
                ("List terminal panels.", "列出终端面板。")
            }
            HelpTopic::Command(CommandPath::PanelCurrent) => (
                "Identify the caller's current panel.",
                "查找调用者所在的当前面板。",
            ),
            HelpTopic::Command(CommandPath::PanelGet) => {
                ("Show one terminal panel.", "查看一个终端面板。")
            }
            HelpTopic::Command(CommandPath::PanelProcessInfo) => {
                ("Show the process running in a panel.", "查看面板中的进程。")
            }
            HelpTopic::Command(CommandPath::PanelCreateTerminal) => (
                "Open another terminal in a panel.",
                "在面板中打开另一个终端。",
            ),
            HelpTopic::Command(CommandPath::PanelSplit) => {
                ("Split a panel and open a terminal.", "拆分面板并打开终端。")
            }
            HelpTopic::Command(CommandPath::PanelClose) => {
                ("Close a panel and its terminals.", "关闭面板及其终端。")
            }
            HelpTopic::Command(CommandPath::PanelMove) => (
                "Move a panel to another location.",
                "将面板移动到其他位置。",
            ),
            HelpTopic::Command(CommandPath::TerminalList) => {
                ("List available terminals.", "列出可用终端。")
            }
            HelpTopic::Command(CommandPath::TerminalGet) => {
                ("Show one terminal.", "查看一个终端。")
            }
            HelpTopic::Command(CommandPath::TerminalRead) => (
                "Read visible output or retained history.",
                "读取可见输出或保留的历史。",
            ),
            HelpTopic::Command(CommandPath::TerminalSelectionRead) => {
                ("Read the terminal's selected text.", "读取终端选中的文本。")
            }
            HelpTopic::Command(CommandPath::TerminalCopyMotion) => (
                "Move or extend the terminal selection.",
                "移动或扩展终端选区。",
            ),
            HelpTopic::Command(CommandPath::TerminalCopySearch) => (
                "Search output and move the selection.",
                "搜索输出并移动选区。",
            ),
            HelpTopic::Command(CommandPath::TerminalLinkResolve) => (
                "Resolve a link from terminal output.",
                "识别终端输出中的链接。",
            ),
            HelpTopic::Command(CommandPath::TerminalLinkActivate) => (
                "Open a link from terminal output.",
                "打开终端输出中的链接。",
            ),
            HelpTopic::Command(CommandPath::TerminalClear) => (
                "Clear output, keeping the cursor's row on Windows and its whole line elsewhere.",
                "清除输出；在 Windows 上保留光标所在的屏幕行，在其他平台上保留光标所在的整行。",
            ),
            HelpTopic::Command(CommandPath::TerminalEditScrollback) => (
                "Open terminal history in an editor.",
                "在编辑器中打开终端历史。",
            ),
            HelpTopic::Command(CommandPath::TerminalWaitOutput) => (
                "Wait for output matching text or a pattern.",
                "等待终端输出匹配文字或模式。",
            ),
            HelpTopic::Command(CommandPath::TerminalSend) => (
                "Send text or a key to a terminal.",
                "向终端发送文本或按键。",
            ),
            HelpTopic::Command(CommandPath::TerminalRun) => (
                "Paste one command and press Enter.",
                "粘贴一条命令并发送回车。",
            ),
            HelpTopic::Family(Family::Worktree) => (
                "Create and manage Git worktrees.",
                "创建和管理 Git 工作树。",
            ),
            HelpTopic::Command(CommandPath::WorktreeList) => (
                "List Git worktrees and their workspaces.",
                "列出 Git 工作树及其工作区。",
            ),
            HelpTopic::Command(CommandPath::WorktreeCreate) => (
                "Create a Git worktree and open its workspace.",
                "创建 Git 工作树并打开工作区。",
            ),
            HelpTopic::Command(CommandPath::WorktreeOpen) => (
                "Open a workspace for an existing Git worktree.",
                "为已有 Git 工作树打开工作区。",
            ),
            HelpTopic::Command(CommandPath::WorktreeRemove) => (
                "Remove a Git worktree after confirmation.",
                "确认后移除 Git 工作树。",
            ),
            HelpTopic::Command(CommandPath::WorktreeStatus) => (
                "Check a Git worktree operation.",
                "查看 Git 工作树操作状态。",
            ),
            HelpTopic::Command(CommandPath::WorktreeResume) => (
                "Resume an unfinished Git worktree operation.",
                "继续尚未完成的 Git 工作树操作。",
            ),
            HelpTopic::Family(Family::LaunchProfile) => (
                "Manage reusable terminal launch settings.",
                "管理可复用的终端启动配置。",
            ),
            HelpTopic::Command(CommandPath::LaunchProfileList) => (
                "List saved terminal launch profiles.",
                "列出已保存的终端启动配置。",
            ),
            HelpTopic::Command(CommandPath::LaunchProfileGet) => (
                "Show one terminal launch profile.",
                "查看一个终端启动配置。",
            ),
            HelpTopic::Command(CommandPath::LaunchProfileCreate) => (
                "Save a reusable terminal launch profile.",
                "保存可复用的终端启动配置。",
            ),
            HelpTopic::Command(CommandPath::LaunchProfileDelete) => (
                "Delete a saved terminal launch profile.",
                "删除已保存的终端启动配置。",
            ),
            HelpTopic::Command(CommandPath::ApiSnapshot) => (
                "Print a machine-readable workspace snapshot.",
                "输出可供程序读取的工作区快照。",
            ),
            HelpTopic::Command(CommandPath::ApiSchema) => (
                "Print an embedded API schema or its catalog.",
                "输出内嵌 API 定义或定义目录。",
            ),
            HelpTopic::Command(CommandPath::ConfigCheck) => (
                "Validate settings without starting a session.",
                "校验设置，不启动会话。",
            ),
            HelpTopic::Command(CommandPath::ConfigResetKeys) => {
                ("Restore default keyboard shortcuts.", "恢复默认快捷键。")
            }
            HelpTopic::Family(Family::Diagnostics) => (
                "Collect diagnostics to send when you ask for help.",
                "收集诊断信息，以便在求助时发送。",
            ),
            HelpTopic::Command(CommandPath::DiagnosticsCreate) => (
                "Write a diagnostic bundle to a file you choose.",
                "将诊断包写入你选择的文件。",
            ),
            HelpTopic::Command(CommandPath::ServerStart) => (
                "Ensure a local terminal session is running.",
                "确保本地终端会话已启动。",
            ),
            HelpTopic::Command(CommandPath::ServerConnect) => (
                "Relay a Client connection over standard input and output.",
                "通过标准输入和输出转发一条 Client 连接。",
            ),
            HelpTopic::Family(Family::Session) => (
                "Find and reconnect to named local sessions.",
                "查找和重新连接命名的本地会话。",
            ),
            HelpTopic::Command(CommandPath::SessionList) => (
                "List named local terminal sessions.",
                "列出命名的本地终端会话。",
            ),
            HelpTopic::Command(CommandPath::SessionAttach) => (
                "Open a named local terminal session.",
                "打开命名的本地终端会话。",
            ),
            HelpTopic::Command(CommandPath::SessionStop) => (
                "Stop a named local terminal session.",
                "停止命名的本地终端会话。",
            ),
            HelpTopic::Command(CommandPath::SessionDelete) => {
                ("Delete a stopped local session.", "删除已停止的本地会话。")
            }
            HelpTopic::Command(CommandPath::ServerDiagnose) => (
                "Inspect a local session without changing it.",
                "检查本地会话，不作修改。",
            ),
            HelpTopic::Command(CommandPath::ServerUpdateAgentManifests) => (
                "Download and apply Agent detection rules.",
                "下载并应用智能体识别规则。",
            ),
            HelpTopic::Command(CommandPath::ServerConfig) => (
                "Show the running session's configuration.",
                "显示运行中会话的配置。",
            ),
            HelpTopic::Command(CommandPath::ServerReloadConfig) => (
                "Reload the running session's configuration.",
                "重新加载会话配置。",
            ),
            HelpTopic::Family(Family::TerminalTitle) => (
                "Set or clear the foreground Client's window title.",
                "设置或清除前台客户端的窗口标题。",
            ),
            HelpTopic::Command(CommandPath::TerminalTitleSet) => (
                "Set the foreground Client's window title: the TUI's terminal window or the Desktop window.",
                "设置前台客户端的窗口标题：TUI 所在的终端窗口或桌面端窗口。",
            ),
            HelpTopic::Command(CommandPath::TerminalTitleClear) => (
                "Return the Client window title to the configured template.",
                "将客户端窗口标题恢复为配置的模板。",
            ),
            HelpTopic::Family(Family::Plugin) => (
                "Install, configure and use plugins.",
                "安装、配置和使用插件。",
            ),
            HelpTopic::Family(Family::Command) => (
                "Find and run your configured commands.",
                "查找并运行已配置的命令。",
            ),
            HelpTopic::Family(Family::Events) => (
                "Observe events from a running session.",
                "观察运行中会话的事件。",
            ),
            HelpTopic::Command(CommandPath::EventsSubscribe) => (
                "Stream events from a running session.",
                "持续读取运行中会话的事件。",
            ),
            HelpTopic::Command(CommandPath::EventsWait) => {
                ("Wait for a matching session event.", "等待匹配的会话事件。")
            }
            HelpTopic::Command(CommandPath::CommandList) => {
                ("List configured commands.", "列出已配置的命令。")
            }
            HelpTopic::Command(CommandPath::CommandInvoke) => {
                ("Run a configured command.", "运行已配置的命令。")
            }
            HelpTopic::Family(Family::PluginAction) => {
                ("Find and invoke plugin actions.", "查找并执行插件操作。")
            }
            HelpTopic::Family(Family::PluginLog) => ("Read plugin logs.", "读取插件日志。"),
            HelpTopic::Family(Family::PluginPane) => {
                ("Open and manage plugin panels.", "打开并管理插件面板。")
            }
            HelpTopic::Command(CommandPath::PluginPaneOpen) => {
                ("Open a plugin panel.", "打开插件面板。")
            }
            HelpTopic::Family(Family::Popup) => ("Manage open popups.", "管理已打开的弹出窗口。"),
            HelpTopic::Command(CommandPath::PopupClose) => {
                ("Close an open popup.", "关闭已打开的弹出窗口。")
            }
            HelpTopic::Command(CommandPath::PluginPaneFocus) => {
                ("Focus a plugin panel.", "聚焦插件面板。")
            }
            HelpTopic::Command(CommandPath::PluginPaneClose) => {
                ("Close a plugin panel.", "关闭插件面板。")
            }
            HelpTopic::Command(CommandPath::PluginActionInvoke) => {
                ("Run a plugin action.", "运行插件操作。")
            }
            HelpTopic::Command(CommandPath::PluginLogList) => {
                ("Read a plugin's logs.", "读取插件日志。")
            }
            HelpTopic::Command(CommandPath::PluginInstall) => ("Install a plugin.", "安装插件。"),
            HelpTopic::Command(CommandPath::PluginUninstall) => {
                ("Uninstall a plugin.", "卸载插件。")
            }
            HelpTopic::Command(CommandPath::PluginUpdate) => {
                ("Update an installed plugin.", "更新已安装的插件。")
            }
            HelpTopic::Command(CommandPath::PluginLink) => {
                ("Use a local plugin directory.", "使用本地插件目录。")
            }
            HelpTopic::Command(CommandPath::PluginUnlink) => {
                ("Unlink a local plugin directory.", "解除本地插件目录关联。")
            }
            HelpTopic::Command(CommandPath::PluginEnable) => {
                ("Enable an installed plugin.", "启用已安装的插件。")
            }
            HelpTopic::Command(CommandPath::PluginDisable) => {
                ("Disable an installed plugin.", "停用已安装的插件。")
            }
            HelpTopic::Command(CommandPath::PluginList) => {
                ("List installed plugins.", "列出已安装的插件。")
            }
            HelpTopic::Command(CommandPath::PluginConfigDir) => (
                "Show a plugin's configuration directory.",
                "显示插件的配置目录。",
            ),
            HelpTopic::Command(CommandPath::PluginActionList) => {
                ("List available plugin actions.", "列出可用的插件操作。")
            }
            HelpTopic::Family(Family::Notification) => (
                "Show notifications in the terminal interface.",
                "在终端界面显示通知。",
            ),
            HelpTopic::Command(CommandPath::NotificationShow) => (
                "Show a notification in the terminal interface.",
                "在终端界面显示通知。",
            ),
            HelpTopic::Command(CommandPath::ServerAgentManifests) => (
                "Show the active Agent detection rules.",
                "显示当前智能体识别规则。",
            ),
            HelpTopic::Command(CommandPath::ServerReloadAgentManifests) => (
                "Reload local Agent detection rules.",
                "重新加载本地智能体识别规则。",
            ),
            HelpTopic::Command(CommandPath::ServerAgentManifestsStatus) => (
                "Check an Agent detection rule update.",
                "查看智能体识别规则的更新状态。",
            ),
            HelpTopic::Command(CommandPath::ServerStop) => (
                "Stop the selected local session and its terminals; requires confirmation (--confirm outside release).",
                "停止所选本地会话及其终端；需要确认（正式版以外使用 --confirm）。",
            ),
            HelpTopic::Command(CommandPath::ServerStopStatus) => {
                ("Check a session stop request.", "查看会话停止请求的状态。")
            }
        };
        match self.locale {
            Locale::EnUs => english,
            Locale::ZhCn => chinese,
        }
    }
}

/// Shared help headings and navigation copy.
#[derive(Clone, Copy)]
pub(crate) enum HelpText {
    Usage,
    QuickStart,
    SessionStart,
    Commands,
    Options,
    Arguments,
    Notes,
    Examples,
    GlobalOptions,
    Terminals,
    Agents,
    Sessions,
    Setup,
    Introduction,
    OpenTerminal,
    NewWorkspace,
    ListAgents,
    ShowHelp,
    ShowVersion,
    ShowSkill,
    Session,
    Machine,
    DefaultConfig,
    Descend,
    FormatScope,
    Synopsis,
    ConfigLocation,
    Reference,
    Completion,
    CompletionExample,
    SessionRules,
    MachineRules,
}

impl super::CliText {
    pub(crate) const fn cli_help_text(self, text: HelpText) -> &'static str {
        let (english, chinese) = match text {
            HelpText::SessionRules => (
                "Windows: --session <name> selects a local session (default: default). Names use 1..64 ASCII letters, digits, dot, underscore or hyphen, neither start nor end with a dot, and are no Windows device name such as con or nul. Letter case does not tell sessions apart. Explicit --session overrides inherited routing and ORCHBEE_SESSION.",
                "Windows：--session <name> 选择本地会话，默认 default。名称为 1..64 个 ASCII 字母、数字、点、下划线或连字符，不以点开头或结尾，也不能是 con、nul 等 Windows 设备名。只差大小写的名称指同一会话。显式 --session 覆盖继承的路由和 ORCHBEE_SESSION。",
            ),
            HelpText::MachineRules => (
                "Use orchbee --machine <name-or-id> <command> for a saved remote machine. Duplicate names, disabled machines and connection kinds this version does not support are refused. --file/--stdin read locally; --cwd names a remote path. TUI and terminal streams are not routed. Removing or disabling a machine leaves remote sessions running.",
                "使用 orchbee --machine <name-or-id> <command> 选择已保存的远程机器。拒绝重名、已停用的机器和此版本不支持的连接方式。--file/--stdin 在本地读取，--cwd 指向远端路径。不转发 TUI 或终端流。移除或停用机器不会停止远端会话。",
            ),
            HelpText::Usage => ("Usage", "用法"),
            HelpText::QuickStart => ("Quick start", "快速开始"),
            HelpText::SessionStart => (
                "Start the default background session for CLI commands.",
                "启动默认后台会话，以使用 CLI 命令。",
            ),
            HelpText::Commands => ("Commands", "命令"),
            HelpText::Options => ("Options", "选项"),
            HelpText::Arguments => ("Arguments", "参数"),
            HelpText::Notes => ("Details and constraints", "说明与限制"),
            HelpText::Examples => ("Examples (PowerShell)", "示例（PowerShell）"),

            HelpText::GlobalOptions => ("Global options", "全局选项"),
            HelpText::Terminals => ("Terminals and layout", "终端与布局"),
            HelpText::Agents => ("Agents and workflows", "智能体与工作流程"),
            HelpText::Sessions => ("Sessions and connections", "会话与连接"),
            HelpText::Setup => ("Setup and integration", "设置与集成"),
            HelpText::Introduction => (
                "A terminal for running and coordinating AI Agents.",
                "运行和协作 AI 智能体的终端。",
            ),
            HelpText::OpenTerminal => ("Open the terminal interface.", "打开终端界面。"),
            HelpText::NewWorkspace => (
                "Create a workspace and its first terminal.",
                "创建工作区及其首个终端。",
            ),
            HelpText::ListAgents => ("See your active Agents.", "查看正在运行的智能体。"),
            HelpText::ShowHelp => ("Show this help and exit.", "显示本帮助后退出。"),
            HelpText::ShowVersion => ("Print the installed version.", "显示已安装的版本。"),
            HelpText::ShowSkill => ("Print the offline Agent guide.", "显示离线智能体使用指南。"),
            HelpText::Session => (
                "Select a local Orchbee session; default: default.",
                "选择本地 Orchbee 会话；默认 default。",
            ),
            HelpText::Machine => (
                "Select a saved SSH machine; place this option first, before the command.",
                "选择已保存的 SSH 机器；此选项必须放在命令之前的第一个位置。",
            ),
            HelpText::DefaultConfig => ("Print the default configuration.", "输出默认配置。"),
            HelpText::Descend => (
                "Use orchbee <command> -h or orchbee help <command> for details.",
                "使用 orchbee <command> -h 或 orchbee help <command> 查看详情。",
            ),
            HelpText::Synopsis => ("Synopsis", "语法概要"),
            HelpText::FormatScope => (
                "Use --format text|json where supported; see each command for its output form.",
                "支持时可使用 --format text|json；输出形式见具体命令帮助。",
            ),
            HelpText::ConfigLocation => (
                "Configuration: {config-file}. ORCHBEE_HOME moves the whole data folder; ORCHBEE_CONFIG_FILE moves only the configuration file. Use orchbee config check to inspect it and orchbee server diagnose for runtime facts.",
                "配置：{config-file}。ORCHBEE_HOME 移动整个数据文件夹；ORCHBEE_CONFIG_FILE 只移动配置文件。使用 orchbee config check 检查配置，使用 orchbee server diagnose 查看运行时事实。",
            ),
            HelpText::Reference => (
                "Offline guide: orchbee --skill. Command help: orchbee help <command>.",
                "离线指南：orchbee --skill。命令帮助：orchbee help <command>。",
            ),
            HelpText::Completion => (
                "Print a shell completion script to stdout. Nothing is installed and no user files are changed.",
                "向标准输出打印 Shell 补全脚本；不安装内容，不更改用户文件。",
            ),
            HelpText::CompletionExample => (
                "Print the PowerShell completion script.",
                "输出 PowerShell 补全脚本。",
            ),
        };
        match self.locale {
            Locale::EnUs => english,
            Locale::ZhCn => chinese,
        }
    }

    pub(crate) fn cli_help_next(self, path: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Use orchbee {path} <command> -h for arguments and examples."),
            Locale::ZhCn => format!("使用 orchbee {path} <command> -h 查看参数和示例。"),
        }
    }

    pub(crate) fn cli_help_alias(self, path: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Alias for {path}."),
            Locale::ZhCn => format!("等同于 {path}。"),
        }
    }
}
