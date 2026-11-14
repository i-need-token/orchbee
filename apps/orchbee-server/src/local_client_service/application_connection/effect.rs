//! Whether an application record only reads the Session or asks it to change. A drained Server keeps
//! serving reads and refuses intents, so every request variant states its effect here.

use orchbee_agent_observation_protocol::{AgentNamesRequest, AgentRequest};
use orchbee_client_server_protocol::{ClientMessage, TerminalControlAction, TerminalViewerAction};
use orchbee_workbench_protocol::{
    AgentIntegrationAction, AgentManifestsAction, AgentViewAction, ClientViewAction,
    ConfigurationAction, ConfiguredCommandAction, NotificationAction, PaneGraphicsAction,
    PanelAgentAction, PanelMetadataAction, PluginAction, SemanticEventsAction, SettingsOperation,
    TabDuplicateAction, TabSetCloseAction, WindowTitleAction, WorkbenchRequest,
    WorkbenchWorktreeAction, WorkspaceGroupCloseAction, WorkspaceMetadataAction,
};

/// What an application record asks of the Session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_client_service) enum Effect {
    /// Observes Session state or keeps the requester's own observation going: queries,
    /// snapshots, subscriptions, viewer attachment, demand and acknowledgements, heartbeats, and
    /// reading an admitted operation's result.
    Read,
    /// Asks the Session to change shared state, a terminal, an Agent, or user storage.
    Intent,
}

use Effect::{Intent, Read};

pub(super) fn terminal(request: &ClientMessage) -> Effect {
    match request {
        ClientMessage::TerminalCreate(_)
        | ClientMessage::TerminalStart(_)
        | ClientMessage::TerminalSend(_) => Intent,
        ClientMessage::TerminalLaunchRead(_) => Read,
        ClientMessage::TerminalControl(request) => match request.action() {
            TerminalControlAction::Observe { .. } => Read,
            TerminalControlAction::Activate { .. }
            | TerminalControlAction::Acquire { .. }
            | TerminalControlAction::Takeover { .. }
            | TerminalControlAction::Clear { .. }
            | TerminalControlAction::Reset { .. }
            | TerminalControlAction::Input { .. }
            | TerminalControlAction::SemanticInput { .. }
            | TerminalControlAction::Resize { .. }
            | TerminalControlAction::Paste { .. }
            | TerminalControlAction::HostColors { .. } => Intent,
        },
        // A viewer's attachment, demand, acknowledgements and selection belong to that viewer.
        ClientMessage::TerminalViewer(request) => match request.action() {
            TerminalViewerAction::Attach { .. }
            | TerminalViewerAction::AttachBinding { .. }
            | TerminalViewerAction::Poll { .. }
            | TerminalViewerAction::ReadProcessInfo { .. }
            | TerminalViewerAction::FrameApplied { .. }
            | TerminalViewerAction::NeedSnapshot { .. }
            | TerminalViewerAction::ReadRows { .. }
            | TerminalViewerAction::SetProjectionDemand { .. }
            | TerminalViewerAction::SetHistoryDemand { .. }
            | TerminalViewerAction::ReadViewportRows { .. }
            | TerminalViewerAction::RowsApplied { .. }
            | TerminalViewerAction::HistoryObserved { .. }
            | TerminalViewerAction::SelectionGesture { .. }
            | TerminalViewerAction::SelectionCommand { .. }
            | TerminalViewerAction::SelectionExport { .. }
            | TerminalViewerAction::SelectionExportChunkApplied { .. }
            | TerminalViewerAction::SelectionExportCancel { .. }
            | TerminalViewerAction::Detach { .. } => Read,
        },
    }
}

pub(super) fn agent(request: &AgentRequest) -> Effect {
    match request {
        AgentRequest::Read(_) | AgentRequest::Changes(_) => Read,
        AgentRequest::Launch(_) => Intent,
        AgentRequest::Names(request) => match request {
            AgentNamesRequest::Resolve { .. } | AgentNamesRequest::NamesList { .. } => Read,
            AgentNamesRequest::NameAssign { .. } | AgentNamesRequest::NameRelease { .. } => Intent,
        },
    }
}

