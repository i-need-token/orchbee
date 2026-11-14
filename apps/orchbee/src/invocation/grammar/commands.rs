//! Command paths and their structural admission, independent of the host platform.

use super::{Availability, CommandSpec, TargetRule};

macro_rules! commands {
    ($($name:ident => [$first:literal $(, $word:literal)*], $operands:expr, $target:ident, $availability:ident, $format:expr, $machine:expr, $cwd:expr;)*) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(crate) enum CommandPath { $($name,)* }
        impl CommandPath {
            pub(crate) const fn as_str(self) -> &'static str { match self { $(Self::$name => concat!($first $(, " ", $word)*),)* } }
        }
        pub(crate) const COMMANDS: &[CommandSpec] = &[$(CommandSpec {
            id: CommandPath::$name, words: &[$first $(, $word)*], key: concat!($first $(, ":", $word)*),
            maximum_operands: $operands, target: TargetRule::$target, availability: Availability::$availability,
            format: $format, machine: $machine, cwd_values: $cwd,
        },)*];
    };
}

commands! {
    WorkspaceStatus => ["workspace", "status"], 1, Operand, Portable, true, true, false;
    WorkspaceResume => ["workspace", "resume"], 1, Operand, Portable, true, true, false;
    TabStatus => ["tab", "status"], 1, Operand, Portable, true, true, false;
    TabResume => ["tab", "resume"], 1, Operand, Portable, true, true, false;
    PanelStatus => ["panel", "status"], 1, Panel, Portable, true, true, false;
    PanelResume => ["panel", "resume"], 1, Panel, Portable, true, true, false;
    ProjectStatus => ["project", "status"], 1, Operand, Portable, true, true, false;
    ProjectResume => ["project", "resume"], 1, Operand, Portable, true, true, false;
    ClientStatus => ["client", "status"], 1, Operand, Portable, true, true, false;
    ServerStatus => ["server", "status"], 1, Operand, Portable, true, true, false;
    PanelGraphicsSet => ["panel", "graphics", "set"], 1, Panel, Windows, true, false, false;
    PanelGraphicsClear => ["panel", "graphics", "clear"], 1, Panel, Windows, true, false, false;
    PanelGraphicsInfo => ["panel", "graphics", "info"], 1, Panel, Windows, true, false, false;
    PanelGraphicsStream => ["panel", "graphics", "stream"], 1, Panel, Windows, true, false, false;
    ClientList => ["client", "list"], 0, Operand, Portable, true, true, false;
    ClientGet => ["client", "get"], 1, Operand, Portable, true, true, false;
    WorkspaceFocus => ["workspace", "focus"], 1, Operand, Portable, true, true, false;
    TabFocus => ["tab", "focus"], 1, Operand, Portable, true, true, false;
    PanelFocus => ["panel", "focus"], 1, Panel, Portable, true, true, false;
    PanelResize => ["panel", "resize"], 1, Panel, Portable, true, true, false;
    LayoutSetSplitRatio => ["layout", "set-split-ratio"], 0, Operand, Portable, true, true, false;
    LayoutExport => ["layout", "export"], 0, Operand, Windows, true, false, false;
    LayoutApply => ["layout", "apply"], 0, Operand, Windows, true, false, false;
    PanelMaximize => ["panel", "maximize"], 1, Panel, Portable, true, true, false;
    PanelLayout => ["panel", "layout"], 1, Panel, Portable, true, true, false;
    PanelNeighbor => ["panel", "neighbor"], 1, Panel, Portable, true, true, false;
    PanelEdges => ["panel", "edges"], 1, Panel, Portable, true, true, false;
    Tui => ["tui"], 0, Operand, Portable, false, false, false;
    Status => ["status"], 0, Operand, Portable, true, true, false;
    StatusServer => ["status", "server"], 0, Operand, Portable, true, true, false;
    StatusClient => ["status", "client"], 0, Operand, Portable, true, false, false;
    AgentList => ["agent", "list"], 0, Operand, Portable, true, true, false;
    AgentGet => ["agent", "get"], 1, Agent, Portable, true, true, false;
    AgentFocus => ["agent", "focus"], 1, Agent, Portable, true, true, false;
    AgentRead => ["agent", "read"], 1, Agent, Portable, true, true, false;
    AgentStart => ["agent", "start"], super::super::MAXIMUM_ARGUMENT_BYTES, Operand, Portable, true, true, false;
    AgentPrompt => ["agent", "prompt"], 2, Agent, Portable, true, true, false;
    AgentSendKeys => ["agent", "send-keys"], super::super::MAXIMUM_SEND_RECORDS + 1, Agent, Portable, true, true, false;
    AgentExplain => ["agent", "explain"], 1, Operand, Portable, true, true, false;
    AgentWait => ["agent", "wait"], 1, Agent, Portable, true, true, false;
    AgentDoctor => ["agent", "doctor"], 0, Operand, Portable, true, false, false;
    AgentResolve => ["agent", "resolve"], 1, Operand, Portable, true, true, false;
    AgentRename => ["agent", "rename"], 2, Operand, Portable, true, true, false;
    AgentIntegrationList => ["agent", "integration", "list"], 0, Operand, Portable, true, false, false;
    AgentIntegrationExplain => ["agent", "integration", "explain"], 0, Operand, Portable, true, false, false;
    AgentIntegrationEnable => ["agent", "integration", "enable"], 0, Operand, Portable, true, false, false;
    AgentIntegrationDisable => ["agent", "integration", "disable"], 0, Operand, Portable, true, false, false;
    TerminalAttach => ["terminal", "attach"], 1, Operand, Windows, true, false, false;
    AgentAttach => ["agent", "attach"], 1, Operand, Windows, true, false, false;
    TerminalObserve => ["terminal", "session", "observe"], 1, Operand, Windows, true, false, false;
    TerminalControl => ["terminal", "session", "control"], 1, Operand, Windows, true, false, false;
    MachineList => ["machine", "list"], 0, Operand, Windows, true, false, false;
    MachineAdd => ["machine", "add"], 1, Operand, Windows, true, false, false;
    MachineRename => ["machine", "rename"], 1, Operand, Windows, true, false, false;
    MachineRemove => ["machine", "remove"], 1, Operand, Windows, true, false, false;
    MachineEnable => ["machine", "enable"], 1, Operand, Windows, true, false, false;
    MachineDisable => ["machine", "disable"], 1, Operand, Windows, true, false, false;
    MachineConnect => ["machine", "connect"], 1, Operand, Windows, true, false, false;
    MachineDisconnect => ["machine", "disconnect"], 1, Operand, Windows, true, false, false;
    ChannelShow => ["channel", "show"], 0, Operand, Windows, true, false, false;
    ChannelSet => ["channel", "set"], 1, Operand, Windows, true, false, false;
    Update => ["update"], 0, Operand, Windows, true, false, false;
    Install => ["install"], 0, Operand, Windows, true, false, false;
    Uninstall => ["uninstall"], 0, Operand, Windows, true, false, false;
    ProjectList => ["project", "list"], 0, Operand, Portable, true, true, false;
    ProjectGet => ["project", "get"], 1, Operand, Portable, true, true, false;
    ProjectDissolve => ["project", "dissolve"], 1, Operand, Portable, true, true, false;
    ProjectCloseAll => ["project", "close-all"], 1, Operand, Portable, true, true, false;
    WorkspaceSetProject => ["workspace", "set-project"], 1, Operand, Portable, true, true, false;
    ProjectCreate => ["project", "create"], 0, Operand, Portable, true, true, false;
    ProjectRename => ["project", "rename"], 1, Operand, Portable, true, true, false;
    WorkspaceList => ["workspace", "list"], 0, Operand, Portable, true, true, false;
    WorkspaceGet => ["workspace", "get"], 1, Operand, Portable, true, true, false;
    WorkspaceCreate => ["workspace", "create"], 0, Operand, Portable, true, true, true;
    WorkspaceRename => ["workspace", "rename"], 1, Operand, Portable, true, true, false;
    PanelReportMetadata => ["panel", "report-metadata"], 1, Panel, Portable, true, true, false;
    PanelReportAgent => ["panel", "report-agent"], 1, Panel, Portable, true, true, false;
    PanelClearAgentAuthority => ["panel", "clear-agent-authority"], 1, Panel, Portable, true, true, false;
    PanelReleaseAgent => ["panel", "release-agent"], 1, Panel, Portable, true, true, false;
    WorkspaceReportMetadata => ["workspace", "report-metadata"], 1, Operand, Portable, true, true, false;
    WorkspaceMove => ["workspace", "move"], 1, Operand, Portable, true, true, false;
    WorkspaceSetDirectory => ["workspace", "set-directory"], 1, Operand, Portable, true, true, false;
    WorkspaceClose => ["workspace", "close"], 1, Operand, Portable, true, true, false;
    TabList => ["tab", "list"], 0, Operand, Portable, true, true, false;
    TabGet => ["tab", "get"], 1, Operand, Portable, true, true, false;
    TabCreate => ["tab", "create"], 0, Operand, Portable, true, true, true;
    TabRename => ["tab", "rename"], 1, Operand, Portable, true, true, false;
    PanelRename => ["panel", "rename"], 1, Panel, Portable, true, true, false;
    PanelInput => ["panel", "input"], 1, Panel, Portable, true, true, false;
    PanelSwap => ["panel", "swap"], 1, Panel, Portable, true, true, false;
    TabMove => ["tab", "move"], 1, Operand, Portable, true, true, false;
    TabClose => ["tab", "close"], 1, Operand, Portable, true, true, false;
    PanelList => ["panel", "list"], 0, Operand, Portable, true, true, false;
    PanelCurrent => ["panel", "current"], 1, Panel, Portable, true, true, false;
    PanelGet => ["panel", "get"], 1, Panel, Portable, true, true, false;
    PanelProcessInfo => ["panel", "process-info"], 1, Panel, Portable, true, true, false;
    PanelCreateTerminal => ["panel", "create-terminal"], 1, Panel, Windows, true, true, true;
    PanelSplit => ["panel", "split"], 1, Panel, Portable, true, true, true;
    PanelClose => ["panel", "close"], 1, Panel, Portable, true, true, false;
    PanelMove => ["panel", "move"], 1, Panel, Portable, true, true, false;
    TerminalList => ["terminal", "list"], 0, Operand, Portable, true, true, false;
    TerminalGet => ["terminal", "get"], 1, Operand, Portable, true, true, false;
    TerminalRead => ["terminal", "read"], 1, TerminalOrPanel, Portable, true, true, false;
    TerminalClear => ["terminal", "clear"], 1, Operand, Windows, true, true, false;
    TerminalWaitOutput => ["terminal", "wait-output"], 1, TerminalOrPanel, Portable, true, true, false;
    TerminalSend => ["terminal", "send"], 1, TerminalOrPanel, Portable, true, true, false;
    TerminalRun => ["terminal", "run"], 2, TerminalOrPanel, Portable, true, true, false;
    WorktreeList => ["worktree", "list"], 0, Operand, Portable, true, true, false;
    WorktreeCreate => ["worktree", "create"], 0, Operand, Portable, true, true, false;
    WorktreeOpen => ["worktree", "open"], 0, Operand, Portable, true, true, false;
    WorktreeRemove => ["worktree", "remove"], 0, Operand, Portable, true, true, false;
    WorktreeStatus => ["worktree", "status"], 1, Operand, Portable, true, true, false;
    WorktreeResume => ["worktree", "resume"], 1, Operand, Portable, true, true, false;
    LaunchProfileList => ["launch-profile", "list"], 0, Operand, Portable, true, true, false;
    LaunchProfileGet => ["launch-profile", "get"], 1, Operand, Portable, true, true, false;
    LaunchProfileCreate => ["launch-profile", "create"], 0, Operand, Portable, true, true, false;
    LaunchProfileDelete => ["launch-profile", "delete"], 1, Operand, Portable, true, true, false;
    ApiSnapshot => ["api", "snapshot"], 0, Operand, Portable, true, true, false;
    ApiSchema => ["api", "schema"], 1, Operand, Portable, true, false, false;
    ConfigCheck => ["config", "check"], 0, Operand, Portable, true, false, false;
    ConfigDefault => ["config", "default"], 0, Operand, Windows, false, false, false;
    ConfigReload => ["config", "reload"], 0, Operand, Portable, true, true, false;
    DiagnosticsCreate => ["diagnostics", "create"], 0, Operand, Windows, true, false, false;
    ServerStart => ["server", "start"], 0, Operand, Portable, true, false, false;
    ServerConnect => ["server", "connect"], 0, Operand, Portable, false, false, false;
    SessionList => ["session", "list"], 0, Operand, Windows, true, false, false;
    SessionAttach => ["session", "attach"], 1, Session, Windows, false, false, false;
    SessionStop => ["session", "stop"], 1, Session, Windows, true, false, false;
    SessionDelete => ["session", "delete"], 1, Session, Windows, true, false, false;
    ServerDiagnose => ["server", "diagnose"], 0, Operand, Portable, true, false, false;
    ServerUpdateAgentManifests => ["server", "update-agent-manifests"], 0, Operand, Portable, true, true, false;
    TerminalTitleSet => ["terminal", "title", "set"], 1, Operand, Portable, true, true, false;
    TerminalTitleClear => ["terminal", "title", "clear"], 0, Operand, Portable, true, true, false;
    EventsSubscribe => ["events", "subscribe"], 0, Operand, Windows, true, false, false;
    EventsWait => ["events", "wait"], 0, Operand, Windows, true, false, false;
    CommandList => ["command", "list"], 0, Operand, Windows, true, false, false;
    CommandInvoke => ["command", "invoke"], 1, Operand, Windows, true, false, false;
    PluginPaneOpen => ["plugin", "panel", "open"], 0, Operand, Windows, true, true, false;
    PopupClose => ["popup", "close"], 0, Operand, Windows, true, true, false;
    PluginPaneFocus => ["plugin", "panel", "focus"], 1, Operand, Windows, true, true, false;
    PluginPaneClose => ["plugin", "panel", "close"], 1, Operand, Windows, true, true, false;
    PluginActionInvoke => ["plugin", "action", "invoke"], 1, Operand, Windows, true, true, false;
    PluginLogList => ["plugin", "log", "list"], 1, Operand, Windows, true, true, false;
    PluginInstall => ["plugin", "install"], 1, Operand, Windows, true, false, false;
    PluginUninstall => ["plugin", "uninstall"], 1, Operand, Windows, true, false, false;
    PluginUpdate => ["plugin", "update"], 1, Operand, Windows, true, false, false;
    PluginLink => ["plugin", "link"], 1, Operand, Windows, true, true, false;
    PluginUnlink => ["plugin", "unlink"], 1, Operand, Windows, true, true, false;
    PluginEnable => ["plugin", "enable"], 1, Operand, Windows, true, true, false;
    PluginDisable => ["plugin", "disable"], 1, Operand, Windows, true, true, false;
    PluginList => ["plugin", "list"], 1, Operand, Windows, true, true, false;
    PluginConfigDir => ["plugin", "config-dir"], 1, Operand, Windows, true, false, false;
    PluginActionList => ["plugin", "action", "list"], 1, Operand, Windows, true, true, false;
    NotificationShow => ["notification", "show"], 1, Operand, Portable, true, true, false;
    ServerAgentManifests => ["server", "agent-manifests"], 0, Operand, Portable, true, true, false;
    ServerReloadAgentManifests => ["server", "reload-agent-manifests"], 0, Operand, Portable, true, true, false;
    ServerStop => ["server", "stop"], 0, Operand, Portable, true, true, false;
}
