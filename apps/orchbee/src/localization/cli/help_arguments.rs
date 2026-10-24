//! Positional operand roles. Flags are described separately by the option catalog.
use super::super::Locale;
use crate::invocation::CommandPath;

#[derive(Clone, Copy)]
enum Argument {
    Project,
    Workspace,
    Workspaces,
    Tab,
    Panel,
    Terminal,
    TerminalOrPanel,
    Agent,
    Command,
    Text,
    Keys,
    NewName,
    Name,
    Args,
    Client,
    Operation,
    Request,
    #[cfg(windows)]
    Ssh,
    #[cfg(windows)]
    Machine,
    #[cfg(windows)]
    Channel,
    Profile,
    Schema,
    #[cfg(windows)]
    Site,
    Title,
    #[cfg(windows)]
    ConfiguredCommand,
    #[cfg(windows)]
    Pane,
    #[cfg(windows)]
    Action,
    #[cfg(windows)]
    Repository,
    #[cfg(windows)]
    InstalledRepository,
    #[cfg(windows)]
    Path,
    #[cfg(windows)]
    Plugin,
}

impl Locale {
    pub(crate) fn cli_help_arguments(self, path: CommandPath) -> Vec<(&'static str, &'static str)> {
        let roles: &[Argument] = match path {
            CommandPath::WorkspaceStatus => &[Argument::Request],
            CommandPath::WorkspaceResume => &[Argument::Request],
            CommandPath::TabStatus => &[Argument::Request],
            CommandPath::TabResume => &[Argument::Request],
            CommandPath::PanelStatus => &[Argument::Request],
            CommandPath::PanelResume => &[Argument::Request],
            CommandPath::ProjectStatus => &[Argument::Request],
            CommandPath::ProjectResume => &[Argument::Request],
            CommandPath::ClientStatus => &[Argument::Request],
            CommandPath::ServerStatus => &[Argument::Request],

            #[cfg(windows)]
            CommandPath::PanelGraphicsSet => &[Argument::Panel],
            #[cfg(windows)]
            CommandPath::PanelGraphicsClear => &[Argument::Panel],
            #[cfg(windows)]
            CommandPath::PanelGraphicsInfo => &[Argument::Panel],
            #[cfg(windows)]
            CommandPath::PanelGraphicsStream => &[Argument::Panel],
            CommandPath::ClientList => &[],
            CommandPath::ClientGet => &[Argument::Client],
            CommandPath::ClientControlStatus => &[Argument::Operation],
            CommandPath::WorkspaceFocus => &[Argument::Workspace],
            CommandPath::TabFocus => &[Argument::Tab],
            CommandPath::PanelFocus => &[Argument::Panel],
            CommandPath::PanelResize => &[Argument::Panel],
            CommandPath::LayoutSetSplitRatio => &[],
            #[cfg(windows)]
            CommandPath::LayoutExport => &[],
            #[cfg(windows)]
            CommandPath::LayoutApply => &[],
            CommandPath::PanelZoom => &[Argument::Panel],
            CommandPath::PanelLayout => &[Argument::Panel],
            CommandPath::PanelNeighbor => &[Argument::Panel],
            CommandPath::PanelEdges => &[Argument::Panel],
            CommandPath::Tui => &[],
            CommandPath::Status => &[],
            CommandPath::StatusServer => &[],
            CommandPath::StatusClient => &[],
            #[cfg(windows)]
            CommandPath::AgentViewGet => &[],
            #[cfg(windows)]
            CommandPath::AgentViewSet => &[],
            #[cfg(windows)]
            CommandPath::AgentViewClear => &[],
            CommandPath::AgentList => &[],
            CommandPath::AgentChanges => &[],
            CommandPath::AgentGet => &[Argument::Agent],
            CommandPath::AgentFocus => &[Argument::Agent],
            CommandPath::AgentAttention => &[Argument::Agent],
            CommandPath::AgentRead => &[Argument::Agent],
            CommandPath::AgentStart => &[Argument::Name, Argument::Args],
            CommandPath::AgentPrompt => &[Argument::Agent, Argument::Text],
            CommandPath::AgentSendKeys => &[Argument::Agent, Argument::Keys],
            CommandPath::AgentExplain => &[Argument::Agent],
            CommandPath::AgentWait => &[Argument::Agent],
            CommandPath::AgentDoctor => &[],
            CommandPath::AgentNames => &[],
            CommandPath::AgentResolve => &[Argument::Agent],
            CommandPath::AgentRename => &[Argument::Agent, Argument::NewName],
            CommandPath::AgentIntegrationList => &[],
            CommandPath::AgentIntegrationExplain => &[],
            CommandPath::AgentIntegrationEnable => &[],
            CommandPath::AgentIntegrationDisable => &[],
            CommandPath::TerminalAttach => &[Argument::Terminal],
            CommandPath::AgentAttach => &[Argument::Agent],
            CommandPath::TerminalObserve => &[Argument::Terminal],
            CommandPath::TerminalControl => &[Argument::Terminal],
            #[cfg(windows)]
            CommandPath::MachineList => &[],
            #[cfg(windows)]
            CommandPath::MachineAdd => &[Argument::Ssh],
            #[cfg(windows)]
            CommandPath::MachineRename => &[Argument::Machine],
            #[cfg(windows)]
            CommandPath::MachineRemove => &[Argument::Machine],
            #[cfg(windows)]
            CommandPath::MachineEnable => &[Argument::Machine],
            #[cfg(windows)]
            CommandPath::MachineDisable => &[Argument::Machine],
            #[cfg(windows)]
            CommandPath::ChannelShow => &[],
            #[cfg(windows)]
            CommandPath::ChannelSet => &[Argument::Channel],
            #[cfg(windows)]
            CommandPath::Update => &[],
            CommandPath::ProjectList => &[],
            CommandPath::ProjectGet => &[Argument::Project],
            CommandPath::ProjectDissolve => &[Argument::Project],
            CommandPath::ProjectCloseAll => &[Argument::Project],
            CommandPath::ProjectCloseStatus => &[],
            CommandPath::ProjectCloseResume => &[],
            CommandPath::WorkspaceSetProject => &[Argument::Workspace],
            CommandPath::WorkspaceCreateProject => &[Argument::Workspace],
            CommandPath::WorkspaceUngroup => &[Argument::Workspace],
            CommandPath::ProjectCreate => &[],
            CommandPath::ProjectRename => &[Argument::Project],
            CommandPath::WorkspaceList => &[],
            CommandPath::WorkspaceGet => &[Argument::Workspace],
            CommandPath::WorkspaceCreate => &[],
            CommandPath::WorkspaceRename => &[Argument::Workspace],
            CommandPath::PanelReportMetadata => &[Argument::Panel],
            CommandPath::PanelReportAgent => &[Argument::Panel],
            CommandPath::PanelReportAgentSession => &[Argument::Panel],
            CommandPath::PanelClearAgentAuthority => &[Argument::Panel],
            CommandPath::PanelReleaseAgent => &[Argument::Panel],
            CommandPath::WorkspaceReportMetadata => &[Argument::Workspace],
            CommandPath::WorkspaceMove => &[Argument::Workspace],
            CommandPath::WorkspaceMoveBlock => &[Argument::Workspaces],
            CommandPath::WorkspaceAcceptDirectory => &[Argument::Workspace],
            CommandPath::WorkspaceClose => &[Argument::Workspace],
            CommandPath::WorkspaceCloseStatus => &[],
            CommandPath::WorkspaceCloseResume => &[],
            CommandPath::TabList => &[],
            CommandPath::TabGet => &[Argument::Tab],
            CommandPath::TabCreate => &[],
            CommandPath::TabRename => &[Argument::Tab],
            CommandPath::PanelRename => &[Argument::Panel],
            CommandPath::PanelInput => &[Argument::Panel],
            CommandPath::PanelSwap => &[Argument::Panel],
            CommandPath::TabMove => &[Argument::Tab],
            CommandPath::TabClose => &[Argument::Tab],
            CommandPath::PanelList => &[],
            CommandPath::PanelCurrent => &[Argument::Panel],
            CommandPath::PanelGet => &[Argument::Panel],
            CommandPath::PanelProcessInfo => &[Argument::Panel],
            #[cfg(windows)]
            CommandPath::PanelCreateTerminal => &[Argument::Panel],
            CommandPath::PanelSplit => &[Argument::Panel],
            CommandPath::PanelClose => &[Argument::Panel],
            CommandPath::PanelMove => &[Argument::Panel],
            CommandPath::TerminalList => &[],
            CommandPath::TerminalGet => &[Argument::Terminal],
            CommandPath::TerminalRead => &[Argument::TerminalOrPanel],
            #[cfg(windows)]
            CommandPath::TerminalSelectionRead => &[Argument::Terminal],
            #[cfg(windows)]
            CommandPath::TerminalCopyMotion => &[Argument::Terminal],
            #[cfg(windows)]
            CommandPath::TerminalCopySearch => &[Argument::Terminal],
            #[cfg(windows)]
            CommandPath::TerminalLinkResolve => &[Argument::Terminal],
            #[cfg(windows)]
            CommandPath::TerminalLinkActivate => &[Argument::Terminal],
            #[cfg(windows)]
            CommandPath::TerminalClear => &[Argument::Terminal],
            #[cfg(windows)]
            CommandPath::TerminalEditScrollback => &[Argument::Terminal],
            CommandPath::TerminalWaitOutput => &[Argument::TerminalOrPanel],
            CommandPath::TerminalSend => &[Argument::TerminalOrPanel],
            CommandPath::TerminalRun => &[Argument::TerminalOrPanel, Argument::Command],
            CommandPath::WorktreeList => &[],
            CommandPath::WorktreeCreate => &[],
            CommandPath::WorktreeOpen => &[],
            CommandPath::WorktreeRemove => &[],
            CommandPath::WorktreeStatus => &[Argument::Operation],
            CommandPath::WorktreeResume => &[Argument::Operation],
            CommandPath::LaunchProfileList => &[],
            CommandPath::LaunchProfileGet => &[Argument::Profile],
            CommandPath::LaunchProfileCreate => &[],
            CommandPath::LaunchProfileDelete => &[Argument::Profile],
            CommandPath::ApiSnapshot => &[],
            CommandPath::ApiSchema => &[Argument::Schema],
            CommandPath::ConfigCheck => &[],
            CommandPath::ConfigResetKeys => &[],
            CommandPath::SiteStart => &[],
            #[cfg(windows)]
            CommandPath::SessionList => &[],
            #[cfg(windows)]
            CommandPath::SessionAttach => &[Argument::Site],
            #[cfg(windows)]
            CommandPath::SessionStop => &[Argument::Site],
            #[cfg(windows)]
            CommandPath::SessionDelete => &[Argument::Site],
            CommandPath::SiteDiagnose => &[],
            CommandPath::SiteUpdateAgentManifests => &[],
            CommandPath::SiteConfig => &[],
            CommandPath::SiteReloadConfig => &[],
            CommandPath::TerminalTitleSet => &[Argument::Title],
            CommandPath::TerminalTitleClear => &[],
            #[cfg(windows)]
            CommandPath::EventsSubscribe => &[],
            #[cfg(windows)]
            CommandPath::EventsWait => &[],
            #[cfg(windows)]
            CommandPath::CommandList => &[],
            #[cfg(windows)]
            CommandPath::CommandInvoke => &[Argument::ConfiguredCommand],
            #[cfg(windows)]
            CommandPath::PluginPaneOpen => &[],
            #[cfg(windows)]
            CommandPath::PopupClose => &[],
            #[cfg(windows)]
            CommandPath::PluginPaneFocus => &[Argument::Pane],
            #[cfg(windows)]
            CommandPath::PluginPaneClose => &[Argument::Pane],
            #[cfg(windows)]
            CommandPath::PluginActionInvoke => &[Argument::Action],
            #[cfg(windows)]
            CommandPath::PluginLogList => &[Argument::Plugin],
            #[cfg(windows)]
            CommandPath::PluginInstall => &[Argument::Repository],
            #[cfg(windows)]
            CommandPath::PluginUninstall => &[Argument::InstalledRepository],
            #[cfg(windows)]
            CommandPath::PluginLink => &[Argument::Path],
            #[cfg(windows)]
            CommandPath::PluginUnlink => &[Argument::Plugin],
            #[cfg(windows)]
            CommandPath::PluginEnable => &[Argument::Plugin],
            #[cfg(windows)]
            CommandPath::PluginDisable => &[Argument::Plugin],
            #[cfg(windows)]
            CommandPath::PluginList => &[Argument::Plugin],
            #[cfg(windows)]
            CommandPath::PluginConfigDir => &[Argument::Plugin],
            #[cfg(windows)]
            CommandPath::PluginActionList => &[Argument::Plugin],
            CommandPath::NotificationShow => &[Argument::Title],
            CommandPath::SiteAgentManifests => &[],
            CommandPath::SiteReloadAgentManifests => &[],
            CommandPath::SiteAgentManifestsStatus => &[Argument::Operation],
            CommandPath::SiteStop => &[],
            CommandPath::SiteStopStatus => &[Argument::Operation],
        };
        roles.iter().map(|role| {
            let (label, english, chinese) = match *role {
                Argument::Project => ("<target>", "Project ID, @project:0x<hex>, or exact label; ID/selector syntax takes priority. Use the real ID for labels with that syntax.", "项目 ID、@project:0x<hex> 或精确名称；优先解析 ID 和选择器。名称形似 ID 或选择器时，请使用实际 ID。"),
                Argument::Workspace => ("<target>", "Workspace ID, @workspace:0x<hex>, or exact label; ID/selector syntax takes priority. Use the real ID for labels with that syntax.", "工作区 ID、@workspace:0x<hex> 或精确名称；优先解析 ID 和选择器。名称形似 ID 或选择器时，请使用实际 ID。"),
                Argument::Workspaces => ("<target> ...", "One or more workspaces in the desired order; use IDs, @workspace:0x<hex>, or exact labels.", "按期望顺序列出一个或多个工作区；使用 ID、@workspace:0x<hex> 或精确名称。"),
                Argument::Tab => ("<target>", "Tab ID, @tab:0x<hex>, or exact label; ID/selector syntax takes priority. Use the real ID for labels with that syntax.", "标签页 ID、@tab:0x<hex> 或精确名称；优先解析 ID 和选择器。名称形似 ID 或选择器时，请使用实际 ID。"),
                Argument::Panel => ("<panel>", "Panel ID or @panel:0x<hex>; list panels with orchbee panel list.", "面板 ID 或 @panel:0x<hex>；使用 orchbee panel list 列出面板。"),
                Argument::TerminalOrPanel => ("<target>", "Terminal or Panel ID; typed @terminal:0x<hex> and @panel:0x<hex> remove ambiguity. A Panel selects its primary Terminal.", "终端或面板 ID；使用 @terminal:0x<hex> 与 @panel:0x<hex> 消除歧义。面板选择其主终端。"),
                Argument::Terminal => ("<terminal>", "Terminal ID or @terminal:0x<hex>; list terminals with orchbee terminal list.", "终端 ID 或 @terminal:0x<hex>；使用 orchbee terminal list 列出终端。"),
                Argument::Agent => ("<target>", "Agent session ID, saved name, or panel target. Choose exactly one positional target or supported selector flag.", "Agent 会话 ID、保存的名称或面板目标。位置参数和受支持的选择器选项中只能选择一种。"),
                Argument::Command => ("<command>", "One shell command string; quote it if it contains spaces.", "一条 Shell 命令字符串；包含空格时必须加引号。"),
                Argument::Text => ("<text>", "Prompt text as one argument; quote spaces.", "作为一个参数传入的提示词；包含空格时加引号。"),
                Argument::Keys => ("<key> ...", "One or more semantic keys, such as enter or ctrl+c.", "一个或多个语义按键，例如 enter 或 ctrl+c。"),
                Argument::NewName => ("<new-name>", "New session name; omit when using --clear.", "新的会话名称；使用 --clear 时省略。"),
                Argument::Name => ("<name>", "Name to bind to the Agent after it is ready.", "Agent 就绪后要绑定的名称。"),
                Argument::Args => ("-- <args> ...", "Arguments passed to the Agent; --arg can be repeated instead.", "传给 Agent 的参数；也可重复使用 --arg。"),
                Argument::Client => ("<client-id>", "Full client ID returned by orchbee client list.", "orchbee client list 返回的完整客户端 ID。"),
                Argument::Request => ("<request-id>", "Exact retained request ID returned by the operation.", "操作返回的精确保留请求 ID。"),
                Argument::Operation => ("<operation-id>", "Complete operation ID returned by the original request.", "原请求返回的完整操作 ID。"),
                #[cfg(windows)]
                Argument::Ssh => ("<ssh-target>", "OpenSSH destination, such as user@host; batch authentication must already work.", "OpenSSH 目标，例如 user@host；须已能进行批处理认证。"),
                #[cfg(windows)]
                Argument::Machine => ("<profile-id>", "Exact saved machine ID from orchbee machine list.", "orchbee machine list 中保存的精确机器 ID。"),
                #[cfg(windows)]
                Argument::Channel => ("<stable|preview>", "Release channel to save and install.", "要保存并安装的发布频道。"),
                Argument::Profile => ("<selector>", "Launch profile ID or an unambiguous saved name.", "启动配置 ID 或无歧义的保存名称。"),
                Argument::Schema => ("[<schema-id>]", "Optional schema ID; omit to print the schema dictionary.", "可选 Schema ID；省略时输出结构字典。"),
                #[cfg(windows)]
                Argument::Site => ("<name>", "Case-sensitive local session name. An explicit --session must match.", "区分大小写的本地会话名称；显式 --session 必须与其一致。"),
                Argument::Title => ("<title>", "Display title as one argument.", "作为一个参数传入的显示标题。"),
                #[cfg(windows)]
                Argument::ConfiguredCommand => ("<command-id>", "Exact ID returned by orchbee command list; starts with cmd_.", "orchbee command list 返回的精确 ID，以 cmd_ 开头。"),
                #[cfg(windows)]
                Argument::Pane => ("<pane-id>", "Exact pane ID belonging to a plugin.", "属于插件的精确面板 ID。"),
                #[cfg(windows)]
                Argument::Action => ("<action-id>", "Local action ID or qualified plugin.action ID.", "本地操作 ID，或 plugin.action 完整 ID。"),
                #[cfg(windows)]
                Argument::Repository => ("<owner/repo[/subdir]>", "GitHub repository, optionally with a plugin subdirectory.", "GitHub 仓库，可带插件子目录。"),
                #[cfg(windows)]
                Argument::InstalledRepository => ("<plugin-id|owner/repo[/subdir]>", "Installed plugin ID or its managed repository reference.", "已安装的插件 ID，或其托管仓库引用。"),
                #[cfg(windows)]
                Argument::Path => ("<path>", "Local plugin manifest file or plugin directory.", "本地插件清单文件或插件目录。"),
                #[cfg(windows)]
                Argument::Plugin => ("<plugin-id>", "Registered plugin ID; optional only where usage shows brackets.", "已注册的插件 ID；仅当用法中带方括号时可省略。"),
            };
            (label, match self { Self::EnUs => english, Self::ZhCn => chinese })
        }).collect()
    }
}
