#[cfg(windows)]
mod agent_view;
use crate::local_terminal_service::LocalTerminalService;
use orchbee_server::WorkbenchReadOrNavigationRequest;
use orchbee_workbench_protocol::{
    LaunchProfileCatalogOutcome, ProtocolVersion, WorkbenchAction, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchLaunchProfileCatalogResult,
    WorkbenchPublicationOutcome, WorkbenchPublicationResult, WorkbenchRequest, WorkbenchResponse,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotResult, WorkbenchSnapshotUnavailableReason,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeResult, WorkbenchTabCloseUnavailableReason,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseReadResult,
    WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceCloseResumeResult,
    WorkbenchWorktreeListOutcome, WorkbenchWorktreeListResult, WorktreeListUnavailableReason,
};
use std::sync::Arc;

pub(crate) struct WorkbenchPolicy {
    pub(crate) read: bool,
    pub(crate) manage_navigation: bool,
    pub(crate) associate_directory: bool,
    pub(crate) create_terminal: bool,
    pub(crate) manage_terminal_lifecycle: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
// The response path is the hot, allocation-free branch; boxing it would add a heap allocation to
// every ordinary Workbench request merely to shrink the exceptional status-only variant.
#[allow(clippy::large_enum_variant)]
pub(crate) enum WorkbenchExecution {
    Response(WorkbenchResponse),
    OutcomeUnknown,
}

#[cfg(test)]
mod tests;

pub(crate) struct WorkbenchService {
    #[cfg(windows)]
    agent_views: std::sync::Mutex<agent_view::Registry>,
    terminals: Arc<LocalTerminalService>,
    policy: WorkbenchPolicy,
    #[cfg(windows)]
    client_views: std::sync::Mutex<crate::client_view_registry::ClientViewRegistry>,
    #[cfg(windows)]
    notifications: std::sync::Mutex<crate::notification_registry::Registry>,
    #[cfg(windows)]
    window_titles: std::sync::Mutex<crate::window_title_registry::Registry>,
}

impl WorkbenchService {
    pub(crate) fn new(policy: WorkbenchPolicy, terminals: Arc<LocalTerminalService>) -> Self {
        Self {
            #[cfg(windows)]
            agent_views: std::sync::Mutex::default(),
            terminals,
            policy,
            #[cfg(windows)]
            client_views: std::sync::Mutex::default(),
            #[cfg(windows)]
            notifications: std::sync::Mutex::default(),
            #[cfg(windows)]
            window_titles: std::sync::Mutex::default(),
        }
    }

    pub(crate) const fn client_session(
        &self,
        generation: std::num::NonZeroU64,
    ) -> WorkbenchClientSession<'_> {
        WorkbenchClientSession {
            service: self,
            generation,
        }
    }

