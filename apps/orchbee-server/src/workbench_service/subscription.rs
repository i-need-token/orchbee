//! Classify operations that do not require an additional generic invalidation.
//! Multiplex subscription admission separately excludes reads with delivery side effects.
use orchbee_workbench_protocol::{
    AgentManifestsAction, ClientViewAction, ConfigurationAction, ConfiguredCommandAction,
    NotificationAction, PaneGraphicsAction, PanelAgentAction, PanelMetadataAction,
    SemanticEventsAction, SettingsOperation, TabDuplicateAction, TabSetCloseAction,
    WindowTitleAction, WorkbenchRequest, WorkbenchWorktreeAction, WorkspaceMetadataAction,
};

pub(crate) fn subscription_request(request: &WorkbenchRequest) -> bool {
    match request {
        WorkbenchRequest::SnapshotRead(_)
        | WorkbenchRequest::PublicationRead(_)
        | WorkbenchRequest::TabCloseRead(_)
        | WorkbenchRequest::WorkspaceCloseRead(_)
        | WorkbenchRequest::CloseOperationRead(_)
        | WorkbenchRequest::WorktreeListRead(_)
        | WorkbenchRequest::LaunchProfileCatalogRead(_)
        | WorkbenchRequest::WorktreeQuery(_)
        | WorkbenchRequest::WorkspaceGit(_)
        | WorkbenchRequest::CurrentDirectory(_) => true,
        // A refresh changes only the catalog, which publishes its own revisions.
        WorkbenchRequest::AgentCatalog(_) => true,
        WorkbenchRequest::Configuration(request) => {
            matches!(request.action, ConfigurationAction::Current)
        }
        WorkbenchRequest::Settings(request) => {
            matches!(request.operation, SettingsOperation::Read {})
        }
        WorkbenchRequest::WindowTitle(request) => {
            matches!(request.action, WindowTitleAction::Listen { .. })
        }
        WorkbenchRequest::Notification(request) => {
            matches!(request.action, NotificationAction::Listen { .. })
        }
        WorkbenchRequest::Plugin(request) => !request.action.mutating(),
        WorkbenchRequest::AgentIntegration(request) => !request.action.mutating(),
        WorkbenchRequest::AgentView(request) => !request.action.mutating(),
        WorkbenchRequest::PaneGraphics(request) => matches!(
            request.action,
            PaneGraphicsAction::Info { .. }
                | PaneGraphicsAction::Catalog { .. }
                | PaneGraphicsAction::Pixels { .. }
                | PaneGraphicsAction::Observe { .. }
        ),
        WorkbenchRequest::PanelAgent(request) => matches!(
            request.action,
            PanelAgentAction::Read { .. }
                | PanelAgentAction::ReadPresentations { .. }
                | PanelAgentAction::Titles { .. }
                | PanelAgentAction::PresentationTitles { .. }
                | PanelAgentAction::Explain { .. }
                | PanelAgentAction::States { .. }
                | PanelAgentAction::Observe { .. }
        ),
        WorkbenchRequest::PanelMetadata(request) => matches!(
            request.action,
            PanelMetadataAction::Read { .. } | PanelMetadataAction::ReadPresentations { .. }
        ),
        WorkbenchRequest::WorkspaceMetadata(request) => {
            matches!(request.action, WorkspaceMetadataAction::Read { .. })
        }
        // A completion-free synchronization may transfer a control but must not invalidate
        // itself: consumers already synchronize after a producer commit or a new UI sample.
        WorkbenchRequest::ClientView(request) => matches!(
            request.action,
            ClientViewAction::Synchronize {
                completion: None,
                ..
            } | ClientViewAction::List { .. }
                | ClientViewAction::Get { .. }
                | ClientViewAction::Read { .. }
        ),
        WorkbenchRequest::Events(request) => {
            matches!(request.action, SemanticEventsAction::Read { .. })
        }
        WorkbenchRequest::WorktreeOperation(request) => {
            matches!(request.action, WorkbenchWorktreeAction::Read { .. })
        }
        WorkbenchRequest::ConfiguredCommand(request) => {
            matches!(request.action, ConfiguredCommandAction::PopupRead { .. })
        }
        // A set's Read is polled while it runs, so it must not invalidate what it reads.
        WorkbenchRequest::TabSetClose(request) => {
            matches!(request.action, TabSetCloseAction::Read { .. })
        }
        WorkbenchRequest::TabDuplicate(request) => {
            matches!(request.action, TabDuplicateAction::Read { .. })
        }
        WorkbenchRequest::AgentManifests(request) => matches!(
            request.action,
            AgentManifestsAction::Current { .. } | AgentManifestsAction::Operation { .. }
        ),
        WorkbenchRequest::Command(_)
        | WorkbenchRequest::TabCloseResume(_)
        | WorkbenchRequest::WorkspaceCloseResume(_)
        | WorkbenchRequest::WorkspaceGroupClose(_)
        | WorkbenchRequest::SessionStop(_) => false,
    }
}