pub(super) fn workbench(request: &WorkbenchRequest) -> Effect {
    match request {
        WorkbenchRequest::CloseOperationRead(_)
        | WorkbenchRequest::WorkspaceGit(_)
        | WorkbenchRequest::CurrentDirectory(_)
        // A refresh only renews the cached lookup of what this machine has installed.
        | WorkbenchRequest::AgentCatalog(_)
        | WorkbenchRequest::WorktreeQuery(_)
        | WorkbenchRequest::SnapshotRead(_)
        | WorkbenchRequest::TabCloseRead(_)
        | WorkbenchRequest::PublicationRead(_)
        | WorkbenchRequest::WorktreeListRead(_)
        | WorkbenchRequest::LaunchProfileCatalogRead(_)
        | WorkbenchRequest::WorkspaceCloseRead(_) => Read,
        WorkbenchRequest::Command(_)
        | WorkbenchRequest::TabCloseResume(_)
        | WorkbenchRequest::WorkspaceCloseResume(_) => Intent,
        WorkbenchRequest::PaneGraphics(request) => match request.action {
            PaneGraphicsAction::Observe { .. }
            | PaneGraphicsAction::Info { .. }
            | PaneGraphicsAction::Catalog { .. }
            | PaneGraphicsAction::Pixels { .. } => Read,
            PaneGraphicsAction::Set { .. }
            | PaneGraphicsAction::Clear { .. }
            | PaneGraphicsAction::StreamOpen { .. }
            | PaneGraphicsAction::StreamFrame { .. }
            | PaneGraphicsAction::StreamClose { .. } => Intent,
        },
        // Each event cursor belongs to its requester, including its close.
        WorkbenchRequest::Events(request) => match request.action {
            SemanticEventsAction::Subscribe { .. }
            | SemanticEventsAction::Read { .. }
            | SemanticEventsAction::Wait { .. }
            | SemanticEventsAction::Close { .. } => Read,
        },
        WorkbenchRequest::ConfiguredCommand(request) => match request.action {
            ConfiguredCommandAction::List {} | ConfiguredCommandAction::PopupRead { .. } => Read,
            ConfiguredCommandAction::Invoke { .. }
            | ConfiguredCommandAction::PopupOpen { .. }
            | ConfiguredCommandAction::PopupClose { .. } => Intent,
        },
        WorkbenchRequest::AgentView(request) => match request.action {
            AgentViewAction::Read {} => Read,
            AgentViewAction::Set { .. } | AgentViewAction::Clear { .. } => Intent,
        },
        WorkbenchRequest::AgentIntegration(request) => match request.action {
            AgentIntegrationAction::Poll { .. } => Read,
            AgentIntegrationAction::Start { .. } | AgentIntegrationAction::Cancel { .. } => Intent,
        },
        WorkbenchRequest::Plugin(request) => match request.action {
            // A pane observation attaches one temporary viewer for its requester.
            PluginAction::PopupRead { .. }
            | PluginAction::PanelObserve { .. }
            | PluginAction::PanelGet { .. }
            | PluginAction::Logs { .. }
            | PluginAction::Get { .. } => Read,
            PluginAction::PopupOpen { .. }
            | PluginAction::PopupClose { .. }
            | PluginAction::Invoke { .. }
            | PluginAction::Cancel { .. } => Intent,
        },
        WorkbenchRequest::Configuration(request) => match request.action {
            ConfigurationAction::Current => Read,
            ConfigurationAction::Reload => Intent,
        },
        WorkbenchRequest::Settings(request) => match request.operation {
            SettingsOperation::Read {} => Read,
            SettingsOperation::Edit { .. } => Intent,
        },
        WorkbenchRequest::Notification(request) => match request.action {
            NotificationAction::Listen { .. } => Read,
            NotificationAction::Show { .. } => Intent,
        },
        WorkbenchRequest::WindowTitle(request) => match request.action {
            WindowTitleAction::Listen { .. } => Read,
            WindowTitleAction::Set { .. } | WindowTitleAction::Clear {} => Intent,
        },
        WorkbenchRequest::AgentManifests(request) => match request.action {
            AgentManifestsAction::Current {} | AgentManifestsAction::Operation { .. } => Read,
            AgentManifestsAction::Reload { .. } | AgentManifestsAction::Update { .. } => Intent,
        },
        WorkbenchRequest::PanelAgent(request) => match request.action {
            PanelAgentAction::Titles { .. }
            | PanelAgentAction::PresentationTitles { .. }
            | PanelAgentAction::ReadPresentations { .. }
            | PanelAgentAction::Explain { .. }
            | PanelAgentAction::Identify { .. }
            | PanelAgentAction::States { .. }
            | PanelAgentAction::Observe { .. }
            | PanelAgentAction::Read { .. } => Read,
            PanelAgentAction::ReportPresentation { .. } | PanelAgentAction::Report { .. } => Intent,
        },
        WorkbenchRequest::PanelMetadata(request) => {
            match request.action {
                PanelMetadataAction::ReadPresentations { .. }
                | PanelMetadataAction::Read { .. } => Read,
                PanelMetadataAction::ReportPresentation { .. }
                | PanelMetadataAction::Report { .. } => Intent,
            }
        }
        WorkbenchRequest::WorkspaceMetadata(request) => match request.action {
            WorkspaceMetadataAction::Read { .. } => Read,
            WorkspaceMetadataAction::Report { .. } => Intent,
        },
        // Synchronize reports the requester's own view; Submit asks another Client to act.
        WorkbenchRequest::ClientView(request) => match request.action {
            ClientViewAction::Synchronize { .. }
            | ClientViewAction::List {}
            | ClientViewAction::Get { .. }
            | ClientViewAction::Read { .. } => Read,
            ClientViewAction::Submit { .. } => Intent,
        },
        WorkbenchRequest::WorkspaceGroupClose(request) => match request.action {
            WorkspaceGroupCloseAction::Read { .. } => Read,
            WorkspaceGroupCloseAction::Start { .. } | WorkspaceGroupCloseAction::Resume { .. } => {
                Intent
            }
        },
        WorkbenchRequest::TabSetClose(request) => match request.action {
            TabSetCloseAction::Read { .. } => Read,
            TabSetCloseAction::Start { .. } => Intent,
        },
        WorkbenchRequest::TabDuplicate(request) => match request.action {
            TabDuplicateAction::Read { .. } => Read,
            TabDuplicateAction::Start { .. } => Intent,
        },
        WorkbenchRequest::SessionStop(_) => Intent,
        WorkbenchRequest::WorktreeOperation(request) => match request.action {
            WorkbenchWorktreeAction::Read { .. } => Read,
            WorkbenchWorktreeAction::Start { .. } | WorkbenchWorktreeAction::Resume { .. } => {
                Intent
            }
        },
    }
}