    pub(crate) fn execute(&self, request: WorkbenchRequest) -> WorkbenchExecution {
        let permitted = match &request {
            WorkbenchRequest::AgentView(request) => if request.action.mutating() { self.policy.manage_navigation } else { self.policy.read },
            WorkbenchRequest::Plugin(request) => self.policy.read && (!request.action.mutating() || (self.policy.create_terminal && self.policy.manage_terminal_lifecycle)),
            WorkbenchRequest::WindowTitle(_) | WorkbenchRequest::Notification(_) => false,
            WorkbenchRequest::Configuration(request) => match request.action { orchbee_workbench_protocol::ConfigurationAction::Reload => self.policy.manage_navigation, _ => self.policy.read },
            WorkbenchRequest::AgentManifests(request) => match request.action {
                orchbee_workbench_protocol::AgentManifestsAction::Reload { .. } | orchbee_workbench_protocol::AgentManifestsAction::Update { .. } => self.policy.manage_navigation,
                _ => self.policy.read,
            },
            WorkbenchRequest::PanelAgent(request) => match request.action {
                orchbee_workbench_protocol::PanelAgentAction::Read { .. }
                | orchbee_workbench_protocol::PanelAgentAction::ReadPresentations { .. }
                | orchbee_workbench_protocol::PanelAgentAction::Titles { .. }
                | orchbee_workbench_protocol::PanelAgentAction::Explain { .. }
                | orchbee_workbench_protocol::PanelAgentAction::States { .. }
                | orchbee_workbench_protocol::PanelAgentAction::Observe { .. } => self.policy.read,
                orchbee_workbench_protocol::PanelAgentAction::Report { .. } | orchbee_workbench_protocol::PanelAgentAction::ReportPresentation { .. } => self.policy.manage_navigation,
            },
            WorkbenchRequest::WorkspaceGit(_) => self.policy.read,
            WorkbenchRequest::WorkspaceMetadata(request) => match request.action {
                orchbee_workbench_protocol::WorkspaceMetadataAction::Read { .. } => self.policy.read,
                orchbee_workbench_protocol::WorkspaceMetadataAction::Report { .. } => self.policy.manage_navigation,
            },
            WorkbenchRequest::PanelMetadata(request) => match request.action {
                orchbee_workbench_protocol::PanelMetadataAction::Read { .. }
                | orchbee_workbench_protocol::PanelMetadataAction::ReadPresentations { .. } => self.policy.read,
                orchbee_workbench_protocol::PanelMetadataAction::Report { .. } | orchbee_workbench_protocol::PanelMetadataAction::ReportPresentation { .. } => self.policy.manage_navigation,
            },
            WorkbenchRequest::ClientView(_) => self.policy.read,
            WorkbenchRequest::WorkspaceGroupClose(request) => {
                if matches!(
                    request.action,
                    orchbee_workbench_protocol::WorkspaceGroupCloseAction::Read { .. }
                ) {
                    self.policy.read
                } else {
                    self.policy.manage_terminal_lifecycle
                }
            }
            WorkbenchRequest::WorktreeOperation(request) => {
                if matches!(
                    request.action,
                    orchbee_workbench_protocol::WorkbenchWorktreeAction::Read { .. }
                ) {
                    self.policy.read
                } else {
                    self.policy.manage_navigation
                        && self.policy.associate_directory
                        && self.policy.create_terminal
                        && self.policy.manage_terminal_lifecycle
                }
            }
            WorkbenchRequest::SnapshotRead(_)
            | WorkbenchRequest::PublicationRead(_)
            | WorkbenchRequest::TabCloseRead(_)
            | WorkbenchRequest::WorkspaceCloseRead(_)
            | WorkbenchRequest::WorktreeListRead(_)
            | WorkbenchRequest::WorktreeQuery(_)
            | WorkbenchRequest::LaunchProfileCatalogRead(_) => self.policy.read,
            WorkbenchRequest::TabCloseResume(_) | WorkbenchRequest::WorkspaceCloseResume(_) => {
                self.policy.manage_terminal_lifecycle
            }
            WorkbenchRequest::Command(command)
                if matches!(command.action, WorkbenchAction::WorkspaceLaunch { .. }) =>
            {
                self.policy.manage_navigation
                    && self.policy.associate_directory
                    && self.policy.create_terminal
            }
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::TabClose { .. }
                        | WorkbenchAction::PanelClose { .. }
                        | WorkbenchAction::WorkspaceClose { .. }
                ) =>
            {
                self.policy.manage_terminal_lifecycle
            }
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
                        | WorkbenchAction::PanelRelocate { destination: orchbee_workbench_protocol::WorkbenchPanelRelocateDestination::NewWorkspace { primary_directory_path: Some(_), .. }, .. }
                ) =>
            {
                self.policy.manage_navigation && self.policy.associate_directory
            }
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::TabCreate { .. } | WorkbenchAction::PanelSplit { .. } | WorkbenchAction::PanelCreateTerminal { .. }
                ) =>
            {
                self.policy.manage_navigation && self.policy.create_terminal
            }
            WorkbenchRequest::Command(_) => self.policy.manage_navigation,
        };
        if !permitted {
            return WorkbenchExecution::Response(unavailable_or_denied(request, true));
        }
        match &request {
            WorkbenchRequest::Plugin(request) => {
                #[cfg(windows)]
                return WorkbenchExecution::Response(WorkbenchResponse::Plugin(self.terminals.execute_plugin(request.clone())));
                #[cfg(not(windows))]
                return WorkbenchExecution::Response(unavailable_or_denied(WorkbenchRequest::Plugin(request.clone()), false));
            }
            WorkbenchRequest::Notification(request) => return WorkbenchExecution::Response(unavailable_or_denied(WorkbenchRequest::Notification(request.clone()), false)),
            WorkbenchRequest::WindowTitle(request) => return WorkbenchExecution::Response(unavailable_or_denied(WorkbenchRequest::WindowTitle(request.clone()), false)),
        WorkbenchRequest::AgentView(request) => {
                #[cfg(windows)]
                return WorkbenchExecution::Response(WorkbenchResponse::AgentView(self.agent_view(request)));
                #[cfg(not(windows))]
                return WorkbenchExecution::Response(unavailable_or_denied(WorkbenchRequest::AgentView(request.clone()), false));
            }
            WorkbenchRequest::Configuration(request) => {
                #[cfg(windows)]
                return WorkbenchExecution::Response(WorkbenchResponse::Configuration(self.terminals.execute_configuration(request.clone())));
                #[cfg(not(windows))]
                return WorkbenchExecution::Response(unavailable_or_denied(WorkbenchRequest::Configuration(request.clone()), false));
            }
            WorkbenchRequest::AgentManifests(request) => {
                #[cfg(windows)]
                return WorkbenchExecution::Response(WorkbenchResponse::AgentManifests(self.terminals.execute_agent_manifests(request.clone())));
                #[cfg(not(windows))]
                { let _ = request; return WorkbenchExecution::Response(unavailable_or_denied(WorkbenchRequest::AgentManifests(request.clone()), false)); }
            }
            WorkbenchRequest::PanelAgent(request) => {
                return WorkbenchExecution::Response(WorkbenchResponse::PanelAgent(
                    self.terminals.execute_panel_agent(request.clone()),
                ));
            }

            WorkbenchRequest::WorkspaceGit(request) => {
                return WorkbenchExecution::Response(WorkbenchResponse::WorkspaceGit(self.terminals.execute_workspace_git(request.clone())));
            }
            WorkbenchRequest::WorkspaceMetadata(request) => {
                return WorkbenchExecution::Response(WorkbenchResponse::WorkspaceMetadata(
                    self.terminals.execute_workspace_metadata(request.clone()),
                ));
            }
            WorkbenchRequest::PanelMetadata(request) => {
                return WorkbenchExecution::Response(WorkbenchResponse::PanelMetadata(
                    self.terminals.execute_panel_metadata(request.clone()),
                ));
            }
            WorkbenchRequest::ClientView(_) => {
                return WorkbenchExecution::Response(unavailable_or_denied(request, false));
            }
            WorkbenchRequest::WorkspaceGroupClose(request) => {
                return WorkbenchExecution::Response(WorkbenchResponse::WorkspaceGroupClose(
                    self.terminals
                        .execute_workspace_group_close(request.clone()),
                ));
            }
            WorkbenchRequest::WorktreeQuery(query) => {
                return WorkbenchExecution::Response(WorkbenchResponse::WorktreeQuery(
                    self.terminals.execute_worktree_query(query.clone()),
                ));
            }
            WorkbenchRequest::WorktreeOperation(operation) => {
                return WorkbenchExecution::Response(WorkbenchResponse::WorktreeOperation(
                    self.terminals.execute_worktree_operation(operation.clone()),
                ));
            }
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
                        | WorkbenchAction::PanelRelocate { destination: orchbee_workbench_protocol::WorkbenchPanelRelocateDestination::NewWorkspace { primary_directory_path: Some(_), .. }, .. }
                ) =>
            {
                return WorkbenchExecution::Response(WorkbenchResponse::Command(
                    self.terminals.execute_workspace_directory(command.clone()),
                ));
            }
            WorkbenchRequest::TabCloseRead(read) => {
                let denied = self.terminals.workbench_site_matches(&read.site_id) == Some(false);
                let response = if denied {
                    None
                } else {
                    self.terminals
                        .read_terminal_tab_close(read.clone())
                        .map(WorkbenchResponse::TabCloseRead)
                };
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, denied)),
                );
            }
            WorkbenchRequest::TabCloseResume(resume) => {
                let denied = self.terminals.workbench_site_matches(&resume.site_id) == Some(false);
                let response = if denied {
                    None
                } else {
                    self.terminals
                        .resume_terminal_tab_close(resume.clone())
                        .map(WorkbenchResponse::TabCloseResume)
                };
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, denied)),
                );
            }
            WorkbenchRequest::WorkspaceCloseRead(read) => {
                let denied = self.terminals.workbench_site_matches(&read.site_id) == Some(false);
                let response = if denied {
                    None
                } else {
                    self.terminals
                        .read_workspace_close(read.clone())
                        .map(WorkbenchResponse::WorkspaceCloseRead)
                };
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, denied)),
                );
            }
            WorkbenchRequest::WorkspaceCloseResume(resume) => {
                let denied = self.terminals.workbench_site_matches(&resume.site_id) == Some(false);
                let response = if denied {
                    None
                } else {
                    self.terminals
                        .resume_workspace_close(resume.clone())
                        .map(WorkbenchResponse::WorkspaceCloseResume)
                };
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, denied)),
                );
            }
            WorkbenchRequest::Command(command)
                if matches!(&command.action, WorkbenchAction::WorkspaceClose { .. }) =>
            {
                let response = self
                    .terminals
                    .execute_workspace_close(command.clone())
                    .map(WorkbenchResponse::Command);
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, false)),
                );
            }
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::TabClose { .. } | WorkbenchAction::PanelClose { .. }
                ) =>
            {
                let response = self
                    .terminals
                    .execute_terminal_tab_close(command.clone())
                    .map(WorkbenchResponse::Command);
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, false)),
                );
            }
            WorkbenchRequest::WorktreeListRead(read) => {
                let response = self
                    .terminals
                    .execute_worktree_list(read.clone())
                    .map(WorkbenchResponse::WorktreeList);
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, false)),
                );
            }
            WorkbenchRequest::SnapshotRead(_)
            | WorkbenchRequest::PublicationRead(_)
            | WorkbenchRequest::LaunchProfileCatalogRead(_)
            | WorkbenchRequest::Command(_) => {}
        }
        let fallback = request.clone();
        match WorkbenchReadOrNavigationRequest::try_from(request) {
            Ok(pure) => WorkbenchExecution::Response(
                self.terminals
                    .execute_workbench(pure, self.policy.manage_navigation)
                    .unwrap_or_else(|| unavailable_or_denied(fallback, false)),
            ),
            Err(WorkbenchRequest::Command(command))
                if matches!(
                    &command.action,
                    WorkbenchAction::TabMove { .. } | WorkbenchAction::TabMoveIndex { .. }
                ) =>
            {
                WorkbenchExecution::Response(
                    self.terminals
                        .execute_terminal_tab_move(command)
                        .map(WorkbenchResponse::Command)
                        .unwrap_or_else(|| unavailable_or_denied(fallback, false)),
                )
            }
            Err(WorkbenchRequest::Command(command))
                if matches!(
                    &command.action,
                    WorkbenchAction::PanelMove { .. } | WorkbenchAction::PanelSwap { .. }
                ) =>
            {
                WorkbenchExecution::Response(
                    self.terminals
                        .execute_terminal_panel_placement(command)
                        .map(WorkbenchResponse::Command)
                        .unwrap_or_else(|| unavailable_or_denied(fallback, false)),
                )
            }
            Err(WorkbenchRequest::Command(command))
                if matches!(&command.action, WorkbenchAction::PanelRelocate { .. }) =>
            {
                WorkbenchExecution::Response(
                    self.terminals
                        .execute_terminal_panel_relocate(command)
                        .map(WorkbenchResponse::Command)
                        .unwrap_or_else(|| unavailable_or_denied(fallback, false)),
                )
            }
            Err(WorkbenchRequest::Command(command))
                if matches!(&command.action, WorkbenchAction::WorkspaceLaunch { .. }) =>
            {
                match self.terminals.execute_workspace_launch(command) {
                    Ok(result) => WorkbenchExecution::Response(WorkbenchResponse::Command(result)),
                    Err(_) => WorkbenchExecution::OutcomeUnknown,
                }
            }
            Err(WorkbenchRequest::Command(command))
                if matches!(&command.action, WorkbenchAction::TabCreate { .. }) =>
            {
                match self.terminals.execute_terminal_tab_create(command) {
                    Ok(result) => WorkbenchExecution::Response(WorkbenchResponse::Command(result)),
                    Err(_postcommit) => WorkbenchExecution::OutcomeUnknown,
                }
            }
            Err(WorkbenchRequest::Command(command))
                if matches!(
                    &command.action,
                    WorkbenchAction::PanelSplit { .. }
                        | WorkbenchAction::PanelCreateTerminal { .. }
                ) =>
            {
                match self.terminals.execute_terminal_panel_create(command) {
                    Ok(result) => WorkbenchExecution::Response(WorkbenchResponse::Command(result)),
                    Err(_postcommit) => WorkbenchExecution::OutcomeUnknown,
                }
            }
            Err(request) => WorkbenchExecution::Response(unavailable_or_denied(request, false)),
        }
    }
}

