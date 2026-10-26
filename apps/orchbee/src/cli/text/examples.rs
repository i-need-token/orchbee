//! Executable syntax examples for each command, checked by the real parser.
//! Operand values are illustrative; tests parse them without executing effects.
use crate::invocation::CommandPath;

pub(super) fn arguments(path: CommandPath) -> &'static [&'static str] {
    match path {
        CommandPath::WorkspaceStatus => &["00000000000000000000000000000001"],
        CommandPath::WorkspaceResume => &[
            "00000000000000000000000000000001",
            "--generation",
            "1",
            "--confirm",
        ],
        CommandPath::TabStatus => &["00000000000000000000000000000001"],
        CommandPath::TabResume => &[
            "00000000000000000000000000000001",
            "--generation",
            "1",
            "--confirm",
        ],
        CommandPath::PanelStatus => &["00000000000000000000000000000001"],
        CommandPath::PanelResume => &[
            "00000000000000000000000000000001",
            "--generation",
            "1",
            "--confirm",
        ],
        CommandPath::ProjectStatus => &["00000000000000000000000000000001"],
        CommandPath::ProjectResume => &[
            "00000000000000000000000000000001",
            "--generation",
            "1",
            "--confirm",
        ],
        CommandPath::ClientStatus => &["00000000000000000000000000000001"],
        CommandPath::ServerStatus => &["00000000000000000000000000000001"],

        #[cfg(windows)]
        CommandPath::PanelGraphicsSet => &["@panel:0xabcd", "--spec", "@graphics.png"],
        #[cfg(windows)]
        CommandPath::PanelGraphicsClear => &["@panel:0xabcd"],
        #[cfg(windows)]
        CommandPath::PanelGraphicsInfo => &["@panel:0xabcd"],
        #[cfg(windows)]
        CommandPath::PanelGraphicsStream => &["@panel:0xabcd"],
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
        CommandPath::TerminalAttach => &["@terminal:0xabcd"],
        CommandPath::AgentAttach => &["review"],
        CommandPath::TerminalObserve => &["@terminal:0xabcd"],
        CommandPath::TerminalControl => &["@terminal:0xabcd"],
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
            &["@panel:0xabcd", "--source", "cli", "--title", "review"]
        }
        CommandPath::PanelReportAgent => &[
            "@panel:0xabcd",
            "--source",
            "cli",
            "--agent",
            "codex",
            "--state",
            "working",
        ],
        CommandPath::PanelClearAgentAuthority => &["@panel:0xabcd", "--source", "cli"],
        CommandPath::PanelReleaseAgent => &["@panel:0xabcd", "--source", "cli", "--agent", "codex"],
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
        CommandPath::PanelRename => &["@panel:0xabcd", "--label", "review"],
        CommandPath::PanelInput => &["--current", "--right-click", "panel"],
        CommandPath::PanelSwap => &["--current", "--direction", "right"],
        CommandPath::TabMove => &["main", "--order", "next"],
        CommandPath::TabClose => &["main", "--confirm"],
        CommandPath::PanelList => &[],
        CommandPath::PanelCurrent => &[],
        CommandPath::PanelGet => &["@panel:0xabcd"],
        CommandPath::PanelProcessInfo => &["--current"],
        #[cfg(windows)]
        CommandPath::PanelCreateTerminal => &["--current", "--child", "--label", "review"],
        CommandPath::PanelSplit => &["--current", "--direction", "right"],
        CommandPath::PanelClose => &["@panel:0xabcd", "--confirm"],
        CommandPath::PanelMove => {
            if cfg!(windows) {
                &[
                    "@panel:0xabcd",
                    "--new-tab",
                    "--workspace",
                    "work",
                    "--no-focus",
                ]
            } else {
                &["@panel:0xabcd", "--new-tab", "--label", "review"]
            }
        }
        CommandPath::TerminalList => &[],
        CommandPath::TerminalGet => &["@terminal:0xabcd"],
        CommandPath::TerminalRead => &["@terminal:0xabcd", "--source", "recent", "--lines", "40"],
        #[cfg(windows)]
        CommandPath::TerminalSelectionRead => {
            &["@terminal:0xabcd", "--anchor", "0:0", "--cursor", "0:10"]
        }
        #[cfg(windows)]
        CommandPath::TerminalCopyMotion => &[
            "@terminal:0xabcd",
            "--cursor",
            "0:0",
            "--motion",
            "line-end",
        ],
        #[cfg(windows)]
        CommandPath::TerminalCopySearch => &[
            "@terminal:0xabcd",
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
        CommandPath::TerminalLinkResolve => &["@terminal:0xabcd", "--cursor", "0:0"],
        #[cfg(windows)]
        CommandPath::TerminalLinkActivate => &["@terminal:0xabcd", "--cursor", "0:0"],
        #[cfg(windows)]
        CommandPath::TerminalClear => &["@terminal:0xabcd"],
        #[cfg(windows)]
        CommandPath::TerminalEditScrollback => &["@terminal:0xabcd"],
        CommandPath::TerminalWaitOutput => {
            &["@terminal:0xabcd", "--match", "ready", "--timeout", "30000"]
        }
        CommandPath::TerminalSend => &["@terminal:0xabcd", "--text", "git status", "--enter"],
        CommandPath::TerminalRun => &["@terminal:0xabcd", "git status"],
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
        CommandPath::ServerStart => &[],
        #[cfg(windows)]
        CommandPath::SessionList => &[],
        #[cfg(windows)]
        CommandPath::SessionAttach => &["work"],
        #[cfg(windows)]
        CommandPath::SessionStop => &["work", "--confirm"],
        #[cfg(windows)]
        CommandPath::SessionDelete => &["work", "--confirm"],
        CommandPath::ServerDiagnose => &[],
        CommandPath::ServerUpdateAgentManifests => &[],
        CommandPath::ServerConfig => &[],
        CommandPath::ServerReloadConfig => &[],
        CommandPath::TerminalTitleSet => &["Review"],
        CommandPath::TerminalTitleClear => &[],
        #[cfg(windows)]
        CommandPath::EventsSubscribe => &["--event", "workspace.created", "--timeout", "30000"],
        #[cfg(windows)]
        CommandPath::EventsWait => &[
            "--event",
            "panel.output_matched",
            "--panel",
            "@panel:0xabcd",
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
        CommandPath::ServerAgentManifests => &[],
        CommandPath::ServerReloadAgentManifests => &[],
        CommandPath::ServerAgentManifestsStatus => &["00000000000000000000000000000001"],
        CommandPath::ServerStop => &["--confirm"],
        CommandPath::ServerStopStatus => &["00000000000000000000000000000001"],
    }
}
