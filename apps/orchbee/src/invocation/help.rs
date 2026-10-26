//! Offline help-path grammar shared by recognition and help navigation. The catalog
//! declares only accepted paths; it does not inspect configuration or a running Site.

use super::{CommandPath, Family, HelpTopic};

/// One build-admitted help path and its typed presentation topic.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HelpPath {
    words: &'static [&'static str],
    topic: HelpTopic,
}

impl HelpPath {
    const fn new(words: &'static [&'static str], topic: HelpTopic) -> Self {
        Self { words, topic }
    }

    /// Exact command words, without the executable or help flag.
    pub(crate) const fn words(self) -> &'static [&'static str] {
        self.words
    }

    /// The localized topic recognized for these exact words.
    pub(crate) const fn topic(self) -> HelpTopic {
        self.topic
    }
}

/// Every accepted help path in this build.
pub(crate) fn help_paths() -> &'static [HelpPath] {
    HELP_PATHS
}

pub(super) fn topic(words: &[&str]) -> Option<HelpTopic> {
    HELP_PATHS
        .iter()
        .find(|path| path.words == words)
        .map(|path| path.topic)
}

/// A command's operands do not become help-path words. Families still require
/// an exact match so a misspelled verb cannot silently show its parent's help.
pub(super) fn invocation_topic(words: &[&str]) -> Option<HelpTopic> {
    HELP_PATHS
        .iter()
        .filter(|path| {
            words.starts_with(path.words)
                && (matches!(
                    path.topic,
                    HelpTopic::Command(_) | HelpTopic::Family(Family::Completion)
                ) || words == path.words)
        })
        .max_by_key(|path| path.words.len())
        .map(|path| path.topic)
}

pub(super) fn context_topic(words: &[&str]) -> Option<HelpTopic> {
    HELP_PATHS
        .iter()
        .filter(|path| words.starts_with(path.words))
        .max_by_key(|path| path.words.len())
        .map(|path| path.topic)
}

/// Directory presentation must not rename existing machine diagnostic contexts.
/// These two directory paths previously displayed their first command's help.
pub(super) const fn diagnostic_path(topic: HelpTopic) -> &'static str {
    match topic {
        #[cfg(windows)]
        HelpTopic::Family(Family::AgentView) => CommandPath::AgentViewGet.as_str(),
        #[cfg(windows)]
        HelpTopic::Family(Family::PanelGraphics) => CommandPath::PanelGraphicsSet.as_str(),
        HelpTopic::Family(family) => family.as_str(),
        HelpTopic::Command(command) => command.as_str(),
    }
}

