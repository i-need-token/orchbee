//! Selects localized command details and owns top-level help copy; invocation owns grammar.

use super::Locale;
use crate::cli_text::CliText;

const HELP_TUI_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tui [--mouse on|off]",
    "Open the terminal interface on Windows or Linux. Running orchbee without a command does the same. Mouse capture defaults to on; --mouse is accepted on this explicit tui route.",
);

const HELP_TUI_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tui [--mouse on|off]",
    "在 Windows 或 Linux 上打开终端界面。不带命令运行 orchbee 也会打开该界面。鼠标捕获默认开启；显式 tui 路由支持 --mouse 选项。",
);

const HELP_STATUS_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee status [--format text|json]",
    "Read the running session's reachability and resource counts. This command does not create or start a session. Use status server for service compatibility details, or status client to inspect this executable offline.",
);

const HELP_STATUS_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee status [--format text|json]",
    "读取正在运行的会话的可达性和资源数量。本命令不创建或启动会话。使用 status server 查看服务兼容性，或使用 status client 离线检查当前程序。",
);

const HELP_API_SNAPSHOT_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee api snapshot [--format text|json]",
    "Emit the complete validated workbench.snapshot document inside the\norchbee.api.snapshot envelope. Successful output is JSON even under\n'--format text'; a failure envelope still requires '--format json'.",
);

const HELP_API_SNAPSHOT_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee api snapshot [--format text|json]",
    "在 orchbee.api.snapshot 封套内输出完整已验证 workbench.snapshot 文档。成功输出即使 '--format text' 也为 JSON；失败封套仍需显式 '--format json'。",
);

const HELP_API_SCHEMA_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]",
    "Without <schema-id>, list every embedded schema ID with its versions and title.\nWith <schema-id>, emit that JSON Schema; --format json or --output emits the\ncomplete dictionary. Schema documents are always JSON; unknown IDs are\nnot-found, and failure envelopes require '--format json'.\nOn Windows, --output writes identical JSON to a literal file path, replacing\nan existing regular file only after complete preparation. Parent directories\nmust exist. The command stays offline and also emits the JSON on stdout.",
);

const HELP_API_SCHEMA_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]",
    "不带 <schema-id> 时列出每个内嵌 schema 的 ID、版本和标题；带 <schema-id> 时输出该 JSON Schema；--format json 或 --output 输出完整字典。schema 文档始终为 JSON；未知 ID 为 not-found，失败封套需显式 '--format json'。\nWindows 的 --output 将相同 JSON 写入字面文件路径，完整准备后才替换已有普通文件。父目录必须存在；全程离线，stdout 同时输出该 JSON。",
);