fn unavailable_or_denied(request: WorkbenchRequest, denied: bool) -> WorkbenchResponse {
    match request {
        WorkbenchRequest::Plugin(request) => {
            WorkbenchResponse::Plugin(orchbee_workbench_protocol::PluginResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::PluginOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::PluginFailure::Denied
                    } else {
                        orchbee_workbench_protocol::PluginFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::WindowTitle(request) => {
            WorkbenchResponse::WindowTitle(orchbee_workbench_protocol::WindowTitleResult {
                protocol_version: request.protocol_version,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::WindowTitleOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::WindowTitleFailure::Denied
                    } else {
                        orchbee_workbench_protocol::WindowTitleFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::Notification(request) => {
            WorkbenchResponse::Notification(orchbee_workbench_protocol::NotificationResult {
                protocol_version: request.protocol_version,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::NotificationOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::NotificationFailure::Denied
                    } else {
                        orchbee_workbench_protocol::NotificationFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::Configuration(request) => {
            WorkbenchResponse::Configuration(orchbee_workbench_protocol::ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::ConfigurationOutcome::failed(if denied {
                    orchbee_workbench_protocol::ConfigurationFailure::Denied
                } else {
                    orchbee_workbench_protocol::ConfigurationFailure::Unavailable
                }),
            })
        }
        WorkbenchRequest::AgentManifests(request) => {
            WorkbenchResponse::AgentManifests(orchbee_workbench_protocol::AgentManifestsResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::AgentManifestsOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::AgentManifestsFailure::Denied
                    } else {
                        orchbee_workbench_protocol::AgentManifestsFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::PanelAgent(request) => {
            WorkbenchResponse::PanelAgent(orchbee_workbench_protocol::PanelAgentResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::PanelAgentOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::PanelAgentFailure::Denied
                    } else {
                        orchbee_workbench_protocol::PanelAgentFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::WorkspaceGit(request) => {
            WorkbenchResponse::WorkspaceGit(orchbee_workbench_protocol::WorkspaceGitResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id.clone(),
                site_id: request.site_id.clone(),
                workspace_id: request.workspace_id.clone(),
                outcome: orchbee_workbench_protocol::WorkspaceGitOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::WorkspaceGitFailure::Denied
                    } else {
                        orchbee_workbench_protocol::WorkspaceGitFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::WorkspaceMetadata(request) => WorkbenchResponse::WorkspaceMetadata(
            orchbee_workbench_protocol::WorkspaceMetadataResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::WorkspaceMetadataOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::WorkspaceMetadataFailure::Denied
                    } else {
                        orchbee_workbench_protocol::WorkspaceMetadataFailure::Unavailable
                    },
                },
            },
        ),
        WorkbenchRequest::AgentView(request) => {
            WorkbenchResponse::AgentView(orchbee_workbench_protocol::AgentViewResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::AgentViewOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::AgentViewFailure::Denied
                    } else {
                        orchbee_workbench_protocol::AgentViewFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::PanelMetadata(request) => {
            WorkbenchResponse::PanelMetadata(orchbee_workbench_protocol::PanelMetadataResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: orchbee_workbench_protocol::PanelMetadataOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::PanelMetadataFailure::Denied
                    } else {
                        orchbee_workbench_protocol::PanelMetadataFailure::Unavailable
                    },
                },
            })
        }
        WorkbenchRequest::ClientView(request) => {
            WorkbenchResponse::ClientView(orchbee_workbench_protocol::WorkbenchClientViewResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: if denied {
                    orchbee_workbench_protocol::ClientViewOutcome::Denied {}
                } else {
                    orchbee_workbench_protocol::ClientViewOutcome::Unavailable {
                        reason: orchbee_workbench_protocol::ClientViewUnavailable::Unsupported,
                    }
                },
            })
        }
        WorkbenchRequest::WorkspaceGroupClose(request) => WorkbenchResponse::WorkspaceGroupClose(
            orchbee_workbench_protocol::WorkbenchWorkspaceGroupCloseResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                operation_id: request.action.operation_id().clone(),
                outcome: if denied {
                    orchbee_workbench_protocol::WorkspaceGroupCloseOutcome::Denied {}
                } else {
                    orchbee_workbench_protocol::WorkspaceGroupCloseOutcome::Unavailable { reason: orchbee_workbench_protocol::WorkspaceGroupCloseUnavailable::ServiceUnavailable }
                },
            },
        ),
        WorkbenchRequest::WorktreeQuery(request) => WorkbenchResponse::WorktreeQuery(
            orchbee_workbench_protocol::WorkbenchWorktreeQueryResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: orchbee_workbench_protocol::WorkbenchWorktreeQueryOutcome::Failed {
                    reason: if denied {
                        orchbee_workbench_protocol::WorktreeQueryFailure::Denied {}
                    } else {
                        orchbee_workbench_protocol::WorktreeQueryFailure::Unavailable {
                            reason: WorktreeListUnavailableReason::ServerBusy,
                        }
                    },
                },
            },
        ),
        WorkbenchRequest::WorktreeOperation(request) => {
            WorkbenchResponse::WorktreeOperation(
                orchbee_workbench_protocol::WorkbenchWorktreeResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    operation_id: request.action.operation_id().clone(),
                    outcome: if denied {
                        orchbee_workbench_protocol::WorkbenchWorktreeOutcome::Denied {}
                    } else {
                        orchbee_workbench_protocol::WorkbenchWorktreeOutcome::Unavailable { reason: orchbee_workbench_protocol::WorktreeAdmissionFailure::ServiceUnavailable }
                    },
                },
            )
        }
        WorkbenchRequest::WorktreeListRead(read) => {
            WorkbenchResponse::WorktreeList(WorkbenchWorktreeListResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: if denied {
                    WorkbenchWorktreeListOutcome::Denied {}
                } else {
                    WorkbenchWorktreeListOutcome::Unavailable {
                        reason: WorktreeListUnavailableReason::ServerBusy,
                    }
                },
            })
        }
        WorkbenchRequest::LaunchProfileCatalogRead(read) => {
            WorkbenchResponse::LaunchProfileCatalog(WorkbenchLaunchProfileCatalogResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: if denied {
                    LaunchProfileCatalogOutcome::Denied {}
                } else {
                    LaunchProfileCatalogOutcome::Unavailable {
                        reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    }
                },
            })
        }
        WorkbenchRequest::TabCloseRead(read) => {
            WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: if denied {
                    WorkbenchTabCloseReadOutcome::Denied {}
                } else {
                    WorkbenchTabCloseReadOutcome::Unavailable {
                        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
                    }
                },
            })
        }
        WorkbenchRequest::TabCloseResume(resume) => {
            WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
                protocol_version: ProtocolVersion,
                request_id: resume.request_id,
                site_id: resume.site_id,
                terminal_context_id: resume.terminal_context_id,
                tab_id: resume.tab_id,
                close_request_id: resume.close_request_id,
                outcome: if denied {
                    WorkbenchTabCloseResumeOutcome::Denied {}
                } else {
                    WorkbenchTabCloseResumeOutcome::Unavailable {
                        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
                    }
                },
            })
        }
        WorkbenchRequest::WorkspaceCloseRead(read) => {
            WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: if denied {
                    WorkbenchWorkspaceCloseReadOutcome::Denied {}
                } else {
                    WorkbenchWorkspaceCloseReadOutcome::Unavailable {
                        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
                    }
                },
            })
        }
        WorkbenchRequest::WorkspaceCloseResume(resume) => {
            WorkbenchResponse::WorkspaceCloseResume(WorkbenchWorkspaceCloseResumeResult {
                protocol_version: ProtocolVersion,
                request_id: resume.request_id,
                site_id: resume.site_id,
                project_id: resume.project_id,
                workspace_id: resume.workspace_id,
                close_request_id: resume.close_request_id,
                outcome: if denied {
                    WorkbenchWorkspaceCloseResumeOutcome::Denied {}
                } else {
                    WorkbenchWorkspaceCloseResumeOutcome::Unavailable {
                        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
                    }
                },
            })
        }
        WorkbenchRequest::SnapshotRead(intent) => {
            WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: intent.request_id,
                outcome: if denied {
                    WorkbenchSnapshotOutcome::Denied {}
                } else {
                    WorkbenchSnapshotOutcome::Unavailable {
                        reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    }
                },
            })
        }
        WorkbenchRequest::PublicationRead(intent) => {
            WorkbenchResponse::Publication(WorkbenchPublicationResult {
                protocol_version: ProtocolVersion,
                request_id: intent.request_id,
                outcome: if denied {
                    WorkbenchPublicationOutcome::Denied {}
                } else {
                    WorkbenchPublicationOutcome::Unavailable {
                        reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    }
                },
            })
        }
        WorkbenchRequest::Command(command) => WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: if denied {
                WorkbenchCommandOutcome::Denied {}
            } else {
                WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
                }
            },
        }),
    }
}

