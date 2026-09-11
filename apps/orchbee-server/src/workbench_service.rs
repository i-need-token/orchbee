use crate::local_terminal_service::LocalTerminalService;
use orchbee_server::WorkbenchReadOrNavigationRequest;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchPublicationOutcome, WorkbenchPublicationResult,
    WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchSnapshotUnavailableReason, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult,
    WorkbenchTabCloseResumeOutcome, WorkbenchTabCloseResumeResult,
    WorkbenchTabCloseUnavailableReason,
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
    terminals: Arc<LocalTerminalService>,
    policy: WorkbenchPolicy,
}

impl WorkbenchService {
    pub(crate) fn new(policy: WorkbenchPolicy, terminals: Arc<LocalTerminalService>) -> Self {
        Self { terminals, policy }
    }

    pub(crate) fn execute(&self, request: WorkbenchRequest) -> WorkbenchExecution {
        let permitted = match &request {
            WorkbenchRequest::SnapshotRead(_)
            | WorkbenchRequest::PublicationRead(_)
            | WorkbenchRequest::TabCloseRead(_) => self.policy.read,
            WorkbenchRequest::TabCloseResume(_) => self.policy.manage_terminal_lifecycle,
            WorkbenchRequest::Command(command)
                if matches!(&command.action, WorkbenchAction::TabClose { .. }) =>
            {
                self.policy.manage_terminal_lifecycle
            }
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
                ) =>
            {
                self.policy.manage_navigation && self.policy.associate_directory
            }
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::TabCreate { .. } | WorkbenchAction::PanelSplit { .. }
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
            WorkbenchRequest::Command(command)
                if matches!(
                    &command.action,
                    WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
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
            WorkbenchRequest::Command(command)
                if matches!(&command.action, WorkbenchAction::TabClose { .. }) =>
            {
                let response = self
                    .terminals
                    .execute_terminal_tab_close(command.clone())
                    .map(WorkbenchResponse::Command);
                return WorkbenchExecution::Response(
                    response.unwrap_or_else(|| unavailable_or_denied(request, false)),
                );
            }
            WorkbenchRequest::SnapshotRead(_)
            | WorkbenchRequest::PublicationRead(_)
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
                if matches!(&command.action, WorkbenchAction::TabMove { .. }) =>
            {
                WorkbenchExecution::Response(
                    self.terminals
                        .execute_terminal_tab_move(command)
                        .map(WorkbenchResponse::Command)
                        .unwrap_or_else(|| unavailable_or_denied(fallback, false)),
                )
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
                if matches!(&command.action, WorkbenchAction::PanelSplit { .. }) =>
            {
                match self.terminals.execute_terminal_panel_split(command) {
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
