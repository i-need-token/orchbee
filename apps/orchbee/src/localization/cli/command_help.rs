//! Selects localized command details and owns top-level help copy; invocation owns grammar.

use super::Locale;

const HELP_TUI_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tui [--mouse on|off]",
    "Open the terminal interface on Windows or Linux. Running orchbee without a command does the same. Mouse capture defaults to on; --mouse is accepted on this explicit tui route.",
);

const HELP_TUI_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tui [--mouse on|off]",
    "在 Windows 或 Linux 上打开终端界面。不带命令运行 orchbee 也会打开该界面。鼠标捕获默认开启；显式 tui 路由支持 --mouse 选项。",
);

const HELP_STATUS_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee status [--format text|json]",
    "Read the running session's reachability and resource counts. This command does not create or start a session. Use status server for service compatibility details, or status client to inspect this executable offline.",
);

const HELP_STATUS_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee status [--format text|json]",
    "读取正在运行的会话的可达性和资源数量。本命令不创建或启动会话。使用 status server 查看服务兼容性，或使用 status client 离线检查当前程序。",
);

const HELP_API_SNAPSHOT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee api snapshot [--format text|json]",
        "Emit the complete validated workbench.snapshot document inside the\norchbee.api.snapshot envelope. Successful output is JSON even under\n'--format text'; a failure envelope still requires '--format json'.",
    );

const HELP_API_SNAPSHOT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee api snapshot [--format text|json]",
        "在 orchbee.api.snapshot 封套内输出完整已验证 workbench.snapshot 文档。成功输出即使 '--format text' 也为 JSON；失败封套仍需显式 '--format json'。",
    );

const HELP_API_SCHEMA_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]",
    "Without <schema-id>, list every embedded schema ID with its versions and title.\nWith <schema-id>, emit that JSON Schema; --format json or --output emits the\ncomplete dictionary. Schema documents are always JSON; unknown IDs are\nnot-found, and failure envelopes require '--format json'.\nOn Windows, --output writes identical JSON to a literal file path, replacing\nan existing regular file only after complete preparation. Parent directories\nmust exist. The command stays offline and also emits the JSON on stdout.",
);

const HELP_API_SCHEMA_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]",
    "不带 <schema-id> 时列出每个内嵌 schema 的 ID、版本和标题；带 <schema-id> 时输出该 JSON Schema；--format json 或 --output 输出完整字典。schema 文档始终为 JSON；未知 ID 为 not-found，失败封套需显式 '--format json'。\nWindows 的 --output 将相同 JSON 写入字面文件路径，完整准备后才替换已有普通文件。父目录必须存在；全程离线，stdout 同时输出该 JSON。",
);