/// The authenticated Session, rather than a caller-controlled payload, owns view publication.
pub(crate) struct WorkbenchClientSession<'a> {
    service: &'a WorkbenchService,
    generation: std::num::NonZeroU64,
}
impl WorkbenchClientSession<'_> {
    pub(crate) fn execute(&self, request: WorkbenchRequest) -> WorkbenchExecution {
        #[cfg(windows)]
        if let WorkbenchRequest::WindowTitle(request) = &request {
            use orchbee_workbench_protocol::{
                WindowTitleAction, WindowTitleFailure as Failure, WindowTitleOutcome,
                WindowTitleResult,
            };
            let allowed = self.service.policy.read
                && (!matches!(
                    request.action,
                    WindowTitleAction::Set { .. } | WindowTitleAction::Clear {}
                ) || self.service.policy.manage_navigation);
            let outcome = if !allowed {
                WindowTitleOutcome::Failed {
                    reason: Failure::Denied,
                }
            } else if self
                .service
                .terminals
                .workbench_site_matches(&request.site_id)
                != Some(true)
            {
                WindowTitleOutcome::Failed {
                    reason: Failure::SiteMismatch,
                }
            } else if let Ok(mut registry) = self.service.window_titles.lock() {
                let Ok(profile) = self.service.terminals.window_title_configuration() else {
                    return WorkbenchExecution::Response(unavailable_or_denied(
                        WorkbenchRequest::WindowTitle(request.clone()),
                        false,
                    ));
                };
                registry.configure(&profile);
                registry.execute(
                    self.generation,
                    request.request_id.clone(),
                    request.action.clone(),
                    std::time::Instant::now(),
                )
            } else {
                WindowTitleOutcome::Failed {
                    reason: Failure::Unavailable,
                }
            };
            return WorkbenchExecution::Response(WorkbenchResponse::WindowTitle(
                WindowTitleResult {
                    protocol_version: request.protocol_version,
                    request_id: request.request_id.clone(),
                    site_id: request.site_id.clone(),
                    outcome,
                },
            ));
        }
        #[cfg(windows)]
        if let WorkbenchRequest::Notification(request) = &request {
            use orchbee_workbench_protocol::{
                NotificationAction, NotificationFailure as Failure, NotificationOutcome,
                NotificationResult,
            };
            let allowed = self.service.policy.read
                && (!matches!(request.action, NotificationAction::Show { .. })
                    || self.service.policy.manage_navigation);
            let outcome = if !allowed {
                NotificationOutcome::Failed {
                    reason: Failure::Denied,
                }
            } else if self
                .service
                .terminals
                .workbench_site_matches(&request.site_id)
                != Some(true)
            {
                NotificationOutcome::Failed {
                    reason: Failure::SiteMismatch,
                }
            } else if let Ok(mut registry) = self.service.notifications.lock() {
                let Ok(disabled) = self.service.terminals.notifications_disabled() else {
                    return WorkbenchExecution::Response(unavailable_or_denied(
                        WorkbenchRequest::Notification(request.clone()),
                        false,
                    ));
                };
                registry.disabled = disabled;
                registry.execute(
                    self.generation,
                    request.request_id.clone(),
                    request.action.clone(),
                    std::time::Instant::now(),
                )
            } else {
                NotificationOutcome::Failed {
                    reason: Failure::Unavailable,
                }
            };
            return WorkbenchExecution::Response(WorkbenchResponse::Notification(
                NotificationResult {
                    protocol_version: request.protocol_version,
                    request_id: request.request_id.clone(),
                    site_id: request.site_id.clone(),
                    outcome,
                },
            ));
        }
        #[cfg(windows)]
        if let WorkbenchRequest::ClientView(request) = request {
            use orchbee_workbench_protocol::{
                ClientViewAction, ClientViewOutcome, ClientViewUnavailable,
                WorkbenchClientViewResult,
            };
            let allowed = self.service.policy.read
                && (!matches!(request.action, ClientViewAction::Submit { .. })
                    || self.service.policy.manage_navigation);
            let outcome = if !allowed
                || self
                    .service
                    .terminals
                    .workbench_site_matches(&request.site_id)
                    != Some(true)
            {
                ClientViewOutcome::Denied {}
            } else if let Ok(mut registry) = self.service.client_views.lock() {
                registry.execute(self.generation, request.action, std::time::Instant::now())
            } else {
                ClientViewOutcome::Unavailable {
                    reason: ClientViewUnavailable::ServerBusy,
                }
            };
            return WorkbenchExecution::Response(WorkbenchResponse::ClientView(
                WorkbenchClientViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome,
                },
            ));
        }
        self.service.execute(request)
    }
}
impl Drop for WorkbenchClientSession<'_> {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let mut registry = self
                .service
                .client_views
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registry.disconnect(self.generation, std::time::Instant::now());
            drop(registry);
            self.service
                .notifications
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .disconnect(self.generation);
            self.service
                .window_titles
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .disconnect(self.generation);
        }
        #[cfg(not(windows))]
        let _generation = self.generation;
    }
}
