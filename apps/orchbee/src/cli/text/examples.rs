//! Executable syntax examples for each command, checked by the real parser.
//! Operand values are illustrative; tests parse them without executing effects.
use crate::invocation::CommandPath;

pub(super) fn arguments(path: CommandPath) -> &'static [&'static str] {
    match path {
        #[cfg(windows)]
        CommandPath::PanelGraphicsSet => &["@panel:abcd", "--spec", "@graphics.png"],
        #[cfg(windows)]
        CommandPath::PanelGraphicsClear => &["@panel:abcd"],
        #[cfg(windows)]
        CommandPath::PanelGraphicsInfo => &["@panel:abcd"],
        #[cfg(windows)]
        CommandPath::PanelGraphicsStream => &["@panel:abcd"],
        CommandPath::ClientList => &[],
        CommandPath::ClientGet => &["00000000000000000000000000000001"],
        CommandPath::ClientControlStatus => &["00000000000000000000000000000001"],
        CommandPath::WorkspaceFocus => &["work"],
        CommandPath::TabFocus => &["main"],
        CommandPath::PanelFocus => &["--current"],
        CommandPath::PanelResize => &["--current", "--direction", "right", "--amount", "0.05"],
        CommandPath::LayoutSetSplitRatio => &["--current", "--path", "root", "--ratio", "0.5"],
        #[cfg(windows)]
        CommandPath::LayoutExport => &["--current"],
        #[cfg(windows)]
        CommandPath::LayoutApply => &["--file", "layout.json", "--workspace", "work"],
        CommandPath::PanelZoom => &["--current", "--toggle"],
        CommandPath::PanelLayout => &["--current"],
        CommandPath::PanelNeighbor => &["--current", "--direction", "right"],
        CommandPath::PanelEdges => &["--current"],
        CommandPath::Tui => &["--mouse", "on"],
        CommandPath::Status => &[],
        CommandPath::StatusServer => &[],
        CommandPath::StatusClient => &[],
        #[cfg(windows)]
        CommandPath::AgentViewGet => &[],
        #[cfg(windows)]
        CommandPath::AgentViewSet => &[
            "--spec",
            "{\"source\":\"cli\",\"filter\":{\"op\":\"eq\",\"field\":\"status\",\"value\":\"idle\"}}",
        ],
        #[cfg(windows)]
        CommandPath::AgentViewClear => &["--source", "cli"],
        CommandPath::AgentList => &[],
        CommandPath::AgentChanges => &["--limit", "20"],
        CommandPath::AgentGet => &["review"],
        CommandPath::AgentFocus => &["review"],
        CommandPath::AgentAttention => &["review"],
        CommandPath::AgentRead => &["review", "--source", "recent", "--lines", "40"],
        CommandPath::AgentStart => &["review", "--provider", "codex"],
        CommandPath::AgentPrompt => &["review", "Review the current changes"],
        CommandPath::AgentSendKeys => &["review", "enter"],
        CommandPath::AgentExplain => &["review"],
        CommandPath::AgentWait => &["review", "--until", "idle", "--timeout", "30000"],
        CommandPath::AgentDoctor => &["--provider", "codex"],
        CommandPath::AgentNames => &[],
        CommandPath::AgentResolve => &["--name", "review"],
        CommandPath::AgentRename => &["review", "reviewer"],
        CommandPath::AgentIntegrationList => &[],
        CommandPath::AgentIntegrationExplain => &["--provider", "codex"],
        CommandPath::AgentIntegrationEnable => &["--provider", "codex"],
        CommandPath::AgentIntegrationDisable => &["--provider", "codex"],
        CommandPath::TerminalAttach => &["@terminal:abcd"],
        CommandPath::AgentAttach => &["review"],
        CommandPath::TerminalObserve => &["@terminal:abcd"],
        CommandPath::TerminalControl => &["@terminal:abcd"],
        #[cfg(windows)]
        CommandPath::MachineList => &[],
        #[cfg(windows)]
        CommandPath::MachineAdd => &["user@host", "--label", "remote"],
        #[cfg(windows)]
        CommandPath::MachineRename => &["00000000000000000000000000000001", "--label", "remote"],
        #[cfg(windows)]
        CommandPath::MachineRemove => &["00000000000000000000000000000001"],
        #[cfg(windows)]
        CommandPath::MachineEnable => &["00000000000000000000000000000001"],
        #[cfg(windows)]
        CommandPath::MachineDisable => &["00000000000000000000000000000001"],
        #[cfg(windows)]
        CommandPath::ChannelShow => &[],
        #[cfg(windows)]
        CommandPath::ChannelSet => &["stable"],
        #[cfg(windows)]
        CommandPath::Update => &[],
        CommandPath::ProjectList => &[],
        CommandPath::ProjectGet => &["app"],
        CommandPath::ProjectDissolve => &["app"],
        CommandPath::ProjectCloseAll => &["app", "--confirm"],
        CommandPath::ProjectCloseStatus => &["--request", "00000000000000000000000000000001"],
        CommandPath::ProjectCloseResume => &[
            "--request",
            "00000000000000000000000000000001",
            "--generation",
            "1",
            "--confirm",
        ],
        CommandPath::WorkspaceSetProject => &["work", "--project", "app"],
        CommandPath::WorkspaceCreateProject => &["work", "--label", "app"],
        CommandPath::WorkspaceUngroup => &["work"],
        CommandPath::ProjectCreate => &["--label", "app"],
        CommandPath::ProjectRename => &["app", "--label", "product"],
        CommandPath::WorkspaceList => &[],
        CommandPath::WorkspaceGet => &["work"],
        CommandPath::WorkspaceCreate => &["--cwd", ".", "--label", "work"],
        CommandPath::WorkspaceRename => &["work", "--label", "review"],
        CommandPath::PanelReportMetadata => {
            &["@panel:abcd", "--source", "cli", "--title", "review"]
        }
        CommandPath::PanelReportAgent => &[
            "@panel:abcd",
            "--source",
            "cli",
            "--agent",
            "codex",
            "--state",
            "working",
        ],
        CommandPath::PanelReportAgentSession => &[
            "@panel:abcd",
            "--source",
            "cli",
            "--agent",
            "codex",
            "--agent-session-id",
            "session-1",
        ],
        CommandPath::PanelClearAgentAuthority => &["@panel:abcd", "--source", "cli"],
        CommandPath::PanelReleaseAgent => &["@panel:abcd", "--source", "cli", "--agent", "codex"],
        CommandPath::WorkspaceReportMetadata => {
            &["work", "--source", "cli", "--token", "task=review"]
        }
        CommandPath::WorkspaceMove => &["work", "--insert-index", "0"],
        CommandPath::WorkspaceMoveBlock => &["work", "review", "--before", "main"],
        CommandPath::WorkspaceAcceptDirectory => {
            if cfg!(windows) {
                &["work", "--path", "C:/src/project"]
            } else {
                &["work", "--path", "/src/project"]
            }
        }
        CommandPath::WorkspaceClose => &["work", "--confirm"],
        CommandPath::WorkspaceCloseStatus => &["--workspace", "00000000000000000000000000000001"],
        CommandPath::WorkspaceCloseResume => &[
            "--workspace",
            "00000000000000000000000000000001",
            "--request",
            "00000000000000000000000000000001",
            "--generation",
            "1",
            "--confirm",
        ],
        CommandPath::TabList => &["--workspace", "work"],
        CommandPath::TabGet => &["main"],
        CommandPath::TabCreate => &["--workspace", "work", "--label", "main"],
        CommandPath::TabRename => &["main", "--label", "review"],
        CommandPath::PanelRename => &["@panel:abcd", "--label", "review"],
        CommandPath::PanelInput => &["--current", "--right-click", "panel"],
        CommandPath::PanelSwap => &["--current", "--direction", "right"],
        CommandPath::TabMove => &["main", "--order", "next"],
        CommandPath::TabClose => &["main", "--confirm"],
        CommandPath::PanelList => &[],
        CommandPath::PanelCurrent => &[],
        CommandPath::PanelGet => &["@panel:abcd"],
        CommandPath::PanelProcessInfo => &["--current"],
        #[cfg(windows)]
        CommandPath::PanelCreateTerminal => &["--current", "--child", "--label", "review"],
        CommandPath::PanelSplit => &["--current", "--direction", "right"],
        CommandPath::PanelClose => &["@panel:abcd", "--confirm"],
        CommandPath::PanelMove => {
            if cfg!(windows) {
                &[
                    "@panel:abcd",
                    "--new-tab",
                    "--workspace",
                    "work",
                    "--no-focus",
                ]
            } else {
                &["@panel:abcd", "--new-tab", "--label", "review"]
            }
        }
        CommandPath::TerminalList => &[],
        CommandPath::TerminalGet => &["@terminal:abcd"],
        CommandPath::TerminalRead => &["@terminal:abcd", "--source", "recent", "--lines", "40"],
        #[cfg(windows)]
        CommandPath::TerminalSelectionRead => {
            &["@terminal:abcd", "--anchor", "0:0", "--cursor", "0:10"]
        }
        #[cfg(windows)]
        CommandPath::TerminalCopyMotion => {
            &["@terminal:abcd", "--cursor", "0:0", "--motion", "line-end"]
        }
        #[cfg(windows)]
        CommandPath::TerminalCopySearch => &[
            "@terminal:abcd",
            "--cursor",
            "0:0",
            "--query",
            "ready",
            "--search-direction",
            "forward",
            "--content-revision",
            "1",
        ],
        #[cfg(windows)]
        CommandPath::TerminalLinkResolve => &["@terminal:abcd", "--cursor", "0:0"],
        #[cfg(windows)]
        CommandPath::TerminalLinkActivate => &["@terminal:abcd", "--cursor", "0:0"],
        #[cfg(windows)]
        CommandPath::TerminalClear => &["@terminal:abcd"],
        #[cfg(windows)]
        CommandPath::TerminalEditScrollback => &["@terminal:abcd"],
        CommandPath::TerminalWaitOutput => {
            &["@terminal:abcd", "--match", "ready", "--timeout", "30000"]
        }
        CommandPath::TerminalSend => &["@terminal:abcd", "--text", "git status", "--enter"],
        CommandPath::TerminalRun => &["@terminal:abcd", "git status"],
        CommandPath::WorktreeList => &["--cwd", "."],
        CommandPath::WorktreeCreate => &["--cwd", ".", "--branch", "review"],
        CommandPath::WorktreeOpen => &["--cwd", ".", "--branch", "review"],
        CommandPath::WorktreeRemove => &["--workspace", "work", "--confirm"],
        CommandPath::WorktreeStatus => &["00000000000000000000000000000001"],
        CommandPath::WorktreeResume => &[
            "00000000000000000000000000000001",
            "--generation",
            "1",
            "--confirm",
        ],
        CommandPath::LaunchProfileList => &[],
        CommandPath::LaunchProfileGet => &["review"],
        CommandPath::LaunchProfileCreate => {
            if cfg!(windows) {
                &[
                    "--name",
                    "review",
                    "--exec",
                    "C:/Tools/agent.exe",
                    "--arg",
                    "--version",
                ]
            } else {
                &[
                    "--name",
                    "review",
                    "--exec",
                    "/usr/bin/env",
                    "--arg",
                    "--version",
                ]
            }
        }
        CommandPath::LaunchProfileDelete => &["review"],
        CommandPath::ApiSnapshot => &[],
        CommandPath::ApiSchema => &["orchbee.status"],
        CommandPath::ConfigCheck => &[],
        CommandPath::ConfigResetKeys => &[],
        #[cfg(windows)]
        CommandPath::ConfigImportHerdr => &[
            "--input-file",
            "herdr.toml",
            "--output-file",
            "orchbee.toml",
            "--dry-run",
        ],
        CommandPath::SiteStart => &[],
        #[cfg(windows)]
        CommandPath::SessionList => &[],
        #[cfg(windows)]
        CommandPath::SessionAttach => &["work"],
        #[cfg(windows)]
        CommandPath::SessionStop => &["work", "--confirm"],
        #[cfg(windows)]
        CommandPath::SessionDelete => &["work", "--confirm"],
        CommandPath::SiteDiagnose => &[],
        CommandPath::SiteUpdateAgentManifests => &[],
        CommandPath::SiteConfig => &[],
        CommandPath::SiteReloadConfig => &[],
        CommandPath::TerminalTitleSet => &["Review"],
        CommandPath::TerminalTitleClear => &[],
        #[cfg(windows)]
        CommandPath::EventsSubscribe => &["--event", "workspace.created", "--timeout", "30000"],
        #[cfg(windows)]
        CommandPath::EventsWait => &[
            "--event",
            "panel.output_matched",
            "--panel",
            "@panel:abcd",
            "--match",
            "ready",
        ],
        #[cfg(windows)]
        CommandPath::CommandList => &[],
        #[cfg(windows)]
        CommandPath::CommandInvoke => &["cmd_example"],
        #[cfg(windows)]
        CommandPath::PluginPaneOpen => &["--plugin", "example.plugin", "--entrypoint", "main"],
        #[cfg(windows)]
        CommandPath::PopupClose => &[],
        #[cfg(windows)]
        CommandPath::PluginPaneFocus => &["00000000000000000000000000000001"],
        #[cfg(windows)]
        CommandPath::PluginPaneClose => &["00000000000000000000000000000001"],
        #[cfg(windows)]
        CommandPath::PluginActionInvoke => &[
            "example.plugin.action",
            "--context",
            "{\"selectedText\":\"example\"}",
        ],
        #[cfg(windows)]
        CommandPath::PluginLogList => &["--plugin", "example.plugin", "--limit", "20"],
        #[cfg(windows)]
        CommandPath::PluginInstall => &["owner/plugin", "--yes"],
        #[cfg(windows)]
        CommandPath::PluginUninstall => &["owner/plugin"],
        #[cfg(windows)]
        CommandPath::PluginLink => &["./plugin"],
        #[cfg(windows)]
        CommandPath::PluginUnlink => &["example.plugin"],
        #[cfg(windows)]
        CommandPath::PluginEnable => &["example.plugin"],
        #[cfg(windows)]
        CommandPath::PluginDisable => &["example.plugin"],
        #[cfg(windows)]
        CommandPath::PluginList => &[],
        #[cfg(windows)]
        CommandPath::PluginConfigDir => &["example.plugin"],
        #[cfg(windows)]
        CommandPath::PluginActionList => &["--plugin", "example.plugin"],
        CommandPath::NotificationShow => &["Ready", "--body", "Review is complete"],
        CommandPath::SiteAgentManifests => &[],
        CommandPath::SiteReloadAgentManifests => &[],
        CommandPath::SiteAgentManifestsStatus => &["00000000000000000000000000000001"],
        CommandPath::SiteStop => &["--confirm"],
        CommandPath::SiteStopStatus => &["00000000000000000000000000000001"],
    }
}