const HELP_PATHS: &[HelpPath] = &[
    HelpPath::new(
        &["workspace", "status"],
        HelpTopic::Command(CommandPath::WorkspaceStatus),
    ),
    HelpPath::new(
        &["workspace", "resume"],
        HelpTopic::Command(CommandPath::WorkspaceResume),
    ),
    HelpPath::new(
        &["tab", "status"],
        HelpTopic::Command(CommandPath::TabStatus),
    ),
    HelpPath::new(
        &["tab", "resume"],
        HelpTopic::Command(CommandPath::TabResume),
    ),
    HelpPath::new(
        &["panel", "status"],
        HelpTopic::Command(CommandPath::PanelStatus),
    ),
    HelpPath::new(
        &["panel", "resume"],
        HelpTopic::Command(CommandPath::PanelResume),
    ),
    HelpPath::new(
        &["project", "status"],
        HelpTopic::Command(CommandPath::ProjectStatus),
    ),
    HelpPath::new(
        &["project", "resume"],
        HelpTopic::Command(CommandPath::ProjectResume),
    ),
    HelpPath::new(
        &["client", "status"],
        HelpTopic::Command(CommandPath::ClientStatus),
    ),
    HelpPath::new(
        &["server", "status"],
        HelpTopic::Command(CommandPath::ServerStatus),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["panel", "graphics"],
        HelpTopic::Family(Family::PanelGraphics),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["panel", "graphics", "set"],
        HelpTopic::Command(CommandPath::PanelGraphicsSet),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["panel", "graphics", "clear"],
        HelpTopic::Command(CommandPath::PanelGraphicsClear),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["panel", "graphics", "info"],
        HelpTopic::Command(CommandPath::PanelGraphicsInfo),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["panel", "graphics", "stream"],
        HelpTopic::Command(CommandPath::PanelGraphicsStream),
    ),
    HelpPath::new(&["client"], HelpTopic::Family(Family::Client)),
    HelpPath::new(
        &["client", "list"],
        HelpTopic::Command(CommandPath::ClientList),
    ),
    HelpPath::new(
        &["client", "get"],
        HelpTopic::Command(CommandPath::ClientGet),
    ),
    HelpPath::new(
        &["client", "control-status"],
        HelpTopic::Command(CommandPath::ClientControlStatus),
    ),
    HelpPath::new(
        &["workspace", "focus"],
        HelpTopic::Command(CommandPath::WorkspaceFocus),
    ),
    HelpPath::new(&["tab", "focus"], HelpTopic::Command(CommandPath::TabFocus)),
    HelpPath::new(
        &["panel", "focus"],
        HelpTopic::Command(CommandPath::PanelFocus),
    ),
    HelpPath::new(
        &["panel", "resize"],
        HelpTopic::Command(CommandPath::PanelResize),
    ),
    HelpPath::new(&["layout"], HelpTopic::Family(Family::Layout)),
    HelpPath::new(
        &["layout", "set-split-ratio"],
        HelpTopic::Command(CommandPath::LayoutSetSplitRatio),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["layout", "export"],
        HelpTopic::Command(CommandPath::LayoutExport),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["layout", "apply"],
        HelpTopic::Command(CommandPath::LayoutApply),
    ),
    HelpPath::new(
        &["panel", "zoom"],
        HelpTopic::Command(CommandPath::PanelZoom),
    ),
    HelpPath::new(
        &["panel", "layout"],
        HelpTopic::Command(CommandPath::PanelLayout),
    ),
    HelpPath::new(
        &["panel", "neighbor"],
        HelpTopic::Command(CommandPath::PanelNeighbor),
    ),
    HelpPath::new(
        &["panel", "edges"],
        HelpTopic::Command(CommandPath::PanelEdges),
    ),
    HelpPath::new(&["tui"], HelpTopic::Command(CommandPath::Tui)),
    HelpPath::new(&["status"], HelpTopic::Command(CommandPath::Status)),
    HelpPath::new(
        &["status", "server"],
        HelpTopic::Command(CommandPath::StatusServer),
    ),
    HelpPath::new(
        &["status", "client"],
        HelpTopic::Command(CommandPath::StatusClient),
    ),
    HelpPath::new(&["agent"], HelpTopic::Family(Family::Agent)),
    #[cfg(windows)]
    HelpPath::new(&["agent", "view"], HelpTopic::Family(Family::AgentView)),
    #[cfg(windows)]
    HelpPath::new(
        &["agent", "view", "get"],
        HelpTopic::Command(CommandPath::AgentViewGet),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["agent", "view", "set"],
        HelpTopic::Command(CommandPath::AgentViewSet),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["agent", "view", "clear"],
        HelpTopic::Command(CommandPath::AgentViewClear),
    ),
    HelpPath::new(
        &["agent", "list"],
        HelpTopic::Command(CommandPath::AgentList),
    ),
    HelpPath::new(
        &["agent", "changes"],
        HelpTopic::Command(CommandPath::AgentChanges),
    ),
    HelpPath::new(&["agent", "get"], HelpTopic::Command(CommandPath::AgentGet)),
    HelpPath::new(
        &["agent", "focus"],
        HelpTopic::Command(CommandPath::AgentFocus),
    ),
    HelpPath::new(
        &["agent", "attention"],
        HelpTopic::Command(CommandPath::AgentAttention),
    ),
    HelpPath::new(
        &["agent", "read"],
        HelpTopic::Command(CommandPath::AgentRead),
    ),
    HelpPath::new(
        &["agent", "start"],
        HelpTopic::Command(CommandPath::AgentStart),
    ),
    HelpPath::new(
        &["agent", "prompt"],
        HelpTopic::Command(CommandPath::AgentPrompt),
    ),
    HelpPath::new(
        &["agent", "send-keys"],
        HelpTopic::Command(CommandPath::AgentSendKeys),
    ),
    HelpPath::new(
        &["agent", "explain"],
        HelpTopic::Command(CommandPath::AgentExplain),
    ),
    HelpPath::new(
        &["agent", "wait"],
        HelpTopic::Command(CommandPath::AgentWait),
    ),
    HelpPath::new(
        &["agent", "doctor"],
        HelpTopic::Command(CommandPath::AgentDoctor),
    ),
    HelpPath::new(
        &["agent", "names"],
        HelpTopic::Command(CommandPath::AgentNames),
    ),
    HelpPath::new(
        &["agent", "resolve"],
        HelpTopic::Command(CommandPath::AgentResolve),
    ),
    HelpPath::new(
        &["agent", "rename"],
        HelpTopic::Command(CommandPath::AgentRename),
    ),
    HelpPath::new(
        &["agent", "integration"],
        HelpTopic::Family(Family::AgentIntegration),
    ),
    HelpPath::new(
        &["agent", "integration", "list"],
        HelpTopic::Command(CommandPath::AgentIntegrationList),
    ),
    HelpPath::new(
        &["agent", "integration", "explain"],
        HelpTopic::Command(CommandPath::AgentIntegrationExplain),
    ),
    HelpPath::new(
        &["agent", "integration", "enable"],
        HelpTopic::Command(CommandPath::AgentIntegrationEnable),
    ),
    HelpPath::new(
        &["agent", "integration", "disable"],
        HelpTopic::Command(CommandPath::AgentIntegrationDisable),
    ),
    HelpPath::new(&["project"], HelpTopic::Family(Family::Project)),
    HelpPath::new(&["workspace"], HelpTopic::Family(Family::Workspace)),
    HelpPath::new(&["tab"], HelpTopic::Family(Family::Tab)),
    HelpPath::new(&["panel"], HelpTopic::Family(Family::Panel)),
    HelpPath::new(&["terminal"], HelpTopic::Family(Family::Terminal)),
    HelpPath::new(
        &["terminal", "attach"],
        HelpTopic::Command(CommandPath::TerminalAttach),
    ),
    HelpPath::new(
        &["agent", "attach"],
        HelpTopic::Command(CommandPath::AgentAttach),
    ),
    HelpPath::new(
        &["terminal", "session"],
        HelpTopic::Family(Family::WorkbenchClient),
    ),
    HelpPath::new(
        &["terminal", "session", "observe"],
        HelpTopic::Command(CommandPath::TerminalObserve),
    ),
    HelpPath::new(
        &["terminal", "session", "control"],
        HelpTopic::Command(CommandPath::TerminalControl),
    ),
    HelpPath::new(&["api"], HelpTopic::Family(Family::Api)),
    HelpPath::new(&["server"], HelpTopic::Family(Family::Server)),
    HelpPath::new(&["config"], HelpTopic::Family(Family::Config)),
    #[cfg(windows)]
    HelpPath::new(&["channel"], HelpTopic::Family(Family::Channel)),
    #[cfg(windows)]
    HelpPath::new(&["machine"], HelpTopic::Family(Family::Machine)),
    #[cfg(windows)]
    HelpPath::new(
        &["machine", "list"],
        HelpTopic::Command(CommandPath::MachineList),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["machine", "add"],
        HelpTopic::Command(CommandPath::MachineAdd),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["machine", "rename"],
        HelpTopic::Command(CommandPath::MachineRename),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["machine", "remove"],
        HelpTopic::Command(CommandPath::MachineRemove),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["machine", "enable"],
        HelpTopic::Command(CommandPath::MachineEnable),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["machine", "disable"],
        HelpTopic::Command(CommandPath::MachineDisable),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["channel", "show"],
        HelpTopic::Command(CommandPath::ChannelShow),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["channel", "set"],
        HelpTopic::Command(CommandPath::ChannelSet),
    ),
    #[cfg(windows)]
    HelpPath::new(&["update"], HelpTopic::Command(CommandPath::Update)),
    HelpPath::new(&["completion"], HelpTopic::Family(Family::Completion)),
    HelpPath::new(
        &["project", "list"],
        HelpTopic::Command(CommandPath::ProjectList),
    ),
    HelpPath::new(
        &["project", "get"],
        HelpTopic::Command(CommandPath::ProjectGet),
    ),
    HelpPath::new(
        &["project", "dissolve"],
        HelpTopic::Command(CommandPath::ProjectDissolve),
    ),
    HelpPath::new(
        &["project", "close-all"],
        HelpTopic::Command(CommandPath::ProjectCloseAll),
    ),
    HelpPath::new(
        &["project", "close-status"],
        HelpTopic::Command(CommandPath::ProjectCloseStatus),
    ),
    HelpPath::new(
        &["project", "close-resume"],
        HelpTopic::Command(CommandPath::ProjectCloseResume),
    ),
    HelpPath::new(
        &["workspace", "set-project"],
        HelpTopic::Command(CommandPath::WorkspaceSetProject),
    ),
    HelpPath::new(
        &["workspace", "create-project"],
        HelpTopic::Command(CommandPath::WorkspaceCreateProject),
    ),
    HelpPath::new(
        &["workspace", "ungroup"],
        HelpTopic::Command(CommandPath::WorkspaceUngroup),
    ),
    HelpPath::new(
        &["project", "create"],
        HelpTopic::Command(CommandPath::ProjectCreate),
    ),
    HelpPath::new(
        &["project", "rename"],
        HelpTopic::Command(CommandPath::ProjectRename),
    ),
    HelpPath::new(
        &["workspace", "list"],
        HelpTopic::Command(CommandPath::WorkspaceList),
    ),
    HelpPath::new(
        &["workspace", "get"],
        HelpTopic::Command(CommandPath::WorkspaceGet),
    ),
    HelpPath::new(
        &["workspace", "create"],
        HelpTopic::Command(CommandPath::WorkspaceCreate),
    ),
    HelpPath::new(
        &["workspace", "rename"],
        HelpTopic::Command(CommandPath::WorkspaceRename),
    ),
    HelpPath::new(
        &["panel", "report-metadata"],
        HelpTopic::Command(CommandPath::PanelReportMetadata),
    ),
    HelpPath::new(
        &["panel", "report-agent"],
        HelpTopic::Command(CommandPath::PanelReportAgent),
    ),
    HelpPath::new(
        &["panel", "clear-agent-authority"],
        HelpTopic::Command(CommandPath::PanelClearAgentAuthority),
    ),
    HelpPath::new(
        &["panel", "release-agent"],
        HelpTopic::Command(CommandPath::PanelReleaseAgent),
    ),
    HelpPath::new(
        &["workspace", "report-metadata"],
        HelpTopic::Command(CommandPath::WorkspaceReportMetadata),
    ),
    HelpPath::new(
        &["workspace", "move"],
        HelpTopic::Command(CommandPath::WorkspaceMove),
    ),
    HelpPath::new(
        &["workspace", "move-block"],
        HelpTopic::Command(CommandPath::WorkspaceMoveBlock),
    ),
    HelpPath::new(
        &["workspace", "accept-directory"],
        HelpTopic::Command(CommandPath::WorkspaceAcceptDirectory),
    ),
    HelpPath::new(
        &["workspace", "close"],
        HelpTopic::Command(CommandPath::WorkspaceClose),
    ),
    HelpPath::new(
        &["workspace", "close-status"],
        HelpTopic::Command(CommandPath::WorkspaceCloseStatus),
    ),
    HelpPath::new(
        &["workspace", "close-resume"],
        HelpTopic::Command(CommandPath::WorkspaceCloseResume),
    ),
    HelpPath::new(&["tab", "list"], HelpTopic::Command(CommandPath::TabList)),
    HelpPath::new(&["tab", "get"], HelpTopic::Command(CommandPath::TabGet)),
    HelpPath::new(
        &["tab", "create"],
        HelpTopic::Command(CommandPath::TabCreate),
    ),
    HelpPath::new(
        &["tab", "rename"],
        HelpTopic::Command(CommandPath::TabRename),
    ),
    HelpPath::new(
        &["panel", "rename"],
        HelpTopic::Command(CommandPath::PanelRename),
    ),
    HelpPath::new(
        &["panel", "input"],
        HelpTopic::Command(CommandPath::PanelInput),
    ),
    HelpPath::new(
        &["panel", "swap"],
        HelpTopic::Command(CommandPath::PanelSwap),
    ),
    HelpPath::new(&["tab", "move"], HelpTopic::Command(CommandPath::TabMove)),
    HelpPath::new(&["tab", "close"], HelpTopic::Command(CommandPath::TabClose)),
    HelpPath::new(
        &["panel", "list"],
        HelpTopic::Command(CommandPath::PanelList),
    ),
    HelpPath::new(
        &["panel", "current"],
        HelpTopic::Command(CommandPath::PanelCurrent),
    ),
    HelpPath::new(&["panel", "get"], HelpTopic::Command(CommandPath::PanelGet)),
    HelpPath::new(
        &["panel", "process-info"],
        HelpTopic::Command(CommandPath::PanelProcessInfo),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["panel", "create-terminal"],
        HelpTopic::Command(CommandPath::PanelCreateTerminal),
    ),
    HelpPath::new(
        &["panel", "split"],
        HelpTopic::Command(CommandPath::PanelSplit),
    ),
    HelpPath::new(
        &["panel", "close"],
        HelpTopic::Command(CommandPath::PanelClose),
    ),
    HelpPath::new(
        &["panel", "move"],
        HelpTopic::Command(CommandPath::PanelMove),
    ),
    HelpPath::new(
        &["terminal", "list"],
        HelpTopic::Command(CommandPath::TerminalList),
    ),
    HelpPath::new(
        &["terminal", "get"],
        HelpTopic::Command(CommandPath::TerminalGet),
    ),
    HelpPath::new(
        &["terminal", "read"],
        HelpTopic::Command(CommandPath::TerminalRead),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["terminal", "selection-read"],
        HelpTopic::Command(CommandPath::TerminalSelectionRead),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["terminal", "copy-motion"],
        HelpTopic::Command(CommandPath::TerminalCopyMotion),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["terminal", "copy-search"],
        HelpTopic::Command(CommandPath::TerminalCopySearch),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["terminal", "link-resolve"],
        HelpTopic::Command(CommandPath::TerminalLinkResolve),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["terminal", "link-activate"],
        HelpTopic::Command(CommandPath::TerminalLinkActivate),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["terminal", "clear"],
        HelpTopic::Command(CommandPath::TerminalClear),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["terminal", "edit-scrollback"],
        HelpTopic::Command(CommandPath::TerminalEditScrollback),
    ),
    HelpPath::new(
        &["terminal", "wait-output"],
        HelpTopic::Command(CommandPath::TerminalWaitOutput),
    ),
    HelpPath::new(
        &["terminal", "send"],
        HelpTopic::Command(CommandPath::TerminalSend),
    ),
    HelpPath::new(
        &["terminal", "run"],
        HelpTopic::Command(CommandPath::TerminalRun),
    ),
    HelpPath::new(&["worktree"], HelpTopic::Family(Family::Worktree)),
    HelpPath::new(
        &["worktree", "list"],
        HelpTopic::Command(CommandPath::WorktreeList),
    ),
    HelpPath::new(
        &["worktree", "create"],
        HelpTopic::Command(CommandPath::WorktreeCreate),
    ),
    HelpPath::new(
        &["worktree", "open"],
        HelpTopic::Command(CommandPath::WorktreeOpen),
    ),
    HelpPath::new(
        &["worktree", "remove"],
        HelpTopic::Command(CommandPath::WorktreeRemove),
    ),
    HelpPath::new(
        &["worktree", "status"],
        HelpTopic::Command(CommandPath::WorktreeStatus),
    ),
    HelpPath::new(
        &["worktree", "resume"],
        HelpTopic::Command(CommandPath::WorktreeResume),
    ),
    HelpPath::new(
        &["launch-profile"],
        HelpTopic::Family(Family::LaunchProfile),
    ),
    HelpPath::new(
        &["launch-profile", "list"],
        HelpTopic::Command(CommandPath::LaunchProfileList),
    ),
    HelpPath::new(
        &["launch-profile", "get"],
        HelpTopic::Command(CommandPath::LaunchProfileGet),
    ),
    HelpPath::new(
        &["launch-profile", "create"],
        HelpTopic::Command(CommandPath::LaunchProfileCreate),
    ),
    HelpPath::new(
        &["launch-profile", "delete"],
        HelpTopic::Command(CommandPath::LaunchProfileDelete),
    ),
    HelpPath::new(
        &["api", "snapshot"],
        HelpTopic::Command(CommandPath::ApiSnapshot),
    ),
    HelpPath::new(
        &["api", "schema"],
        HelpTopic::Command(CommandPath::ApiSchema),
    ),
    HelpPath::new(
        &["config", "check"],
        HelpTopic::Command(CommandPath::ConfigCheck),
    ),
    HelpPath::new(
        &["config", "reset-keys"],
        HelpTopic::Command(CommandPath::ConfigResetKeys),
    ),
    HelpPath::new(
        &["server", "start"],
        HelpTopic::Command(CommandPath::SiteStart),
    ),
    #[cfg(windows)]
    HelpPath::new(&["session"], HelpTopic::Family(Family::Session)),
    #[cfg(windows)]
    HelpPath::new(
        &["session", "list"],
        HelpTopic::Command(CommandPath::SessionList),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["session", "attach"],
        HelpTopic::Command(CommandPath::SessionAttach),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["session", "stop"],
        HelpTopic::Command(CommandPath::SessionStop),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["session", "delete"],
        HelpTopic::Command(CommandPath::SessionDelete),
    ),
    HelpPath::new(
        &["server", "diagnose"],
        HelpTopic::Command(CommandPath::SiteDiagnose),
    ),
    HelpPath::new(
        &["server", "update-agent-manifests"],
        HelpTopic::Command(CommandPath::SiteUpdateAgentManifests),
    ),
    HelpPath::new(
        &["server", "config"],
        HelpTopic::Command(CommandPath::SiteConfig),
    ),
    HelpPath::new(
        &["server", "reload-config"],
        HelpTopic::Command(CommandPath::SiteReloadConfig),
    ),
    HelpPath::new(
        &["terminal", "title"],
        HelpTopic::Family(Family::TerminalTitle),
    ),
    HelpPath::new(
        &["terminal", "title", "set"],
        HelpTopic::Command(CommandPath::TerminalTitleSet),
    ),
    HelpPath::new(
        &["terminal", "title", "clear"],
        HelpTopic::Command(CommandPath::TerminalTitleClear),
    ),
    #[cfg(windows)]
    HelpPath::new(&["plugin"], HelpTopic::Family(Family::Plugin)),
    #[cfg(windows)]
    HelpPath::new(&["command"], HelpTopic::Family(Family::Command)),
    #[cfg(windows)]
    HelpPath::new(&["events"], HelpTopic::Family(Family::Events)),
    #[cfg(windows)]
    HelpPath::new(
        &["events", "subscribe"],
        HelpTopic::Command(CommandPath::EventsSubscribe),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["events", "wait"],
        HelpTopic::Command(CommandPath::EventsWait),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["command", "list"],
        HelpTopic::Command(CommandPath::CommandList),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["command", "invoke"],
        HelpTopic::Command(CommandPath::CommandInvoke),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "action"],
        HelpTopic::Family(Family::PluginAction),
    ),
    #[cfg(windows)]
    HelpPath::new(&["plugin", "log"], HelpTopic::Family(Family::PluginLog)),
    #[cfg(windows)]
    HelpPath::new(&["plugin", "pane"], HelpTopic::Family(Family::PluginPane)),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "pane", "open"],
        HelpTopic::Command(CommandPath::PluginPaneOpen),
    ),
    #[cfg(windows)]
    HelpPath::new(&["popup"], HelpTopic::Family(Family::Popup)),
    #[cfg(windows)]
    HelpPath::new(
        &["popup", "close"],
        HelpTopic::Command(CommandPath::PopupClose),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "pane", "focus"],
        HelpTopic::Command(CommandPath::PluginPaneFocus),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "pane", "close"],
        HelpTopic::Command(CommandPath::PluginPaneClose),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "action", "invoke"],
        HelpTopic::Command(CommandPath::PluginActionInvoke),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "log", "list"],
        HelpTopic::Command(CommandPath::PluginLogList),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "install"],
        HelpTopic::Command(CommandPath::PluginInstall),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "uninstall"],
        HelpTopic::Command(CommandPath::PluginUninstall),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "link"],
        HelpTopic::Command(CommandPath::PluginLink),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "unlink"],
        HelpTopic::Command(CommandPath::PluginUnlink),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "enable"],
        HelpTopic::Command(CommandPath::PluginEnable),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "disable"],
        HelpTopic::Command(CommandPath::PluginDisable),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "list"],
        HelpTopic::Command(CommandPath::PluginList),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "config-dir"],
        HelpTopic::Command(CommandPath::PluginConfigDir),
    ),
    #[cfg(windows)]
    HelpPath::new(
        &["plugin", "action", "list"],
        HelpTopic::Command(CommandPath::PluginActionList),
    ),
    HelpPath::new(&["notification"], HelpTopic::Family(Family::Notification)),
    HelpPath::new(
        &["notification", "show"],
        HelpTopic::Command(CommandPath::NotificationShow),
    ),
    HelpPath::new(
        &["server", "agent-manifests"],
        HelpTopic::Command(CommandPath::SiteAgentManifests),
    ),
    HelpPath::new(
        &["server", "reload-agent-manifests"],
        HelpTopic::Command(CommandPath::SiteReloadAgentManifests),
    ),
    HelpPath::new(
        &["server", "agent-manifests-status"],
        HelpTopic::Command(CommandPath::SiteAgentManifestsStatus),
    ),
    HelpPath::new(
        &["server", "stop"],
        HelpTopic::Command(CommandPath::SiteStop),
    ),
    HelpPath::new(
        &["server", "stop-status"],
        HelpTopic::Command(CommandPath::SiteStopStatus),
    ),
];