impl super::CliText {
    // ---- help topics ---------------------------------------------------------
    pub(crate) fn cli_command_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::cli_text::HelpDetails {
        use crate::invocation::CommandPath;
        match (self.locale, path) {
            (_, CommandPath::WorkspaceStatus) => {
                self.operation_help("orchbee workspace status <request-id> [--format text|json]", false)
            }
            (_, CommandPath::WorkspaceResume) => self.operation_help(
                "orchbee workspace resume <request-id> --generation <n> --confirm [--format text|json]",
                true,
            ),
            (_, CommandPath::TabStatus) => {
                self.operation_help("orchbee tab status <request-id> [--format text|json]", false)
            }
            (_, CommandPath::TabResume) => self.operation_help(
                "orchbee tab resume <request-id> --generation <n> --confirm [--format text|json]",
                true,
            ),
            (_, CommandPath::PanelStatus) => {
                self.operation_help("orchbee panel status <request-id> [--format text|json]", false)
            }
            (_, CommandPath::PanelResume) => self.operation_help(
                "orchbee panel resume <request-id> --generation <n> --confirm [--format text|json]",
                true,
            ),
            (_, CommandPath::ProjectStatus) => {
                self.operation_help("orchbee project status <request-id> [--format text|json]", false)
            }
            (_, CommandPath::ProjectResume) => self.operation_help(
                "orchbee project resume <request-id> --generation <n> --confirm [--detach] [--timeout <ms>] [--format text|json]",
                true,
            ),
            (_, CommandPath::ClientStatus) => {
                self.operation_help("orchbee client status <request-id> [--format text|json]", false)
            }
            (_, CommandPath::ServerStatus) => {
                self.operation_help("orchbee server status <request-id> [--format text|json]", false)
            }

            (_, CommandPath::EventsSubscribe | CommandPath::EventsWait) => self.events_help(path),
            (
                _,
                CommandPath::PanelGraphicsSet
                | CommandPath::PanelGraphicsClear
                | CommandPath::PanelGraphicsInfo
                | CommandPath::PanelGraphicsStream,
            ) => self.pane_graphics_help(path),
            (_, CommandPath::CommandList | CommandPath::CommandInvoke) => {
                self.configured_command_help(path)
            }
            (_, CommandPath::TerminalTitleSet | CommandPath::TerminalTitleClear) => {
                self.window_title_help(path)
            }
            (
                _,
                CommandPath::PluginLink
                | CommandPath::PluginUnlink
                | CommandPath::PluginEnable
                | CommandPath::PluginDisable
                | CommandPath::PluginList
                | CommandPath::PluginConfigDir
                | CommandPath::PluginActionList
                | CommandPath::PluginActionInvoke
                | CommandPath::PluginPaneFocus
                | CommandPath::PluginPaneClose
                | CommandPath::PopupClose
                | CommandPath::PluginPaneOpen
                | CommandPath::PluginLogList
                | CommandPath::PluginInstall
                | CommandPath::PluginUninstall
                | CommandPath::PluginUpdate,
            ) => self.plugin_help(path),
            (_, CommandPath::NotificationShow) => self.notification_help(),
            (_, CommandPath::ConfigReload) => {
                self.runtime_configuration_help(path)
            }
            (Locale::EnUs, CommandPath::ClientList) => super::client_control::HELP_CLIENT_LIST_EN,
            (Locale::ZhCn, CommandPath::ClientList) => super::client_control::HELP_CLIENT_LIST_ZH,
            (Locale::EnUs, CommandPath::ClientGet) => super::client_control::HELP_CLIENT_GET_EN,
            (Locale::ZhCn, CommandPath::ClientGet) => super::client_control::HELP_CLIENT_GET_ZH,
            (Locale::EnUs, CommandPath::WorkspaceFocus) => super::client_control::HELP_WORKSPACE_FOCUS_EN,
            (Locale::ZhCn, CommandPath::WorkspaceFocus) => super::client_control::HELP_WORKSPACE_FOCUS_ZH,
            (Locale::EnUs, CommandPath::TabFocus) => super::client_control::HELP_TAB_FOCUS_EN,
            (Locale::ZhCn, CommandPath::TabFocus) => super::client_control::HELP_TAB_FOCUS_ZH,
            (Locale::EnUs, CommandPath::PanelResize) => super::client_control::HELP_PANEL_RESIZE_EN,
            (Locale::ZhCn, CommandPath::PanelResize) => super::client_control::HELP_PANEL_RESIZE_ZH,
            (Locale::EnUs, CommandPath::LayoutSetSplitRatio) => super::client_control::HELP_LAYOUT_RATIO_EN,
            (Locale::EnUs, CommandPath::LayoutExport) => super::layouts::EXPORT_EN,
            (Locale::EnUs, CommandPath::LayoutApply) => super::layouts::APPLY_EN,
            (Locale::ZhCn, CommandPath::LayoutApply) => super::layouts::APPLY_ZH,
            (Locale::ZhCn, CommandPath::LayoutExport) => super::layouts::EXPORT_ZH,
            (Locale::ZhCn, CommandPath::LayoutSetSplitRatio) => super::client_control::HELP_LAYOUT_RATIO_ZH,
            (Locale::EnUs, CommandPath::PanelFocus) => super::client_control::HELP_PANEL_FOCUS_EN,
            (Locale::ZhCn, CommandPath::PanelFocus) => super::client_control::HELP_PANEL_FOCUS_ZH,
            (Locale::EnUs, CommandPath::PanelLayout) => super::client_layout::HELP_LAYOUT_EN,
            (Locale::ZhCn, CommandPath::PanelLayout) => super::client_layout::HELP_LAYOUT_ZH,
            (Locale::EnUs, CommandPath::PanelNeighbor) => super::client_layout::HELP_NEIGHBOR_EN,
            (Locale::ZhCn, CommandPath::PanelNeighbor) => super::client_layout::HELP_NEIGHBOR_ZH,
            (Locale::EnUs, CommandPath::PanelEdges) => super::client_layout::HELP_EDGES_EN,
            (Locale::ZhCn, CommandPath::PanelEdges) => super::client_layout::HELP_EDGES_ZH,
            (Locale::EnUs, CommandPath::PanelMaximize) => super::client_control::HELP_PANEL_MAXIMIZE_EN,
            (Locale::ZhCn, CommandPath::PanelMaximize) => super::client_control::HELP_PANEL_MAXIMIZE_ZH,

            (Locale::EnUs, CommandPath::Tui) => HELP_TUI_EN,
            (Locale::ZhCn, CommandPath::Tui) => HELP_TUI_ZH,
            (_, CommandPath::StatusClient) => super::runtime_status::help(self.locale, false),
            (_, CommandPath::StatusServer) => super::runtime_status::help(self.locale, true),
            (Locale::EnUs, CommandPath::Status) => HELP_STATUS_EN,
            (Locale::ZhCn, CommandPath::Status) => HELP_STATUS_ZH,
            (Locale::EnUs, CommandPath::ProjectList) => super::project::HELP_PROJECT_LIST_EN,
            (Locale::ZhCn, CommandPath::ProjectList) => super::project::HELP_PROJECT_LIST_ZH,
            (Locale::EnUs, CommandPath::ProjectGet) => super::project::HELP_PROJECT_GET_EN,
            (Locale::ZhCn, CommandPath::ProjectGet) => super::project::HELP_PROJECT_GET_ZH,
            (_, CommandPath::ProjectDissolve) => super::organization::Help::ProjectDissolve.text(self.locale),
            (_, CommandPath::ProjectCloseAll) => super::organization::Help::ProjectCloseAll.text(self.locale),
            (_, CommandPath::WorkspaceSetProject) => {
                super::organization::Help::WorkspaceSetProject.text(self.locale)
            }
            (Locale::EnUs, CommandPath::ProjectCreate) => super::project::HELP_PROJECT_CREATE_EN,
            (Locale::ZhCn, CommandPath::ProjectCreate) => super::project::HELP_PROJECT_CREATE_ZH,
            (Locale::EnUs, CommandPath::ProjectRename) => super::project::HELP_PROJECT_RENAME_EN,
            (Locale::ZhCn, CommandPath::ProjectRename) => super::project::HELP_PROJECT_RENAME_ZH,
            (Locale::EnUs, CommandPath::WorkspaceList) => super::workspace::HELP_WORKSPACE_LIST_EN,
            (Locale::ZhCn, CommandPath::WorkspaceList) => super::workspace::HELP_WORKSPACE_LIST_ZH,
            (Locale::EnUs, CommandPath::WorkspaceGet) => super::workspace::HELP_WORKSPACE_GET_EN,
            (Locale::ZhCn, CommandPath::WorkspaceGet) => super::workspace::HELP_WORKSPACE_GET_ZH,
            (Locale::EnUs, CommandPath::WorkspaceCreate) => super::workspace::HELP_WORKSPACE_CREATE_EN,
            (Locale::ZhCn, CommandPath::WorkspaceCreate) => super::workspace::HELP_WORKSPACE_CREATE_ZH,
            (Locale::EnUs, CommandPath::WorkspaceMove) => super::workspace::HELP_WORKSPACE_MOVE_EN,
            (Locale::ZhCn, CommandPath::WorkspaceMove) => super::workspace::HELP_WORKSPACE_MOVE_ZH,
            (Locale::EnUs, CommandPath::PanelReportAgent) => super::panel_agent::PANELREPORTAGENT_EN,
            (Locale::ZhCn, CommandPath::PanelReportAgent) => super::panel_agent::PANELREPORTAGENT_ZH,
            (Locale::EnUs, CommandPath::PanelClearAgentAuthority) => {
                super::panel_agent::PANELCLEARAGENTAUTHORITY_EN
            }
            (Locale::ZhCn, CommandPath::PanelClearAgentAuthority) => {
                super::panel_agent::PANELCLEARAGENTAUTHORITY_ZH
            }
            (Locale::EnUs, CommandPath::PanelReleaseAgent) => super::panel_agent::PANELRELEASEAGENT_EN,
            (Locale::ZhCn, CommandPath::PanelReleaseAgent) => super::panel_agent::PANELRELEASEAGENT_ZH,
            (Locale::EnUs, CommandPath::PanelReportMetadata) => super::panel_metadata::HELP_EN,
            (Locale::ZhCn, CommandPath::PanelReportMetadata) => super::panel_metadata::HELP_ZH,
            (Locale::EnUs, CommandPath::WorkspaceReportMetadata) => super::workspace_metadata::HELP_EN,
            (Locale::ZhCn, CommandPath::WorkspaceReportMetadata) => super::workspace_metadata::HELP_ZH,
            (Locale::EnUs, CommandPath::WorkspaceRename) => super::workspace::HELP_WORKSPACE_RENAME_EN,
            (Locale::ZhCn, CommandPath::WorkspaceRename) => super::workspace::HELP_WORKSPACE_RENAME_ZH,
            (Locale::EnUs, CommandPath::WorkspaceAcceptDirectory) => {
                super::workspace::HELP_WORKSPACE_ACCEPT_DIRECTORY_EN
            }
            (Locale::ZhCn, CommandPath::WorkspaceAcceptDirectory) => {
                super::workspace::HELP_WORKSPACE_ACCEPT_DIRECTORY_ZH
            }
            (Locale::EnUs, CommandPath::WorkspaceClose) => super::workspace::HELP_WORKSPACE_CLOSE_EN,
            (Locale::ZhCn, CommandPath::WorkspaceClose) => super::workspace::HELP_WORKSPACE_CLOSE_ZH,
            (Locale::EnUs, CommandPath::TabList) => super::tab::HELP_TAB_LIST_EN,
            (Locale::ZhCn, CommandPath::TabList) => super::tab::HELP_TAB_LIST_ZH,
            (Locale::EnUs, CommandPath::TabGet) => super::tab::HELP_TAB_GET_EN,
            (Locale::ZhCn, CommandPath::TabGet) => super::tab::HELP_TAB_GET_ZH,
            (Locale::EnUs, CommandPath::TabCreate) => super::tab::HELP_TAB_CREATE_EN,
            (Locale::ZhCn, CommandPath::TabCreate) => super::tab::HELP_TAB_CREATE_ZH,
            (Locale::EnUs, CommandPath::PanelSwap) => super::panel::HELP_PANEL_SWAP_EN,
            (Locale::ZhCn, CommandPath::PanelSwap) => super::panel::HELP_PANEL_SWAP_ZH,
            (Locale::EnUs, CommandPath::PanelCreateTerminal) => super::panel_terminal_creation::HELP_EN,
            (Locale::ZhCn, CommandPath::PanelCreateTerminal) => super::panel_terminal_creation::HELP_ZH,
            (Locale::EnUs, CommandPath::PanelInput) => super::panel::HELP_PANEL_INPUT_EN,
            (Locale::ZhCn, CommandPath::PanelInput) => super::panel::HELP_PANEL_INPUT_ZH,
            (Locale::EnUs, CommandPath::PanelRename) => super::panel::HELP_PANEL_RENAME_EN,
            (Locale::ZhCn, CommandPath::PanelRename) => super::panel::HELP_PANEL_RENAME_ZH,
            (Locale::EnUs, CommandPath::TabRename) => super::tab::HELP_TAB_RENAME_EN,
            (Locale::ZhCn, CommandPath::TabRename) => super::tab::HELP_TAB_RENAME_ZH,
            (Locale::EnUs, CommandPath::TabMove) => super::tab::HELP_TAB_MOVE_EN,
            (Locale::ZhCn, CommandPath::TabMove) => super::tab::HELP_TAB_MOVE_ZH,
            (Locale::EnUs, CommandPath::TabClose) => super::tab::HELP_TAB_CLOSE_EN,
            (Locale::ZhCn, CommandPath::TabClose) => super::tab::HELP_TAB_CLOSE_ZH,
            (Locale::EnUs, CommandPath::PanelList) => super::panel::HELP_PANEL_LIST_EN,
            (Locale::ZhCn, CommandPath::PanelList) => super::panel::HELP_PANEL_LIST_ZH,
            (Locale::EnUs, CommandPath::PanelCurrent) => super::panel::HELP_PANEL_CURRENT_EN,
            (Locale::ZhCn, CommandPath::PanelCurrent) => super::panel::HELP_PANEL_CURRENT_ZH,
            (Locale::EnUs, CommandPath::PanelGet) => super::panel::HELP_PANEL_GET_EN,
            (Locale::ZhCn, CommandPath::PanelGet) => super::panel::HELP_PANEL_GET_ZH,
            (Locale::EnUs, CommandPath::PanelProcessInfo) => super::panel::HELP_PANEL_PROCESS_INFO_EN,
            (Locale::ZhCn, CommandPath::PanelProcessInfo) => super::panel::HELP_PANEL_PROCESS_INFO_ZH,
            (Locale::EnUs, CommandPath::PanelSplit) => super::panel::HELP_PANEL_SPLIT_EN,
            (Locale::ZhCn, CommandPath::PanelSplit) => super::panel::HELP_PANEL_SPLIT_ZH,
            (Locale::EnUs, CommandPath::PanelClose) => super::panel::HELP_PANEL_CLOSE_EN,
            (Locale::ZhCn, CommandPath::PanelClose) => super::panel::HELP_PANEL_CLOSE_ZH,
            (Locale::EnUs, CommandPath::PanelMove) => super::panel::HELP_PANEL_MOVE_EN,
            (Locale::ZhCn, CommandPath::PanelMove) => super::panel::HELP_PANEL_MOVE_ZH,
            (Locale::EnUs, CommandPath::TerminalList) => super::terminal::HELP_TERMINAL_LIST_EN,
            (Locale::ZhCn, CommandPath::TerminalList) => super::terminal::HELP_TERMINAL_LIST_ZH,
            (Locale::EnUs, CommandPath::TerminalGet) => super::terminal::HELP_TERMINAL_GET_EN,
            (Locale::ZhCn, CommandPath::TerminalGet) => super::terminal::HELP_TERMINAL_GET_ZH,
            (Locale::EnUs, CommandPath::TerminalAttach) => super::terminal::HELP_TERMINAL_ATTACH_EN,
            (Locale::ZhCn, CommandPath::TerminalAttach) => super::terminal::HELP_TERMINAL_ATTACH_ZH,
            (Locale::EnUs, CommandPath::AgentAttach) => super::agent::HELP_AGENT_ATTACH_EN,
            (Locale::ZhCn, CommandPath::AgentAttach) => super::agent::HELP_AGENT_ATTACH_ZH,
            (Locale::EnUs, CommandPath::TerminalControl) => super::terminal::HELP_TERMINAL_CONTROL_EN,
            (Locale::ZhCn, CommandPath::TerminalControl) => super::terminal::HELP_TERMINAL_CONTROL_ZH,
            (Locale::EnUs, CommandPath::TerminalObserve) => super::terminal::HELP_TERMINAL_OBSERVE_EN,
            (Locale::ZhCn, CommandPath::TerminalObserve) => super::terminal::HELP_TERMINAL_OBSERVE_ZH,
            (Locale::EnUs, CommandPath::TerminalRead) => super::terminal::HELP_TERMINAL_READ_EN,
            (Locale::ZhCn, CommandPath::TerminalRead) => super::terminal::HELP_TERMINAL_READ_ZH,
            (Locale::EnUs, CommandPath::LayoutHistory) => super::layouts::HISTORY_EN,
            (Locale::ZhCn, CommandPath::LayoutHistory) => super::layouts::HISTORY_ZH,
            (Locale::EnUs, CommandPath::LayoutReopen) => super::layouts::REOPEN_EN,
            (Locale::ZhCn, CommandPath::LayoutReopen) => super::layouts::REOPEN_ZH,
            (Locale::EnUs, CommandPath::TerminalClear) => super::terminal_clear::HELP_EN,
            (Locale::ZhCn, CommandPath::TerminalClear) => super::terminal_clear::HELP_ZH,

            (Locale::EnUs, CommandPath::TerminalWaitOutput) => super::terminal::HELP_TERMINAL_WAIT_OUTPUT_EN,
            (Locale::ZhCn, CommandPath::TerminalWaitOutput) => super::terminal::HELP_TERMINAL_WAIT_OUTPUT_ZH,
            (Locale::EnUs, CommandPath::TerminalRun) => super::terminal::HELP_TERMINAL_RUN_EN,
            (Locale::ZhCn, CommandPath::TerminalRun) => super::terminal::HELP_TERMINAL_RUN_ZH,
            (Locale::EnUs, CommandPath::TerminalSend) => super::terminal::HELP_TERMINAL_SEND_EN,
            (Locale::ZhCn, CommandPath::TerminalSend) => super::terminal::HELP_TERMINAL_SEND_ZH,
            (Locale::EnUs, CommandPath::WorktreeList) => super::worktree::HELP_WORKTREE_LIST_EN,
            (Locale::ZhCn, CommandPath::WorktreeList) => super::worktree::HELP_WORKTREE_LIST_ZH,
            (
                _,
                path @ (CommandPath::WorktreeCreate
                | CommandPath::WorktreeOpen
                | CommandPath::WorktreeRemove
                | CommandPath::WorktreeStatus
                | CommandPath::WorktreeResume),
            ) => self.cli_worktree_help(path),
            (Locale::EnUs, CommandPath::LaunchProfileList) => super::launch_profile::HELP_LAUNCH_PROFILE_LIST_EN,
            (Locale::ZhCn, CommandPath::LaunchProfileList) => super::launch_profile::HELP_LAUNCH_PROFILE_LIST_ZH,
            (Locale::EnUs, CommandPath::LaunchProfileGet) => super::launch_profile::HELP_LAUNCH_PROFILE_GET_EN,
            (Locale::ZhCn, CommandPath::LaunchProfileGet) => super::launch_profile::HELP_LAUNCH_PROFILE_GET_ZH,
            (Locale::EnUs, CommandPath::LaunchProfileCreate) => super::launch_profile::HELP_LAUNCH_PROFILE_CREATE_EN,
            (Locale::ZhCn, CommandPath::LaunchProfileCreate) => super::launch_profile::HELP_LAUNCH_PROFILE_CREATE_ZH,
            (Locale::EnUs, CommandPath::LaunchProfileDelete) => super::launch_profile::HELP_LAUNCH_PROFILE_DELETE_EN,
            (Locale::ZhCn, CommandPath::LaunchProfileDelete) => super::launch_profile::HELP_LAUNCH_PROFILE_DELETE_ZH,
            (Locale::EnUs, CommandPath::AgentChanges) => crate::cli_text::HelpDetails::new(
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]",
                "Windows: read one bounded Agent change page (limit 1-100, default 50).\nOmit cursor to establish a current-head checkpoint, then reuse its cursor.\nreset-required means history was lost; rebuild your baseline. A previous\nsession boot or invalid cursor is rejected. No subscription or automatic retry.\nTotal read deadline: 5 seconds. JSON schema: orchbee.agent.changes.",
            ),
            (Locale::ZhCn, CommandPath::AgentChanges) => crate::cli_text::HelpDetails::new(
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]",
                "Windows：读取一页有界智能体变更（limit 1-100，默认 50）。省略 cursor 建立当前头部检查点，再使用其 cursor 继续观察。\nreset-required 表示历史丢失，必须重建基线；先前会话启动实例或无效\ncursor 会被拒绝。不订阅、不自动重试。总读取期限 5 秒。\nJSON schema：orchbee.agent.changes。",
            ),
            (
                locale,
                CommandPath::AgentViewGet | CommandPath::AgentViewSet | CommandPath::AgentViewClear,
            ) => CliText::new(locale).agent_view_help(path),
            (Locale::EnUs, CommandPath::AgentList) => super::agent::HELP_AGENT_LIST_EN,
            (Locale::ZhCn, CommandPath::AgentList) => super::agent::HELP_AGENT_LIST_ZH,
            (Locale::EnUs, CommandPath::AgentStart) => crate::cli_text::HelpDetails::new(
                "orchbee agent start <provider> [--new-tab|--here|--split right|down] [--name <name>] [--panel <id|@panel:0x<hex>>|--current] [--timeout <ms>] [--format text|json] [--arg <value>]... [-- <args>...]",
                "Windows: start an interactive Agent in a new Tab (the default), in a new Panel split right or\ndown (--split), or at the prompt of an existing idle terminal (--here). The reference Panel is\n--panel, --current or the caller's own Panel: a new Tab joins its Workspace, a split divides it,\nand --here types into it; --here is refused for the caller's own Panel.\nWith no arguments and no --name, a new Tab or split runs the Agent as its shell's first command\nand returns once the Panel exists. Otherwise the command is typed once at the confirmed prompt.\nWithout --name it returns after a 3-second check: a shell back at its prompt reports the command\nnot found or the Agent exited. With --name it also waits until the Agent is ready: an enabled\nintegration's session confirms it and receives the name; otherwise the Agent seen waiting\nconfirms it, and the name stays unbound. A new Tab or Panel stays when a later step fails.\nTimeout: 3001–300000 ms, default 30000. Use --arg=<value> repeatedly as an alternative to\ntrailing arguments. Cancellation or timeout releases a name when confirmed; it never kills the Agent.\n<provider> is the id integration and doctor take with --provider, for example antigravity.",
            ),
            (Locale::ZhCn, CommandPath::AgentStart) => crate::cli_text::HelpDetails::new(
                "orchbee agent start <provider> [--new-tab|--here|--split right|down] [--name <name>] [--panel <id|@panel:0x<hex>>|--current] [--timeout <ms>] [--format text|json] [--arg <value>]... [-- <args>...]",
                "Windows：在新标签页（默认）、向右或向下拆分出的新面板（--split）中启动交互式智能体，或在现有空闲终端的提示符处启动（--here）。参照面板为 --panel、--current 或调用者自己的面板：新标签页加入其工作区，拆分会分割它，--here 在其中输入；--here 不能用于调用者自己的面板。不带参数也不带 --name 时，新标签页或拆分面板的 shell 把智能体作为第一条命令运行，面板出现后即返回。否则在已确认的提示符处输入一次启动命令。不带 --name 时，命令在 3 秒检查后返回：shell 回到提示符则报告找不到命令或智能体已退出。带 --name 时还会等待智能体就绪：已启用集成时，由其会话确认就绪并获得名称；否则以终端中观察到智能体等待输入作为就绪，名称不绑定。之后的步骤失败时，新标签页或面板会保留。超时范围 3001–300000 毫秒，默认 30000。可重复使用 --arg=<value> 代替尾随参数。取消或超时在确认后释放名称，但不会终止智能体。\n<provider> 与 integration 和 doctor 的 --provider 取值相同，例如 antigravity。",
            ),
            (Locale::EnUs, CommandPath::AgentPrompt) => super::agent::HELP_AGENT_PROMPT_WINDOWS_EN,
            (Locale::ZhCn, CommandPath::AgentPrompt) => super::agent::HELP_AGENT_PROMPT_WINDOWS_ZH,
            (Locale::EnUs, CommandPath::AgentRead) => super::agent::HELP_AGENT_OUTPUT_EN,
            (Locale::ZhCn, CommandPath::AgentRead) => super::agent::HELP_AGENT_OUTPUT_ZH,
            (Locale::EnUs, CommandPath::AgentSendKeys) => {
                super::agent::HELP_AGENT_SEND_KEYS_WINDOWS_EN
            }
            (Locale::ZhCn, CommandPath::AgentSendKeys) => {
                super::agent::HELP_AGENT_SEND_KEYS_WINDOWS_ZH
            }
            (Locale::EnUs, CommandPath::AgentAttention) => super::client_control::HELP_AGENT_ATTENTION_EN,
            (Locale::ZhCn, CommandPath::AgentAttention) => super::client_control::HELP_AGENT_ATTENTION_ZH,
            (Locale::EnUs, CommandPath::AgentFocus) => super::client_control::HELP_AGENT_FOCUS_EN,
            (Locale::ZhCn, CommandPath::AgentFocus) => super::client_control::HELP_AGENT_FOCUS_ZH,
            (Locale::EnUs, CommandPath::AgentGet) => super::agent::HELP_AGENT_GET_EN,
            (Locale::ZhCn, CommandPath::AgentGet) => super::agent::HELP_AGENT_GET_ZH,
            (Locale::EnUs, CommandPath::AgentExplain) => super::agent::HELP_AGENT_EXPLAIN_EN,
            (Locale::ZhCn, CommandPath::AgentExplain) => super::agent::HELP_AGENT_EXPLAIN_ZH,
            (Locale::EnUs, CommandPath::AgentNames) => crate::cli_text::HelpDetails::new(
                "orchbee agent names [--format text|json]",
                "List this session runtime's live Agent names. Names disappear when their session ends or the session stops.",
            ),
            (Locale::ZhCn, CommandPath::AgentNames) => crate::cli_text::HelpDetails::new(
                "orchbee agent names [--format text|json]",
                "列出当前会话运行时中的智能体名称。会话结束或会话停止后名称会释放。",
            ),
            (Locale::EnUs, CommandPath::AgentResolve) => crate::cli_text::HelpDetails::new(
                "orchbee agent resolve <target> [--format text|json]\n       orchbee agent resolve --agent-session <id> | --name <alias> | --panel <id|@panel:0x<hex>> | --terminal <id|@terminal:0x<hex>>",
                "Resolve an exact Session ID first, then a runtime name, then a Panel ID.\nA Panel selects its primary terminal and requires one live Agent. Use --name to select an alias that collides with an ID.",
            ),
            (Locale::ZhCn, CommandPath::AgentResolve) => crate::cli_text::HelpDetails::new(
                "orchbee agent resolve <target> [--format text|json]\n      orchbee agent resolve --agent-session <id> | --name <alias> | --panel <id|@panel:0x<hex>> | --terminal <id|@terminal:0x<hex>>",
                "优先按完整会话 ID 定位，再查运行时名称，最后回退到面板 ID。\nPanel 选择主终端，要求唯一存活智能体。名称与 ID 冲突时，用 --name 明确选择名称。",
            ),
            (Locale::EnUs, CommandPath::AgentRename) => crate::cli_text::HelpDetails::new(
                "orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <new-name> [--format text|json]\n       orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) --clear",
                "Assign or atomically rename one live Agent. Name: [a-z][a-z0-9_-]{0,31}. Duplicate names are rejected. --clear releases only the name. Replace <target> with --agent-session <id>, --name <alias>, or --panel <id|@panel:0x<hex>> to select explicitly. Names reset when the session ends or the session stops.",
            ),
            (Locale::ZhCn, CommandPath::AgentRename) => crate::cli_text::HelpDetails::new(
                "orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <new-name> [--format text|json]\n      orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) --clear",
                "为存活智能体命名或原子重命名。名称格式为 [a-z][a-z0-9_-]{0,31}，拒绝重名。--clear 只释放名称。可将 <target> 替换成 --agent-session <id> 或 --name <alias>。会话结束或会话停止后名称释放。",
            ),
            (Locale::EnUs, CommandPath::AgentWait) => super::agent::HELP_AGENT_WAIT_EN,
            (Locale::ZhCn, CommandPath::AgentWait) => super::agent::HELP_AGENT_WAIT_ZH,
            (Locale::EnUs, CommandPath::AgentDoctor) => super::agent::HELP_AGENT_DOCTOR_EN,
            (Locale::ZhCn, CommandPath::AgentDoctor) => super::agent::HELP_AGENT_DOCTOR_ZH,
            (Locale::EnUs, CommandPath::AgentIntegrationList) => super::integration::HELP_AGENT_INTEGRATION_LIST_EN,
            (Locale::ZhCn, CommandPath::AgentIntegrationList) => super::integration::HELP_AGENT_INTEGRATION_LIST_ZH,
            (Locale::EnUs, CommandPath::AgentIntegrationExplain) => super::integration::HELP_AGENT_INTEGRATION_EXPLAIN_EN,
            (Locale::ZhCn, CommandPath::AgentIntegrationExplain) => super::integration::HELP_AGENT_INTEGRATION_EXPLAIN_ZH,
            (Locale::EnUs, CommandPath::AgentIntegrationEnable) => super::integration::HELP_AGENT_INTEGRATION_ENABLE_EN,
            (Locale::ZhCn, CommandPath::AgentIntegrationEnable) => super::integration::HELP_AGENT_INTEGRATION_ENABLE_ZH,
            (Locale::EnUs, CommandPath::AgentIntegrationDisable) => super::integration::HELP_AGENT_INTEGRATION_DISABLE_EN,
            (Locale::ZhCn, CommandPath::AgentIntegrationDisable) => super::integration::HELP_AGENT_INTEGRATION_DISABLE_ZH,
            (Locale::EnUs, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_EN,
            (Locale::ZhCn, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_ZH,
            (Locale::EnUs, CommandPath::ApiSchema) => HELP_API_SCHEMA_EN,
            (Locale::ZhCn, CommandPath::ApiSchema) => HELP_API_SCHEMA_ZH,
            (Locale::EnUs, CommandPath::ConfigDefault) => super::configuration::HELP_CONFIG_DEFAULT_EN,
            (Locale::ZhCn, CommandPath::ConfigDefault) => super::configuration::HELP_CONFIG_DEFAULT_ZH,
            (Locale::EnUs, CommandPath::DiagnosticsCreate) => super::diagnostics::HELP_DIAGNOSTICS_CREATE_EN,
            (Locale::ZhCn, CommandPath::DiagnosticsCreate) => super::diagnostics::HELP_DIAGNOSTICS_CREATE_ZH,
            (_, CommandPath::MachineList) => self.cli_machine_help(path),
            (_, CommandPath::MachineAdd) => self.cli_machine_help(path),
            (_, CommandPath::MachineRename) => self.cli_machine_help(path),
            (_, CommandPath::MachineRemove) => self.cli_machine_help(path),
            (_, CommandPath::MachineEnable) => self.cli_machine_help(path),
            (_, CommandPath::MachineDisable) => self.cli_machine_help(path),
            (_, CommandPath::MachineConnect) => self.cli_machine_help(path),
            (_, CommandPath::MachineDisconnect) => self.cli_machine_help(path),
            (Locale::EnUs, CommandPath::ChannelShow) => crate::cli_text::HelpDetails::new(
                "orchbee channel show [--format text|json]",
                "Read the Windows update.channel preference (stable or preview). Defaults to stable.\nValidates the complete config; a missing explicit ORCHBEE_CONFIG_FILE fails.\nDoes not write files, connect to a session, download, or change the installed cohort.",
            ),
            (Locale::ZhCn, CommandPath::ChannelShow) => crate::cli_text::HelpDetails::new(
                "orchbee channel show [--format text|json]",
                "读取 Windows update.channel 偏好（stable 或 preview），默认 stable。校验完整配置；显式 ORCHBEE_CONFIG_FILE 不存在时失败。不写文件、不连接会话、不下载，也不改变已安装 cohort。",
            ),
            (Locale::EnUs, CommandPath::ChannelSet) => crate::cli_text::HelpDetails::new(
                "orchbee channel set <stable|preview> [--format text|json]",
                "Save the channel and immediately run update. Update failure retains the saved channel.\nUses ORCHBEE_UPDATE_MANIFEST; see orchbee update --help.",
            ),
            (Locale::ZhCn, CommandPath::ChannelSet) => crate::cli_text::HelpDetails::new(
                "orchbee channel set <stable|preview> [--format text|json]",
                "保存频道后立即更新；更新失败仍保留已保存的频道。使用 ORCHBEE_UPDATE_MANIFEST；参见 orchbee update --help。",
            ),
            (Locale::EnUs, CommandPath::Update) => crate::cli_text::HelpDetails::new(
                "orchbee update [--format text|json]",
                "Install the configured channel as a complete immutable Windows cohort.\nORCHBEE_UPDATE_MANIFEST: explicit HTTPS URL or absolute local manifest path; {channel} expands to stable or preview.\nORCHBEE_INSTALL_DIR defaults to %LOCALAPPDATA%/Programs/Orchbee.\nValidates every file, publishes versions/<version>, then makes it the active version. Existing sessions keep their version.\nTotal deadline: 600 seconds. Ctrl+C cancels; receipts retain published paths and cleanup obligations.",
            ),
            (Locale::ZhCn, CommandPath::Update) => crate::cli_text::HelpDetails::new(
                "orchbee update [--format text|json]",
                "按配置的频道安装完整、独立的 Windows cohort。\nORCHBEE_UPDATE_MANIFEST：显式 HTTPS URL 或本地清单绝对路径；{channel} 替换为 stable 或 preview。\nORCHBEE_INSTALL_DIR 默认为 %LOCALAPPDATA%/Programs/Orchbee。逐文件校验并发布到 versions/<版本>，再设为当前版本；现有会话保留原版本。总时限 600 秒。Ctrl+C 取消；回执保留已发布路径和待清理项。",
            ),
            (Locale::EnUs, CommandPath::Install) => crate::cli_text::HelpDetails::new(
                "orchbee install [--no-path] [--allow-downgrade] [--format text|json]",
                "Install the signed release directory this orchbee runs from and make it the active version.\nCopies every file to versions/<version> under ORCHBEE_INSTALL_DIR (default %LOCALAPPDATA%/Programs/Orchbee; a test build uses its own folder), verifies it, then moves active.json.\nAn installed version is verified and reused, never overwritten. A version lower than the active one is refused unless --allow-downgrade.\nUnless --no-path, the PATH step copies the version's launcher into bin/ there and adds that folder to the user Path when it is missing, removing entries for other folders inside the installation.\nTotal deadline: 600 seconds. Ctrl+C cancels.",
            ),
            (Locale::ZhCn, CommandPath::Install) => crate::cli_text::HelpDetails::new(
                "orchbee install [--no-path] [--allow-downgrade] [--format text|json]",
                "安装当前 orchbee 所在的已签名发布目录，并设为当前版本。\n把每个文件复制到 ORCHBEE_INSTALL_DIR（默认 %LOCALAPPDATA%/Programs/Orchbee；测试构建使用自己的目录）下的 versions/<版本>，校验后更新 active.json。\n已安装的版本校验后复用，绝不覆盖。低于当前版本时拒绝，除非加 --allow-downgrade。\n除非加 --no-path，PATH 步骤会把该版本的启动器复制到其中的 bin/，在用户 Path 缺少该文件夹时添加它，并移除指向安装目录内其他文件夹的条目。\n总时限 600 秒。Ctrl+C 取消。",
            ),
            (Locale::EnUs, CommandPath::Uninstall) => crate::cli_text::HelpDetails::new(
                "orchbee uninstall [--delete-data] [--confirm] [--format text|json]",
                "Remove the installed Orchbee of this environment completely, in this order: list what goes and confirm; stop every Session; disable every Agent integration; remove the user Path entry, the Start menu shortcut and the uninstall registration; remove logs, internal/integrations, internal/cache, internal/runtime and internal/tmp from the data folder; remove the program folder (ORCHBEE_INSTALL_DIR, default %LOCALAPPDATA%/Programs/Orchbee) a few seconds after the command ends.
Settings, Sessions and the rest of the data folder stay unless --delete-data, which removes the whole data folder.
The Windows release build asks a person in a console to type orchbee; development and test builds require --confirm.
A failure says what was done and what remains; running the command again continues.",
            ),
            (Locale::ZhCn, CommandPath::Uninstall) => crate::cli_text::HelpDetails::new(
                "orchbee uninstall [--delete-data] [--confirm] [--format text|json]",
                "完整移除本环境已安装的 Orchbee，顺序为：列出将移除的内容并确认；停止所有会话；停用所有智能体集成；移除用户 Path 条目、开始菜单快捷方式和卸载登记；从数据文件夹移除 logs、internal/integrations、internal/cache、internal/runtime 和 internal/tmp；命令结束几秒后移除程序文件夹（ORCHBEE_INSTALL_DIR，默认 %LOCALAPPDATA%/Programs/Orchbee）。
除非加 --delete-data（删除整个数据文件夹），否则设置、会话和数据文件夹中的其余内容都会保留。
Windows 正式版要求在控制台中输入 orchbee；开发和测试构建需要 --confirm。
失败时会说明已完成和尚余的内容；再次运行命令即可继续。",
            ),
            (Locale::EnUs, CommandPath::ConfigCheck) => super::configuration::HELP_CONFIG_CHECK_EN,
            (Locale::ZhCn, CommandPath::ConfigCheck) => super::configuration::HELP_CONFIG_CHECK_ZH,
            (
                _,
                path @ (CommandPath::SessionList
                | CommandPath::SessionAttach
                | CommandPath::SessionStop
                | CommandPath::SessionDelete),
            ) => self.cli_session_command_help(path),
            (Locale::EnUs, CommandPath::ServerStart) => super::server::HELP_SERVER_START_EN,
            (Locale::ZhCn, CommandPath::ServerStart) => super::server::HELP_SERVER_START_ZH,
            (Locale::EnUs, CommandPath::ServerConnect) => super::server::HELP_SERVER_CONNECT_EN,
            (Locale::ZhCn, CommandPath::ServerConnect) => super::server::HELP_SERVER_CONNECT_ZH,
            (
                _,
                CommandPath::ServerAgentManifests
                | CommandPath::ServerUpdateAgentManifests
                | CommandPath::ServerReloadAgentManifests,
            ) => self.cli_agent_manifests_help(path),
            (Locale::EnUs, CommandPath::ServerDiagnose) => super::server::HELP_SERVER_DIAGNOSE_EN,
            (Locale::ZhCn, CommandPath::ServerDiagnose) => super::server::HELP_SERVER_DIAGNOSE_ZH,
            (Locale::EnUs, CommandPath::ServerStop) => super::server::HELP_SERVER_STOP_EN,
            (Locale::ZhCn, CommandPath::ServerStop) => super::server::HELP_SERVER_STOP_ZH,
        }
    }
}
