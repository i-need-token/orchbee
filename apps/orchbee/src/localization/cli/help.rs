//! Localized purposes for the build-admitted help topics. Command words remain
//! the invocation grammar's responsibility; this file owns only user-facing copy.

use super::super::Locale;
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

impl Locale {
    /// Explain the purpose of one help topic without exposing its implementation.
    pub(crate) fn cli_help_summary(self, topic: HelpTopic) -> &'static str {
        let (english, chinese) = match topic {
            #[cfg(windows)]
            HelpTopic::Family(Family::PanelGraphics) => (
                "Display and inspect graphics in a panel.",
                "在面板中显示和查看图像。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PanelGraphicsSet) => {
                ("Display graphics in a panel.", "在面板中显示图像。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PanelGraphicsClear) => {
                ("Clear a panel's graphics.", "清除面板中的图像。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PanelGraphicsInfo) => {
                ("Show a panel's graphics state.", "查看面板图像状态。")
            }
            #[cfg(windows)]
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
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::LayoutExport) => (
                "Export a reusable terminal layout.",
                "导出可复用的终端布局。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::LayoutApply) => {
                ("Apply a saved terminal layout.", "应用已保存的终端布局。")
            }
            HelpTopic::Command(CommandPath::PanelZoom) => (
                "Toggle or set a panel's zoom state.",
                "切换或设置面板的放大状态。",
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
                "查找、启动并操作 AI Agent。",
            ),
            #[cfg(windows)]
            HelpTopic::Family(Family::AgentView) => (
                "Choose which Agents appear in the sidebar.",
                "选择侧边栏显示的 Agent。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::AgentViewGet) => {
                ("Show the Agent view settings.", "显示 Agent 视图设置。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::AgentViewSet) => (
                "Save Agent filters, grouping and sorting.",
                "保存 Agent 筛选、分组和排序设置。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::AgentViewClear) => {
                ("Restore the default Agent view.", "恢复默认 Agent 视图。")
            }
            HelpTopic::Command(CommandPath::AgentList) => {
                ("List observed Agents.", "列出已发现的 Agent。")
            }
            HelpTopic::Command(CommandPath::AgentChanges) => (
                "Read a page of Agent state changes.",
                "分页读取 Agent 状态变化。",
            ),
            HelpTopic::Command(CommandPath::AgentGet) => {
                ("Show one Agent and its state.", "查看一个 Agent 及其状态。")
            }
            HelpTopic::Command(CommandPath::AgentFocus) => (
                "Focus the terminal containing an Agent.",
                "聚焦 Agent 所在的终端。",
            ),
            HelpTopic::Command(CommandPath::AgentAttention) => (
                "Show an Agent's seen/unseen state in a client.",
                "查看客户端对 Agent 的已读／未读状态。",
            ),
            HelpTopic::Command(CommandPath::AgentRead) => (
                "Read an Agent's terminal output.",
                "读取 Agent 的终端输出。",
            ),
            HelpTopic::Command(CommandPath::AgentStart) => (
                "Start an Agent with the requested launch settings.",
                "按指定启动配置运行 Agent。",
            ),
            HelpTopic::Command(CommandPath::AgentPrompt) => {
                ("Submit a prompt to an Agent.", "向 Agent 提交提示词。")
            }
            HelpTopic::Command(CommandPath::AgentSendKeys) => {
                ("Send key presses to an Agent.", "向 Agent 发送按键。")
            }
            HelpTopic::Command(CommandPath::AgentExplain) => (
                "Explain how an Agent was identified.",
                "说明 Agent 的识别依据。",
            ),
            HelpTopic::Command(CommandPath::AgentWait) => (
                "Wait for an Agent to match a condition.",
                "等待 Agent 满足指定条件。",
            ),
            HelpTopic::Command(CommandPath::AgentDoctor) => {
                ("Check local Agent integrations.", "检查本地 Agent 集成。")
            }
            HelpTopic::Command(CommandPath::AgentNames) => (
                "List saved Agent session names.",
                "列出已保存的 Agent 会话名称。",
            ),
            HelpTopic::Command(CommandPath::AgentResolve) => (
                "Resolve an Agent name to its session.",
                "按名称查找 Agent 会话。",
            ),
            HelpTopic::Command(CommandPath::AgentRename) => {
                ("Give an Agent session a name.", "设置 Agent 会话名称。")
            }
            HelpTopic::Family(Family::AgentIntegration) => {
                ("Set up local Agent integrations.", "配置本地 Agent 集成。")
            }
            HelpTopic::Command(CommandPath::AgentIntegrationList) => (
                "List local Agent integration states.",
                "列出本地 Agent 集成状态。",
            ),
            HelpTopic::Command(CommandPath::AgentIntegrationExplain) => (
                "Explain an Agent integration's setup.",
                "说明 Agent 集成的配置情况。",
            ),
            HelpTopic::Command(CommandPath::AgentIntegrationEnable) => {
                ("Enable a local Agent integration.", "启用本地 Agent 集成。")
            }
            HelpTopic::Command(CommandPath::AgentIntegrationDisable) => (
                "Disable a local Agent integration.",
                "停用本地 Agent 集成。",
            ),
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
                "进入 Agent 所在的终端。",
            ),
            HelpTopic::Family(Family::TerminalSession) => (
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
            #[cfg(windows)]
            HelpTopic::Family(Family::Channel) => ("Choose the release channel.", "选择更新频道。"),
            #[cfg(windows)]
            HelpTopic::Family(Family::Machine) => (
                "Manage saved remote SSH machines.",
                "管理已保存的远程 SSH 机器。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::MachineList) => {
                ("List saved remote machines.", "列出已保存的远程机器。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::MachineAdd) => (
                "Save an SSH machine for remote commands.",
                "保存用于远程命令的 SSH 机器。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::MachineRename) => {
                ("Rename a saved remote machine.", "重命名已保存的远程机器。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::MachineRemove) => {
                ("Remove a saved remote machine.", "移除已保存的远程机器。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::MachineEnable) => {
                ("Enable a saved remote machine.", "启用已保存的远程机器。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::MachineDisable) => {
                ("Disable a saved remote machine.", "停用已保存的远程机器。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::ChannelShow) => {
                ("Show the configured release channel.", "显示当前更新频道。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::ChannelSet) => (
                "Save the release channel and update.",
                "保存更新频道并执行更新。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::Update) => {
                ("Install the configured release.", "安装所选频道的版本。")
            }
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
                "报告面板关联的 Agent。",
            ),
            HelpTopic::Command(CommandPath::PanelReportAgentSession) => (
                "Report a panel's Agent session.",
                "报告面板中的 Agent 会话。",
            ),
            HelpTopic::Command(CommandPath::PanelClearAgentAuthority) => (
                "Clear an Agent reporting assignment.",
                "清除 Agent 状态报告关联。",
            ),
            HelpTopic::Command(CommandPath::PanelReleaseAgent) => (
                "Release a panel's Agent association.",
                "解除面板的 Agent 关联。",
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
                "Send input to a panel's terminal.",
                "向面板中的终端发送输入。",
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
            #[cfg(windows)]
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
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::TerminalSelectionRead) => {
                ("Read the terminal's selected text.", "读取终端选中的文本。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::TerminalCopyMotion) => (
                "Move or extend the terminal selection.",
                "移动或扩展终端选区。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::TerminalCopySearch) => (
                "Search output and move the selection.",
                "搜索输出并移动选区。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::TerminalLinkResolve) => (
                "Resolve a link from terminal output.",
                "识别终端输出中的链接。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::TerminalLinkActivate) => (
                "Open a link from terminal output.",
                "打开终端输出中的链接。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::TerminalClear) => (
                "Clear output while preserving the current prompt.",
                "清除输出并保留当前提示行。",
            ),
            #[cfg(windows)]
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
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::ConfigImportHerdr) => {
                ("Import settings from H+DR.", "从 H+DR 导入设置。")
            }
            HelpTopic::Command(CommandPath::SiteStart) => (
                "Ensure a local terminal session is running.",
                "确保本地终端会话已启动。",
            ),
            #[cfg(windows)]
            HelpTopic::Family(Family::Session) => (
                "Find and reconnect to named local sessions.",
                "查找和重新连接命名的本地会话。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::SessionList) => (
                "List named local terminal sessions.",
                "列出命名的本地终端会话。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::SessionAttach) => (
                "Open a named local terminal session.",
                "打开命名的本地终端会话。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::SessionStop) => (
                "Stop a named local terminal session.",
                "停止命名的本地终端会话。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::SessionDelete) => {
                ("Delete a stopped local session.", "删除已停止的本地会话。")
            }
            HelpTopic::Command(CommandPath::SiteDiagnose) => (
                "Inspect a local session without changing it.",
                "检查本地会话，不作修改。",
            ),
            HelpTopic::Command(CommandPath::SiteUpdateAgentManifests) => (
                "Download and apply Agent detection rules.",
                "下载并应用 Agent 识别规则。",
            ),
            HelpTopic::Command(CommandPath::SiteConfig) => (
                "Show the running session's configuration.",
                "显示运行中会话的配置。",
            ),
            HelpTopic::Command(CommandPath::SiteReloadConfig) => (
                "Reload the running session's configuration.",
                "重新加载会话配置。",
            ),
            HelpTopic::Family(Family::TerminalTitle) => (
                "Set or clear the outer terminal window title.",
                "设置或清除外层终端窗口标题。",
            ),
            HelpTopic::Command(CommandPath::TerminalTitleSet) => (
                "Set the outer terminal window title.",
                "设置外层终端窗口标题。",
            ),
            HelpTopic::Command(CommandPath::TerminalTitleClear) => (
                "Restore the outer terminal's original title.",
                "恢复外层终端的原始标题。",
            ),
            #[cfg(windows)]
            HelpTopic::Family(Family::Plugin) => (
                "Install, configure and use plugins.",
                "安装、配置和使用插件。",
            ),
            #[cfg(windows)]
            HelpTopic::Family(Family::Command) => (
                "Find and run your configured commands.",
                "查找并运行已配置的命令。",
            ),
            #[cfg(windows)]
            HelpTopic::Family(Family::Events) => (
                "Observe events from a running session.",
                "观察运行中会话的事件。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::EventsSubscribe) => (
                "Stream events from a running session.",
                "持续读取运行中会话的事件。",
            ),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::EventsWait) => {
                ("Wait for a matching session event.", "等待匹配的会话事件。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::CommandList) => {
                ("List configured commands.", "列出已配置的命令。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::CommandInvoke) => {
                ("Run a configured command.", "运行已配置的命令。")
            }
            #[cfg(windows)]
            HelpTopic::Family(Family::PluginAction) => {
                ("Find and invoke plugin actions.", "查找并执行插件操作。")
            }
            #[cfg(windows)]
            HelpTopic::Family(Family::PluginLog) => ("Read plugin logs.", "读取插件日志。"),
            #[cfg(windows)]
            HelpTopic::Family(Family::PluginPane) => {
                ("Open and manage plugin panels.", "打开并管理插件面板。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginPaneOpen) => {
                ("Open a plugin panel.", "打开插件面板。")
            }
            #[cfg(windows)]
            HelpTopic::Family(Family::Popup) => ("Manage open popups.", "管理已打开的弹出窗口。"),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PopupClose) => {
                ("Close an open popup.", "关闭已打开的弹出窗口。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginPaneFocus) => {
                ("Focus a plugin panel.", "聚焦插件面板。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginPaneClose) => {
                ("Close a plugin panel.", "关闭插件面板。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginActionInvoke) => {
                ("Run a plugin action.", "运行插件操作。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginLogList) => {
                ("Read a plugin's logs.", "读取插件日志。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginInstall) => ("Install a plugin.", "安装插件。"),
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginUninstall) => {
                ("Uninstall a plugin.", "卸载插件。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginLink) => {
                ("Use a local plugin directory.", "使用本地插件目录。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginUnlink) => {
                ("Unlink a local plugin directory.", "解除本地插件目录关联。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginEnable) => {
                ("Enable an installed plugin.", "启用已安装的插件。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginDisable) => {
                ("Disable an installed plugin.", "停用已安装的插件。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginList) => {
                ("List installed plugins.", "列出已安装的插件。")
            }
            #[cfg(windows)]
            HelpTopic::Command(CommandPath::PluginConfigDir) => (
                "Show a plugin's configuration directory.",
                "显示插件的配置目录。",
            ),
            #[cfg(windows)]
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
            HelpTopic::Command(CommandPath::SiteAgentManifests) => (
                "Show the active Agent detection rules.",
                "显示当前 Agent 识别规则。",
            ),
            HelpTopic::Command(CommandPath::SiteReloadAgentManifests) => (
                "Reload local Agent detection rules.",
                "重新加载本地 Agent 识别规则。",
            ),
            HelpTopic::Command(CommandPath::SiteAgentManifestsStatus) => (
                "Check an Agent detection rule update.",
                "查看 Agent 识别规则的更新状态。",
            ),
            HelpTopic::Command(CommandPath::SiteStop) => (
                "Stop a local session and its terminals.",
                "停止本地会话及其终端。",
            ),
            HelpTopic::Command(CommandPath::SiteStopStatus) => {
                ("Check a session stop request.", "查看会话停止请求的状态。")
            }
        };
        match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
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
    Reference,
    Completion,
    CompletionExample,
    #[cfg(windows)]
    SessionRules,
    #[cfg(windows)]
    MachineRules,
}

impl Locale {
    pub(crate) const fn cli_help_text(self, text: HelpText) -> &'static str {
        let (english, chinese) = match text {
            #[cfg(windows)]
            HelpText::SessionRules => (
                "Windows: --session <name> selects a local session (default: default). Names use 1..64 ASCII letters, digits, dot, underscore or hyphen; . and .. are invalid. Explicit --session overrides inherited routing and ORCHBEE_SESSION.",
                "Windows：--session <name> 选择本地会话，默认 default。名称为 1..64 个 ASCII 字母、数字、点、下划线或连字符；不能是 . 或 ..。显式 --session 覆盖继承的路由和 ORCHBEE_SESSION。",
            ),
            #[cfg(windows)]
            HelpText::MachineRules => (
                "Use orchbee --machine <name-or-id> <command> for a saved SSH machine. Duplicate names and disabled profiles are refused. --file/--stdin read locally; --cwd names a remote path. TUI and terminal streams are not routed. Removing or disabling a profile leaves remote sessions running.",
                "使用 orchbee --machine <name-or-id> <command> 选择已保存的 SSH 机器。拒绝重名和已禁用配置。--file/--stdin 在本地读取，--cwd 指向远端路径。不转发 TUI 或终端流。移除或禁用配置不会停止远端会话。",
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
            HelpText::Examples if cfg!(windows) => ("Examples (PowerShell)", "示例（PowerShell）"),
            HelpText::Examples => ("Examples (POSIX shell)", "示例（POSIX Shell）"),
            HelpText::GlobalOptions => ("Global options", "全局选项"),
            HelpText::Terminals => ("Terminals and layout", "终端与布局"),
            HelpText::Agents => ("Agents and workflows", "Agent 与工作流程"),
            HelpText::Sessions => ("Sessions and connections", "会话与连接"),
            HelpText::Setup => ("Setup and integration", "设置与集成"),
            HelpText::Introduction => (
                "A terminal for running and coordinating AI Agents.",
                "运行和协作 AI Agent 的终端。",
            ),
            HelpText::OpenTerminal => ("Open the terminal interface.", "打开终端界面。"),
            HelpText::NewWorkspace => (
                "Create a workspace and its first terminal.",
                "创建工作区及其首个终端。",
            ),
            HelpText::ListAgents => ("See your active Agents.", "查看正在运行的 Agent。"),
            HelpText::ShowHelp => ("Show help for the current command.", "显示当前命令的帮助。"),
            HelpText::ShowVersion => ("Print the installed version.", "显示已安装的版本。"),
            HelpText::ShowSkill => (
                "Print the offline Agent guide.",
                "显示离线 Agent 使用指南。",
            ),
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
            HelpText::FormatScope => (
                "Use --format text|json where supported; see each command for its output form.",
                "支持时可使用 --format text|json；输出形式见具体命令帮助。",
            ),
            HelpText::Reference => (
                "Agent guide: orchbee --skill. Reference: apps/orchbee/docs/native-cli-query.md",
                "Agent 指南：orchbee --skill。参考：apps/orchbee/docs/native-cli-query.zh-CN.md",
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
        match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
        }
    }

    pub(crate) fn cli_help_next(self, path: &str) -> String {
        match self {
            Self::EnUs => format!("Use orchbee {path} <command> -h for arguments and examples."),
            Self::ZhCn => format!("使用 orchbee {path} <command> -h 查看参数和示例。"),
        }
    }

    pub(crate) fn cli_help_alias(self, path: &str) -> String {
        match self {
            Self::EnUs => format!("Alias for {path}."),
            Self::ZhCn => format!("等同于 {path}。"),
        }
    }
}