impl Locale {
    // ---- help topics ---------------------------------------------------------
    pub(crate) fn cli_command_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath;
        match (self, path) {
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

            #[cfg(windows)]
            (_, CommandPath::EventsSubscribe | CommandPath::EventsWait) => self.events_help(path),
            #[cfg(windows)]
            (
                _,
                CommandPath::PanelGraphicsSet
                | CommandPath::PanelGraphicsClear
                | CommandPath::PanelGraphicsInfo
                | CommandPath::PanelGraphicsStream,
            ) => self.pane_graphics_help(path),
            #[cfg(windows)]
            (_, CommandPath::CommandList | CommandPath::CommandInvoke) => {
                self.configured_command_help(path)
            }
            (_, CommandPath::TerminalTitleSet | CommandPath::TerminalTitleClear) => {
                self.window_title_help(path)
            }
            #[cfg(windows)]
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
                | CommandPath::PluginUninstall,
            ) => self.plugin_help(path),
            (_, CommandPath::NotificationShow) => self.notification_help(),
            (_, CommandPath::ServerConfig | CommandPath::ServerReloadConfig) => {
                self.runtime_configuration_help(path)
            }
            (Self::EnUs, CommandPath::ClientList) => super::client_control::HELP_CLIENT_LIST_EN,
            (Self::ZhCn, CommandPath::ClientList) => super::client_control::HELP_CLIENT_LIST_ZH,
            (Self::EnUs, CommandPath::ClientGet) => super::client_control::HELP_CLIENT_GET_EN,
            (Self::ZhCn, CommandPath::ClientGet) => super::client_control::HELP_CLIENT_GET_ZH,
            (Self::EnUs, CommandPath::ClientControlStatus) => {
                super::client_control::HELP_CLIENT_CONTROL_STATUS_EN
            }
            (Self::ZhCn, CommandPath::ClientControlStatus) => {
                super::client_control::HELP_CLIENT_CONTROL_STATUS_ZH
            }
            (Self::EnUs, CommandPath::WorkspaceFocus) => super::client_control::HELP_WORKSPACE_FOCUS_EN,
            (Self::ZhCn, CommandPath::WorkspaceFocus) => super::client_control::HELP_WORKSPACE_FOCUS_ZH,
            (Self::EnUs, CommandPath::TabFocus) => super::client_control::HELP_TAB_FOCUS_EN,
            (Self::ZhCn, CommandPath::TabFocus) => super::client_control::HELP_TAB_FOCUS_ZH,
            (Self::EnUs, CommandPath::PanelResize) => super::client_control::HELP_PANEL_RESIZE_EN,
            (Self::ZhCn, CommandPath::PanelResize) => super::client_control::HELP_PANEL_RESIZE_ZH,
            (Self::EnUs, CommandPath::LayoutSetSplitRatio) => super::client_control::HELP_LAYOUT_RATIO_EN,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::LayoutExport) => super::layouts::EXPORT_EN,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::LayoutApply) => super::layouts::APPLY_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::LayoutApply) => super::layouts::APPLY_ZH,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::LayoutExport) => super::layouts::EXPORT_ZH,
            (Self::ZhCn, CommandPath::LayoutSetSplitRatio) => super::client_control::HELP_LAYOUT_RATIO_ZH,
            (Self::EnUs, CommandPath::PanelFocus) => super::client_control::HELP_PANEL_FOCUS_EN,
            (Self::ZhCn, CommandPath::PanelFocus) => super::client_control::HELP_PANEL_FOCUS_ZH,
            (Self::EnUs, CommandPath::PanelLayout) => super::client_layout::HELP_LAYOUT_EN,
            (Self::ZhCn, CommandPath::PanelLayout) => super::client_layout::HELP_LAYOUT_ZH,
            (Self::EnUs, CommandPath::PanelNeighbor) => super::client_layout::HELP_NEIGHBOR_EN,
            (Self::ZhCn, CommandPath::PanelNeighbor) => super::client_layout::HELP_NEIGHBOR_ZH,
            (Self::EnUs, CommandPath::PanelEdges) => super::client_layout::HELP_EDGES_EN,
            (Self::ZhCn, CommandPath::PanelEdges) => super::client_layout::HELP_EDGES_ZH,
            (Self::EnUs, CommandPath::PanelZoom) => super::client_control::HELP_PANEL_ZOOM_EN,
            (Self::ZhCn, CommandPath::PanelZoom) => super::client_control::HELP_PANEL_ZOOM_ZH,

            (Self::EnUs, CommandPath::Tui) => HELP_TUI_EN,
            (Self::ZhCn, CommandPath::Tui) => HELP_TUI_ZH,
            (_, CommandPath::StatusClient) => super::runtime_status::help(self, false),
            (_, CommandPath::StatusServer) => super::runtime_status::help(self, true),
            (Self::EnUs, CommandPath::Status) => HELP_STATUS_EN,
            (Self::ZhCn, CommandPath::Status) => HELP_STATUS_ZH,
            (Self::EnUs, CommandPath::ProjectList) => super::project::HELP_PROJECT_LIST_EN,
            (Self::ZhCn, CommandPath::ProjectList) => super::project::HELP_PROJECT_LIST_ZH,
            (Self::EnUs, CommandPath::ProjectGet) => super::project::HELP_PROJECT_GET_EN,
            (Self::ZhCn, CommandPath::ProjectGet) => super::project::HELP_PROJECT_GET_ZH,
            (_, CommandPath::ProjectDissolve) => super::organization::Help::ProjectDissolve.text(self),
            (_, CommandPath::ProjectCloseAll) => super::organization::Help::ProjectCloseAll.text(self),
            (_, CommandPath::ProjectCloseStatus) => {
                super::organization::Help::ProjectCloseStatus.text(self)
            }
            (_, CommandPath::ProjectCloseResume) => {
                super::organization::Help::ProjectCloseResume.text(self)
            }
            (_, CommandPath::WorkspaceSetProject) => {
                super::organization::Help::WorkspaceSetProject.text(self)
            }
            (_, CommandPath::WorkspaceCreateProject) => {
                super::organization::Help::WorkspaceCreateProject.text(self)
            }
            (_, CommandPath::WorkspaceUngroup) => super::organization::Help::WorkspaceUngroup.text(self),
            (Self::EnUs, CommandPath::ProjectCreate) => super::project::HELP_PROJECT_CREATE_EN,
            (Self::ZhCn, CommandPath::ProjectCreate) => super::project::HELP_PROJECT_CREATE_ZH,
            (Self::EnUs, CommandPath::ProjectRename) => super::project::HELP_PROJECT_RENAME_EN,
            (Self::ZhCn, CommandPath::ProjectRename) => super::project::HELP_PROJECT_RENAME_ZH,
            (Self::EnUs, CommandPath::WorkspaceList) => super::workspace::HELP_WORKSPACE_LIST_EN,
            (Self::ZhCn, CommandPath::WorkspaceList) => super::workspace::HELP_WORKSPACE_LIST_ZH,
            (Self::EnUs, CommandPath::WorkspaceGet) => super::workspace::HELP_WORKSPACE_GET_EN,
            (Self::ZhCn, CommandPath::WorkspaceGet) => super::workspace::HELP_WORKSPACE_GET_ZH,
            (Self::EnUs, CommandPath::WorkspaceCreate) => super::workspace::HELP_WORKSPACE_CREATE_EN,
            (Self::ZhCn, CommandPath::WorkspaceCreate) => super::workspace::HELP_WORKSPACE_CREATE_ZH,
            (Self::EnUs, CommandPath::WorkspaceMove) => super::workspace::HELP_WORKSPACE_MOVE_EN,
            (Self::ZhCn, CommandPath::WorkspaceMove) => super::workspace::HELP_WORKSPACE_MOVE_ZH,
            (Self::EnUs, CommandPath::PanelReportAgent) => super::panel_agent::PANELREPORTAGENT_EN,
            (Self::ZhCn, CommandPath::PanelReportAgent) => super::panel_agent::PANELREPORTAGENT_ZH,
            (Self::EnUs, CommandPath::PanelClearAgentAuthority) => {
                super::panel_agent::PANELCLEARAGENTAUTHORITY_EN
            }
            (Self::ZhCn, CommandPath::PanelClearAgentAuthority) => {
                super::panel_agent::PANELCLEARAGENTAUTHORITY_ZH
            }
            (Self::EnUs, CommandPath::PanelReleaseAgent) => super::panel_agent::PANELRELEASEAGENT_EN,
            (Self::ZhCn, CommandPath::PanelReleaseAgent) => super::panel_agent::PANELRELEASEAGENT_ZH,
            (Self::EnUs, CommandPath::PanelReportMetadata) => super::panel_metadata::HELP_EN,
            (Self::ZhCn, CommandPath::PanelReportMetadata) => super::panel_metadata::HELP_ZH,
            (Self::EnUs, CommandPath::WorkspaceReportMetadata) => super::workspace_metadata::HELP_EN,
            (Self::ZhCn, CommandPath::WorkspaceReportMetadata) => super::workspace_metadata::HELP_ZH,
            (Self::EnUs, CommandPath::WorkspaceMoveBlock) => super::workspace::HELP_WORKSPACE_MOVE_BLOCK_EN,
            (Self::ZhCn, CommandPath::WorkspaceMoveBlock) => super::workspace::HELP_WORKSPACE_MOVE_BLOCK_ZH,
            (Self::EnUs, CommandPath::WorkspaceRename) => super::workspace::HELP_WORKSPACE_RENAME_EN,
            (Self::ZhCn, CommandPath::WorkspaceRename) => super::workspace::HELP_WORKSPACE_RENAME_ZH,
            (Self::EnUs, CommandPath::WorkspaceAcceptDirectory) => {
                super::workspace::HELP_WORKSPACE_ACCEPT_DIRECTORY_EN
            }
            (Self::ZhCn, CommandPath::WorkspaceAcceptDirectory) => {
                super::workspace::HELP_WORKSPACE_ACCEPT_DIRECTORY_ZH
            }
            (Self::EnUs, CommandPath::WorkspaceClose) => super::workspace::HELP_WORKSPACE_CLOSE_EN,
            (Self::ZhCn, CommandPath::WorkspaceClose) => super::workspace::HELP_WORKSPACE_CLOSE_ZH,
            (Self::EnUs, CommandPath::WorkspaceCloseStatus) => super::workspace::HELP_WORKSPACE_CLOSE_STATUS_EN,
            (Self::ZhCn, CommandPath::WorkspaceCloseStatus) => super::workspace::HELP_WORKSPACE_CLOSE_STATUS_ZH,
            (Self::EnUs, CommandPath::WorkspaceCloseResume) => super::workspace::HELP_WORKSPACE_CLOSE_RESUME_EN,
            (Self::ZhCn, CommandPath::WorkspaceCloseResume) => super::workspace::HELP_WORKSPACE_CLOSE_RESUME_ZH,
            (Self::EnUs, CommandPath::TabList) => super::tab::HELP_TAB_LIST_EN,
            (Self::ZhCn, CommandPath::TabList) => super::tab::HELP_TAB_LIST_ZH,
            (Self::EnUs, CommandPath::TabGet) => super::tab::HELP_TAB_GET_EN,
            (Self::ZhCn, CommandPath::TabGet) => super::tab::HELP_TAB_GET_ZH,
            (Self::EnUs, CommandPath::TabCreate) => super::tab::HELP_TAB_CREATE_EN,
            (Self::ZhCn, CommandPath::TabCreate) => super::tab::HELP_TAB_CREATE_ZH,
            (Self::EnUs, CommandPath::PanelSwap) => super::panel::HELP_PANEL_SWAP_EN,
            (Self::ZhCn, CommandPath::PanelSwap) => super::panel::HELP_PANEL_SWAP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::PanelCreateTerminal) => super::panel_terminal_creation::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::PanelCreateTerminal) => super::panel_terminal_creation::HELP_ZH,
            (Self::EnUs, CommandPath::PanelInput) => super::panel::HELP_PANEL_INPUT_EN,
            (Self::ZhCn, CommandPath::PanelInput) => super::panel::HELP_PANEL_INPUT_ZH,
            (Self::EnUs, CommandPath::PanelRename) => super::panel::HELP_PANEL_RENAME_EN,
            (Self::ZhCn, CommandPath::PanelRename) => super::panel::HELP_PANEL_RENAME_ZH,
            (Self::EnUs, CommandPath::TabRename) => super::tab::HELP_TAB_RENAME_EN,
            (Self::ZhCn, CommandPath::TabRename) => super::tab::HELP_TAB_RENAME_ZH,
            (Self::EnUs, CommandPath::TabMove) => super::tab::HELP_TAB_MOVE_EN,
            (Self::ZhCn, CommandPath::TabMove) => super::tab::HELP_TAB_MOVE_ZH,
            (Self::EnUs, CommandPath::TabClose) => super::tab::HELP_TAB_CLOSE_EN,
            (Self::ZhCn, CommandPath::TabClose) => super::tab::HELP_TAB_CLOSE_ZH,
            (Self::EnUs, CommandPath::PanelList) => super::panel::HELP_PANEL_LIST_EN,
            (Self::ZhCn, CommandPath::PanelList) => super::panel::HELP_PANEL_LIST_ZH,
            (Self::EnUs, CommandPath::PanelCurrent) => super::panel::HELP_PANEL_CURRENT_EN,
            (Self::ZhCn, CommandPath::PanelCurrent) => super::panel::HELP_PANEL_CURRENT_ZH,
            (Self::EnUs, CommandPath::PanelGet) => super::panel::HELP_PANEL_GET_EN,
            (Self::ZhCn, CommandPath::PanelGet) => super::panel::HELP_PANEL_GET_ZH,
            (Self::EnUs, CommandPath::PanelProcessInfo) => super::panel::HELP_PANEL_PROCESS_INFO_EN,
            (Self::ZhCn, CommandPath::PanelProcessInfo) => super::panel::HELP_PANEL_PROCESS_INFO_ZH,
            (Self::EnUs, CommandPath::PanelSplit) => super::panel::HELP_PANEL_SPLIT_EN,
            (Self::ZhCn, CommandPath::PanelSplit) => super::panel::HELP_PANEL_SPLIT_ZH,
            (Self::EnUs, CommandPath::PanelClose) => super::panel::HELP_PANEL_CLOSE_EN,
            (Self::ZhCn, CommandPath::PanelClose) => super::panel::HELP_PANEL_CLOSE_ZH,
            (Self::EnUs, CommandPath::PanelMove) => super::panel::HELP_PANEL_MOVE_EN,
            (Self::ZhCn, CommandPath::PanelMove) => super::panel::HELP_PANEL_MOVE_ZH,
            (Self::EnUs, CommandPath::TerminalList) => super::terminal::HELP_TERMINAL_LIST_EN,
            (Self::ZhCn, CommandPath::TerminalList) => super::terminal::HELP_TERMINAL_LIST_ZH,
            (Self::EnUs, CommandPath::TerminalGet) => super::terminal::HELP_TERMINAL_GET_EN,
            (Self::ZhCn, CommandPath::TerminalGet) => super::terminal::HELP_TERMINAL_GET_ZH,
            (Self::EnUs, CommandPath::TerminalAttach) => super::terminal::HELP_TERMINAL_ATTACH_EN,
            (Self::ZhCn, CommandPath::TerminalAttach) => super::terminal::HELP_TERMINAL_ATTACH_ZH,
            (Self::EnUs, CommandPath::AgentAttach) => super::agent::HELP_AGENT_ATTACH_EN,
            (Self::ZhCn, CommandPath::AgentAttach) => super::agent::HELP_AGENT_ATTACH_ZH,
            (Self::EnUs, CommandPath::TerminalControl) => super::terminal::HELP_TERMINAL_CONTROL_EN,
            (Self::ZhCn, CommandPath::TerminalControl) => super::terminal::HELP_TERMINAL_CONTROL_ZH,
            (Self::EnUs, CommandPath::TerminalObserve) => super::terminal::HELP_TERMINAL_OBSERVE_EN,
            (Self::ZhCn, CommandPath::TerminalObserve) => super::terminal::HELP_TERMINAL_OBSERVE_ZH,
            (Self::EnUs, CommandPath::TerminalRead) => super::terminal::HELP_TERMINAL_READ_EN,
            (Self::ZhCn, CommandPath::TerminalRead) => super::terminal::HELP_TERMINAL_READ_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalSelectionRead) => super::terminal::HELP_TERMINAL_SELECTION_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalSelectionRead) => super::terminal::HELP_TERMINAL_SELECTION_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalLinkResolve) => super::terminal_links::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalLinkResolve) => super::terminal_links::HELP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalClear) => super::terminal_clear::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalClear) => super::terminal_clear::HELP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalEditScrollback) => super::terminal_editor::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalEditScrollback) => super::terminal_editor::HELP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalLinkActivate) => super::terminal_links::ACTIVATE_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalLinkActivate) => super::terminal_links::ACTIVATE_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalCopyMotion) => super::terminal_copy::HELP_MOTION_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalCopyMotion) => super::terminal_copy::HELP_MOTION_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalCopySearch) => super::terminal_copy::HELP_SEARCH_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalCopySearch) => super::terminal_copy::HELP_SEARCH_ZH,

            (Self::EnUs, CommandPath::TerminalWaitOutput) => super::terminal::HELP_TERMINAL_WAIT_OUTPUT_EN,
            (Self::ZhCn, CommandPath::TerminalWaitOutput) => super::terminal::HELP_TERMINAL_WAIT_OUTPUT_ZH,
            (Self::EnUs, CommandPath::TerminalRun) => super::terminal::HELP_TERMINAL_RUN_EN,
            (Self::ZhCn, CommandPath::TerminalRun) => super::terminal::HELP_TERMINAL_RUN_ZH,
            (Self::EnUs, CommandPath::TerminalSend) => super::terminal::HELP_TERMINAL_SEND_EN,
            (Self::ZhCn, CommandPath::TerminalSend) => super::terminal::HELP_TERMINAL_SEND_ZH,
            (Self::EnUs, CommandPath::WorktreeList) => super::worktree::HELP_WORKTREE_LIST_EN,
            (Self::ZhCn, CommandPath::WorktreeList) => super::worktree::HELP_WORKTREE_LIST_ZH,
            (
                _,
                path @ (CommandPath::WorktreeCreate
                | CommandPath::WorktreeOpen
                | CommandPath::WorktreeRemove
                | CommandPath::WorktreeStatus
                | CommandPath::WorktreeResume),
            ) => self.cli_worktree_help(path),
            (Self::EnUs, CommandPath::LaunchProfileList) => super::launch_profile::HELP_LAUNCH_PROFILE_LIST_EN,
            (Self::ZhCn, CommandPath::LaunchProfileList) => super::launch_profile::HELP_LAUNCH_PROFILE_LIST_ZH,
            (Self::EnUs, CommandPath::LaunchProfileGet) => super::launch_profile::HELP_LAUNCH_PROFILE_GET_EN,
            (Self::ZhCn, CommandPath::LaunchProfileGet) => super::launch_profile::HELP_LAUNCH_PROFILE_GET_ZH,
            (Self::EnUs, CommandPath::LaunchProfileCreate) => super::launch_profile::HELP_LAUNCH_PROFILE_CREATE_EN,
            (Self::ZhCn, CommandPath::LaunchProfileCreate) => super::launch_profile::HELP_LAUNCH_PROFILE_CREATE_ZH,
            (Self::EnUs, CommandPath::LaunchProfileDelete) => super::launch_profile::HELP_LAUNCH_PROFILE_DELETE_EN,
            (Self::ZhCn, CommandPath::LaunchProfileDelete) => super::launch_profile::HELP_LAUNCH_PROFILE_DELETE_ZH,
            (Self::EnUs, CommandPath::AgentChanges) => crate::localization::HelpDetails::new(
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]",
                "Windows: read one bounded Agent change page (limit 1-100, default 50).\nOmit cursor to establish a current-head checkpoint, then reuse its cursor.\nreset-required means history was lost; rebuild your baseline. A previous\nsession boot or invalid cursor is rejected. No subscription or automatic retry.\nTotal read deadline: 5 seconds. JSON schema: orchbee.agent.changes.",
            ),
            (Self::ZhCn, CommandPath::AgentChanges) => crate::localization::HelpDetails::new(
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]",
                "Windows：读取一页有界智能体变更（limit 1-100，默认 50）。省略 cursor 建立当前头部检查点，再使用其 cursor 继续观察。\nreset-required 表示历史丢失，必须重建基线；先前会话启动实例或无效\ncursor 会被拒绝。不订阅、不自动重试。总读取期限 5 秒。\nJSON schema：orchbee.agent.changes。",
            ),
            #[cfg(windows)]
            (
                locale,
                CommandPath::AgentViewGet | CommandPath::AgentViewSet | CommandPath::AgentViewClear,
            ) => locale.agent_view_help(path),
            (Self::EnUs, CommandPath::AgentList) => super::agent::HELP_AGENT_LIST_EN,
            (Self::ZhCn, CommandPath::AgentList) => super::agent::HELP_AGENT_LIST_ZH,
            (Self::EnUs, CommandPath::AgentStart) => crate::localization::HelpDetails::new(
                "orchbee agent start <name> --provider <provider> [--panel <id|@panel:0x<hex>>|--current] [--timeout <ms>] [--format text|json] [--arg <value>]... [-- <args>...]",
                "Windows: start an interactive Agent in an idle terminal, then bind its name after verified readiness.\nThe default target is the caller Panel. Timeout: 3001–300000 ms, default 30000.\nUse --arg=<value> repeatedly as an alternative to trailing arguments. Input is submitted once.\nCancellation or timeout releases the name when confirmed; it never kills the Agent.\nProvider hooks must report readiness. Use agent integration and agent doctor to check setup.\nUse --provider antigravity consistently for start, integration and doctor.",
            ),
            (Self::ZhCn, CommandPath::AgentStart) => crate::localization::HelpDetails::new(
                "orchbee agent start <name> --provider <provider> [--panel <id|@panel:0x<hex>>|--current] [--timeout <ms>] [--format text|json] [--arg <value>]... [-- <args>...]",
                "Windows：在空闲终端启动交互式智能体，确认就绪后绑定名称。默认目标为调用者面板。超时范围 3001–300000 毫秒，默认 30000。可重复使用 --arg=<value> 代替尾随参数。启动输入只提交一次。取消或超时在确认后释放名称，但不会终止智能体。提供方 hook 必须能报告就绪；可用 agent integration 和 agent doctor 检查配置。\nstart、integration 和 doctor 统一使用 --provider antigravity。",
            ),
            (Self::EnUs, CommandPath::AgentPrompt) if cfg!(windows) => super::agent::HELP_AGENT_PROMPT_WINDOWS_EN,
            (Self::ZhCn, CommandPath::AgentPrompt) if cfg!(windows) => super::agent::HELP_AGENT_PROMPT_WINDOWS_ZH,
            (Self::EnUs, CommandPath::AgentPrompt) => crate::localization::HelpDetails::new(
                "orchbee agent prompt (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--format text|json]",
                "Send a prompt to one running Agent. Use --wait to observe its next state.\nFind targets with orchbee agent list; use --panel to select an Agent by its Panel.\n--until, --timeout and --forever require --wait. Inspect the terminal before retrying uncertain input.",
            ),
            (Self::ZhCn, CommandPath::AgentPrompt) => crate::localization::HelpDetails::new(
                "orchbee agent prompt (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--format text|json]",
                "向正在运行的智能体发送提示。使用 --wait 观察其后续状态。\n用 orchbee agent list 查找目标，或用 --panel 按面板选择智能体。\n--until、--timeout 和 --forever 要求 --wait。投递结果不确定时，重试前先检查终端。",
            ),
            (Self::EnUs, CommandPath::AgentRead) if cfg!(windows) => super::agent::HELP_AGENT_OUTPUT_EN,
            (Self::ZhCn, CommandPath::AgentRead) if cfg!(windows) => super::agent::HELP_AGENT_OUTPUT_ZH,
            (Self::EnUs, CommandPath::AgentRead) => crate::localization::HelpDetails::new(
                "orchbee agent read (<target>|--agent-session <id>|--name <name>|--panel <panel>|--terminal <terminal>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
                "Read a bound terminal through one temporary read-only viewer. Default source: recent, 80 physical rows including history and the live screen. --source visible reads the screen; recent-unwrapped joins verified soft wraps; detection reads a cursor-centered screen-height window. --lines accepts 0..4294967295 and clamps to 1000. No input, focus or resize effect. --plain emits UTF-8 and conflicts with --format json. --ansi emits styled SGR text; JSON always uses orchbee.terminal.output v1 with explicit text/ansi encoding. A nullable output cursor can fence a later --after wait. The cursor is not a command completion marker. Sources never silently widen after a stale sample.",
            ),
            (Self::ZhCn, CommandPath::AgentRead) => crate::localization::HelpDetails::new(
                "orchbee agent read (<target>|--agent-session <id>|--name <name>|--panel <panel>|--terminal <terminal>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
                "通过一个临时只读 viewer 读取已绑定终端。默认来源 recent，包含历史及当前屏幕的 80 个物理行。--source visible 读取屏幕；recent-unwrapped 拼接已验证软换行；detection 读取光标附近一个屏幕高度的窗口。--lines 接受 0..4294967295 并钳制到 1000。不输入、不改焦点或尺寸。--plain 输出 UTF-8，与 --format json 互斥。--ansi 输出带 SGR 样式的文本，JSON 均使用 orchbee.terminal.output v1，并明确 text/ansi 编码。可空输出游标可用于后续 --after 等待，但不表示命令完成。样本陈旧时不会悄悄扩大来源范围。",
            ),
            (Self::EnUs, CommandPath::AgentSendKeys) if cfg!(windows) => {
                super::agent::HELP_AGENT_SEND_KEYS_WINDOWS_EN
            }
            (Self::ZhCn, CommandPath::AgentSendKeys) if cfg!(windows) => {
                super::agent::HELP_AGENT_SEND_KEYS_WINDOWS_ZH
            }
            (Self::EnUs, CommandPath::AgentSendKeys) => crate::localization::HelpDetails::new(
                "orchbee agent send-keys (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <key>... [--format text|json]",
                "Send 1..64 semantic keys or combinations (enter, ctrl+c, alt+left, ctrl+shift+a)\nto one live Agent terminal. Target: Session ID/name/Panel ID, --agent-session <id>, --name <alias>,\nor --panel <id|@panel:0x<hex>>. Panel selects its primary terminal.\nUses orchbee.terminal.send receipts. Never replays uncertain input.",
            ),
            (Self::ZhCn, CommandPath::AgentSendKeys) => crate::localization::HelpDetails::new(
                "orchbee agent send-keys (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <key>... [--format text|json]",
                "向一个存活智能体终端发送 1..64 个语义按键或组合（enter、ctrl+c、alt+left、ctrl+shift+a）。目标：会话 ID/名称/面板 ID、--agent-session <id>、--name <alias>\n或 --panel <id|@panel:0x<hex>>。面板选择主终端。使用 orchbee.terminal.send 回执。不重放不确定输入。",
            ),
            (Self::EnUs, CommandPath::AgentAttention) => super::client_control::HELP_AGENT_ATTENTION_EN,
            (Self::ZhCn, CommandPath::AgentAttention) => super::client_control::HELP_AGENT_ATTENTION_ZH,
            (Self::EnUs, CommandPath::AgentFocus) => super::client_control::HELP_AGENT_FOCUS_EN,
            (Self::ZhCn, CommandPath::AgentFocus) => super::client_control::HELP_AGENT_FOCUS_ZH,
            (Self::EnUs, CommandPath::AgentGet) => super::agent::HELP_AGENT_GET_EN,
            (Self::ZhCn, CommandPath::AgentGet) => super::agent::HELP_AGENT_GET_ZH,
            (Self::EnUs, CommandPath::AgentExplain) => super::agent::HELP_AGENT_EXPLAIN_EN,
            (Self::ZhCn, CommandPath::AgentExplain) => super::agent::HELP_AGENT_EXPLAIN_ZH,
            (Self::EnUs, CommandPath::AgentNames) => crate::localization::HelpDetails::new(
                "orchbee agent names [--format text|json]",
                "List this session runtime's live Agent names. Names disappear when their session ends or the session stops.",
            ),
            (Self::ZhCn, CommandPath::AgentNames) => crate::localization::HelpDetails::new(
                "orchbee agent names [--format text|json]",
                "列出当前会话运行时中的智能体名称。会话结束或会话停止后名称会释放。",
            ),
            (Self::EnUs, CommandPath::AgentResolve) => crate::localization::HelpDetails::new(
                "orchbee agent resolve <target> [--format text|json]\n       orchbee agent resolve --agent-session <id> | --name <alias> | --panel <id|@panel:0x<hex>> | --terminal <id|@terminal:0x<hex>>",
                "Resolve an exact Session ID first, then a runtime name, then a Panel ID.\nA Panel selects its primary terminal and requires one live Agent. Use --name to select an alias that collides with an ID.",
            ),
            (Self::ZhCn, CommandPath::AgentResolve) => crate::localization::HelpDetails::new(
                "orchbee agent resolve <target> [--format text|json]\n      orchbee agent resolve --agent-session <id> | --name <alias> | --panel <id|@panel:0x<hex>> | --terminal <id|@terminal:0x<hex>>",
                "优先按完整会话 ID 定位，再查运行时名称，最后回退到面板 ID。\nPanel 选择主终端，要求唯一存活智能体。名称与 ID 冲突时，用 --name 明确选择名称。",
            ),
            (Self::EnUs, CommandPath::AgentRename) => crate::localization::HelpDetails::new(
                "orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <new-name> [--format text|json]\n       orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) --clear",
                "Assign or atomically rename one live Agent. Name: [a-z][a-z0-9_-]{0,31}. Duplicate names are rejected. --clear releases only the name. Replace <target> with --agent-session <id>, --name <alias>, or --panel <id|@panel:0x<hex>> to select explicitly. Names reset when the session ends or the session stops.",
            ),
            (Self::ZhCn, CommandPath::AgentRename) => crate::localization::HelpDetails::new(
                "orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <new-name> [--format text|json]\n      orchbee agent rename (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) --clear",
                "为存活智能体命名或原子重命名。名称格式为 [a-z][a-z0-9_-]{0,31}，拒绝重名。--clear 只释放名称。可将 <target> 替换成 --agent-session <id> 或 --name <alias>。会话结束或会话停止后名称释放。",
            ),
            (Self::EnUs, CommandPath::AgentWait) => {
                if cfg!(windows) {
                    super::agent::HELP_AGENT_WAIT_WINDOWS_EN
                } else {
                    super::agent::HELP_AGENT_WAIT_EN
                }
            }
            (Self::ZhCn, CommandPath::AgentWait) => {
                if cfg!(windows) {
                    super::agent::HELP_AGENT_WAIT_WINDOWS_ZH
                } else {
                    super::agent::HELP_AGENT_WAIT_ZH
                }
            }
            (Self::EnUs, CommandPath::AgentDoctor) => super::agent::HELP_AGENT_DOCTOR_EN,
            (Self::ZhCn, CommandPath::AgentDoctor) => super::agent::HELP_AGENT_DOCTOR_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationList) => super::integration::HELP_AGENT_INTEGRATION_LIST_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationList) => super::integration::HELP_AGENT_INTEGRATION_LIST_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationExplain) => super::integration::HELP_AGENT_INTEGRATION_EXPLAIN_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationExplain) => super::integration::HELP_AGENT_INTEGRATION_EXPLAIN_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationEnable) => super::integration::HELP_AGENT_INTEGRATION_ENABLE_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationEnable) => super::integration::HELP_AGENT_INTEGRATION_ENABLE_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationDisable) => super::integration::HELP_AGENT_INTEGRATION_DISABLE_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationDisable) => super::integration::HELP_AGENT_INTEGRATION_DISABLE_ZH,
            (Self::EnUs, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_EN,
            (Self::ZhCn, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_ZH,
            (Self::EnUs, CommandPath::ApiSchema) => HELP_API_SCHEMA_EN,
            (Self::ZhCn, CommandPath::ApiSchema) => HELP_API_SCHEMA_ZH,
            (Self::EnUs, CommandPath::ConfigResetKeys) => super::configuration::HELP_CONFIG_RESET_KEYS_EN,
            (Self::ZhCn, CommandPath::ConfigResetKeys) => super::configuration::HELP_CONFIG_RESET_KEYS_ZH,
            #[cfg(windows)]
            (_, CommandPath::MachineList) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineAdd) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineRename) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineRemove) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineEnable) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineDisable) => self.cli_machine_help(path),
            #[cfg(windows)]
            (Self::EnUs, CommandPath::ChannelShow) => crate::localization::HelpDetails::new(
                "orchbee channel show [--format text|json]",
                "Read the Windows update.channel preference (stable or preview). Defaults to stable.\nValidates the complete config; a missing explicit ORCHBEE_CONFIG_FILE fails.\nDoes not write files, connect to a session, download, or change the installed cohort.",
            ),
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::ChannelShow) => crate::localization::HelpDetails::new(
                "orchbee channel show [--format text|json]",
                "读取 Windows update.channel 偏好（stable 或 preview），默认 stable。校验完整配置；显式 ORCHBEE_CONFIG_FILE 不存在时失败。不写文件、不连接会话、不下载，也不改变已安装 cohort。",
            ),
            #[cfg(windows)]
            (Self::EnUs, CommandPath::ChannelSet) => crate::localization::HelpDetails::new(
                "orchbee channel set <stable|preview> [--format text|json]",
                "Save the channel and immediately run update. Update failure retains the saved channel.\nUses ORCHBEE_UPDATE_MANIFEST; see orchbee update --help.",
            ),
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::ChannelSet) => crate::localization::HelpDetails::new(
                "orchbee channel set <stable|preview> [--format text|json]",
                "保存频道后立即更新；更新失败仍保留已保存的频道。使用 ORCHBEE_UPDATE_MANIFEST；参见 orchbee update --help。",
            ),
            #[cfg(windows)]
            (Self::EnUs, CommandPath::Update) => crate::localization::HelpDetails::new(
                "orchbee update [--format text|json]",
                "Install the configured channel as a complete immutable Windows cohort.\nORCHBEE_UPDATE_MANIFEST: explicit HTTPS URL or absolute local manifest path; {channel} expands to stable or preview.\nORCHBEE_INSTALL_DIR defaults to %LOCALAPPDATA%/Programs/Orchbee.\nValidates every file, publishes a release, then updates user PATH. Existing sessions keep their version.\nTotal deadline: 600 seconds. Ctrl+C cancels; receipts retain published paths and cleanup obligations.",
            ),
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::Update) => crate::localization::HelpDetails::new(
                "orchbee update [--format text|json]",
                "按配置的频道安装完整、独立的 Windows cohort。\nORCHBEE_UPDATE_MANIFEST：显式 HTTPS URL 或本地清单绝对路径；{channel} 替换为 stable 或 preview。\nORCHBEE_INSTALL_DIR 默认为 %LOCALAPPDATA%/Programs/Orchbee。逐文件校验并发布版本，再更新用户 PATH；现有会话保留原版本。总时限 600 秒。Ctrl+C 取消；回执保留已发布路径和待清理项。",
            ),
            (Self::EnUs, CommandPath::ConfigCheck) => super::configuration::HELP_CONFIG_CHECK_EN,
            (Self::ZhCn, CommandPath::ConfigCheck) => super::configuration::HELP_CONFIG_CHECK_ZH,
            #[cfg(windows)]
            (
                _,
                path @ (CommandPath::SessionList
                | CommandPath::SessionAttach
                | CommandPath::SessionStop
                | CommandPath::SessionDelete),
            ) => self.cli_session_command_help(path),
            (Self::EnUs, CommandPath::ServerStart) => super::server::HELP_SERVER_START_EN,
            (Self::ZhCn, CommandPath::ServerStart) => super::server::HELP_SERVER_START_ZH,
            (
                _,
                CommandPath::ServerAgentManifests
                | CommandPath::ServerUpdateAgentManifests
                | CommandPath::ServerReloadAgentManifests
                | CommandPath::ServerAgentManifestsStatus,
            ) => self.cli_agent_manifests_help(path),
            (Self::EnUs, CommandPath::ServerDiagnose) => super::server::HELP_SERVER_DIAGNOSE_EN,
            (Self::ZhCn, CommandPath::ServerDiagnose) => super::server::HELP_SERVER_DIAGNOSE_ZH,
            (Self::EnUs, CommandPath::ServerStop) => super::server::HELP_SERVER_STOP_EN,
            (Self::ZhCn, CommandPath::ServerStop) => super::server::HELP_SERVER_STOP_ZH,
            (Self::EnUs, CommandPath::ServerStopStatus) => super::server::HELP_SERVER_STOP_STATUS_EN,
            (Self::ZhCn, CommandPath::ServerStopStatus) => super::server::HELP_SERVER_STOP_STATUS_ZH,
        }
    }
}
